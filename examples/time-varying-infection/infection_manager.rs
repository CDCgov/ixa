use ixa::context::Context;
use ixa::define_rng;
use ixa::people::{ContextPeopleExt, PersonId, PersonPropertyChangeEvent};
use ixa::random::ContextRandomExt;

use rand_distr::Exp;

use crate::population_loader::{DiseaseStatus, DiseaseStatusType};
use crate::INFECTION_DURATION;

define_rng!(InfectionRng);

fn schedule_recovery(context: &mut Context, person_id: PersonId) {
    let recovery_time = context.get_current_time()
        + context.sample_distr(InfectionRng, Exp::new(1.0 / INFECTION_DURATION).unwrap());
    context.add_plan(recovery_time, move |context| {
        context.set_person_property(person_id, DiseaseStatusType, DiseaseStatus::R);
    });
}

fn handle_infection_status_change(
    context: &mut Context,
    event: PersonPropertyChangeEvent<DiseaseStatusType>,
) {
    if matches!(event.current, DiseaseStatus::I) {
        schedule_recovery(context, event.person_id);
    }
}

pub fn init(context: &mut Context) {
    context.subscribe_to_event(
        move |context, event: PersonPropertyChangeEvent<DiseaseStatusType>| {
            handle_infection_status_change(context, event);
        },
    );
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::SEED;
    use ixa::context::Context;
    use ixa::people::{ContextPeopleExt, PersonPropertyChangeEvent};
    use ixa::random::ContextRandomExt;

    use crate::population_loader::{DiseaseStatus, DiseaseStatusType};

    fn handle_recovery_event(
        _context: &mut Context,
        event: PersonPropertyChangeEvent<DiseaseStatusType>,
    ) {
        assert_eq!(event.current, DiseaseStatus::R);
        assert_eq!(event.previous, DiseaseStatus::I);
    }

    #[test]
    fn test_handle_infection_change() {
        let mut context = Context::new();
        context.init_random(SEED);
        init(&mut context);

        let population_size = 1;
        for id in 0..population_size {
            context.add_person();
            context.set_person_property(
                context.get_person_id(id),
                DiseaseStatusType,
                DiseaseStatus::I,
            );
        }

        context.subscribe_to_event(
            move |context, event: PersonPropertyChangeEvent<DiseaseStatusType>| {
                handle_recovery_event(context, event);
            },
        );

        context.execute();
    }
}
