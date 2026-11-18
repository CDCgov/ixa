use rand_distr::Exp;

use crate::core_epi::infection_manager::{
    advance_forecast_generation, forecast_generation, InfectionManager, InfectiousnessAssignedEvent,
};
use crate::core_epi::rate::{InvertibleRate, ScaledRateFn};
use crate::entity::query::Query;
use crate::entity::EntityId;
use crate::prelude::*;

define_rng!(CoreEpiTransmissionRng);
define_rng!(CoreEpiInfectionRng);

/// Relative slack when checking a total against its forecast: the two take
/// different float paths to the same value when the bound is tight.
const BOUND_TOLERANCE: f64 = 1e-9;

/// Infection loop transmission dynamics.
///
/// Transmission occurs due to a combination of three factors: (potential)
/// infector's infectiousness, the contact rate between the infector and
/// (potential) infectees, and the susceptibility of (potential) infectees.
///
/// A transmission "attempt" is when a transmission would occur if the
/// potential infectee is susceptible. Attempts account for infectiousness
/// and contact rates.
///
/// The loop algorithm is:
/// 1. Draw attempt proposal times from the model's
///    [`forecast_total_infectiousness`](Self::forecast_total_infectiousness).
/// 2. Thin proposals to attempts by current over forecast total
///    infectiousness in the model's
///    [`evaluate_forecast`](Self::evaluate_forecast), which also
///    picks the contact.
/// 3. Attempts become transmissions based on the potential infectee's
///    [`susceptibility`](InfectionManager::susceptibility).
///
/// Every method has a default, which together are homogeneous mixing: an
/// empty impl is a complete model. Most models describe their contact
/// structure with three methods:
/// [`contact_multiplier`](Self::contact_multiplier),
/// [`max_contact_multiplier`](Self::max_contact_multiplier) and
/// [`sample_contact`](Self::sample_contact). Override
/// [`forecast_total_infectiousness`] and [`evaluate_forecast`] instead when
/// the bound is not a constant multiple of the intrinsic one, or when the
/// total and the contact must come from the same draw.
///
/// [`forecast_total_infectiousness`]: Self::forecast_total_infectiousness
/// [`evaluate_forecast`]: Self::evaluate_forecast
///
/// A forecast must bound the total until the next proposal. When something
/// raises a person's total above it, call
/// [`reforecast_transmission`](Self::reforecast_transmission).
///
/// The loop ends when the forecast's integrated infectiousness is exhausted
/// (its inverse cumulative returns `None`, so no further proposal is
/// scheduled).
///
/// `M` is a marker type the model defines, so that it can implement these
/// traits for `Context` (a model crate owns neither, which the orphan rule
/// would otherwise forbid). Plans run on `Context` itself, not `Self`, hence
/// the `Context: TransmissionManager<M>` bound.
pub trait TransmissionManager<M: 'static>:
    InfectionManager<M> + crate::ContextRandomExt + ContextEntitiesExt
