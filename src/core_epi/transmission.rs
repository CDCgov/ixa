use rand_distr::Exp;

use crate::core_epi::infection_manager::{InfectionManager, InfectiousnessAssignedEvent};
use crate::core_epi::person::{Person, PersonId};
use crate::core_epi::rate::InvertibleRate;
use crate::entity::query::Query;
use crate::prelude::*;

define_rng!(TransmissionRng);
define_rng!(InfectionRng);

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
/// 1. Draw attempt proposal times based on majorizing total infectiousness.
/// 2. Thin proposed attempts to attempts by current over forecast total
///    infectiousness in the model's
///    [`evaluate_forecast`](Self::evaluate_forecast), which also
///    picks the contact.
/// 3. Attempts become transmissions based on the potential infectee's susceptibility.
///
/// The loop ends when infectiousness ends for good
/// ([`intrinsic_infectiousness`](InfectionManager::intrinsic_infectiousness) is `None`) or
/// the majorizing rate's integrated infectiousness is exhausted (its inverse
/// cumulative returns `None`, so no further proposal is scheduled).
///
/// `M` is a marker type the model defines, so that it can implement these
/// traits for `Context` (a model crate owns neither, which the orphan rule
/// would otherwise forbid). Plans run on `Context` itself, not `Self`, hence
/// the `Context: Transmission<M>` bound.
pub trait Transmission<M: 'static>:
    InfectionManager<M> + crate::ContextRandomExt + ContextEntitiesExt
where
    Context: Transmission<M>,
{
    /// Upper bound on `person`'s total infectiousness, indexed by time from now
    /// (`t = 0` is the current simulation time). Built on
    /// [`majorizing_infectiousness_rate_fn`](InfectionManager::majorizing_infectiousness_rate_fn),
    /// e.g. scaled by the largest contact rate `person` can have this run.
    fn forecast_total_infectiousness(&self, person: PersonId) -> impl InvertibleRate;

    /// Evaluate an attempt forecast for `person` at total infectiousness
    /// `forecast`. Accept it with probability `total / forecast`, where `total`
    /// is their current
    /// [`intrinsic_infectiousness`](InfectionManager::intrinsic_infectiousness)
    /// times the contact rate for wherever they are right now, and return who
    /// they contact. `None` means nobody was contacted this time; the attempt is
    /// dropped and the loop continues.
    fn evaluate_forecast(&self, person: PersonId, forecast: f64) -> Option<PersonId>;

    /// Probability that a person, subjected to an infection attempt, will be infected.
    fn susceptibility(&self, person: PersonId) -> f64;

    /// Infect `infectee`: assign their rate function, which starts their own
    /// loop (see [`setup_transmission_loop`](Self::setup_transmission_loop)),
    /// then call [`on_infected`](InfectionManager::on_infected). `infector` is
    /// `None` for an infection with no source in the population, e.g. an
    /// initial seed.
    fn transmit(&mut self, infector: Option<PersonId>, infectee: PersonId) {
        self.assign_infectiousness_rate_fn(infectee);
        self.on_infected(infector, infectee);
    }

    /// Infect up to `n` people (fewer if fewer match) sampled uniformly from
    /// those matching `query`, with no infector.
    fn seed_infections<Q: Query<Person>>(&mut self, query: Q, n: usize) {
        for person in self.sample_entities(InfectionRng, query, n) {
            self.transmit(None, person);
        }
    }

    /// Start each person's transmission loop when they are assigned a rate
    /// function.
    /// Call once at setup, before anyone is infected. Without it, only loops
    /// started by hand run (e.g. an index case with no cascade).
    fn setup_transmission_loop(&mut self) {
        self.subscribe_to_event(|context, event: InfectiousnessAssignedEvent| {
            Transmission::<M>::schedule_next_transmission_attempt_proposal(context, event.person);
        });
    }

    /// Schedule the next attempt proposal.
    ///
    /// This function begins the transmission loop for an infector.
    fn schedule_next_transmission_attempt_proposal(&mut self, infector: PersonId) {
        // Determine if & when the next attempt proposal will occur. The majorizing
        // rate function is indexed by time from now, so inverting an Exp(1) draw
        // gives the delay to the proposal. If `e` is beyond the support of the
        // inverse cumulative there is no next event and the loop ends.
        let e = self.sample_distr(TransmissionRng, Exp::new(1.0).unwrap());
        let proposal = {
            let f = self.forecast_total_infectiousness(infector);
            f.inverse_cum_rate(e).map(|delay| (delay, f.rate(delay)))
        };

        if let Some((delay, forecast)) = proposal {
            let t = self.get_current_time() + delay;
            self.add_plan(t, move |ctx| {
                Transmission::<M>::handle_transmission_attempt_proposal(ctx, infector, forecast);
            });
        };
    }

    /// Handle a transmission attempt proposal forecast at total infectiousness
    /// `forecast`: hand it to [`evaluate_forecast`](Self::evaluate_forecast),
    /// then thin attempts to transmissions based on susceptibility.
    fn handle_transmission_attempt_proposal(&mut self, infector: PersonId, forecast: f64) {
        // A `None` infectiousness means it has ended for good: do not
        // reschedule, so this is the infector's last proposal for this infection
        if self.intrinsic_infectiousness(infector).is_none() {
            return;
        }
        // A zero forecast (a proposal exactly at the end of the support) can
        // never attempt.
        if forecast > 0.0 {
            // Thin the attempts to infections based on susceptibility, drawing
            // only when the outcome is uncertain.
            if let Some(infectee) = self.evaluate_forecast(infector, forecast) {
                let p = self.susceptibility(infectee);
                let infected = p >= 1.0 || (p > 0.0 && self.sample_bool(TransmissionRng, p));
                if infected {
                    self.transmit(Some(infector), infectee);
                } // else: infectee non-susceptibility prevented transmission
            } // else: total infectiousness thinned it out, or nobody to contact
        }
        self.schedule_next_transmission_attempt_proposal(infector);
    }
}
