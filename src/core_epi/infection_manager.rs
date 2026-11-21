use std::any::{Any, TypeId};
use std::marker::PhantomData;

use crate::core_epi::modifier_kinds::{ModifierKey, ModifierKind};
use crate::core_epi::rate::{AnyInfectionRate, InvertibleRate, TransformedInvertibleRateFn};
use crate::data_structures::entity_map::EntityMap;
use crate::entity::{Entity, EntityId};
use crate::prelude::*;
use crate::{ContextBase, HashMap, IxaEvent};

/// Which rate function a person was assigned: an index into the loaded
/// library of shared curves, or one of their own.
#[derive(Clone, Debug)]
pub enum RateAssignment {
    /// Index into the library loaded with
    /// [`load_infectiousness_rate_library`](InfectionManager::load_infectiousness_rate_library).
    Library(usize),
    /// Boxed so a record is not sized for an empirical curve.
    Own(Box<AnyInfectionRate>),
}

impl RateAssignment {
    /// A rate function of the person's own.
    pub fn own(rate_fn: impl Into<AnyInfectionRate>) -> Self {
        Self::Own(Box::new(rate_fn.into()))
    }
}

impl<R: Into<AnyInfectionRate>> From<R> for RateAssignment {
    fn from(rate_fn: R) -> Self {
        Self::own(rate_fn)
    }
}

/// An infection that a reinfection replaced.
#[derive(Clone, Debug, PartialEq)]
pub struct PastInfection<D> {
    /// When it began.
    pub time: f64,
    /// When its infectiousness ended: when its rate function ran out, or at
    /// the reinfection if that came first.
    pub infectiousness_end: f64,
    pub details: D,
}

/// Emitted when a person is assigned a rate function by model `M`, i.e.
/// their infectiousness with it begins.
#[derive(IxaEvent)]
pub struct InfectiousnessAssignedEvent<M: 'static>
where
    Context: InfectionManager<M>,
{
    pub person: EntityId<<Context as InfectionManager<M>>::Entity>,
    model: PhantomData<fn() -> M>,
}

impl<M: 'static> std::fmt::Debug for InfectiousnessAssignedEvent<M>
where
    Context: InfectionManager<M>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InfectiousnessAssignedEvent")
            .field("person", &self.person)
            .finish()
    }
}

/// Emitted when a modifier change raises an infected person's bound, so the
/// transmission loop can forecast again.
#[derive(IxaEvent)]
pub(super) struct ModifierBoundRaisedEvent<M: 'static, E: Entity> {
    pub(super) person: EntityId<E>,
    model: PhantomData<fn() -> M>,
}

struct Assigned<D> {
    rate: RateAssignment,
    details: D,
    infection_time: f64,
    /// How many times the person was infected before this infection.
    infection_count: u32,
    /// Upper bound on the product of the person's infectiousness modifiers
    /// until it is next refreshed. Kept here so a forecast reads one record.
    modifier_bound: f64,
    /// Which forecast of the person's transmission loop is live: a pending
    /// proposal made under an earlier one is dropped when it fires.
    forecast_generation: u32,
}

/// A modifier factor computed per read from the context, the person, the
/// time elapsed since the modifier was set and an argument `A` of the read:
/// the infection details for susceptibility, nothing for infectiousness.
type ModifierFn<E, A> = Box<dyn Fn(&Context, EntityId<E>, f64, &A) -> f64>;

/// One modifier's factor: a constant, or a function of elapsed time.
enum Factor<E: Entity, A> {
    Constant(f64),
    Fn {
        f: ModifierFn<E, A>,
        since: f64,
        /// Declared upper bound on what `f` returns.
        max: f64,
    },
}

/// How a modifier came to be on a person. A person can have one of each
/// under the same key, and the two multiply.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    /// Set directly: stays until removed.
    Set,
    /// Started by a schedule. `schedule` identifies the call that made it, so
    /// that the end of an earlier schedule does not remove a later one. A
    /// scheduled infectiousness modifier also ends with the infection.
    Scheduled { schedule: u32 },
}

impl Scope {
    fn is_scheduled(self) -> bool {
        matches!(self, Scope::Scheduled { .. })
    }
}

impl<E: Entity, A: 'static> Factor<E, A> {
    /// A function factor set at `since`, checked in debug builds to return
    /// finite values in `[0, max]`.
    fn checked_fn(
        kind: &'static str,
        since: f64,
        max: f64,
        f: impl Fn(&Context, EntityId<E>, f64, &A) -> f64 + 'static,
    ) -> Self {
        let checked = move |context: &Context, person: EntityId<E>, elapsed: f64, argument: &A| {
            let factor = f(context, person, elapsed, argument);
            debug_assert!(
                factor.is_finite() && (0.0..=max).contains(&factor),
                "{kind} modifier must return finite values in [0, {max}], got {factor}"
            );
            factor
        };
        Factor::Fn {
            f: Box::new(checked),
            since,
            max,
        }
    }
}

struct Modifier<E: Entity, A> {
    key: ModifierKey,
    scope: Scope,
    factor: Factor<E, A>,
}

/// Active modifiers on one person, keyed by modifier kind. The product of the
/// constant factors is cached, so a read with no function factors is one
/// lookup; function factors are evaluated per read.
struct Modifiers<E: Entity, A> {
    factors: Vec<Modifier<E, A>>,
    /// Schedules that have not started yet, as the key and the number of the
    /// call. Removing the key cancels them.
    pending: Vec<(ModifierKey, u32)>,
    /// Product of the constant factors.
    constant_product: f64,
    /// Product of the constant factors and the function factors' maxima.
    bound_product: f64,
    function_count: usize,
}

