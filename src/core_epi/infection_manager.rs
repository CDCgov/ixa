use std::any::TypeId;

use crate::core_epi::person::{Person, PersonId};
use crate::core_epi::rate::{AnyInfectionRate, InvertibleRate, TransformedInvertibleRateFn};
use crate::data_structures::entity_map::EntityMap;
use crate::prelude::*;
use crate::{ContextBase, IxaEvent};

/// Which rate function a person was assigned: an index into the loaded
/// library (shared curves), or one of their own (e.g. a constant rate truncated
/// at a per-person infectious period).
#[derive(Clone, Debug)]
pub enum RateAssignment {
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

/// Emitted when a person is assigned a rate function, i.e. their
/// infectiousness begins.
#[derive(Debug, IxaEvent)]
pub struct InfectiousnessAssignedEvent {
    pub person: PersonId,
}

struct Assigned {
    rate: RateAssignment,
    infection_time: f64,
}

/// A modifier factor computed per read from the context, the person and the
/// time elapsed (since infection for infectiousness, since it was set for
/// susceptibility).
pub type ModifierFn = Box<dyn Fn(&Context, PersonId, f64) -> f64>;

/// One modifier's factor: a constant, or a function of elapsed time.
enum Factor {
    Constant(f64),
    Fn { f: ModifierFn, since: f64 },
}

/// Active modifiers on one person, keyed by modifier type. The product of the
/// constant factors is cached, so a read with no function factors is one
/// lookup; function factors are evaluated per read.
struct Modifiers {
    factors: Vec<(TypeId, Factor)>,
    product: f64,
    fns: usize,
}

impl Modifiers {
    fn recompute(&mut self) {
        self.product = 1.0;
        self.fns = 0;
        for (_, factor) in &self.factors {
            match factor {
                Factor::Constant(f) => self.product *= f,
                Factor::Fn { .. } => self.fns += 1,
            }
        }
    }

    fn set(map: &mut EntityMap<Person, Self>, person: PersonId, key: TypeId, factor: Factor) {
        let modifiers = map.get_or_insert_with(person, || Modifiers {
            factors: Vec::new(),
            product: 1.0,
            fns: 0,
        });
        match modifiers.factors.iter_mut().find(|(k, _)| *k == key) {
            Some((_, f)) => *f = factor,
            None => modifiers.factors.push((key, factor)),
        }
        modifiers.recompute();
    }

    fn remove(map: &mut EntityMap<Person, Self>, person: PersonId, key: TypeId) {
        let Some(modifiers) = map.get_mut(person) else {
            return;
        };
        modifiers.factors.retain(|(k, _)| *k != key);
        if modifiers.factors.is_empty() {
            let _ = map.remove(person);
        } else {
            modifiers.recompute();
        }
    }

