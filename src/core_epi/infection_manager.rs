use std::any::{Any, TypeId};
use std::marker::PhantomData;

use crate::core_epi::rate::{AnyInfectionRate, InvertibleRate, TransformedInvertibleRateFn};
use crate::data_structures::entity_map::EntityMap;
use crate::entity::{Entity, EntityId};
use crate::prelude::*;
use crate::{ContextBase, IxaEvent};

/// Which rate function a person was assigned: an index into the loaded
/// library (shared curves), or one of their own (e.g. a constant rate truncated
/// at a per-person infectious period).
#[derive(Clone, Debug)]
pub enum RateAssignment {
    /// Index into the library loaded with
    /// [`load_infectiousness_rate_library`](InfectionManager::load_infectiousness_rate_library).
    Library(usize),
    /// Boxed so a record stays small: the inline enum would be sized for an
    /// empirical curve even though own rate fns are typically constant.
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

struct Assigned {
    rate: RateAssignment,
    infection_time: f64,
    /// Which forecast of the person's transmission loop is live: a pending
    /// proposal made under an earlier one is dropped when it fires.
    forecast_generation: u32,
}

/// A modifier factor computed per read from the context, the person and the
/// time elapsed (since infection for infectiousness, since it was set for
/// susceptibility).
pub type ModifierFn<E> = Box<dyn Fn(&Context, EntityId<E>, f64) -> f64>;

/// One modifier's factor: a constant, or a function of elapsed time.
enum Factor<E: Entity> {
    Constant(f64),
    Fn { f: ModifierFn<E>, since: f64 },
}

/// Active modifiers on one person, keyed by modifier type. The product of the
/// constant factors is cached, so a read with no function factors is one
/// lookup; function factors are evaluated per read.
struct Modifiers<E: Entity> {
    factors: Vec<(TypeId, Factor<E>)>,
    /// Product of the constant factors.
    constant_product: f64,
    function_count: usize,
}

impl<E: Entity> Modifiers<E> {
    fn recompute(&mut self) {
        self.constant_product = 1.0;
        self.function_count = 0;
        for (_, factor) in &self.factors {
            match factor {
                Factor::Constant(value) => self.constant_product *= value,
                Factor::Fn { .. } => self.function_count += 1,
            }
        }
    }

    fn set(map: &mut EntityMap<E, Self>, person: EntityId<E>, key: TypeId, factor: Factor<E>) {
        let modifiers = map.get_or_insert_with(person, || Modifiers {
            factors: Vec::new(),
            constant_product: 1.0,
            function_count: 0,
        });
        match modifiers
            .factors
            .iter_mut()
            .find(|(existing_key, _)| *existing_key == key)
        {
            Some((_, existing)) => *existing = factor,
            None => modifiers.factors.push((key, factor)),
        }
        modifiers.recompute();
    }

    fn remove(map: &mut EntityMap<E, Self>, person: EntityId<E>, key: TypeId) {
        let Some(modifiers) = map.get_mut(person) else {
            return;
        };
        modifiers
            .factors
            .retain(|(existing_key, _)| *existing_key != key);
        if modifiers.factors.is_empty() {
            let _ = map.remove(person);
        } else {
            modifiers.recompute();
        }
    }

