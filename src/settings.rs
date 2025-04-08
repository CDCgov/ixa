//! Settings (locations) represent places that are contexts in which transmission takes place. A
//! setting determines which other people are in contact with a particular person and for how long.
//! The `SettingProperties::alpha` parameter determines how the hazard is distributed among an
//! infected person's contacts within a setting.
//!
//! Data related to settings is managed by the `SettingsDataContainer`. A setting is a type that
//! implements the `SettingType` trait.
//!
//!

use crate::people::PersonId;
use crate::{
    define_data_plugin, define_rng, Context, ContextRandomExt, HashMap, HashSet, IxaError,
};
use std::any::TypeId;
use std::marker::PhantomData;

define_rng!(SettingsRng);

/// In a setting with `n` people (including the source of infection), the rate of the total infectiousness process
/// is computed as
///      (intrinsic infectiousness) × (n - 1)ᵅ
/// where 0 ≤ 𝛼 ≤ 1. This interpolates between having the total hazard _distributed_ equally and the total hazard
/// applying equally to the nonsources.
#[derive(Debug, Clone, Copy)]
pub struct SettingProperties {
    alpha: f64,
}

pub trait SettingType {
    fn calculate_multiplier(
        &self,
        members: &[PersonId],
        setting_properties: SettingProperties,
    ) -> f64;
}

#[derive(Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct SettingId<T: SettingType + 'static> {
    pub id: usize,

    phantom: PhantomData<*const T>,
}

#[allow(dead_code)]
impl<T: SettingType + 'static> SettingId<T> {
    pub fn new(id: usize) -> SettingId<T> {
        SettingId {
            id,
            phantom: PhantomData,
        }
    }
}

pub struct ItineraryEntry {
    setting_type: TypeId,
    setting_id: usize,
    ratio: f64,
}

#[allow(dead_code)]
impl ItineraryEntry {
    fn new<T: SettingType>(setting_id: &SettingId<T>, ratio: f64) -> ItineraryEntry {
        ItineraryEntry {
            setting_type: TypeId::of::<T>(),
            setting_id: setting_id.id,
            ratio,
        }
    }
}

pub struct SettingsDataContainer {
    setting_types: HashMap<TypeId, Box<dyn SettingType>>,
    // For each setting type (e.g., Home) store the properties (e.g., alpha)
    setting_properties: HashMap<TypeId, SettingProperties>,
    // For each setting type, have a map of each setting id and a list of members
    // Maps `T: SettingType` -> `Map<SettingId<T>, People>`
    members: HashMap<TypeId, HashMap<usize, Vec<PersonId>>>,
    itineraries: HashMap<PersonId, Vec<ItineraryEntry>>,
}

impl SettingsDataContainer {
    fn new() -> Self {
        SettingsDataContainer {
            setting_types: HashMap::default(),
            setting_properties: HashMap::default(),
            members: HashMap::default(),
            itineraries: HashMap::default(),
        }
    }
    fn get_setting_members(
        &self,
        setting_type: &TypeId,
        setting_id: usize,
    ) -> Option<&Vec<PersonId>> {
        self.members.get(setting_type)?.get(&setting_id)
    }
    fn with_itinerary<F>(&self, person_id: PersonId, mut callback: F)
    where
        F: FnMut(&dyn SettingType, &SettingProperties, &Vec<PersonId>, f64),
    {
        if let Some(itinerary) = self.itineraries.get(&person_id) {
            for entry in itinerary {
                let setting_type = self.setting_types.get(&entry.setting_type).unwrap();
                let setting_props = self.setting_properties.get(&entry.setting_type).unwrap();
                let members = self
                    .get_setting_members(&entry.setting_type, entry.setting_id)
                    .unwrap();
                callback(setting_type.as_ref(), setting_props, members, entry.ratio);
            }
        }
    }
}

// Define a home setting
#[derive(Default, Debug, Hash, Eq, PartialEq)]
pub struct Home {}

