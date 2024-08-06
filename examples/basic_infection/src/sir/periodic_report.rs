use clap::Arg;
use eosim::{
    context::{Component, Context},
    global_properties::GlobalPropertyContext,
    people::PersonId,
    person_properties::{PersonProperty, PersonPropertyContext},
    reports::{Report, ReportsContext},
};
use serde_derive::Serialize;
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    hash::Hash,
};
use strum::IntoEnumIterator;

use super::{
    global_properties::{MaxDays, Population},
    person_properties::{DiseaseStatus, HealthStatus},
};

pub struct PropertyCounter {
    map: HashMap<TypeId, Box<dyn Any>>,
}

pub struct PeriodicReport {}

#[derive(Serialize)]
pub struct PeriodicStatus {
    pub time: f64,
    pub person_property: String,
    pub person_property_value: DiseaseStatus,
    pub value: usize,
}

impl Report for PeriodicReport {
    type Item = PeriodicStatus;
}

impl PropertyCounter {
    fn get_counter_map<A: PersonProperty + 'static>(&mut self) -> &mut HashMap<A::Value, usize> {
        self.map
            .entry(TypeId::of::<A>())
            .or_insert(Box::new(HashMap::<A::Value, usize>::default()))
            .downcast_mut::<HashMap<A::Value, usize>>()
            .unwrap()
    }
    pub fn increment_count<A: PersonProperty + 'static>(&mut self, value: A::Value)
    where
        A::Value: Hash + Eq,
    {
        let counter_map = self.get_counter_map::<A>();
        *counter_map.entry(value).or_insert(0) += 1;
    }

    pub fn decrement_count<A: PersonProperty + 'static>(&mut self, value: A::Value)
    where
        A::Value: Hash + Eq,
    {
        let counter_map = self.get_counter_map::<A>();
        *counter_map.entry(value).or_insert(0) -= 1;
    }

    pub fn get_count<A: PersonProperty + 'static>(&self, value: A::Value) -> usize
    where
        A::Value: Hash + Eq,
    {
        match self.map.get(&TypeId::of::<A>()) {
            Some(counter_map) => match counter_map
                .downcast_ref::<HashMap<A::Value, usize>>()
                .unwrap()
                .get(&value)
            {
                Some(count) => *count,
                None => 0,
            },
            None => 0,
        }
    }
}

pub fn report_items(context: &mut Context) {
    let mut property_counter = PropertyCounter {
        map: HashMap::default(),
    };

    let reporting_period = 7.0;
    let next_report_time = context.get_time() + reporting_period;
    let population = *context
        .get_global_property_value::<Population>()
        .expect("Population not specified");

    for id in 0..population {
        let p_id = PersonId::new(id);
        let dis_status = context.get_person_property_value::<DiseaseStatus>(p_id);
        let health_status = context.get_person_property_value::<HealthStatus>(p_id);
        property_counter.increment_count::<DiseaseStatus>(dis_status);
        property_counter.increment_count::<HealthStatus>(health_status);
    }

    for dis_status in DiseaseStatus::iter() {
        context.release_report_item::<PeriodicReport>(PeriodicStatus {
            time: context.get_time(),
            person_property: "DiseaseStatus".to_owned(),
            person_property_value: dis_status,
            value: property_counter.get_count::<DiseaseStatus>(dis_status),
        });
    }
    /*
    for health_status in HealthStatus::iter() {
        context.release_report_item::<PeriodicReport>(PeriodicStatus {
            time: context.get_time(),
            person_property: "HealthStatus".to_owned(),
            person_property_value: health_status,
            value: property_counter.get_count::<HealthStatus>(health_status)
        });
    }
    */

    let max_days = *context
        .get_global_property_value::<MaxDays>()
        .expect("MaxDays not specified") as f64;
    if next_report_time < max_days {
        context.add_plan(next_report_time, move |context| report_items(context));
    }
}

impl Component for PeriodicReport {
    fn init(context: &mut Context) {
        /*
        1. Initialize map with all the population
        2. Decide on a period (days, weeks, etc)
        3. Use a map and go for every person and keep track of disease status
        4. Add report item
        5. Clean map and schedule next reporting day
         */
        // Initialize
        // Loop through everyone and report their current status
        // Subscribe to changes in person properties

        //Schedule and release items
        report_items(context);
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_property_counter() {
        let mut property_counter = PropertyCounter {
            map: HashMap::default(),
        };

        assert_eq!(
            property_counter.get_count::<DiseaseStatus>(DiseaseStatus::S),
            0
        );
        assert_eq!(
            property_counter.get_count::<DiseaseStatus>(DiseaseStatus::E),
            0
        );
        assert_eq!(
            property_counter.get_count::<DiseaseStatus>(DiseaseStatus::I),
            0
        );
        assert_eq!(
            property_counter.get_count::<DiseaseStatus>(DiseaseStatus::R),
            0
        );
        assert_eq!(
            property_counter.get_count::<DiseaseStatus>(DiseaseStatus::D),
            0
        );

        property_counter.increment_count::<DiseaseStatus>(DiseaseStatus::S);
        property_counter.increment_count::<DiseaseStatus>(DiseaseStatus::R);

        assert_eq!(
            property_counter.get_count::<DiseaseStatus>(DiseaseStatus::S),
            1
        );
        assert_eq!(
            property_counter.get_count::<DiseaseStatus>(DiseaseStatus::R),
            1
        );

        property_counter.decrement_count::<DiseaseStatus>(DiseaseStatus::S);

        assert_eq!(
            property_counter.get_count::<DiseaseStatus>(DiseaseStatus::S),
            0
        );
        assert_eq!(
            property_counter.get_count::<DiseaseStatus>(DiseaseStatus::R),
            1
        );
    }

    //Add test for report creation
}