    fn product(map: &EntityMap<E, Self>, person: EntityId<E>, context: &Context) -> f64 {
        let Some(modifiers) = map.get(person) else {
            return 1.0;
        };
        let mut product = modifiers.constant_product;
        if modifiers.function_count > 0 {
            let now = context.get_current_time();
            for (_, factor) in &modifiers.factors {
                if let Factor::Fn { f, since } = factor {
                    product *= f(context, person, now - since);
                }
            }
        }
        product
    }
}

/// Everything `core_epi` keeps for one model: its rate library, who is
/// infected with what, and the modifiers on them.
struct InfectionState<E: Entity> {
    library: Vec<AnyInfectionRate>,
    assigned: EntityMap<E, Assigned>,
    /// Kept out of `Assigned` so the per-infection record stays small: most
    /// infections are never modified.
    modifiers: EntityMap<E, Modifiers<E>>,
    /// On the person, not the infection: they apply while uninfected and
    /// survive reinfection.
    susceptibility_modifiers: EntityMap<E, Modifiers<E>>,
}

impl<E: Entity> Default for InfectionState<E> {
    fn default() -> Self {
        Self {
            library: Vec::new(),
            assigned: EntityMap::new(),
            modifiers: EntityMap::new(),
            susceptibility_modifiers: EntityMap::new(),
        }
    }
}

impl<E: Entity> InfectionState<E> {
    /// The rate function `assigned` points at.
    fn rate_fn<'a>(&'a self, assigned: &'a Assigned) -> &'a AnyInfectionRate {
        match &assigned.rate {
            RateAssignment::Library(index) => &self.library[*index],
            RateAssignment::Own(rate_fn) => rate_fn.as_ref(),
        }
    }
}

// One `InfectionState<E>` per model marker `M`, so two pathogens on one
// population keep separate infections. A data plugin cannot be generic, so
// the stores are type-erased; there are only ever a few, so a scan beats a
// hash.
define_data_plugin!(
    InfectionStatePlugin,
    Vec<(TypeId, Box<dyn Any>)>,
    Vec::new()
);

fn state<M: 'static, E: Entity, C: ContextBase>(context: &C) -> Option<&InfectionState<E>> {
    let model = TypeId::of::<M>();
    let stores = context.get_data(InfectionStatePlugin);
    let (_, store) = stores.iter().find(|(id, _)| *id == model)?;
    Some(
        store
            .downcast_ref()
            .expect("infection state has the wrong type"),
    )
}

fn state_mut<M: 'static, E: Entity, C: ContextBase>(context: &mut C) -> &mut InfectionState<E> {
    let model = TypeId::of::<M>();
    let stores = context.get_data_mut(InfectionStatePlugin);
    let index = match stores.iter().position(|(id, _)| *id == model) {
        Some(index) => index,
        None => {
            stores.push((model, Box::new(InfectionState::<E>::default())));
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
pub(super) fn forecast_generation<M: 'static, E: Entity, C: ContextBase>(
    context: &C,
    person: EntityId<E>,
) -> Option<u32> {
    Some(
        state::<M, E, _>(context)?
            .assigned
            .get(person)?
            .forecast_generation,
    )
}

/// Supersede `person`'s pending proposal, returning the new live forecast;
/// `None` if they were never infected.
pub(super) fn advance_forecast_generation<M: 'static, E: Entity, C: ContextBase>(
    context: &mut C,
    person: EntityId<E>,
) -> Option<u32> {
    let assigned = state_mut::<M, E, _>(context).assigned.get_mut(person)?;
    assigned.forecast_generation = assigned.forecast_generation.wrapping_add(1);
    Some(assigned.forecast_generation)
}

