use std::any::{Any, TypeId};
use std::marker::PhantomData;

use crate::core_epi::rate::{AnyInfectionRate, InvertibleRate, TransformedInvertibleRateFn};
use crate::data_structures::entity_map::EntityMap;
use crate::entity::{Entity, EntityId};
use crate::prelude::*;
use crate::{ContextBase, IxaEvent};

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

/// What a modifier belongs to: the person, or the infection they had when it
/// was scheduled, in which case a reinfection drops it. A person can have one
/// of each under the same key.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    Person,
    /// `schedule` identifies the call that scheduled it, so that the end of
    /// an earlier schedule does not remove a later one.
    Infection {
        schedule: u32,
    },
}

impl Scope {
    fn is_person(self) -> bool {
        self == Scope::Person
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
    key: TypeId,
    scope: Scope,
    factor: Factor<E, A>,
}

/// Active modifiers on one person, keyed by modifier type. The product of the
/// constant factors is cached, so a read with no function factors is one
/// lookup; function factors are evaluated per read.
struct Modifiers<E: Entity, A> {
    factors: Vec<Modifier<E, A>>,
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

    fn set(
        map: &mut EntityMap<E, Self>,
        person: EntityId<E>,
        key: TypeId,
        scope: Scope,
        factor: Factor<E, A>,
    ) {
        let modifiers = map.get_or_insert_with(person, || Modifiers {
            factors: Vec::new(),
            constant_product: 1.0,
            bound_product: 1.0,
            function_count: 0,
        });
        let modifier = Modifier { key, scope, factor };
        match modifiers
            .factors
            .iter_mut()
            .find(|existing| existing.key == key && existing.scope.is_person() == scope.is_person())
        {
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
        let Some(modifiers) = map.get_mut(person) else {
            return;
        };
        modifiers.factors.retain(keep);
        if modifiers.factors.is_empty() {
            let _ = map.remove(person);
        } else {
            modifiers.recompute();
        }
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
    key: TypeId,
    f: PopulationModifierFn<E, A>,
    /// Declared upper bound on what `f` returns.
    max: f64,
}

impl<E: Entity, A: 'static> PopulationModifier<E, A> {
    /// A modifier checked in debug builds to return finite values in
    /// `[0, max]`.
    fn checked(
        kind: &'static str,
        key: TypeId,
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
    /// Infectiousness modifiers. On the person, so they can be set before an
    /// infection and survive reinfection, except those scheduled for one
    /// infection.
    modifiers: EntityMap<E, Modifiers<E, ()>>,
    /// Always on the person.
    susceptibility_modifiers: EntityMap<E, Modifiers<E, D>>,
    /// Number of modifiers scheduled so far, which numbers the next one.
    schedule_count: u32,
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
            modifiers: EntityMap::new(),
            susceptibility_modifiers: EntityMap::new(),
            schedule_count: 0,
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

fn state<M: 'static, E: Entity, D: 'static, C: ContextBase>(
    context: &C,
) -> Option<&InfectionState<E, D>> {
    let model = TypeId::of::<M>();
    let stores = context.get_data(InfectionStatePlugin);
    let (_, store) = stores.iter().find(|(id, _)| *id == model)?;
    Some(
        store
            .downcast_ref()
            .expect("infection state has the wrong type"),
    )
}

fn state_mut<M: 'static, E: Entity, D: 'static, C: ContextBase>(
    context: &mut C,
) -> &mut InfectionState<E, D> {
    let model = TypeId::of::<M>();
    let stores = context.get_data_mut(InfectionStatePlugin);
    let index = match stores.iter().position(|(id, _)| *id == model) {
        Some(index) => index,
        None => {
            stores.push((model, Box::new(InfectionState::<E, D>::default())));
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
pub(super) fn forecast_generation<M: 'static, E: Entity, D: 'static, C: ContextBase>(
    context: &C,
    person: EntityId<E>,
) -> Option<u32> {
    Some(
        state::<M, E, D, _>(context)?
            .assigned
            .get(person)?
            .forecast_generation,
    )
}

/// Supersede `person`'s pending proposal, returning the new live forecast;
/// `None` if they were never infected.
pub(super) fn advance_forecast_generation<M: 'static, E: Entity, D: 'static, C: ContextBase>(
    context: &mut C,
    person: EntityId<E>,
) -> Option<u32> {
    let assigned = state_mut::<M, E, D, _>(context).assigned.get_mut(person)?;
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
fn set_infectiousness_factor<M: 'static, E: Entity, D: 'static, C: ContextBase>(
    context: &mut C,
    person: EntityId<E>,
    key: TypeId,
    scope: Scope,
    factor: Factor<E, ()>,
) {
    let state = state_mut::<M, E, D, _>(context);
    Modifiers::set(&mut state.modifiers, person, key, scope, factor);
    refresh_modifier_bound::<M, E, D, _>(context, person);
}

/// Keep only the infectiousness modifiers on `person` that `keep` accepts, and
/// refresh their bound.
fn retain_infectiousness_factors<M: 'static, E: Entity, D: 'static, C: ContextBase>(
    context: &mut C,
    person: EntityId<E>,
    keep: impl Fn(&Modifier<E, ()>) -> bool,
) {
    let state = state_mut::<M, E, D, _>(context);
    Modifiers::retain(&mut state.modifiers, person, keep);
    refresh_modifier_bound::<M, E, D, _>(context, person);
}

/// Whether `person` has modifier `key` from a schedule made after `schedule`.
fn has_later_schedule<M: 'static, E: Entity, D: 'static, C: ContextBase>(
    context: &C,
    person: EntityId<E>,
    key: TypeId,
    schedule: u32,
) -> bool {
    let Some(modifiers) =
        state::<M, E, D, _>(context).and_then(|state| state.modifiers.get(person))
    else {
        return false;
    };
    modifiers.factors.iter().any(|modifier| {
        modifier.key == key
            && matches!(modifier.scope, Scope::Infection { schedule: other } if other > schedule)
    })
}

fn assert_end_not_before_start(start: f64, end: Option<f64>) {
    if let Some(end) = end {
        assert!(
            end >= start,
            "modifier end {end} is before its start {start}"
        );
    }
}

/// Which of `person`'s infections is current; `None` if never infected.
fn infection_count<M: 'static, E: Entity, D: 'static, C: ContextBase>(
    context: &C,
    person: EntityId<E>,
) -> Option<u32> {
    Some(
        state::<M, E, D, _>(context)?
            .assigned
            .get(person)?
            .infection_count,
    )
}

/// Recompute `person`'s modifier bound after their infectiousness modifiers
/// changed. A rise makes their pending proposal invalid, so it is announced
/// for the transmission loop to forecast again.
fn refresh_modifier_bound<M: 'static, E: Entity, D: 'static, C: ContextBase>(
    context: &mut C,
    person: EntityId<E>,
) {
    let state = state_mut::<M, E, D, _>(context);
    let bound = Modifiers::bound(&state.modifiers, person);
    let Some(assigned) = state.assigned.get_mut(person) else {
        return;
    };
    let raised = bound > assigned.modifier_bound;
    assigned.modifier_bound = bound;
    if raised {
        context.emit_event(ModifierBoundRaisedEvent::<M, E> {
            person,
            model: PhantomData,
        });
    }
}

/// Recompute the bound on the population infectiousness modifiers after they
/// changed. A rise makes every pending proposal invalid, so it is announced
/// for each person still infectious.
fn refresh_population_bound<M: 'static, E: Entity, D: 'static, C: ContextBase>(context: &mut C) {
    let now = context.get_current_time();
    let state = state_mut::<M, E, D, _>(context);
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
        context.emit_event(ModifierBoundRaisedEvent::<M, E> {
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
    /// into. Call once at setup, before anyone is infected.
    fn load_infectiousness_rate_library(&mut self, rate_fns: Vec<AnyInfectionRate>) {
        state_mut::<M, Self::Entity, Self::Details, _>(self).library = rate_fns;
    }

    /// Draw and store `person`'s rate function, starting now, and emit
    /// [`InfectiousnessAssignedEvent`]. A reinfection replaces the previous one.
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
        let state = state_mut::<M, Self::Entity, Self::Details, _>(self);
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
            modifier.scope.is_person()
        });
        let modifier_bound = Modifiers::bound(&state.modifiers, person);
        state.assigned.insert(
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
        self.emit_event(InfectiousnessAssignedEvent::<M> {
            person,
            model: PhantomData,
        });
    }

    /// `person`'s rate function, indexed by time since infection; `None` if
    /// they were never assigned one.
    fn infectiousness_rate_fn(&self, person: EntityId<Self::Entity>) -> Option<&AnyInfectionRate> {
        let state = state::<M, Self::Entity, Self::Details, _>(self)?;
        Some(state.rate_fn(state.assigned.get(person)?))
    }

    /// When `person`'s current infection began; `None` if they were never infected.
    fn infection_time(&self, person: EntityId<Self::Entity>) -> Option<f64> {
        let assigned = state::<M, Self::Entity, Self::Details, _>(self)?
            .assigned
            .get(person)?;
        Some(assigned.infection_time)
    }

    /// The details of `person`'s current infection; `None` if they were
    /// never infected.
    fn infection_details(&self, person: EntityId<Self::Entity>) -> Option<&Self::Details> {
        let assigned = state::<M, Self::Entity, Self::Details, _>(self)?
            .assigned
            .get(person)?;
        Some(&assigned.details)
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
        let state =
            state::<M, Self::Entity, Self::Details, _>(self).expect("person was never infected");
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
        let state = state::<M, Self::Entity, Self::Details, _>(self)?;
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
        state::<M, Self::Entity, Self::Details, _>(self).map_or(1.0, |state| {
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
    /// until modifier `K` is removed; setting `K` again replaces it. It
    /// multiplies with each person's own modifiers.
    fn set_population_infectiousness_modifier<K: 'static>(
        &mut self,
        f: impl Fn(&Context, EntityId<Self::Entity>) -> f64 + 'static,
    ) {
        self.set_population_infectiousness_modifier_with_max::<K>(1.0, f);
    }

    /// As [`set_population_infectiousness_modifier`](Self::set_population_infectiousness_modifier)
    /// for a function returning values in `[0, max]`. Everyone is forecast
    /// at `max` throughout, so a loose `max` costs rejected proposals.
    fn set_population_infectiousness_modifier_with_max<K: 'static>(
        &mut self,
        max: f64,
        f: impl Fn(&Context, EntityId<Self::Entity>) -> f64 + 'static,
    ) {
        assert_valid_factor("infectiousness", max);
        let state = state_mut::<M, Self::Entity, Self::Details, _>(self);
        PopulationModifier::set(
            &mut state.population_modifiers,
            PopulationModifier::checked(
                "infectiousness",
                TypeId::of::<K>(),
                max,
                move |context, person, _| f(context, person),
            ),
        );
        refresh_population_bound::<M, Self::Entity, Self::Details, _>(self);
    }

    /// Remove population infectiousness modifier `K`, if set.
    fn remove_population_infectiousness_modifier<K: 'static>(&mut self) {
        state_mut::<M, Self::Entity, Self::Details, _>(self)
            .population_modifiers
            .retain(|modifier| modifier.key != TypeId::of::<K>());
        refresh_population_bound::<M, Self::Entity, Self::Details, _>(self);
    }

    /// Scale `person`'s infectiousness by a finite, non-negative `factor`
    /// until modifier `K` is removed; setting `K` again replaces it. It
    /// belongs to the person, so it survives reinfection.
    fn set_infectiousness_modifier<K: 'static>(
        &mut self,
        person: EntityId<Self::Entity>,
        factor: f64,
    ) {
        assert_valid_factor("infectiousness", factor);
        set_infectiousness_factor::<M, _, Self::Details, _>(
            self,
            person,
            TypeId::of::<K>(),
            Scope::Person,
            Factor::Constant(factor),
        );
    }

    /// As [`set_infectiousness_modifier`](Self::set_infectiousness_modifier),
    /// with a factor in `[0, 1]` computed at every read from the context, the
    /// person and the time since it was set.
    fn set_infectiousness_modifier_fn<K: 'static>(
        &mut self,
        person: EntityId<Self::Entity>,
        f: impl Fn(&Context, EntityId<Self::Entity>, f64) -> f64 + 'static,
    ) {
        self.set_infectiousness_modifier_fn_with_max::<K>(person, 1.0, f);
    }

    /// As [`set_infectiousness_modifier_fn`](Self::set_infectiousness_modifier_fn)
    /// for a function returning values in `[0, max]`. The person is forecast
    /// at `max` throughout, so a loose `max` costs rejected proposals.
    fn set_infectiousness_modifier_fn_with_max<K: 'static>(
        &mut self,
        person: EntityId<Self::Entity>,
        max: f64,
        f: impl Fn(&Context, EntityId<Self::Entity>, f64) -> f64 + 'static,
    ) {
        assert_valid_factor("infectiousness", max);
        let since = self.get_current_time();
        set_infectiousness_factor::<M, _, Self::Details, _>(
            self,
            person,
            TypeId::of::<K>(),
            Scope::Person,
            Factor::checked_fn(
                "infectiousness",
                since,
                max,
                move |context, person, elapsed, _| f(context, person, elapsed),
            ),
        );
    }

    /// Remove infectiousness modifier `K` from `person`, if active, whether
    /// set or scheduled.
    fn remove_infectiousness_modifier<K: 'static>(&mut self, person: EntityId<Self::Entity>) {
        retain_infectiousness_factors::<M, _, Self::Details, _>(self, person, |modifier| {
            modifier.key != TypeId::of::<K>()
        });
    }

    /// Set modifier `K` on `person`'s current infection at `start` and, if
    /// given, remove it at `end`. It belongs to this infection: a reinfection
    /// drops it, and it multiplies with a modifier `K` set on the person. One
    /// scheduled `K` is active at a time: where two overlap, the later call's
    /// applies and the earlier one does not resume after it.
    /// No-op if `person` was never infected.
    ///
    /// # Panics
    ///
    /// If `end` is before `start`.
    fn schedule_infectiousness_modifier<K: 'static>(
        &mut self,
        person: EntityId<Self::Entity>,
        factor: f64,
        start: f64,
        end: Option<f64>,
    ) {
        assert_valid_factor("infectiousness", factor);
        assert_end_not_before_start(start, end);
        let Some(scheduled_for) = infection_count::<M, _, Self::Details, _>(self, person) else {
            return;
        };
        let state = state_mut::<M, Self::Entity, Self::Details, _>(self);
        let schedule = state.schedule_count;
        let scope = Scope::Infection { schedule };
        state.schedule_count = state.schedule_count.wrapping_add(1);
        self.add_plan(start, move |ctx| {
            if infection_count::<M, _, Self::Details, _>(ctx, person) == Some(scheduled_for)
                && InfectionManager::<M>::is_infectious(ctx, person)
                && !has_later_schedule::<M, _, Self::Details, _>(
                    ctx,
                    person,
                    TypeId::of::<K>(),
                    schedule,
                )
            {
                set_infectiousness_factor::<M, _, Self::Details, _>(
                    ctx,
                    person,
                    TypeId::of::<K>(),
                    scope,
                    Factor::Constant(factor),
                );
            }
        });
        if let Some(end) = end {
            // Gone already if a reinfection or a later schedule replaced it.
            self.add_plan(end, move |ctx| {
                retain_infectiousness_factors::<M, _, Self::Details, _>(ctx, person, |modifier| {
                    modifier.key != TypeId::of::<K>() || modifier.scope != scope
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
        state::<M, Self::Entity, Self::Details, _>(self).map_or(1.0, |state| {
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
    /// modifier `K` is removed; setting `K` again replaces it. It multiplies
    /// with each person's own modifiers.
    fn set_population_susceptibility_modifier<K: 'static>(
        &mut self,
        f: impl Fn(&Context, EntityId<Self::Entity>, &Self::Details) -> f64 + 'static,
    ) {
        let state = state_mut::<M, Self::Entity, Self::Details, _>(self);
        PopulationModifier::set(
            &mut state.population_susceptibility_modifiers,
            // No maximum: susceptibility has no forecast to bound.
            PopulationModifier::checked("susceptibility", TypeId::of::<K>(), f64::INFINITY, f),
        );
    }

    /// Remove population susceptibility modifier `K`, if set.
    fn remove_population_susceptibility_modifier<K: 'static>(&mut self) {
        state_mut::<M, Self::Entity, Self::Details, _>(self)
            .population_susceptibility_modifiers
            .retain(|modifier| modifier.key != TypeId::of::<K>());
    }

    /// Scale `person`'s susceptibility by a finite, non-negative `factor`
    /// until modifier `K` is removed; setting `K` again replaces it.
    fn set_susceptibility_modifier<K: 'static>(
        &mut self,
        person: EntityId<Self::Entity>,
        factor: f64,
    ) {
        assert_valid_factor("susceptibility", factor);
        let state = state_mut::<M, Self::Entity, Self::Details, _>(self);
        Modifiers::set(
            &mut state.susceptibility_modifiers,
            person,
            TypeId::of::<K>(),
            Scope::Person,
            Factor::Constant(factor),
        );
    }

    /// As [`set_susceptibility_modifier`](Self::set_susceptibility_modifier),
    /// with a factor computed at every read from the context, the person,
    /// the time since it was set and the details of the attempt, so it can
    /// differ by variant.
    fn set_susceptibility_modifier_fn<K: 'static>(
        &mut self,
        person: EntityId<Self::Entity>,
        f: impl Fn(&Context, EntityId<Self::Entity>, f64, &Self::Details) -> f64 + 'static,
    ) {
        let since = self.get_current_time();
        let state = state_mut::<M, Self::Entity, Self::Details, _>(self);
        Modifiers::set(
            &mut state.susceptibility_modifiers,
            person,
            TypeId::of::<K>(),
            Scope::Person,
            // No maximum: susceptibility has no forecast to bound.
            Factor::checked_fn("susceptibility", since, f64::INFINITY, f),
        );
    }

    /// Remove susceptibility modifier `K` from `person`, if active.
    fn remove_susceptibility_modifier<K: 'static>(&mut self, person: EntityId<Self::Entity>) {
        let state = state_mut::<M, Self::Entity, Self::Details, _>(self);
        Modifiers::retain(&mut state.susceptibility_modifiers, person, |modifier| {
            modifier.key != TypeId::of::<K>()
        });
    }

    /// Set susceptibility modifier `K` on `person` at time `start` and, if
    /// given, remove it at `end`.
    ///
    /// # Panics
    ///
    /// If `end` is before `start`.
    fn schedule_susceptibility_modifier<K: 'static>(
        &mut self,
        person: EntityId<Self::Entity>,
        factor: f64,
        start: f64,
        end: Option<f64>,
    ) {
        assert_valid_factor("susceptibility", factor);
        assert_end_not_before_start(start, end);
        self.add_plan(start, move |ctx| {
            InfectionManager::<M>::set_susceptibility_modifier::<K>(ctx, person, factor);
        });
        if let Some(end) = end {
            self.add_plan(end, move |ctx| {
                InfectionManager::<M>::remove_susceptibility_modifier::<K>(ctx, person);
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
