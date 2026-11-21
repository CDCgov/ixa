use rand_distr::Exp;

use crate::core_epi::infection_manager::{
    advance_forecast_generation, forecast_generation, start_transmission_loop, InfectionManager,
    InfectiousnessAssignedEvent, ModifierBoundRaisedEvent,
};
use crate::core_epi::rate::InvertibleRate;
use crate::entity::query::Query;
use crate::entity::EntityId;
use crate::prelude::*;

define_rng!(CoreEpiTransmissionRng);
define_rng!(CoreEpiInfectionRng);

/// Relative slack when checking a total against its forecast, since the two
/// take different float paths. Also the slack on a susceptibility of one.
const BOUND_TOLERANCE: f64 = 1e-9;

/// Absolute slack on the same check, for a total next to a zero of its rate
/// function, where a float's worth of time is a large relative change.
const BOUND_SLACK: f64 = 1e-12;

/// A Bernoulli draw that uses the RNG only when the outcome is uncertain.
fn sample_if_uncertain<C: crate::ContextRandomExt>(context: &C, probability: f64) -> bool {
    probability >= 1.0
        || (probability > 0.0 && context.sample_bool(CoreEpiTransmissionRng, probability))
}

/// The transmission loop: proposals drawn from
/// [`forecast_total_infectiousness`](Self::forecast_total_infectiousness) are
/// thinned to attempts by [`evaluate_forecast`](Self::evaluate_forecast),
/// which picks the contact, and attempts to transmissions by the contact's
/// [`susceptibility`](InfectionManager::susceptibility).
///
/// Every method has a default and together they are homogeneous mixing. `M`
/// is a marker type the model defines so it can implement the trait for
/// `Context`. See the `core_epi` topic in the book.
pub trait TransmissionManager<M: 'static>:
    InfectionManager<M> + crate::ContextRandomExt + ContextEntitiesExt