impl SettingType for Home {
    // Read members and setting_properties as arguments
    fn calculate_multiplier(
        &self,
        members: &[PersonId],
        setting_properties: SettingProperties,
    ) -> f64 {
        let n_members = members.len();
        #[allow(clippy::cast_precision_loss)]
        ((n_members - 1) as f64).powf(setting_properties.alpha)
    }
}

#[derive(Default, Debug, Hash, Eq, PartialEq)]
pub struct CensusTract {}
impl SettingType for CensusTract {
    fn calculate_multiplier(
        &self,
        members: &[PersonId],
        setting_properties: SettingProperties,
    ) -> f64 {
        let n_members = members.len();
        #[allow(clippy::cast_precision_loss)]
        ((n_members - 1) as f64).powf(setting_properties.alpha)
    }
}

define_data_plugin!(
    SettingDataPlugin,
    SettingsDataContainer,
    SettingsDataContainer::new()
);

#[allow(dead_code)]
pub trait ContextSettingExt {
    fn get_setting_properties<T: SettingType + 'static>(&self) -> SettingProperties;
    fn register_setting_type<T: SettingType + 'static>(
        &mut self,
        setting: T,
        setting_props: SettingProperties,
    );
    fn add_itinerary(
        &mut self,
        person_id: PersonId,
        itinerary: Vec<ItineraryEntry>,
    ) -> Result<(), IxaError>;
    fn get_setting_members<T: SettingType + 'static>(
        &self,
        setting_id: SettingId<T>,
    ) -> Option<&Vec<PersonId>>;
    fn calculate_total_infectiousness_multiplier_for_person(&self, person_id: PersonId) -> f64;
    fn get_itinerary(&self, person_id: PersonId) -> Option<&Vec<ItineraryEntry>>;
    fn get_contact<T: SettingType + 'static>(
        &self,
        person_id: PersonId,
        setting_id: SettingId<T>,
    ) -> Option<PersonId>;
    fn draw_contact_from_itinerary(&self, person_id: PersonId) -> Option<PersonId>;
}

trait ContextSettingInternalExt {
    fn get_contact_internal(
        &self,
        person_id: PersonId,
        setting_type: TypeId,
        setting_id: usize,
    ) -> Option<PersonId>;
    fn get_setting_members_internal(
        &self,
        setting_type: TypeId,
        setting_id: usize,
    ) -> Option<&Vec<PersonId>>;
}

impl ContextSettingInternalExt for Context {
    fn get_contact_internal(
        &self,
        person_id: PersonId,
        setting_type: TypeId,
        setting_id: usize,
    ) -> Option<PersonId> {
        if let Some(members) = self.get_setting_members_internal(setting_type, setting_id) {
            if members.len() == 1 {
                return None;
            }
            let mut contact_id = person_id;
            while contact_id == person_id {
                contact_id = members[self.sample_range(SettingsRng, 0..members.len())];
            }
            Some(contact_id)
        } else {
            None
        }
    }
    fn get_setting_members_internal(
        &self,
        setting_type: TypeId,
        setting_id: usize,
    ) -> Option<&Vec<PersonId>> {
        self.get_data_container(SettingDataPlugin)?
            .get_setting_members(&setting_type, setting_id)
    }
}