where
    Context: TransmissionManager<M>
        + InfectionManager<M, Entity = <Self as InfectionManager<M>>::Entity>,
{
    /// Factor on `person`'s intrinsic infectiousness from their contacts now:
    /// how many people they can reach, how closely. The default is 1.
    fn contact_multiplier(&self, _person: EntityId<Self::Entity>) -> f64 {
        1.0
    }

    /// Upper bound on [`contact_multiplier`](Self::contact_multiplier) for
    /// `person` until their next proposal. It must hold whatever happens in
    /// between, or be followed by
    /// [`reforecast_transmission`](Self::reforecast_transmission). The
    /// default is 1.
    fn max_contact_multiplier(&self, _person: EntityId<Self::Entity>) -> f64 {
        1.0
    }

    /// Who `person` contacts in an attempt, and what to record about the
    /// exposure; `None` if nobody. The default is anyone else, uniformly.
    fn sample_contact(
        &self,
        person: EntityId<Self::Entity>,
    ) -> Option<(EntityId<Self::Entity>, Self::Exposure)> {
        Some((self.sample_anyone_else(person)?, Self::Exposure::default()))
    }

    /// Someone other than `person`, uniformly from the population; `None` if
    /// there is nobody else.
    fn sample_anyone_else(&self, person: EntityId<Self::Entity>) -> Option<EntityId<Self::Entity>> {
        self.sample(CoreEpiTransmissionRng, |rng| {
            self.query(Self::Entity::default())
                .sample_entity_excluding(rng, person)
        })
    }

    /// Upper bound on `person`'s total infectiousness, indexed by time from now
    /// (`t = 0` is the current simulation time). The loop inverts it to
    /// propose attempt times. The default is
    /// [`majorizing_infectiousness_rate_fn`](InfectionManager::majorizing_infectiousness_rate_fn)
    /// scaled by [`max_contact_multiplier`](Self::max_contact_multiplier).
    /// Override it for a bound that is not a constant multiple of the
    /// intrinsic one, e.g. one that follows a schedule.
    fn forecast_total_infectiousness(&self, person: EntityId<Self::Entity>) -> impl InvertibleRate {
        ScaledRateFn::new(
            self.majorizing_infectiousness_rate_fn(person),
            self.max_contact_multiplier(person),
        )
    }

    /// Evaluate an attempt proposal for `person` forecast at total
    /// infectiousness `forecast`: compute their total infectiousness now,
    /// thin with [`accept_proposal`](Self::accept_proposal), and return who
    /// they contact. `None` drops the proposal; the loop continues. The
    /// default total is
    /// [`intrinsic_infectiousness`](InfectionManager::intrinsic_infectiousness)
    /// times [`contact_multiplier`](Self::contact_multiplier), and the
    /// contact comes from [`sample_contact`](Self::sample_contact). Override
    /// it when the total and the contact must come from the same draw, e.g.
    /// a setting drawn first.
    fn evaluate_forecast(
        &self,
        person: EntityId<Self::Entity>,
        forecast: f64,
    ) -> Option<(EntityId<Self::Entity>, Self::Exposure)> {
        let total = self.intrinsic_infectiousness(person)? * self.contact_multiplier(person);
        if !self.accept_proposal(person, total, forecast) {
            return None;
        }
        self.sample_contact(person)
    }

    /// Whether a proposal at `forecast` becomes an attempt at `total`
    /// infectiousness: with probability `total / forecast`, drawing only when
    /// the outcome is uncertain.
    ///
    /// # Panics
    ///
    /// If `total` exceeds `forecast`: the forecast must bound the total until
    /// the next proposal, or attempts are under-thinned.
    fn accept_proposal(&self, person: EntityId<Self::Entity>, total: f64, forecast: f64) -> bool {
        assert!(
            total <= forecast * (1.0 + BOUND_TOLERANCE),
            "total infectiousness {total} of {person:?} exceeds its forecast {forecast}: \
             the forecast must bound the total until the next proposal"
        );
        total > 0.0
            && (total >= forecast || self.sample_bool(CoreEpiTransmissionRng, total / forecast))
    }

    /// Infect `infectee`: assign their rate function, which starts their own
    /// loop once [`setup_transmission_loop`](Self::setup_transmission_loop)
    /// has been called, then call [`on_infected`](InfectionManager::on_infected). `infector` is
    /// `None` for an infection with no source in the population; for those,
    /// [`seed_infection`](Self::seed_infection) is shorter.
    fn transmit(
        &mut self,
        infector: Option<EntityId<Self::Entity>>,
        infectee: EntityId<Self::Entity>,
        exposure: Self::Exposure,
    ) {
        self.assign_infectiousness_rate_fn(infectee);
        self.on_infected(infector, infectee, exposure);
    }

    /// Infect `person` with no infector and the default exposure.
    fn seed_infection(&mut self, person: EntityId<Self::Entity>) {
        self.transmit(None, person, Self::Exposure::default());
    }

    /// Infect up to `n` people (fewer if fewer match) sampled uniformly from
    /// those matching `query`, with no infector.
    fn seed_infections<Q: Query<Self::Entity>>(&mut self, query: Q, n: usize) {
        for person in self.sample_entities(CoreEpiInfectionRng, query, n) {
            self.seed_infection(person);
        }
    }

    /// Start each person's transmission loop when they are assigned a rate
    /// function.
    /// Call once at setup, before anyone is infected. Without it, only loops
    /// started by hand run (e.g. an index case with no cascade).
    fn setup_transmission_loop(&mut self) {
        self.subscribe_to_event(|context, event: InfectiousnessAssignedEvent<M>| {
            // Supersedes anything already pending: the person may have
            // been reforecast or infected again since this event was
            // emitted.
            TransmissionManager::<M>::reforecast_transmission(context, event.person);
        });
    }

    /// Drop `person`'s pending attempt proposal and forecast again from now.
    /// Call after anything that can raise their total infectiousness above
    /// what [`forecast_total_infectiousness`](Self::forecast_total_infectiousness)
    /// returned when the pending proposal was drawn, e.g. their group growing
    /// or a contact restriction ending: the loop panics if a total exceeds its
    /// forecast, and a forecast of zero ends the loop. It also restarts a loop
    /// that ended that way. Redrawing is always statistically valid, so when
    /// in doubt, call it. It also begins the loop of someone infected without
    /// [`setup_transmission_loop`](Self::setup_transmission_loop). No-op if
    /// `person` was never infected.
    fn reforecast_transmission(&mut self, person: EntityId<Self::Entity>) {
        if let Some(generation) = advance_forecast_generation::<M, _, _>(self, person) {
            schedule_proposal::<M, Self>(self, person, generation);
        }
    }
}