where
    Context: TransmissionManager<M>
        + InfectionManager<M, Entity = <Self as InfectionManager<M>>::Entity>,
{
    /// Factor on `person`'s intrinsic infectiousness from their contacts now;
    /// 1 by default. Read only by the default
    /// [`evaluate_forecast`](Self::evaluate_forecast).
    fn contact_multiplier(&self, _person: EntityId<Self::Entity>) -> f64 {
        1.0
    }

    /// Upper bound on [`contact_multiplier`](Self::contact_multiplier) until
    /// `person`'s next proposal; by default the multiplier now. Read only by
    /// the default
    /// [`forecast_total_infectiousness`](Self::forecast_total_infectiousness).
    fn max_contact_multiplier(&self, person: EntityId<Self::Entity>) -> f64 {
        self.contact_multiplier(person)
    }

    /// Who `person` contacts in an attempt and the details the infection
    /// would carry; by default anyone else, uniformly, with
    /// [`details_from`](Self::details_from). Called only by the default
    /// [`evaluate_forecast`](Self::evaluate_forecast).
    fn sample_contact(
        &self,
        person: EntityId<Self::Entity>,
    ) -> Option<(EntityId<Self::Entity>, Self::Details)> {
        Some((self.sample_anyone_else(person)?, self.details_from(person)))
    }

    /// The details of an infection `infector` passes on; by default the
    /// default details. Override it to pass on something of the infector's,
    /// such as their variant. Called only by the default
    /// [`sample_contact`](Self::sample_contact).
    fn details_from(&self, _infector: EntityId<Self::Entity>) -> Self::Details {
        Self::Details::default()
    }

    /// Someone other than `person`, uniformly from the population; `None` if
    /// there is nobody else.
    fn sample_anyone_else(&self, person: EntityId<Self::Entity>) -> Option<EntityId<Self::Entity>> {
        self.sample(CoreEpiTransmissionRng, |rng| {
            self.query(Self::Entity::default())
                .sample_entity_excluding(rng, person)
        })
    }

    /// Upper bound on `person`'s total infectiousness, indexed by time from
    /// now, which the loop inverts to propose attempt times. The default is
    /// [`majorizing_infectiousness_rate_fn`](InfectionManager::majorizing_infectiousness_rate_fn)
    /// scaled by [`max_contact_multiplier`](Self::max_contact_multiplier).
    ///
    /// # Panics
    ///
    /// The default panics if `max_contact_multiplier` is negative or not finite.
    fn forecast_total_infectiousness(&self, person: EntityId<Self::Entity>) -> impl InvertibleRate {
        let max_multiplier = self.max_contact_multiplier(person);
        assert!(
            max_multiplier.is_finite() && max_multiplier >= 0.0,
            "max_contact_multiplier of {person:?} must be finite and non-negative, \
             got {max_multiplier}"
        );
        self.majorizing_infectiousness_rate_fn(person)
            .scaled(max_multiplier)
    }

    /// Evaluate a proposal for `person` forecast at `forecast`: compute
    /// their total infectiousness now, thin with
    /// [`accept_proposal`](Self::accept_proposal) and return who they
    /// contact. `None` drops the proposal and the loop continues.
    fn evaluate_forecast(
        &self,
        person: EntityId<Self::Entity>,
        forecast: f64,
    ) -> Option<(EntityId<Self::Entity>, Self::Details)> {
        let total = self.intrinsic_infectiousness(person)? * self.contact_multiplier(person);
        if !self.accept_proposal(person, total, forecast) {
            return None;
        }
        self.sample_contact(person)
    }

    /// Whether a proposal at `forecast` becomes an attempt at `total`
    /// infectiousness: with probability `total / forecast`.
    ///
    /// # Panics
    ///
    /// If `forecast` is not finite or not positive, or if `total` is negative
    /// or exceeds `forecast`, which would under-thin attempts.
    fn accept_proposal(&self, person: EntityId<Self::Entity>, total: f64, forecast: f64) -> bool {
        assert!(
            forecast.is_finite() && forecast > 0.0,
            "forecast {forecast} of {person:?} must be finite and positive"
        );
        assert!(
            total >= 0.0,
            "total infectiousness {total} of {person:?} must be non-negative"
        );
        assert!(
            total <= forecast * (1.0 + BOUND_TOLERANCE) + BOUND_SLACK,
            "total infectiousness {total} of {person:?} exceeds its forecast {forecast}: \
             the forecast must bound the total until the next proposal"
        );
        sample_if_uncertain(self, total / forecast)
    }

    /// Infect `infectee`: assign their rate function, then call
    /// [`on_infected`](InfectionManager::on_infected).
    fn transmit(
        &mut self,
        infector: Option<EntityId<Self::Entity>>,
        infectee: EntityId<Self::Entity>,
        details: Self::Details,
    ) {
        self.assign_infectiousness_rate_fn(infectee, details.clone());
        self.on_infected(infector, infectee, details);
    }

    /// Infect `person` with no infector and the default details.
    fn seed_infection(&mut self, person: EntityId<Self::Entity>) {
        self.transmit(None, person, Self::Details::default());
    }

    /// Infect up to `n` people (fewer if fewer match) sampled uniformly from
    /// those matching `query`, with no infector and the default details.
    fn seed_infections<Q: Query<Self::Entity>>(&mut self, query: Q, n: usize) {
        for person in self.sample_entities(CoreEpiInfectionRng, query, n) {
            self.seed_infection(person);
        }
    }

    /// As [`seed_infections`](Self::seed_infections) with the given details,
    /// e.g. to seed a variant.
    fn seed_infections_with<Q: Query<Self::Entity>>(
        &mut self,
        query: Q,
        n: usize,
        details: Self::Details,
    ) {
        for person in self.sample_entities(CoreEpiInfectionRng, query, n) {
            self.transmit(None, person, details.clone());
        }
    }

    /// Start each person's transmission loop when they are assigned a rate
    /// function, and now for anyone already infected. Call at setup; a second
    /// call does nothing.
    fn setup_transmission_loop(&mut self) {
        let Some(already_infected) = start_transmission_loop::<M, _>(self) else {
            return;
        };
        // A reforecast, so it supersedes anything already pending.
        self.subscribe_to_event(|context, event: InfectiousnessAssignedEvent<M>| {
            TransmissionManager::<M>::reforecast_transmission(context, event.person);
        });
        self.subscribe_to_event(
            |context, event: ModifierBoundRaisedEvent<M, <Self as InfectionManager<M>>::Entity>| {
                TransmissionManager::<M>::reforecast_transmission(context, event.person);
            },
        );
        // They were assigned before the subscription, so nothing started them.
        for person in already_infected {
            self.reforecast_transmission(person);
        }
    }

    /// Drop `person`'s pending proposal and forecast again from now. Call it
    /// after anything that can raise their total infectiousness above their
    /// forecast; redrawing is always valid. No-op if never infected.
    fn reforecast_transmission(&mut self, person: EntityId<Self::Entity>) {
        if let Some(generation) = advance_forecast_generation::<M, _>(self, person) {
            schedule_proposal::<M, Self>(self, person, generation);
        }
    }
}