impl<E: Entity, A> Modifiers<E, A> {
    fn recompute(&mut self) {
        self.constant_product = 1.0;
        self.bound_product = 1.0;
        self.function_count = 0;
        for modifier in &self.factors {
            match &modifier.factor {
                Factor::Constant(value) => {
                    self.constant_product *= value;
                    self.bound_product *= value;
                }
                Factor::Fn { max, .. } => {
                    self.bound_product *= max;
                    self.function_count += 1;
                }
            }
        }
    }

    fn entry(map: &mut EntityMap<E, Self>, person: EntityId<E>) -> &mut Self {
        map.get_or_insert_with(person, || Modifiers {
            factors: Vec::new(),
            pending: Vec::new(),
            constant_product: 1.0,
            bound_product: 1.0,
            function_count: 0,
        })
    }

    /// Recompute the cached products after a change, or drop `person`'s
    /// record if nothing is left on it.
    fn refresh(map: &mut EntityMap<E, Self>, person: EntityId<E>) {
        let Some(modifiers) = map.get_mut(person) else {
            return;
        };
        if modifiers.factors.is_empty() && modifiers.pending.is_empty() {
            let _ = map.remove(person);
        } else {
            modifiers.recompute();
        }
    }

    fn set(
        map: &mut EntityMap<E, Self>,
        person: EntityId<E>,
        key: ModifierKey,
        scope: Scope,
        factor: Factor<E, A>,
    ) {
        let modifiers = Self::entry(map, person);
        let modifier = Modifier { key, scope, factor };
        match modifiers.factors.iter_mut().find(|existing| {
            existing.key == key && existing.scope.is_scheduled() == scope.is_scheduled()
        }) {
            Some(existing) => *existing = modifier,
            None => modifiers.factors.push(modifier),
        }
        modifiers.recompute();
    }

    /// Keep only the modifiers on `person` that `keep` accepts.
    fn retain(
        map: &mut EntityMap<E, Self>,
        person: EntityId<E>,
        keep: impl Fn(&Modifier<E, A>) -> bool,
    ) {
        if let Some(modifiers) = map.get_mut(person) {
            modifiers.factors.retain(keep);
        }
        Self::refresh(map, person);
    }

    /// Remove every modifier `key` on `person`, set or scheduled, and cancel
    /// its schedules that have not started.
    fn remove_key(map: &mut EntityMap<E, Self>, person: EntityId<E>, key: ModifierKey) {
        if let Some(modifiers) = map.get_mut(person) {
            modifiers.factors.retain(|modifier| modifier.key != key);
            modifiers
                .pending
                .retain(|&(pending_key, _)| pending_key != key);
        }
        Self::refresh(map, person);
    }

    /// Record that the schedule numbered `schedule` will start `key` later.
    fn add_pending(
        map: &mut EntityMap<E, Self>,
        person: EntityId<E>,
        key: ModifierKey,
        schedule: u32,
    ) {
        Self::entry(map, person).pending.push((key, schedule));
    }

    /// Forget a pending schedule at its start. Returns whether it was still
    /// pending, which it is not after the key was removed.
    fn take_pending(
        map: &mut EntityMap<E, Self>,
        person: EntityId<E>,
        key: ModifierKey,
        schedule: u32,
    ) -> bool {
        let Some(modifiers) = map.get_mut(person) else {
            return false;
        };
        let count_before = modifiers.pending.len();
        modifiers
            .pending
            .retain(|&pending| pending != (key, schedule));
        let was_pending = modifiers.pending.len() < count_before;
        Self::refresh(map, person);
        was_pending
    }

    /// Whether `person` has modifier `key` from a schedule made after
    /// `schedule`.
    fn has_later_schedule(
        map: &EntityMap<E, Self>,
        person: EntityId<E>,
        key: ModifierKey,
        schedule: u32,
    ) -> bool {
        let Some(modifiers) = map.get(person) else {
            return false;
        };
        modifiers.factors.iter().any(|modifier| {
            modifier.key == key
                && matches!(modifier.scope, Scope::Scheduled { schedule: other } if other > schedule)
        })
    }

    /// Upper bound on `person`'s product while their modifiers are unchanged,
    /// and at least one, so that lowering a factor never needs a reforecast.
    fn bound(map: &EntityMap<E, Self>, person: EntityId<E>) -> f64 {
        let Some(modifiers) = map.get(person) else {
            return 1.0;
        };
        let product = modifiers.bound_product;
        assert!(
            product.is_finite(),
            "infectiousness modifiers of {person:?} multiply to {product}"
        );
        product.max(1.0)
    }

    fn product(
        map: &EntityMap<E, Self>,
        person: EntityId<E>,
        context: &Context,
        argument: &A,
    ) -> f64 {
        let Some(modifiers) = map.get(person) else {
            return 1.0;
        };
        let mut product = modifiers.constant_product;
        if modifiers.function_count > 0 {
            let now = context.get_current_time();
            for modifier in &modifiers.factors {
                if let Factor::Fn { f, since, .. } = &modifier.factor {
                    product *= f(context, person, now - since, argument);
                }
            }
        }
        product
    }
}