/// Draw `infector`'s next attempt proposal under `generation` and schedule
/// it. Private, with [`handle_proposal`], so that a proposal only ever runs
/// under the generation it was drawn with and a person never has two loops.
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
    // Determine if & when the next attempt proposal will occur. The majorizing
    // rate function is indexed by time from now, so inverting an Exp(1) draw
    // gives the delay to the proposal. If the draw is beyond the support of
    // the inverse cumulative there is no next event and the loop ends.
    let exp_draw = context.sample_distr(CoreEpiTransmissionRng, Exp::new(1.0).unwrap());
    let proposal = {
        let forecast_fn = context.forecast_total_infectiousness(infector);
        forecast_fn
            .inverse_cum_rate(exp_draw)
            .map(|delay| (delay, forecast_fn.rate(delay)))
    };

    if let Some((delay, forecast)) = proposal {
        let proposal_time = context.get_current_time() + delay;
        context.add_plan(proposal_time, move |ctx| {
            handle_proposal::<M, Context>(ctx, infector, forecast, generation);
        });
    };
}

/// Handle a transmission attempt proposal forecast at total infectiousness
/// `forecast`: hand it to `evaluate_forecast`, then thin attempts to
/// transmissions based on susceptibility.
fn handle_proposal<M, C>(
    context: &mut C,
    infector: EntityId<<C as InfectionManager<M>>::Entity>,
    forecast: f64,
    generation: u32,
) where
    M: 'static,
    C: TransmissionManager<M>,
    Context:
        TransmissionManager<M> + InfectionManager<M, Entity = <C as InfectionManager<M>>::Entity>,
{
    // Superseded by a reforecast or a reinfection, which drew its own.
    if forecast_generation::<M, _, _>(context, infector) != Some(generation) {
        return;
    }
    // A zero forecast (a proposal exactly at the end of the support) can
    // never attempt.
    if forecast > 0.0 {
        if let Some((infectee, exposure)) = context.evaluate_forecast(infector, forecast) {
            // Draws only when the outcome is uncertain.
            let susceptibility = context.susceptibility(infectee);
            let infected = susceptibility >= 1.0
                || (susceptibility > 0.0
                    && context.sample_bool(CoreEpiTransmissionRng, susceptibility));
            if infected {
                context.transmit(Some(infector), infectee, exposure);
                // Only a transmission runs model code, which may have
                // reforecast the infector: then the next proposal is already
                // drawn, and one drawn here would be dropped when it fired.
                if forecast_generation::<M, _, _>(context, infector) != Some(generation) {
                    return;
                }
            }
        }
    }
    schedule_proposal::<M, C>(context, infector, generation);
}