/// Draw `infector`'s next attempt proposal under `generation` and schedule it.
fn schedule_proposal<M, C>(
    context: &mut C,
    infector: EntityId<<C as InfectionManager<M>>::Entity>,
    generation: u32,
) where
    M: 'static,
    C: TransmissionManager<M>,
    Context:
        TransmissionManager<M> + InfectionManager<M, Entity = <C as InfectionManager<M>>::Entity>,
{
    // The forecast is indexed by time from now, so inverting an Exp(1) draw
    // gives the delay. `None` means its infectiousness is exhausted: the loop ends.
    let exp_draw = context.sample_distr(CoreEpiTransmissionRng, Exp::new(1.0).unwrap());
    let now = context.get_current_time();
    let proposal = {
        let forecast_fn = context.forecast_total_infectiousness(infector);
        forecast_fn.inverse_cum_rate(exp_draw).map(|delay| {
            // The forecast at the delay the plan will see once the proposal
            // time is rounded, bracketing by 1 ULP so a step discontinuity
            // cannot fall on the wrong side when `proposal_time - infection_time`
            // is later evaluated directly.
            let proposal_time = now + delay;
            let d = proposal_time - now;
            let d_prev = (proposal_time.next_down() - now).max(0.0);
            let d_next = proposal_time.next_up() - now;
            let forecast = forecast_fn
                .rate(d)
                .max(forecast_fn.rate(d_prev))
                .max(forecast_fn.rate(d_next));
            (proposal_time, forecast)
        })
    };

    if let Some((proposal_time, forecast)) = proposal {
        context.add_plan(proposal_time, move |ctx| {
            handle_proposal::<M>(ctx, infector, forecast, generation);
        });
    }
}

/// Handle a proposal forecast at total infectiousness `forecast`: thin it to
/// an attempt, then to a transmission by the contact's susceptibility.
fn handle_proposal<M>(
    context: &mut Context,
    infector: EntityId<<Context as InfectionManager<M>>::Entity>,
    forecast: f64,
    generation: u32,
) where
    M: 'static,
    Context: TransmissionManager<M>,
{
    // Superseded by a reforecast or a reinfection, which drew its own.
    if forecast_generation::<M, _>(context, infector) != Some(generation) {
        return;
    }
    // A proposal exactly at the end of the support has a zero forecast.
    let attempt = if forecast > 0.0 {
        context.evaluate_forecast(infector, forecast)
    } else {
        None
    };
    if let Some((infectee, details)) = attempt {
        let susceptibility = context.susceptibility(infectee, Some(infector), &details);
        // A silent cap would understate that person's relative risk.
        assert!(
            (0.0..=1.0 + BOUND_TOLERANCE).contains(&susceptibility),
            "susceptibility {susceptibility} of {infectee:?} is not in [0, 1]: scale \
             susceptibilities so the largest is 1 and put the scale in the infectiousness rate"
        );
        if sample_if_uncertain(context, susceptibility) {
            context.transmit(Some(infector), infectee, details);
            // The model's `on_infected` may have reforecast the infector,
            // which already drew the next proposal.
            if forecast_generation::<M, _>(context, infector) != Some(generation) {
                return;
            }
        }
    }
    schedule_proposal::<M, Context>(context, infector, generation);
}