/// A factor computed per read from the context, the person and the same
/// argument `A` as a [`ModifierFn`].
type PopulationModifierFn<E, A> = Box<dyn Fn(&Context, EntityId<E>, &A) -> f64>;

/// A modifier on everyone.
struct PopulationModifier<E: Entity, A> {
    key: ModifierKey,
    f: PopulationModifierFn<E, A>,
    /// Declared upper bound on what `f` returns.
    max: f64,
}

impl<E: Entity, A: 'static> PopulationModifier<E, A> {
    /// A modifier checked in debug builds to return finite values in
    /// `[0, max]`.
    fn checked(
        kind: &'static str,
        key: ModifierKey,
        max: f64,
        f: impl Fn(&Context, EntityId<E>, &A) -> f64 + 'static,
    ) -> Self {
        let checked = move |context: &Context, person: EntityId<E>, argument: &A| {
            let factor = f(context, person, argument);
            debug_assert!(
                factor.is_finite() && (0.0..=max).contains(&factor),
                "population {kind} modifier must return finite values in [0, {max}], got {factor}"
            );
            factor
        };
        Self {
            key,
            f: Box::new(checked),
            max,
        }
    }

    /// Add `modifier`, replacing one with the same key.
    fn set(modifiers: &mut Vec<Self>, modifier: Self) {
        match modifiers
            .iter_mut()
            .find(|existing| existing.key == modifier.key)
        {
            Some(existing) => *existing = modifier,
            None => modifiers.push(modifier),
        }
    }

    fn product(modifiers: &[Self], context: &Context, person: EntityId<E>, argument: &A) -> f64 {
        let mut product = 1.0;
        for modifier in modifiers {
            product *= (modifier.f)(context, person, argument);
        }
        product
    }
}

/// Everything `core_epi` keeps for one model: its rate library, who is
/// infected with what, and the modifiers on them.
struct InfectionState<E: Entity, D> {
    library: Vec<AnyInfectionRate>,
    assigned: EntityMap<E, Assigned<D>>,
    /// The infections each reinfected person had before their current one,
    /// oldest first.
    past: HashMap<EntityId<E>, Vec<PastInfection<D>>>,
    /// Infectiousness modifiers. On the person, so they can be set before an
    /// infection and survive reinfection, except those scheduled for one
    /// infection.
    modifiers: EntityMap<E, Modifiers<E, ()>>,
    /// Always on the person.
    susceptibility_modifiers: EntityMap<E, Modifiers<E, D>>,
    /// Number of modifiers scheduled so far, which numbers the next one.
    schedule_count: u32,
    /// Whether `setup_transmission_loop` has run.
    loop_started: bool,
    population_modifiers: Vec<PopulationModifier<E, ()>>,
    population_susceptibility_modifiers: Vec<PopulationModifier<E, D>>,
    /// Upper bound on the product of the population infectiousness
    /// modifiers, and at least one. Every forecast is scaled by it.
    population_bound: f64,
}

impl<E: Entity, D> Default for InfectionState<E, D> {
    fn default() -> Self {
        Self {
            library: Vec::new(),
            assigned: EntityMap::new(),
            past: HashMap::default(),
            modifiers: EntityMap::new(),
            susceptibility_modifiers: EntityMap::new(),
            schedule_count: 0,
            loop_started: false,
            population_modifiers: Vec::new(),
            population_susceptibility_modifiers: Vec::new(),
            population_bound: 1.0,
        }
    }
}

impl<E: Entity, D> InfectionState<E, D> {
    /// The rate function `assigned` points at.
    fn rate_fn<'a>(&'a self, assigned: &'a Assigned<D>) -> &'a AnyInfectionRate {
        match &assigned.rate {
            RateAssignment::Library(index) => &self.library[*index],
            RateAssignment::Own(rate_fn) => rate_fn.as_ref(),
        }
    }
}

// One `InfectionState<E, D>` per model marker `M`, so two pathogens on one
// population keep separate infections. A data plugin cannot be generic, so
// the stores are type-erased; there are only ever a few, so a scan beats a
// hash.
define_data_plugin!(
    InfectionStatePlugin,
    Vec<(TypeId, Box<dyn Any>)>,
    Vec::new()
);

fn state<M: 'static, C: InfectionManager<M>>(
    context: &C,
) -> Option<&InfectionState<C::Entity, C::Details>>
where
    Context: InfectionManager<M, Entity = C::Entity>,
{
    let model = TypeId::of::<M>();
    let stores = context.get_data(InfectionStatePlugin);
    let (_, store) = stores.iter().find(|(id, _)| *id == model)?;
    Some(
        store
            .downcast_ref()
            .expect("infection state has the wrong type"),
    )
}

fn state_mut<M: 'static, C: InfectionManager<M>>(
    context: &mut C,
) -> &mut InfectionState<C::Entity, C::Details>
where
    Context: InfectionManager<M, Entity = C::Entity>,
{
    let model = TypeId::of::<M>();
    let stores = context.get_data_mut(InfectionStatePlugin);
    let index = match stores.iter().position(|(id, _)| *id == model) {
        Some(index) => index,
        None => {
            stores.push((
                model,
                Box::new(InfectionState::<C::Entity, C::Details>::default()),
            ));
            stores.len() - 1
        }
    };
    stores[index]
        .1
        .downcast_mut()
        .expect("infection state has the wrong type")
}