impl ContextSettingExt for Context {
    fn get_setting_properties<T: SettingType + 'static>(&self) -> SettingProperties {
        let data_container = self
            .get_data_container(SettingDataPlugin)
            .unwrap()
            .setting_properties
            .get(&TypeId::of::<T>())
            .unwrap();
        *data_container
    }
    fn register_setting_type<T: SettingType + 'static>(
        &mut self,
        setting_type: T,
        setting_props: SettingProperties,
    ) {
        let container = self.get_data_container_mut(SettingDataPlugin);

        // Add the setting
        container
            .setting_types
            .insert(TypeId::of::<T>(), Box::new(setting_type));

        // Add properties
        container
            .setting_properties
            .insert(TypeId::of::<T>(), setting_props);
    }
    fn add_itinerary(
        &mut self,
        person_id: PersonId,
        itinerary: Vec<ItineraryEntry>,
    ) -> Result<(), IxaError> {
        let container = self.get_data_container_mut(SettingDataPlugin);
        // `setting_counts` maps `T: SettingType` to set of `SettingId<T>`,
        // its list of setting instances for this person.
        let mut setting_counts: HashMap<TypeId, HashSet<usize>> = HashMap::default();
        for itinerary_entry in &itinerary {
            let setting_id = itinerary_entry.setting_id;
            let setting_type = itinerary_entry.setting_type;
            if let Some(setting_count_set) = setting_counts.get(&setting_type) {
                if setting_count_set.contains(&setting_id) {
                    return Err(IxaError::from("Duplicated setting".to_string()));
                }
            }
            #[allow(clippy::redundant_closure)]
            setting_counts
                .entry(setting_type)
                .or_insert_with(|| HashSet::default())
                .insert(setting_id);
            // TODO: If we are changing a person's itinerary, the person_id should be removed from vector
            // This isn't the same as the concept of being present or not.
            #[allow(clippy::redundant_closure)]
            container
                .members
                .entry(itinerary_entry.setting_type)
                .or_insert_with(|| HashMap::default())
                .entry(setting_id)
                .or_insert_with(|| Vec::new())
                .push(person_id);
        }
        container.itineraries.insert(person_id, itinerary);
        Ok(())
    }

    fn get_setting_members<T: SettingType + 'static>(
        &self,
        setting_id: SettingId<T>,
    ) -> Option<&Vec<PersonId>> {
        self.get_data_container(SettingDataPlugin)?
            .get_setting_members(&TypeId::of::<T>(), setting_id.id)
    }

    fn calculate_total_infectiousness_multiplier_for_person(&self, person_id: PersonId) -> f64 {
        let container = self.get_data_container(SettingDataPlugin).unwrap();
        let mut collector = 0.0;
        container.with_itinerary(person_id, |setting_type, setting_props, members, ratio| {
            let multiplier = setting_type.calculate_multiplier(members, *setting_props);
            collector += ratio * multiplier;
        });
        collector
    }

    // Perhaps setting ids should include type and id so that one can have a vector of setting ids
    fn get_itinerary(&self, person_id: PersonId) -> Option<&Vec<ItineraryEntry>> {
        self.get_data_container(SettingDataPlugin)
            .expect("Person should be added to settings")
            .itineraries
            .get(&person_id)
    }

    fn get_contact<T: SettingType + 'static>(
        &self,
        person_id: PersonId,
        setting_id: SettingId<T>,
    ) -> Option<PersonId> {
        if let Some(members) = self.get_setting_members::<T>(setting_id) {
            if members.len() == 1 {
                return None;
            }
            let mut contact_id = person_id;
            while contact_id == person_id {
                contact_id = members[self.sample_range(SettingsRng, 0..members.len())];
            }
            Some(contact_id)
        } else {
            None
        }
    }
    fn draw_contact_from_itinerary(&self, person_id: PersonId) -> Option<PersonId> {
        let container = self.get_data_container(SettingDataPlugin).unwrap();
        let mut itinerary_multiplier = Vec::new();
        container.with_itinerary(person_id, |setting_type, setting_props, members, ratio| {
            let multiplier = setting_type.calculate_multiplier(members, *setting_props);
            itinerary_multiplier.push(ratio * multiplier);
        });

        let setting_index = self.sample_weighted(SettingsRng, &itinerary_multiplier);

        if let Some(itinerary) = self.get_itinerary(person_id) {
            let itinerary_entry = &itinerary[setting_index];
            self.get_contact_internal(
                person_id,
                itinerary_entry.setting_type,
                itinerary_entry.setting_id,
            )
        } else {
            None
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::assert_almost_eq;
    use crate::settings::ContextSettingExt;
    use crate::ContextPeopleExt;
    #[test]
    fn test_setting_type_creation() {
        let mut context = Context::new();
        context.register_setting_type(Home {}, SettingProperties { alpha: 0.1 });
        context.register_setting_type(CensusTract {}, SettingProperties { alpha: 0.001 });
        let home_props = context.get_setting_properties::<Home>();
        let tract_props = context.get_setting_properties::<CensusTract>();

        assert_almost_eq!(0.1, home_props.alpha, 0.0);
        assert_almost_eq!(0.001, tract_props.alpha, 0.0);
    }

    #[test]
    fn test_duplicated_itinerary() {
        let mut context = Context::new();
        context.register_setting_type(Home {}, SettingProperties { alpha: 1.0 });
        context.register_setting_type(CensusTract {}, SettingProperties { alpha: 1.0 });

        // Different homes is ok. (Different person from person1 with same home is ok.)
        let person0 = context.add_person(()).unwrap();
        let itinerary0 = vec![
            ItineraryEntry::new(&SettingId::<Home>::new(2), 0.5),
            ItineraryEntry::new(&SettingId::<Home>::new(3), 0.5),
        ];

        // Same id but different setting type is ok.
        let person1 = context.add_person(()).unwrap();
        let itinerary1 = vec![
            ItineraryEntry::new(&SettingId::<Home>::new(3), 0.5),
            ItineraryEntry::new(&SettingId::<CensusTract>::new(3), 0.5),
        ];

        // Same ID and same setting type should be an error.
        let person2 = context.add_person(()).unwrap();
        let itinerary2 = vec![
            ItineraryEntry::new(&SettingId::<Home>::new(2), 0.25),
            ItineraryEntry::new(&SettingId::<Home>::new(2), 0.75),
        ];

        context
            .add_itinerary(person0, itinerary0)
            .expect("Failed to add itinerary");

        context
            .add_itinerary(person1, itinerary1)
            .expect("Failed to add itinerary");

        match context.add_itinerary(person2, itinerary2) {
            Err(IxaError::IxaError(msg)) => {
                assert_eq!(msg, "Duplicated setting");
            }
            Err(e) => panic!("Unexpected error in itinerary: {}", e),
            Ok(_) => panic!("itinerary should be error"),
        }
    }

    #[test]
    fn test_add_itinerary() {
        let mut context = Context::new();
        context.register_setting_type(Home {}, SettingProperties { alpha: 1.0 });

        let person = context.add_person(()).unwrap();
        let itinerary = vec![
            ItineraryEntry::new(&SettingId::<Home>::new(1), 0.5),
            ItineraryEntry::new(&SettingId::<Home>::new(2), 0.5),
        ];
        let _ = context.add_itinerary(person, itinerary);
        let members = context
            .get_setting_members::<Home>(SettingId::<Home>::new(2))
            .unwrap();
        assert_eq!(members.len(), 1);

        let person2 = context.add_person(()).unwrap();
        let itinerary2 = vec![ItineraryEntry::new(&SettingId::<Home>::new(2), 1.0)];
        let _ = context.add_itinerary(person2, itinerary2);

        let members2 = context
            .get_setting_members::<Home>(SettingId::<Home>::new(2))
            .unwrap();
        assert_eq!(members2.len(), 2);
    }

    #[test]
    fn test_setting_registration() {
        let mut context = Context::new();
        context.register_setting_type(Home {}, SettingProperties { alpha: 0.1 });
        context.register_setting_type(CensusTract {}, SettingProperties { alpha: 0.01 });
        for s in 0..5 {
            // Create 5 people
            for _ in 0..5 {
                let person = context.add_person(()).unwrap();
                let itinerary = vec![
                    ItineraryEntry::new(&SettingId::<Home>::new(s), 0.5),
                    ItineraryEntry::new(&SettingId::<CensusTract>::new(s), 0.5),
                ];
                let _ = context.add_itinerary(person, itinerary);
            }
            let members = context
                .get_setting_members::<Home>(SettingId::<Home>::new(s))
                .unwrap();
            let tract_members = context
                .get_setting_members::<CensusTract>(SettingId::<CensusTract>::new(s))
                .unwrap();
            // Get the number of people for these settings and should be 5
            assert_eq!(members.len(), 5);
            assert_eq!(tract_members.len(), 5);
        }
    }

    #[test]
    fn test_setting_multiplier() {
        // TODO: if setting not registered, shouldn't be able to register people to setting
        let mut context = Context::new();
        context.register_setting_type(Home {}, SettingProperties { alpha: 0.1 });
        for s in 0..5 {
            // Create 5 people
            for _ in 0..5 {
                let person = context.add_person(()).unwrap();
                let itinerary = vec![ItineraryEntry::new(&SettingId::<Home>::new(s), 0.5)];
                let _ = context.add_itinerary(person, itinerary);
            }
        }

        let home_id = 0;
        let person = context.add_person(()).unwrap();
        let itinerary = vec![ItineraryEntry::new(&SettingId::<Home>::new(home_id), 0.5)];
        let _ = context.add_itinerary(person, itinerary);
        let members = context
            .get_setting_members::<Home>(SettingId::<Home>::new(home_id))
            .unwrap();

        let setting_type = Home {};

        let inf_multiplier =
            setting_type.calculate_multiplier(members, SettingProperties { alpha: 0.1 });

        // This is assuming we know what the function for Home is (N - 1) ^ alpha
        assert_almost_eq!(inf_multiplier, f64::from(6 - 1).powf(0.1), 0.0);
    }

    #[test]
    fn test_total_infectiousness_multiplier() {
        // Go through all the settings and compute infectiousness multiplier
        let mut context = Context::new();
        context.register_setting_type(Home {}, SettingProperties { alpha: 0.1 });
        context.register_setting_type(CensusTract {}, SettingProperties { alpha: 0.01 });

        for s in 0..5 {
            for _ in 0..5 {
                let person = context.add_person(()).unwrap();
                let itinerary = vec![
                    ItineraryEntry::new(&SettingId::<Home>::new(s), 0.5),
                    ItineraryEntry::new(&SettingId::<CensusTract>::new(s), 0.5),
                ];
                let _ = context.add_itinerary(person, itinerary);
            }
        }
        // Create a new person and register to home 0
        let itinerary = vec![ItineraryEntry::new(&SettingId::<Home>::new(0), 1.0)];
        let person = context.add_person(()).unwrap();
        let _ = context.add_itinerary(person, itinerary);

        // If only registered at home, total infectiousness multiplier should be (6 - 1) ^ (alpha)
        let inf_multiplier = context.calculate_total_infectiousness_multiplier_for_person(person);
        assert_almost_eq!(inf_multiplier, f64::from(6 - 1).powf(0.1), 0.0);

        // If person's itinerary is changed for two settings,
        // CensusTract 0 should have 6 members, Home 0 should have 7 members
        // the total infectiousness should be the sum of infs * proportion
        let person = context.add_person(()).unwrap();
        let itinerary_complete = vec![
            ItineraryEntry::new(&SettingId::<Home>::new(0), 0.5),
            ItineraryEntry::new(&SettingId::<CensusTract>::new(0), 0.5),
        ];
        let _ = context.add_itinerary(person, itinerary_complete);
        let members_home = context
            .get_setting_members::<Home>(SettingId::<Home>::new(0))
            .unwrap();
        let members_tract = context
            .get_setting_members::<CensusTract>(SettingId::<CensusTract>::new(0))
            .unwrap();
        assert_eq!(members_home.len(), 7);
        assert_eq!(members_tract.len(), 6);

        let inf_multiplier_two_settings =
            context.calculate_total_infectiousness_multiplier_for_person(person);

        assert_almost_eq!(
            inf_multiplier_two_settings,
            (f64::from(7 - 1).powf(0.1)) * 0.5 + (f64::from(6 - 1).powf(0.01)) * 0.5,
            0.0
        );
    }

    #[test]
    fn test_get_contact_from_setting() {
        // Register two people to a setting and make sure that the person chosen is the other one
        // Attempt to draw a contact from a setting with only the person trying to get a contact
        // TODO: What happens if the person isn't registered in the setting?
        let mut context = Context::new();
        context.init_random(42);
        context.register_setting_type(Home {}, SettingProperties { alpha: 0.1 });
        context.register_setting_type(CensusTract {}, SettingProperties { alpha: 0.01 });

        let person_a = context.add_person(()).unwrap();
        let person_b = context.add_person(()).unwrap();
        let itinerary_a = vec![
            ItineraryEntry::new(&SettingId::<Home>::new(0), 0.5),
            ItineraryEntry::new(&SettingId::<CensusTract>::new(0), 0.5),
        ];
        let itinerary_b = vec![ItineraryEntry::new(&SettingId::<Home>::new(0), 1.0)];
        let _ = context.add_itinerary(person_a, itinerary_a);
        let _ = context.add_itinerary(person_b, itinerary_b);

        assert_eq!(
            person_b,
            context
                .get_contact::<Home>(person_a, SettingId::<Home>::new(0))
                .unwrap()
        );
        assert!(context
            .get_contact::<CensusTract>(person_a, SettingId::<CensusTract>::new(0))
            .is_none());
    }

    #[test]
    fn test_draw_contact_from_itinerary() {
        /*
        Run 100 times
        - Create 3 people at home, and 3 people at censustract
        - Create 7th person with itinerary at home and census tract
        - Call "draw contact from itinerary":
          + Compute total infectiousness
          + Draw a setting weighted by total infectiousness
          + Sample contact from chosen setting
         - Test 1 Itinerary with 0 proportion at census tract, contacts drawn should be from home (0-2)
         - Test 2 Itinerary with 0 proportion at home, contacts should be drawn from census tract (3-6)
        */
        for seed in 0..100 {
            let mut context = Context::new();
            context.init_random(seed);
            context.register_setting_type(Home {}, SettingProperties { alpha: 0.1 });
            context.register_setting_type(CensusTract {}, SettingProperties { alpha: 0.01 });

            for _ in 0..3 {
                let person = context.add_person(()).unwrap();
                let itinerary = vec![ItineraryEntry::new(&SettingId::<Home>::new(0), 1.0)];
                let _ = context.add_itinerary(person, itinerary);
            }

            for _ in 0..3 {
                let person = context.add_person(()).unwrap();
                let itinerary = vec![ItineraryEntry::new(&SettingId::<CensusTract>::new(0), 1.0)];
                let _ = context.add_itinerary(person, itinerary);
            }

            let person = context.add_person(()).unwrap();
            let itinerary_home = vec![
                ItineraryEntry::new(&SettingId::<Home>::new(0), 1.0),
                ItineraryEntry::new(&SettingId::<CensusTract>::new(0), 0.0),
            ];
            let itinerary_censustract = vec![
                ItineraryEntry::new(&SettingId::<Home>::new(0), 0.0),
                ItineraryEntry::new(&SettingId::<CensusTract>::new(0), 1.0),
            ];
            let home_members = context
                .get_setting_members::<Home>(SettingId::<Home>::new(0))
                .unwrap()
                .clone();
            let tract_members = context
                .get_setting_members::<CensusTract>(SettingId::<CensusTract>::new(0))
                .unwrap()
                .clone();

            let _ = context.add_itinerary(person, itinerary_home);
            let contact_id_home = context.draw_contact_from_itinerary(person);
            assert!(home_members.contains(&contact_id_home.unwrap()));

            let _ = context.add_itinerary(person, itinerary_censustract);
            let contact_id_tract = context.draw_contact_from_itinerary(person);
            assert!(tract_members.contains(&contact_id_tract.unwrap()));
        }
    }
    /*TODO:
    Test failure of getting properties if not initialized
    Test failure if a setting is registered more than once?
    Test that proportions either add to 1 or that they are weighted based on proportion
    */
}
