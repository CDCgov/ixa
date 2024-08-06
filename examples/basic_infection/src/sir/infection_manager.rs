use eosim::{
    context::{Component, Context},
    global_properties::GlobalPropertyContext,
    people::PersonId,
    person_properties::PersonPropertyContext,
    random::RandomContext,
};

use rand_distr::{Distribution, Exp};

use super::{
    global_properties::HospitalizationDelay, global_properties::HospitalizationDuration,
    global_properties::IncubationPeriod, global_properties::InfectiousPeriod,
    global_properties::LatentPeriod, global_properties::ProbabilityHospitalized,
    global_properties::ProbabilitySymptoms, global_properties::SymptomaticPeriod,
    person_properties::DiseaseStatus, person_properties::HealthStatus,
};

use rand::Rng;
pub struct InfectionManager {}

eosim::define_random_id!(InfectionRandomId);

pub fn handle_person_disease_status_change(
    context: &mut Context,
    person_id: PersonId,
    _: DiseaseStatus,
) {
    let disease_status = context.get_person_property_value::<DiseaseStatus>(person_id);
    if matches!(disease_status, DiseaseStatus::E) {
        handle_exposed(context, person_id)
    }
    if matches!(disease_status, DiseaseStatus::I) {
        schedule_recovery(context, person_id)
    }
}

pub fn handle_person_health_status_change(
    context: &mut Context,
    person_id: PersonId,
    _: HealthStatus,
) {
    let health_status = context.get_person_property_value::<HealthStatus>(person_id);
    if matches!(health_status, HealthStatus::Symp) {
        handle_symptomatic(context, person_id)
    }
    if matches!(health_status, HealthStatus::Hospitalized) {
        handle_hospitalized(context, person_id)
    }
}

pub fn handle_hospitalized(context: &mut Context, person_id: PersonId) {
    let hospitalization_duration = *context
        .get_global_property_value::<HospitalizationDuration>()
        .expect("Hospitalization duration not specified");
    let hospitalization_duration_dist = Exp::new(1.0 / hospitalization_duration).unwrap();
    let recovery_time = context.get_time()
        + hospitalization_duration_dist.sample(&mut *context.get_rng::<InfectionRandomId>());
    context.add_plan(recovery_time, move |context| {
        context.set_person_property_value::<HealthStatus>(person_id, HealthStatus::Recovered);
    });
}

pub fn handle_symptomatic(context: &mut Context, person_id: PersonId) {
    let hosp_probability = *context
        .get_global_property_value::<ProbabilityHospitalized>()
        .expect("Probability of hospitalization not specified");
    let mut rng = context.get_rng::<InfectionRandomId>();

    let become_hospitalized = rng.gen::<f64>() < hosp_probability;
    drop(rng);

    if become_hospitalized {
        let hospitalization_delay = *context
            .get_global_property_value::<HospitalizationDelay>()
            .expect("Hospitalization delay not specified");

        let hospitalization_delay_dist = Exp::new(1.0 / hospitalization_delay).unwrap();
        let hospitalization_time = context.get_time()
            + hospitalization_delay_dist.sample(&mut *context.get_rng::<InfectionRandomId>());

        context.add_plan(hospitalization_time, move |context| {
            context
                .set_person_property_value::<HealthStatus>(person_id, HealthStatus::Hospitalized);
        });
    } else {
        let symptomatic_period = *context
            .get_global_property_value::<SymptomaticPeriod>()
            .expect("Symptomatic period not specified");

        let symptomatic_period_dist = Exp::new(1.0 / symptomatic_period).unwrap();
        let recovery_time = context.get_time()
            + symptomatic_period_dist.sample(&mut *context.get_rng::<InfectionRandomId>());

        context.add_plan(recovery_time, move |context| {
            context.set_person_property_value::<HealthStatus>(person_id, HealthStatus::Recovered);
        });
    }
}

pub fn handle_exposed(context: &mut Context, person_id: PersonId) {
    let latent_period = *context
        .get_global_property_value::<LatentPeriod>()
        .expect("Latent Period not specified");
    //What does unwrap do?
    //- returns error or optional
    let latent_period_dist = Exp::new(1.0 / latent_period).unwrap();
    let infectious_time = context.get_time()
        + latent_period_dist.sample(&mut *context.get_rng::<InfectionRandomId>());
    //println!("Exposed for person with time {infectious_time}");
    context.add_plan(infectious_time, move |context| {
        context.set_person_property_value::<DiseaseStatus>(person_id, DiseaseStatus::I);
    });

    let symptoms_probability = *context
        .get_global_property_value::<ProbabilitySymptoms>()
        .expect("Probability of developing symptoms not specified");
    let mut rng = context.get_rng::<InfectionRandomId>();

    let develop_symptoms = rng.gen::<f64>() < symptoms_probability;
    drop(rng);
    let incubation_period = *context
        .get_global_property_value::<IncubationPeriod>()
        .expect("Incubation period not specified");
    if develop_symptoms {
        let incubation_period_dist = Exp::new(1.0 / incubation_period).unwrap();
        let symptoms_onset = context.get_time()
            + incubation_period_dist.sample(&mut *context.get_rng::<InfectionRandomId>());

        context.add_plan(symptoms_onset, move |context| {
            context.set_person_property_value::<HealthStatus>(person_id, HealthStatus::Symp);
        });
    }
}

pub fn schedule_recovery(context: &mut Context, person_id: PersonId) {
    let infectious_period = *context
        .get_global_property_value::<InfectiousPeriod>()
        .expect("Infectious Period not Specified");
    let infectious_period_dist = Exp::new(1.0 / infectious_period).unwrap();
    let recovery_time = context.get_time()
        + infectious_period_dist.sample(&mut *context.get_rng::<InfectionRandomId>());
    context.add_plan(recovery_time, move |context| {
        context.set_person_property_value::<DiseaseStatus>(person_id, DiseaseStatus::R);
    });
}

impl Component for InfectionManager {
    fn init(context: &mut Context) {
        context
            .observe_person_property_changes::<DiseaseStatus>(handle_person_disease_status_change);
        context.observe_person_property_changes::<HealthStatus>(handle_person_health_status_change);
    }
}
