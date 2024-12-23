use ixa::{
    context::Context, 
    people::{ContextPeopleExt,
        PersonPropertyChangeEvent, }, 
    define_person_property,
    define_person_property_with_default,
    ContextGlobalPropertiesExt, 
    network::{ContextNetworkExt, Edge, EdgeType,}, 
    random::{ContextRandomExt, define_rng}, 
    ExecutionPhase,
    PersonId,
};
use crate::parameters::Parameters;
use crate::network::{HH, U5, U18};
use rand_distr::{Bernoulli, Gamma};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum DiseaseStatusValue {
    S,
    E,
    I,
    R,
}

define_rng!(SeirRng);

define_person_property_with_default!(DiseaseStatus, DiseaseStatusValue, DiseaseStatusValue::S);

fn sar_to_beta(sar: f64, infectious_period: f64) -> f64 {
    1.0 - (1.0 - sar).powf(1.0 / infectious_period)
}

fn calculate_waiting_time(context: &Context, shape: f64, mean_period: f64) -> f64 {
    let d = Gamma::new(shape, mean_period/ shape).unwrap();
    context.sample_distr(SeirRng, d)
}

pub fn get_i_s_edges<T: EdgeType + 'static>(context: &Context) -> Vec<Edge<T::Value>> {

    let infected = context.query_people((DiseaseStatus, DiseaseStatusValue::I));
    let mut edges = Vec::new();

    println!("n infected: {:?}", infected.len());
    for i in infected {
               
        edges.extend(context
            .get_matching_edges::<T>(
                i, 
                |context , edge| { 
                    context.match_person(
                        edge.neighbor, 
                        (DiseaseStatus, DiseaseStatusValue::S))
                }
            )
        );
    }

    edges
}

fn expose_network<T: EdgeType + 'static>(context: &mut Context,
    beta: f64) {

    let edges = get_i_s_edges::<T>(context);
    for e in edges {
        if context.sample_distr(SeirRng, Bernoulli::new(beta).unwrap()) {
            context.set_person_property(e.neighbor,
                 DiseaseStatus,
                 DiseaseStatusValue::E);
        }
    }

}

fn schedule_waiting_event(context: &mut Context, person_id: PersonId, 
        shape: f64, mean_period: f64, new_status: DiseaseStatusValue) {

    let ct = context.get_current_time();
    let waiting_time = calculate_waiting_time(context, shape, mean_period);

    context.add_plan(ct + waiting_time, move |context| {
        context.set_person_property(person_id, DiseaseStatus, new_status);
    });

}

fn schedule_infection(context: &mut Context, person_id: PersonId) {

    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();

    schedule_waiting_event(context, person_id, parameters.shape, 
        parameters.incubation_period, DiseaseStatusValue::I);

}

fn schedule_recovery(context: &mut Context, person_id: PersonId) {

    let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();

    schedule_waiting_event(context, person_id, parameters.shape, 
        parameters.infectious_period, DiseaseStatusValue::R);

}

pub fn init(context: &mut Context, initial_infections: Vec<PersonId>) {

    context.add_periodic_plan_with_phase(
        1.0, 
        |context| {

        println!("Current time is {}.", context.get_current_time());

        let parameters = context
        .get_global_property_value(Parameters)
        .unwrap()
        .clone();

        // infect the networks
        expose_network::<HH>(context, sar_to_beta(parameters.sar, 
            parameters.incubation_period));
        expose_network::<U5>(context, sar_to_beta(parameters.sar / parameters.between_hh_transmission_reduction, 
                parameters.incubation_period));
        expose_network::<U18>(context, sar_to_beta(parameters.sar / parameters.between_hh_transmission_reduction, 
                parameters.incubation_period));

    }, ExecutionPhase::Normal);

    context.subscribe_to_event(
        move |context, 
        event: PersonPropertyChangeEvent<DiseaseStatus>| {

        match event.current {
            DiseaseStatusValue::E => schedule_infection(context, event.person_id),
            DiseaseStatusValue::I => schedule_recovery(context, event.person_id),
            _ => (),
        };
        
    });

    // expose the first person to the disease
    for ii in initial_infections {
        context.set_person_property(ii, DiseaseStatus, 
            DiseaseStatusValue::E);
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use ixa::context::Context;
    use crate::parameters::ParametersValues;
    use crate::{loader, network};
    use crate::loader::Id;

    #[test]
    fn test_disease_status() {
        let mut context = Context::new();
        
        context.init_random(42);

        let people = loader::init(&mut context);

        // set sar and between_hh_transmission_reduction to 1.0 so that
        // beta is 1.0
        let parameters = ParametersValues {
            incubation_period: 8.0,
            infectious_period: 27.0,
            sar: 1.0,
            shape: 15.0,
            infection_duration: 5.0,
            between_hh_transmission_reduction: 1.0,
        };
        context.set_global_property_value(Parameters, parameters).unwrap();

        network::init(&mut context, &people);
        
        let to_infect = context.query_people((Id, 335));

        init(&mut context, to_infect);

        context.execute();

    }
}