/// A person's infectiousness over time and their susceptibility: assigning
/// each infected person a rate function, storing it, and exposing both to the
/// transmission loop. `M` is the model's marker type (see
/// [`TransmissionManager`](super::TransmissionManager)); `Entity` is the
/// model's entity that gets infected, e.g. its own `Person`.
pub trait InfectionManager<M: 'static>: ContextBase + AsRef<Context>
where
    Context: InfectionManager<M, Entity = Self::Entity>,
{
    type Entity: Entity;

    /// What the model records about how someone was exposed, passed from the
    /// contact draw to [`on_infected`](Self::on_infected): for example the
    /// setting the attempt happened in, as `Option<Setting>`. Use `()` when
    /// there is nothing to record. A seeded infection gets the default value.
    type Exposure: Default;

    /// Draw the rate function for newly infected `person`. This is where a model
    /// applies its own config (which shape, how the infectious period is drawn).
    fn draw_infectiousness_rate_fn(&mut self, person: EntityId<Self::Entity>) -> RateAssignment;

    /// Load the shared rate functions that [`RateAssignment::Library`] indexes
    /// into. Call once at setup, before anyone is infected.
    fn load_infectiousness_rate_library(&mut self, rate_fns: Vec<AnyInfectionRate>) {
        state_mut::<M, Self::Entity, _>(self).library = rate_fns;
    }

    /// Number of rate functions in the loaded library.
    fn infectiousness_rate_library_len(&self) -> usize {
        state::<M, Self::Entity, _>(self).map_or(0, |state| state.library.len())
    }

    /// Draw and store `person`'s rate function, starting now, and emit
    /// [`InfectiousnessAssignedEvent`]. Their infectiousness ends when the rate
    /// function runs out. Call when they are infected; a reinfection replaces
    /// the previous one.
    fn assign_infectiousness_rate_fn(&mut self, person: EntityId<Self::Entity>) {
        let rate = self.draw_infectiousness_rate_fn(person);
        self.set_infectiousness_rate_fn(person, rate);
    }

    /// As [`assign_infectiousness_rate_fn`](Self::assign_infectiousness_rate_fn)
    /// with a given rate function instead of a drawn one: for an infection
    /// whose course is known (an imported case, a test of an intervention).
    /// Like it, this does not call [`on_infected`](Self::on_infected): the
    /// caller does whatever bookkeeping the model does there.
    ///
    /// # Panics
    ///
    /// If `rate` is a library index outside the loaded library.
    fn set_infectiousness_rate_fn(
        &mut self,
        person: EntityId<Self::Entity>,
        rate: impl Into<RateAssignment>,
    ) {
        let rate = rate.into();
        let infection_time = self.get_current_time();
        let state = state_mut::<M, Self::Entity, _>(self);
        if let RateAssignment::Library(index) = rate {
            assert!(
                index < state.library.len(),
                "rate function {index} is not in the library of {}: load it with \
                 `load_infectiousness_rate_library` before anyone is infected",
                state.library.len()
            );
        }
        // A reinfection supersedes the previous infection's pending proposal.
        let forecast_generation = state
            .assigned
            .get(person)
            .map_or(0, |assigned| assigned.forecast_generation.wrapping_add(1));
        state.assigned.insert(
            person,
            Assigned {
                rate,
                infection_time,
                forecast_generation,
            },
        );
        let _ = state.modifiers.remove(person);
        self.emit_event(InfectiousnessAssignedEvent::<M> {
            person,
            model: PhantomData,
        });
    }

    /// `person`'s rate function, indexed by time since infection; `None` if
    /// they were never assigned one.
    fn infectiousness_rate_fn(&self, person: EntityId<Self::Entity>) -> Option<&AnyInfectionRate> {
        let state = state::<M, Self::Entity, _>(self)?;
        Some(state.rate_fn(state.assigned.get(person)?))
    }

    /// When `person`'s current infection began; `None` if they were never infected.
    fn infection_time(&self, person: EntityId<Self::Entity>) -> Option<f64> {
        let assigned = state::<M, Self::Entity, _>(self)?.assigned.get(person)?;
        Some(assigned.infection_time)
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

    /// How long `person`'s current infection is infectious for, from
    /// infection until their rate function runs out; `None` if they were
    /// never infected.
    fn infectiousness_duration(&self, person: EntityId<Self::Entity>) -> Option<f64> {
        Some(self.infectiousness_rate_fn(person)?.duration())
    }

    /// Whether `person` has a rate function that has not yet run out.
    fn is_infectious(&self, person: EntityId<Self::Entity>) -> bool {
        self.infectiousness_end_time(person)
            .is_some_and(|end| self.get_current_time() < end)
    }

    /// Majorizing infectiousness of `person`, indexed by time from now: the
    /// stored rate function shifted by the time since infection.
    ///
    /// # Panics
    ///
    /// If `person` was never infected.
    fn majorizing_infectiousness_rate_fn(
        &self,
        person: EntityId<Self::Entity>,
    ) -> TransformedInvertibleRateFn<'_, AnyInfectionRate> {
        // One record lookup: this runs once per transmission attempt proposal.
        let state = state::<M, Self::Entity, _>(self).expect("person was never infected");
        let assigned = state
            .assigned
            .get(person)
            .expect("person was never infected");
        TransformedInvertibleRateFn::new(
            state.rate_fn(assigned),
            1.0,
            self.get_current_time() - assigned.infection_time,
        )
    }

    /// Current intrinsic infectiousness, before any contact structure: the
    /// stored rate function now, times [`Self::infectiousness_multiplier`].
    /// Bounded by the majorizing rate at time zero. `None` if they were never
    /// infected; 0 once their rate function has run out.
    fn intrinsic_infectiousness(&self, person: EntityId<Self::Entity>) -> Option<f64> {
        // One record lookup: this runs once per transmission attempt proposal.
        let state = state::<M, Self::Entity, _>(self)?;
        let assigned = state.assigned.get(person)?;
        let rate = state
            .rate_fn(assigned)
            .rate(self.get_current_time() - assigned.infection_time);
        let multiplier = Modifiers::product(&state.modifiers, person, self.as_ref());
        Some(multiplier * rate)
    }

    /// Factor applied to `person`'s stored rate function: the product of the
    /// active infectiousness modifiers on their current infection (`1.0` if
    /// there are none).
    fn infectiousness_multiplier(&self, person: EntityId<Self::Entity>) -> f64 {
        state::<M, Self::Entity, _>(self).map_or(1.0, |state| {
            Modifiers::product(&state.modifiers, person, self.as_ref())
        })
    }

    /// Scale `person`'s infectiousness by `factor` until their current infection
    /// ends or the modifier is removed. `K` identifies the modifier: setting it
    /// again replaces its factor, and different modifiers compose
    /// multiplicatively. `factor` must be in `[0, 1]` so the majorizing rate
    /// stays an upper bound. No-op if `person` was never infected.
    fn set_infectiousness_modifier<K: 'static>(
        &mut self,
        person: EntityId<Self::Entity>,
        factor: f64,
    ) {
        assert!(
            (0.0..=1.0).contains(&factor),
            "infectiousness modifier factor must be in [0, 1], got {factor}"
        );
        let state = state_mut::<M, Self::Entity, _>(self);
        if !state.assigned.contains_key(person) {
            return;
        }
        Modifiers::set(
            &mut state.modifiers,
            person,
            TypeId::of::<K>(),
            Factor::Constant(factor),
        );
    }

    /// As [`set_infectiousness_modifier`](Self::set_infectiousness_modifier),
    /// with a factor computed at every read from the context, the person and
    /// the time since infection (a treatment effect that builds up, a factor
    /// read from a property), so only people with one pay for it. It must
    /// return values in `[0, 1]`, which is checked in debug builds. No-op if
    /// `person` was never infected.
    fn set_infectiousness_modifier_fn<K: 'static>(
        &mut self,
        person: EntityId<Self::Entity>,
        f: impl Fn(&Context, EntityId<Self::Entity>, f64) -> f64 + 'static,
    ) {
        let state = state_mut::<M, Self::Entity, _>(self);
        let Some(assigned) = state.assigned.get(person) else {
            return;
        };
        let since = assigned.infection_time;
        let checked: ModifierFn<Self::Entity> = Box::new(move |ctx, person, elapsed| {
            let factor = f(ctx, person, elapsed);
            debug_assert!(
                (0.0..=1.0).contains(&factor),
                "infectiousness modifier must return values in [0, 1], got {factor}"
            );
            factor
        });
        Modifiers::set(
            &mut state.modifiers,
            person,
            TypeId::of::<K>(),
            Factor::Fn { f: checked, since },
        );
    }

    /// Remove modifier `K` from `person`'s current infection, if active.
    fn remove_infectiousness_modifier<K: 'static>(&mut self, person: EntityId<Self::Entity>) {
        let state = state_mut::<M, Self::Entity, _>(self);
        Modifiers::remove(&mut state.modifiers, person, TypeId::of::<K>());
    }

    /// Set modifier `K` on `person` at time `start` and, if given, remove it at
    /// `end`. Each step applies only if `person` is still infectious with the
    /// infection they had when this was called, so a recovery or reinfection in
    /// between cancels it. No-op if `person` was never infected.
    fn schedule_infectiousness_modifier<K: 'static>(
        &mut self,
        person: EntityId<Self::Entity>,
        factor: f64,
        start: f64,
        end: Option<f64>,
    ) {
        let Some(infection_time) = self.infection_time(person) else {
            return;
        };
        let same_infection = move |ctx: &Context| {
            InfectionManager::<M>::infection_time(ctx, person) == Some(infection_time)
                && InfectionManager::<M>::is_infectious(ctx, person)
        };
        self.add_plan(start, move |ctx| {
            if same_infection(ctx) {
                InfectionManager::<M>::set_infectiousness_modifier::<K>(ctx, person, factor);
            }
        });
        if let Some(end) = end {
            self.add_plan(end, move |ctx| {
                if same_infection(ctx) {
                    InfectionManager::<M>::remove_infectiousness_modifier::<K>(ctx, person);
                }
            });
        }
    }

    /// Probability that `person`, subjected to an infection attempt, is
    /// infected, before any modifiers. The default is permanent immunity:
    /// `1.0` if they were never infected, else `0.0`. Override for waning,
    /// age-dependent or status-dependent susceptibility.
    fn base_susceptibility(&self, person: EntityId<Self::Entity>) -> f64 {
        if self.infection_time(person).is_none() {
            1.0
        } else {
            0.0
        }
    }

    /// Probability that `person`, subjected to an infection attempt, is
    /// infected: [`base_susceptibility`](Self::base_susceptibility) times
    /// [`susceptibility_multiplier`](Self::susceptibility_multiplier). The
    /// transmission loop treats values of `1.0` or more as certain.
    fn susceptibility(&self, person: EntityId<Self::Entity>) -> f64 {
        self.base_susceptibility(person) * self.susceptibility_multiplier(person)
    }

    /// Product of the active susceptibility modifiers on `person` (`1.0` if
    /// there are none).
    fn susceptibility_multiplier(&self, person: EntityId<Self::Entity>) -> f64 {
        state::<M, Self::Entity, _>(self).map_or(1.0, |state| {
            Modifiers::product(&state.susceptibility_modifiers, person, self.as_ref())
        })
    }

    /// Scale `person`'s susceptibility by `factor` until the modifier is
    /// removed. Unlike infectiousness modifiers these belong to the person, not
    /// an infection: they apply while uninfected and survive reinfection. `K`
    /// identifies the modifier: setting it again replaces its factor, and
    /// different modifiers compose multiplicatively. `factor` must be finite
    /// and non-negative; a product above one is as good as one.
    fn set_susceptibility_modifier<K: 'static>(
        &mut self,
        person: EntityId<Self::Entity>,
        factor: f64,
    ) {
        assert!(
            factor.is_finite() && factor >= 0.0,
            "susceptibility modifier factor must be finite and non-negative, got {factor}"
        );
        let state = state_mut::<M, Self::Entity, _>(self);
        Modifiers::set(
            &mut state.susceptibility_modifiers,
            person,
            TypeId::of::<K>(),
            Factor::Constant(factor),
        );
    }

    /// As [`set_susceptibility_modifier`](Self::set_susceptibility_modifier),
    /// with a factor computed at every read from the context, the person and
    /// the time since it was set (protection that wanes, a factor read from a
    /// property). It must return finite, non-negative values, which is checked
    /// in debug builds.
    fn set_susceptibility_modifier_fn<K: 'static>(
        &mut self,
        person: EntityId<Self::Entity>,
        f: impl Fn(&Context, EntityId<Self::Entity>, f64) -> f64 + 'static,
    ) {
        let since = self.get_current_time();
        let checked: ModifierFn<Self::Entity> = Box::new(move |ctx, person, elapsed| {
            let factor = f(ctx, person, elapsed);
            debug_assert!(
                factor.is_finite() && factor >= 0.0,
                "susceptibility modifier must return finite, non-negative values, got {factor}"
            );
            factor
        });
        let state = state_mut::<M, Self::Entity, _>(self);
        Modifiers::set(
            &mut state.susceptibility_modifiers,
            person,
            TypeId::of::<K>(),
            Factor::Fn { f: checked, since },
        );
    }

    /// Remove susceptibility modifier `K` from `person`, if active.
    fn remove_susceptibility_modifier<K: 'static>(&mut self, person: EntityId<Self::Entity>) {
        let state = state_mut::<M, Self::Entity, _>(self);
        Modifiers::remove(
            &mut state.susceptibility_modifiers,
            person,
            TypeId::of::<K>(),
        );
    }

    /// Set susceptibility modifier `K` on `person` at time `start` and, if
    /// given, remove it at `end`.
    fn schedule_susceptibility_modifier<K: 'static>(
        &mut self,
        person: EntityId<Self::Entity>,
        factor: f64,
        start: f64,
        end: Option<f64>,
    ) {
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
    /// assigned. `infector` is `None` for an infection with no source in the
    /// population, e.g. an initial seed. `exposure` is what
    /// [`evaluate_forecast`](super::TransmissionManager::evaluate_forecast)
    /// returned with the contact, or the default value for a seed.
    fn on_infected(
        &mut self,
        _infector: Option<EntityId<Self::Entity>>,
        _infectee: EntityId<Self::Entity>,
        _exposure: Self::Exposure,
    ) {
    }
}