/// The live forecast of `person`'s transmission loop; `None` if they were
/// never infected.
pub(super) fn forecast_generation<M: 'static, C: InfectionManager<M>>(
    context: &C,
    person: EntityId<C::Entity>,
) -> Option<u32>
where
    Context: InfectionManager<M, Entity = C::Entity>,
{
    Some(
        state::<M, _>(context)?
            .assigned
            .get(person)?
            .forecast_generation,
    )
}

/// Record that the transmission loop is set up. Returns everyone infected
/// before that, whose loops have not started; `None` if it was set up already.
pub(super) fn start_transmission_loop<M: 'static, C: InfectionManager<M>>(
    context: &mut C,
) -> Option<Vec<EntityId<C::Entity>>>
where
    Context: InfectionManager<M, Entity = C::Entity>,
{
    let state = state_mut::<M, _>(context);
    if state.loop_started {
        return None;
    }
    state.loop_started = true;
    Some(state.assigned.iter().map(|(person, _)| person).collect())
}

/// Supersede `person`'s pending proposal, returning the new live forecast;
/// `None` if they were never infected.
pub(super) fn advance_forecast_generation<M: 'static, C: InfectionManager<M>>(
    context: &mut C,
    person: EntityId<C::Entity>,
) -> Option<u32>
where
    Context: InfectionManager<M, Entity = C::Entity>,
{
    let assigned = state_mut::<M, _>(context).assigned.get_mut(person)?;
    assigned.forecast_generation = assigned.forecast_generation.wrapping_add(1);
    Some(assigned.forecast_generation)
}

fn assert_valid_factor(kind: &str, factor: f64) {
    assert!(
        factor.is_finite() && factor >= 0.0,
        "{kind} modifier factor must be finite and non-negative, got {factor}"
    );
}

/// Set infectiousness modifier `key` on `person` and refresh their bound.
fn set_infectiousness_factor<M: 'static, C: InfectionManager<M>>(
    context: &mut C,
    person: EntityId<C::Entity>,
    key: ModifierKey,
    scope: Scope,
    factor: Factor<C::Entity, ()>,
) where
    Context: InfectionManager<M, Entity = C::Entity>,
{
    let state = state_mut::<M, _>(context);
    Modifiers::set(&mut state.modifiers, person, key, scope, factor);
    refresh_modifier_bound::<M, _>(context, person);
}

/// Keep only the infectiousness modifiers on `person` that `keep` accepts, and
/// refresh their bound.
fn retain_infectiousness_factors<M: 'static, C: InfectionManager<M>>(
    context: &mut C,
    person: EntityId<C::Entity>,
    keep: impl Fn(&Modifier<C::Entity, ()>) -> bool,
) where
    Context: InfectionManager<M, Entity = C::Entity>,
{
    let state = state_mut::<M, _>(context);
    Modifiers::retain(&mut state.modifiers, person, keep);
    refresh_modifier_bound::<M, _>(context, person);
}

fn assert_end_not_before_start(start: f64, end: Option<f64>) {
    if let Some(end) = end {
        assert!(
            end >= start,
            "modifier end {end} is before its start {start}"
        );
    }
}

/// Which of `person`'s infections is current, from zero; `None` if never
/// infected.
fn infection_index<M: 'static, C: InfectionManager<M>>(
    context: &C,
    person: EntityId<C::Entity>,
) -> Option<u32>
where
    Context: InfectionManager<M, Entity = C::Entity>,
{
    Some(
        state::<M, _>(context)?
            .assigned
            .get(person)?
            .infection_count,
    )
}

/// Recompute `person`'s modifier bound after their infectiousness modifiers
/// changed. A rise makes their pending proposal invalid, so it is announced
/// for the transmission loop to forecast again.
fn refresh_modifier_bound<M: 'static, C: InfectionManager<M>>(
    context: &mut C,
    person: EntityId<C::Entity>,
) where
    Context: InfectionManager<M, Entity = C::Entity>,
{
    let state = state_mut::<M, _>(context);
    let bound = Modifiers::bound(&state.modifiers, person);
    let Some(assigned) = state.assigned.get_mut(person) else {
        return;
    };
    let raised = bound > assigned.modifier_bound;
    assigned.modifier_bound = bound;
    if raised {
        context.emit_event(ModifierBoundRaisedEvent::<M, C::Entity> {
            person,
            model: PhantomData,
        });
    }
}

/// Recompute the bound on the population infectiousness modifiers after they
/// changed. A rise makes every pending proposal invalid, so it is announced
/// for each person still infectious.
fn refresh_population_bound<M: 'static, C: InfectionManager<M>>(context: &mut C)
where
    Context: InfectionManager<M, Entity = C::Entity>,
{
    let now = context.get_current_time();
    let state = state_mut::<M, _>(context);
    let mut product = 1.0;
    for modifier in &state.population_modifiers {
        product *= modifier.max;
    }
    assert!(
        product.is_finite(),
        "population infectiousness modifiers multiply to {product}"
    );
    let bound = product.max(1.0);
    let raised = bound > state.population_bound;
    state.population_bound = bound;
    if !raised {
        return;
    }
    let mut infectious = Vec::new();
    for (person, assigned) in state.assigned.iter() {
        if now - assigned.infection_time < state.rate_fn(assigned).duration() {
            infectious.push(person);
        }
    }
    for person in infectious {
        context.emit_event(ModifierBoundRaisedEvent::<M, C::Entity> {
            person,
            model: PhantomData,
        });
    }
}