    fn product(map: &EntityMap<Person, Self>, person: PersonId, context: &Context) -> f64 {
        let Some(modifiers) = map.get(person) else {
            return 1.0;
        };
        if modifiers.fns == 0 {
            return modifiers.product;
        }
        let now = context.get_current_time();
        modifiers
            .factors
            .iter()
            .fold(modifiers.product, |acc, (_, factor)| match factor {
                Factor::Constant(_) => acc,
                Factor::Fn { f, since } => acc * f(context, person, now - since),
            })
    }
}

#[derive(Default)]
struct InfectiousnessRates {
    library: Vec<AnyInfectionRate>,
    assigned: EntityMap<Person, Assigned>,
    /// Kept out of `Assigned` so the per-infection record stays small: most
    /// infections are never modified.
    modifiers: EntityMap<Person, Modifiers>,
    /// On the person, not the infection: they apply while uninfected and
    /// survive reinfection.
    susceptibility_modifiers: EntityMap<Person, Modifiers>,
}

// Rate functions are indexed by time since infection.
define_data_plugin!(
    InfectiousnessRatePlugin,
    InfectiousnessRates,
    InfectiousnessRates::default()
);

/// A person's infectiousness over time and their susceptibility: assigning
/// each infected person a rate function, storing it, and exposing both to the
/// transmission loop. `M` is the model's marker type (see
/// [`TransmissionManager`](super::TransmissionManager)).
pub trait InfectionManager<M: 'static>: ContextBase + AsRef<Context>
where
    Context: InfectionManager<M>,
{
    /// Draw the rate function for newly infected `person`. This is where a model
    /// applies its own config (which shape, how the infectious period is drawn).
    fn draw_infectiousness_rate_fn(&mut self, person: PersonId) -> RateAssignment;

    /// Load the shared rate functions that [`RateAssignment::Library`] indexes
    /// into. Call once at setup, before anyone is infected.
    fn load_infectiousness_rate_library(&mut self, rate_fns: Vec<AnyInfectionRate>) {
        self.get_data_mut(InfectiousnessRatePlugin).library = rate_fns;
    }

    /// Number of rate functions in the loaded library.
    fn infectiousness_rate_library_len(&self) -> usize {
        self.get_data(InfectiousnessRatePlugin).library.len()
    }

    /// Draw and store `person`'s rate function, starting now, and emit
    /// [`InfectiousnessAssignedEvent`]. Their infectiousness ends when the rate
    /// function runs out. Call when they are infected; a reinfection replaces
    /// the previous one.
    fn assign_infectiousness_rate_fn(&mut self, person: PersonId) {
        let rate = self.draw_infectiousness_rate_fn(person);
        let infection_time = self.get_current_time();
        let rates = self.get_data_mut(InfectiousnessRatePlugin);
        rates.assigned.insert(
            person,
            Assigned {
                rate,
                infection_time,
            },
        );
        let _ = rates.modifiers.remove(person);
        self.emit_event(InfectiousnessAssignedEvent { person });
    }

    /// `person`'s rate function, indexed by time since infection; `None` if
    /// they were never assigned one.
    fn infectiousness_rate_fn(&self, person: PersonId) -> Option<&AnyInfectionRate> {
        let rates = self.get_data(InfectiousnessRatePlugin);
        match &rates.assigned.get(person)?.rate {
            RateAssignment::Library(idx) => Some(&rates.library[*idx]),
            RateAssignment::Own(rate_fn) => Some(rate_fn.as_ref()),
        }
    }

    /// When `person`'s current infection began; `None` if they were never infected.
    fn infection_time(&self, person: PersonId) -> Option<f64> {
        let assigned = self
            .get_data(InfectiousnessRatePlugin)
            .assigned
            .get(person)?;
        Some(assigned.infection_time)
    }

    /// Time since `person`'s rate function was assigned; `None` if it never was.
    fn time_since_infection(&self, person: PersonId) -> Option<f64> {
        Some(self.get_current_time() - self.infection_time(person)?)
    }

    /// When `person`'s current rate function runs out; `None` if they were
    /// never infected.
    fn infectiousness_end_time(&self, person: PersonId) -> Option<f64> {
        Some(self.infection_time(person)? + self.infectiousness_rate_fn(person)?.duration())
    }

    /// Whether `person` has a rate function that has not yet run out.
    fn is_infectious(&self, person: PersonId) -> bool {
        self.infectiousness_end_time(person)
            .is_some_and(|end| self.get_current_time() < end)
    }

    /// Majorizing infectiousness of `person`, indexed by time from now: the
    /// stored rate function shifted by the time since infection.
    fn majorizing_infectiousness_rate_fn(
        &self,
        person: PersonId,
    ) -> TransformedInvertibleRateFn<'_, AnyInfectionRate> {
        TransformedInvertibleRateFn::new(
            self.infectiousness_rate_fn(person)
                .expect("no rate fn assigned to person"),
            1.0,
            self.time_since_infection(person)
                .expect("person was never infected"),
        )
    }

    /// Current intrinsic infectiousness, before any contact structure: the
    /// stored rate function now, times [`Self::infectiousness_multiplier`].
    /// Bounded by the majorizing rate at time zero.
    /// `None` signals that it is zero and will continue to be zero.
    fn intrinsic_infectiousness(&self, person: PersonId) -> Option<f64> {
        // One record lookup: this runs once per transmission attempt proposal.
        let rates = self.get_data(InfectiousnessRatePlugin);
        let assigned = rates.assigned.get(person)?;
        let rate_fn = match &assigned.rate {
            RateAssignment::Library(idx) => &rates.library[*idx],
            RateAssignment::Own(rate_fn) => rate_fn.as_ref(),
        };
        let rate = rate_fn.rate(self.get_current_time() - assigned.infection_time);
        let multiplier = Modifiers::product(&rates.modifiers, person, self.as_ref());
        Some(multiplier * rate)
    }

    /// Factor applied to `person`'s stored rate function: the product of the
    /// active infectiousness modifiers on their current infection (`1.0` if
    /// there are none).
    fn infectiousness_multiplier(&self, person: PersonId) -> f64 {
        Modifiers::product(
            &self.get_data(InfectiousnessRatePlugin).modifiers,
            person,
            self.as_ref(),
        )
    }