/// A person's infectiousness over time and their susceptibility. `M` is the
/// model's marker type (see [`TransmissionManager`](super::TransmissionManager))
/// and `Entity` the entity that gets infected, e.g. the model's `Person`.
pub trait InfectionManager<M: 'static>: ContextBase + AsRef<Context>
where
    Context: InfectionManager<M, Entity = Self::Entity>,
{
    type Entity: Entity;

    /// What an infection carries besides who and when, e.g. the variant or
    /// the setting it happened in, or `()`. The contact draw makes it, the
    /// contact's susceptibility and rate function can depend on it, and it is
    /// stored with the infection. A seed gets the default unless given one.
    type Details: Default + Clone + 'static;

    /// Draw the rate function for `person`, newly infected with `details`.
    /// [`infection_details`](Self::infection_details) is still their previous
    /// infection's here.
    fn draw_infectiousness_rate_fn(
        &mut self,
        person: EntityId<Self::Entity>,
        details: &Self::Details,
    ) -> RateAssignment;

    /// Load the shared rate functions that [`RateAssignment::Library`] indexes
    /// into. Call at setup, before anyone is infected.
    ///
    /// # Panics
    ///
    /// If anyone has been infected, since their index may no longer fit.
    fn load_infectiousness_rate_library(&mut self, rate_fns: Vec<AnyInfectionRate>) {
        let state = state_mut::<M, _>(self);
        assert!(
            state.assigned.is_empty(),
            "the infectiousness rate library cannot be loaded once anyone is infected"
        );
        state.library = rate_fns;
    }

    /// Draw and store `person`'s rate function, starting now, and emit
    /// [`InfectiousnessAssignedEvent`]. A reinfection replaces the previous
    /// one, which moves to [`past_infections`](Self::past_infections).
    fn assign_infectiousness_rate_fn(
        &mut self,
        person: EntityId<Self::Entity>,
        details: Self::Details,
    ) {
        let rate = self.draw_infectiousness_rate_fn(person, &details);
        self.set_infectiousness_rate_fn(person, rate, details);
    }

    /// As [`assign_infectiousness_rate_fn`](Self::assign_infectiousness_rate_fn)
    /// with a given rate function instead of a drawn one. Neither calls
    /// [`on_infected`](Self::on_infected).
    ///
    /// # Panics
    ///
    /// If `rate` is a library index outside the loaded library.
    fn set_infectiousness_rate_fn(
        &mut self,
        person: EntityId<Self::Entity>,
        rate: impl Into<RateAssignment>,
        details: Self::Details,
    ) {
        let rate = rate.into();
        let infection_time = self.get_current_time();
        let state = state_mut::<M, _>(self);
        if let RateAssignment::Library(index) = rate {
            assert!(
                index < state.library.len(),
                "rate function {index} is not in the library of {}: load it with \
                 `load_infectiousness_rate_library` before anyone is infected",
                state.library.len()
            );
        }
        // A reinfection supersedes the previous infection's pending proposal.
        let (infection_count, forecast_generation) =
            state.assigned.get(person).map_or((0, 0), |previous| {
                (
                    previous.infection_count.wrapping_add(1),
                    previous.forecast_generation.wrapping_add(1),
                )
            });
        // Modifiers scheduled for the previous infection end with it.
        Modifiers::retain(&mut state.modifiers, person, |modifier| {
            !modifier.scope.is_scheduled()
        });
        let modifier_bound = Modifiers::bound(&state.modifiers, person);
        let replaced = state.assigned.insert(
            person,
            Assigned {
                rate,
                details,
                infection_time,
                infection_count,
                modifier_bound,
                forecast_generation,
            },
        );
        if let Some(replaced) = replaced {
            let ran_out = replaced.infection_time + state.rate_fn(&replaced).duration();
            state.past.entry(person).or_default().push(PastInfection {
                time: replaced.infection_time,
                infectiousness_end: ran_out.min(infection_time),
                details: replaced.details,
            });
        }
        self.emit_event(InfectiousnessAssignedEvent::<M> {
            person,
            model: PhantomData,
        });
    }

    /// `person`'s rate function, indexed by time since infection; `None` if
    /// they were never assigned one.
    fn infectiousness_rate_fn(&self, person: EntityId<Self::Entity>) -> Option<&AnyInfectionRate> {
        let state = state::<M, _>(self)?;
        Some(state.rate_fn(state.assigned.get(person)?))
    }

    /// When `person`'s current infection began; `None` if they were never infected.
    fn infection_time(&self, person: EntityId<Self::Entity>) -> Option<f64> {
        let assigned = state::<M, _>(self)?.assigned.get(person)?;
        Some(assigned.infection_time)
    }

    /// The details of `person`'s current infection; `None` if they were
    /// never infected.
    fn infection_details(&self, person: EntityId<Self::Entity>) -> Option<&Self::Details> {
        let assigned = state::<M, _>(self)?.assigned.get(person)?;
        Some(&assigned.details)
    }

    /// How many times `person` has been infected.
    fn infection_count(&self, person: EntityId<Self::Entity>) -> usize {
        infection_index::<M, _>(self, person).map_or(0, |index| index as usize + 1)
    }

    /// The infections `person` had before their current one, oldest first.
    fn past_infections(&self, person: EntityId<Self::Entity>) -> &[PastInfection<Self::Details>] {
        state::<M, _>(self)
            .and_then(|state| state.past.get(&person))
            .map_or(&[], Vec::as_slice)
    }

    /// Time since `person`'s rate function was assigned; `None` if it never was.
    fn time_since_infection(&self, person: EntityId<Self::Entity>) -> Option<f64> {
        Some(self.get_current_time() - self.infection_time(person)?)
    }

    /// When `person`'s current rate function runs out; `None` if they were
    /// never infected.
    fn infectiousness_end_time(&self, person: EntityId<Self::Entity>) -> Option<f64> {
        Some(self.infection_time(person)? + self.infectiousness_rate_fn(person)?.duration())
    }

    /// How long `person`'s current infection is infectious for; `None` if
    /// they were never infected.
    fn infectiousness_duration(&self, person: EntityId<Self::Entity>) -> Option<f64> {
        Some(self.infectiousness_rate_fn(person)?.duration())
    }

    /// Whether `person` has a rate function that has not yet run out.
    fn is_infectious(&self, person: EntityId<Self::Entity>) -> bool {
        self.infectiousness_end_time(person)
            .is_some_and(|end| self.get_current_time() < end)
    }

    /// Upper bound on `person`'s intrinsic infectiousness, indexed by time
    /// from now: their rate function, scaled up by modifiers above one.
    ///
    /// # Panics
    ///
    /// If `person` was never infected.
    fn majorizing_infectiousness_rate_fn(
        &self,
        person: EntityId<Self::Entity>,
    ) -> TransformedInvertibleRateFn<'_, AnyInfectionRate> {
        // One record lookup: this runs once per proposal.
        let state = state::<M, _>(self).expect("person was never infected");
        let assigned = state
            .assigned
            .get(person)
            .expect("person was never infected");
        TransformedInvertibleRateFn::new(
            state.rate_fn(assigned),
            assigned.modifier_bound * state.population_bound,
            self.get_current_time() - assigned.infection_time,
        )
    }

    /// Current infectiousness before any contact structure: the rate
    /// function now times [`Self::infectiousness_multiplier`]. `None` if never
    /// infected; 0 once the rate function has run out.
    fn intrinsic_infectiousness(&self, person: EntityId<Self::Entity>) -> Option<f64> {
        let state = state::<M, _>(self)?;
        let assigned = state.assigned.get(person)?;
        let rate = state
            .rate_fn(assigned)
            .rate(self.get_current_time() - assigned.infection_time);
        let multiplier = Modifiers::product(&state.modifiers, person, self.as_ref(), &())
            * PopulationModifier::product(&state.population_modifiers, self.as_ref(), person, &());
        Some(multiplier * rate)
    }

    /// Product of `person`'s active infectiousness modifiers, their own and
    /// the population's (`1.0` if none).
    fn infectiousness_multiplier(&self, person: EntityId<Self::Entity>) -> f64 {
        state::<M, _>(self).map_or(1.0, |state| {
            Modifiers::product(&state.modifiers, person, self.as_ref(), &())
                * PopulationModifier::product(
                    &state.population_modifiers,
                    self.as_ref(),
                    person,
                    &(),
                )
        })
    }

    /// Scale everyone's infectiousness by a factor in `[0, 1]` computed at
    /// every read from the context and the person, e.g. from a property,
    /// until `modifier` is removed; setting it again replaces it. It
    /// multiplies with each person's own modifiers.
    fn set_population_infectiousness_modifier<K: ModifierKind>(
        &mut self,
        modifier: K,
        f: impl Fn(&Context, EntityId<Self::Entity>) -> f64 + 'static,
    ) {
        self.set_population_infectiousness_modifier_with_max(modifier, 1.0, f);
    }

    /// As [`set_population_infectiousness_modifier`](Self::set_population_infectiousness_modifier)
    /// for a function returning values in `[0, max]`. Everyone is forecast
    /// at `max` throughout, so a loose `max` costs rejected proposals.
    fn set_population_infectiousness_modifier_with_max<K: ModifierKind>(
        &mut self,
        modifier: K,
        max: f64,
        f: impl Fn(&Context, EntityId<Self::Entity>) -> f64 + 'static,
    ) {
        assert_valid_factor("infectiousness", max);
        let state = state_mut::<M, _>(self);
        PopulationModifier::set(
            &mut state.population_modifiers,
            PopulationModifier::checked(
                "infectiousness",
                modifier.key(),
                max,
                move |context, person, _| f(context, person),
            ),
        );
        refresh_population_bound::<M, _>(self);
    }

    /// Remove population infectiousness `modifier`, if set.
    fn remove_population_infectiousness_modifier<K: ModifierKind>(&mut self, modifier: K) {
        let key = modifier.key();
        state_mut::<M, _>(self)
            .population_modifiers
            .retain(|existing| existing.key != key);
        refresh_population_bound::<M, _>(self);
    }

    /// Scale `person`'s infectiousness by a finite, non-negative `factor`
    /// until `modifier` is removed; setting it again replaces it. It
    /// belongs to the person, so it survives reinfection.
    fn set_infectiousness_modifier<K: ModifierKind>(
        &mut self,
        person: EntityId<Self::Entity>,
        modifier: K,
        factor: f64,
    ) {
        assert_valid_factor("infectiousness", factor);
        set_infectiousness_factor::<M, _>(
            self,
            person,
            modifier.key(),
            Scope::Set,
            Factor::Constant(factor),
        );
    }

    /// As [`set_infectiousness_modifier`](Self::set_infectiousness_modifier),
    /// with a factor in `[0, 1]` computed at every read from the context, the
    /// person and the time since it was set.
    fn set_infectiousness_modifier_fn<K: ModifierKind>(
        &mut self,
        person: EntityId<Self::Entity>,
        modifier: K,
        f: impl Fn(&Context, EntityId<Self::Entity>, f64) -> f64 + 'static,
    ) {
        self.set_infectiousness_modifier_fn_with_max(person, modifier, 1.0, f);
    }

    /// As [`set_infectiousness_modifier_fn`](Self::set_infectiousness_modifier_fn)
    /// for a function returning values in `[0, max]`. The person is forecast
    /// at `max` throughout, so a loose `max` costs rejected proposals.
    fn set_infectiousness_modifier_fn_with_max<K: ModifierKind>(
        &mut self,
        person: EntityId<Self::Entity>,
        modifier: K,
        max: f64,
        f: impl Fn(&Context, EntityId<Self::Entity>, f64) -> f64 + 'static,
    ) {
        assert_valid_factor("infectiousness", max);
        let since = self.get_current_time();
        set_infectiousness_factor::<M, _>(
            self,
            person,
            modifier.key(),
            Scope::Set,
            Factor::checked_fn(
                "infectiousness",
                since,
                max,
                move |context, person, elapsed, _| f(context, person, elapsed),
            ),
        );
    }

    /// Remove infectiousness `modifier` from `person`, whether set or
    /// scheduled, and cancel schedules of it that have not started.
    fn remove_infectiousness_modifier<K: ModifierKind>(
        &mut self,
        person: EntityId<Self::Entity>,
        modifier: K,
    ) {
        let state = state_mut::<M, _>(self);
        Modifiers::remove_key(&mut state.modifiers, person, modifier.key());
        refresh_modifier_bound::<M, _>(self, person);
    }

    /// Set `modifier` on `person`'s current infection at `start` and, if
    /// given, remove it at `end`. It belongs to this infection: a reinfection
    /// drops it, and it multiplies with the same kind set on the person. One
    /// schedule of a kind is active at a time: where two overlap, the later
    /// call's applies and the earlier one does not resume after it. The start and
    /// end do not keep the simulation running on their own.
    /// No-op if `person` was never infected.
    ///
    /// # Panics
    ///
    /// If `end` is before `start`.
    fn schedule_infectiousness_modifier<K: ModifierKind>(
        &mut self,
        person: EntityId<Self::Entity>,
        modifier: K,
        factor: f64,
        start: f64,
        end: Option<f64>,
    ) {
        assert_valid_factor("infectiousness", factor);
        assert_end_not_before_start(start, end);
        let Some(scheduled_for) = infection_index::<M, _>(self, person) else {
            return;
        };
        let key = modifier.key();
        let state = state_mut::<M, _>(self);
        let schedule = state.schedule_count;
        let scope = Scope::Scheduled { schedule };
        state.schedule_count = state.schedule_count.wrapping_add(1);
        Modifiers::add_pending(&mut state.modifiers, person, key, schedule);
        self.add_passive_plan(start, move |ctx| {
            let state = state_mut::<M, _>(ctx);
            let cancelled = !Modifiers::take_pending(&mut state.modifiers, person, key, schedule);
            if cancelled || Modifiers::has_later_schedule(&state.modifiers, person, key, schedule) {
                return;
            }
            if infection_index::<M, _>(ctx, person) == Some(scheduled_for)
                && InfectionManager::<M>::is_infectious(ctx, person)
            {
                set_infectiousness_factor::<M, _>(
                    ctx,
                    person,
                    key,
                    scope,
                    Factor::Constant(factor),
                );
            }
        });
        if let Some(end) = end {
            // Gone already if a reinfection or a later schedule replaced it.
            self.add_passive_plan(end, move |ctx| {
                retain_infectiousness_factors::<M, _>(ctx, person, |modifier| {
                    modifier.key != key || modifier.scope != scope
                });
            });
        }
    }

    /// Probability that an attempt by `infector` carrying `details` infects
    /// `person`, before modifiers. `infector` is `None` when the model asks
    /// with no particular infector; the loop always passes one. The default
    /// is permanent immunity: `1.0` if never infected, else `0.0`.
    fn base_susceptibility(
        &self,
        person: EntityId<Self::Entity>,
        _infector: Option<EntityId<Self::Entity>>,
        _details: &Self::Details,
    ) -> f64 {
        if self.infection_time(person).is_none() {
            1.0
        } else {
            0.0
        }
    }

    /// Probability that an attempt by `infector` carrying `details` infects
    /// `person`: [`base_susceptibility`](Self::base_susceptibility) times
    /// [`susceptibility_multiplier`](Self::susceptibility_multiplier). The
    /// transmission loop panics if it is outside `[0, 1]`.
    fn susceptibility(
        &self,
        person: EntityId<Self::Entity>,
        infector: Option<EntityId<Self::Entity>>,
        details: &Self::Details,
    ) -> f64 {
        self.base_susceptibility(person, infector, details)
            * self.susceptibility_multiplier(person, details)
    }

    /// Product of `person`'s active susceptibility modifiers, their own and
    /// the population's, against an attempt carrying `details` (`1.0` if
    /// none).
    fn susceptibility_multiplier(
        &self,
        person: EntityId<Self::Entity>,
        details: &Self::Details,
    ) -> f64 {
        state::<M, _>(self).map_or(1.0, |state| {
            Modifiers::product(
                &state.susceptibility_modifiers,
                person,
                self.as_ref(),
                details,
            ) * PopulationModifier::product(
                &state.population_susceptibility_modifiers,
                self.as_ref(),
                person,
                details,
            )
        })
    }

    /// Scale everyone's susceptibility by a factor computed at every read
    /// from the context, the person and the details of the attempt, until
    /// `modifier` is removed; setting it again replaces it. It multiplies
    /// with each person's own modifiers.
    fn set_population_susceptibility_modifier<K: ModifierKind>(
        &mut self,
        modifier: K,
        f: impl Fn(&Context, EntityId<Self::Entity>, &Self::Details) -> f64 + 'static,
    ) {
        let state = state_mut::<M, _>(self);
        PopulationModifier::set(
            &mut state.population_susceptibility_modifiers,
            // No maximum: susceptibility has no forecast to bound.
            PopulationModifier::checked("susceptibility", modifier.key(), f64::INFINITY, f),
        );
    }

    /// Remove population susceptibility `modifier`, if set.
    fn remove_population_susceptibility_modifier<K: ModifierKind>(&mut self, modifier: K) {
        let key = modifier.key();
        state_mut::<M, _>(self)
            .population_susceptibility_modifiers
            .retain(|existing| existing.key != key);
    }

    /// Scale `person`'s susceptibility by a finite, non-negative `factor`
    /// until `modifier` is removed; setting it again replaces it.
    fn set_susceptibility_modifier<K: ModifierKind>(
        &mut self,
        person: EntityId<Self::Entity>,
        modifier: K,
        factor: f64,
    ) {
        assert_valid_factor("susceptibility", factor);
        let state = state_mut::<M, _>(self);
        Modifiers::set(
            &mut state.susceptibility_modifiers,
            person,
            modifier.key(),
            Scope::Set,
            Factor::Constant(factor),
        );
    }

    /// As [`set_susceptibility_modifier`](Self::set_susceptibility_modifier),
    /// with a factor computed at every read from the context, the person,
    /// the time since it was set and the details of the attempt, so it can
    /// differ by variant.
    fn set_susceptibility_modifier_fn<K: ModifierKind>(
        &mut self,
        person: EntityId<Self::Entity>,
        modifier: K,
        f: impl Fn(&Context, EntityId<Self::Entity>, f64, &Self::Details) -> f64 + 'static,
    ) {
        let since = self.get_current_time();
        let state = state_mut::<M, _>(self);
        Modifiers::set(
            &mut state.susceptibility_modifiers,
            person,
            modifier.key(),
            Scope::Set,
            // No maximum: susceptibility has no forecast to bound.
            Factor::checked_fn("susceptibility", since, f64::INFINITY, f),
        );
    }

    /// Remove susceptibility `modifier` from `person`, whether set or
    /// scheduled, and cancel schedules of it that have not started.
    fn remove_susceptibility_modifier<K: ModifierKind>(
        &mut self,
        person: EntityId<Self::Entity>,
        modifier: K,
    ) {
        let state = state_mut::<M, _>(self);
        Modifiers::remove_key(&mut state.susceptibility_modifiers, person, modifier.key());
    }

    /// Set susceptibility `modifier` on `person` at time `start` and, if
    /// given, remove it at `end`. It multiplies with the same kind set on the
    /// person. One schedule of a kind is active at a time: where two overlap, the
    /// later call's applies and the earlier one does not resume after it. The
    /// start and end do not keep the simulation running on their own.
    ///
    /// # Panics
    ///
    /// If `end` is before `start`.
    fn schedule_susceptibility_modifier<K: ModifierKind>(
        &mut self,
        person: EntityId<Self::Entity>,
        modifier: K,
        factor: f64,
        start: f64,
        end: Option<f64>,
    ) {
        assert_valid_factor("susceptibility", factor);
        assert_end_not_before_start(start, end);
        let key = modifier.key();
        let state = state_mut::<M, _>(self);
        let schedule = state.schedule_count;
        let scope = Scope::Scheduled { schedule };
        state.schedule_count = state.schedule_count.wrapping_add(1);
        Modifiers::add_pending(&mut state.susceptibility_modifiers, person, key, schedule);
        self.add_passive_plan(start, move |ctx| {
            let modifiers = &mut state_mut::<M, _>(ctx).susceptibility_modifiers;
            let cancelled = !Modifiers::take_pending(modifiers, person, key, schedule);
            if cancelled || Modifiers::has_later_schedule(modifiers, person, key, schedule) {
                return;
            }
            Modifiers::set(modifiers, person, key, scope, Factor::Constant(factor));
        });
        if let Some(end) = end {
            // Gone already if a later schedule replaced it or it was removed.
            self.add_passive_plan(end, move |ctx| {
                let state = state_mut::<M, _>(ctx);
                Modifiers::retain(&mut state.susceptibility_modifiers, person, |modifier| {
                    modifier.key != key || modifier.scope != scope
                });
            });
        }
    }

    /// Called when `infectee` is infected, after their rate function is
    /// assigned. A seed has no `infector`.
    fn on_infected(
        &mut self,
        _infector: Option<EntityId<Self::Entity>>,
        _infectee: EntityId<Self::Entity>,
        _details: Self::Details,
    ) {
    }
}