    /// Scale `person`'s infectiousness by `factor` until their current infection
    /// ends or the modifier is removed. `K` identifies the modifier: setting it
    /// again replaces its factor, and different modifiers compose
    /// multiplicatively. `factor` must be in `[0, 1]` so the majorizing rate
    /// stays an upper bound. No-op if `person` was never infected.
    fn set_infectiousness_modifier<K: 'static>(&mut self, person: PersonId, factor: f64) {
        assert!(
            (0.0..=1.0).contains(&factor),
            "infectiousness modifier factor must be in [0, 1], got {factor}"
        );
        let rates = self.get_data_mut(InfectiousnessRatePlugin);
        if !rates.assigned.contains_key(person) {
            return;
        }
        Modifiers::set(
            &mut rates.modifiers,
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
        person: PersonId,
        f: impl Fn(&Context, PersonId, f64) -> f64 + 'static,
    ) {
        let rates = self.get_data_mut(InfectiousnessRatePlugin);
        let Some(assigned) = rates.assigned.get(person) else {
            return;
        };
        let since = assigned.infection_time;
        let f: ModifierFn = Box::new(move |ctx, person, elapsed| {
            let v = f(ctx, person, elapsed);
            debug_assert!(
                (0.0..=1.0).contains(&v),
                "infectiousness modifier must return values in [0, 1], got {v}"
            );
            v
        });
        Modifiers::set(
            &mut rates.modifiers,
            person,
            TypeId::of::<K>(),
            Factor::Fn { f, since },
        );
    }

    /// Remove modifier `K` from `person`'s current infection, if active.
    fn remove_infectiousness_modifier<K: 'static>(&mut self, person: PersonId) {
        let rates = self.get_data_mut(InfectiousnessRatePlugin);
        Modifiers::remove(&mut rates.modifiers, person, TypeId::of::<K>());
    }

    /// Set modifier `K` on `person` at time `start` and, if given, remove it at
    /// `end`. Each step applies only if `person` is still infectious with the
    /// infection they had when this was called, so a recovery or reinfection in
    /// between cancels it.
    fn schedule_infectiousness_modifier<K: 'static>(
        &mut self,
        person: PersonId,
        factor: f64,
        start: f64,
        end: Option<f64>,
    ) {
        let infection_time = self
            .infection_time(person)
            .expect("person was never infected");
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
    fn base_susceptibility(&self, person: PersonId) -> f64 {
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
    fn susceptibility(&self, person: PersonId) -> f64 {
        self.base_susceptibility(person) * self.susceptibility_multiplier(person)
    }

    /// Product of the active susceptibility modifiers on `person` (`1.0` if
    /// there are none).
    fn susceptibility_multiplier(&self, person: PersonId) -> f64 {
        Modifiers::product(
            &self
                .get_data(InfectiousnessRatePlugin)
                .susceptibility_modifiers,
            person,
            self.as_ref(),
        )
    }

    /// Scale `person`'s susceptibility by `factor` until the modifier is
    /// removed. Unlike infectiousness modifiers these belong to the person, not
    /// an infection: they apply while uninfected and survive reinfection. `K`
    /// identifies the modifier: setting it again replaces its factor, and
    /// different modifiers compose multiplicatively. `factor` must be finite
    /// and non-negative; a product above one is as good as one.
    fn set_susceptibility_modifier<K: 'static>(&mut self, person: PersonId, factor: f64) {
        assert!(
            factor.is_finite() && factor >= 0.0,
            "susceptibility modifier factor must be finite and non-negative, got {factor}"
        );
        let rates = self.get_data_mut(InfectiousnessRatePlugin);
        Modifiers::set(
            &mut rates.susceptibility_modifiers,
            person,
            TypeId::of::<K>(),
            Factor::Constant(factor),
        );
    }

    /// As [`set_susceptibility_modifier`](Self::set_susceptibility_modifier),
    /// with a factor computed at every read from the context, the person and
    /// the time since it was set (protection that wanes, a factor read from a
    /// property). It must return finite, non-negative values.
    fn set_susceptibility_modifier_fn<K: 'static>(
        &mut self,
        person: PersonId,
        f: impl Fn(&Context, PersonId, f64) -> f64 + 'static,
    ) {
        let since = self.get_current_time();
        let rates = self.get_data_mut(InfectiousnessRatePlugin);
        Modifiers::set(
            &mut rates.susceptibility_modifiers,
            person,
            TypeId::of::<K>(),
            Factor::Fn {
                f: Box::new(f),
                since,
            },
        );
    }

    /// Remove susceptibility modifier `K` from `person`, if active.
    fn remove_susceptibility_modifier<K: 'static>(&mut self, person: PersonId) {
        let rates = self.get_data_mut(InfectiousnessRatePlugin);
        Modifiers::remove(
            &mut rates.susceptibility_modifiers,
            person,
            TypeId::of::<K>(),
        );
    }

    /// Set susceptibility modifier `K` on `person` at time `start` and, if
    /// given, remove it at `end`.
    fn schedule_susceptibility_modifier<K: 'static>(
        &mut self,
        person: PersonId,
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
    /// population, e.g. an initial seed.
    fn on_infected(&mut self, _infector: Option<PersonId>, _infectee: PersonId) {}
}
