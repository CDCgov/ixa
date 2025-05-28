use crate::settings::itinerary::Itinerary;
use crate::settings::{SettingDataPlugin, SettingId, SettingsRng};
use crate::{Context, ContextRandomExt, PersonId};
use ixa_fips::SettingCategoryCode;

#[allow(dead_code)]
pub trait ContextSettingExt {
    /// Associates an alpha value to a `SettingCategoryCode`. If a value of alpha was already set for
    /// the given category, returns the previous value.
    fn set_alpha_for_setting_category(
        &mut self,
        setting_category: SettingCategoryCode,
        alpha: f64,
    ) -> Option<f64>;

    /// Corresponding getter to `set_alpha_for_setting_category()`.
    fn get_alpha_for_setting_category(
        &mut self,
        setting_category: SettingCategoryCode,
    ) -> Option<f64>;

    /// Sets an `Itinerary` for the given person, inserting the person as a member of the settings
    /// in the given `Itinerary`. Returns the old `Itinerary` if the method replaced an existing
    /// itinerary (i.e. an itinerary was already set for this person), `None` otherwise.
    fn set_itinerary_for_person(
        &mut self,
        person_id: PersonId,
        itinerary: Itinerary,
    ) -> Option<Itinerary>;

    /// Corresponding getter to `set_itinerary_for_person()`.
    fn get_itinerary_for_person(&mut self, person_id: PersonId) -> Option<&Itinerary>;

    /// For the given person, computes the inner product $<R, M>$ where $R$ is the vector of ratios
    /// for each setting and $M$ is the vector of multipliers for each setting.
    ///
    /// Recall that the "multiplier" for a setting is computed as
    ///     `((n_members - 1) as f64).powf(alpha)`.
    fn calculate_total_infectiousness_multiplier_for_person(&self, person_id: PersonId) -> f64;

    /// For a given person, use the person's itinerary and associated setting properties
    /// to sample a setting and a contact from that setting. If the person has no
    /// itinerary, or if the person is isolated (alone) in the setting, returns `None`.
    fn draw_contact_from_itinerary(&self, person_id: PersonId) -> Option<(PersonId, SettingId)>;
}

impl ContextSettingExt for Context {
    fn set_alpha_for_setting_category(
        &mut self,
        setting_category: SettingCategoryCode,
        alpha: f64,
    ) -> Option<f64> {
        let container = self.get_data_container_mut(SettingDataPlugin);
        container
            .alpha_for_setting_category
            .insert(setting_category, alpha)
    }

    fn get_alpha_for_setting_category(
        &mut self,
        setting_category: SettingCategoryCode,
    ) -> Option<f64> {
        let container = self.get_data_container_mut(SettingDataPlugin);
        container
            .alpha_for_setting_category
            .get(&setting_category)
            .copied()
    }

    fn set_itinerary_for_person(
        &mut self,
        person_id: PersonId,
        itinerary: Itinerary,
    ) -> Option<Itinerary> {
        let container = self.get_data_container_mut(SettingDataPlugin);
        container.add_itinerary_for_person(person_id, itinerary)
    }

    fn get_itinerary_for_person(&mut self, person_id: PersonId) -> Option<&Itinerary> {
        let container = self.get_data_container_mut(SettingDataPlugin);
        container.itineraries.get(&person_id)
    }

    fn calculate_total_infectiousness_multiplier_for_person(&self, person_id: PersonId) -> f64 {
        let container = self.get_data_container(SettingDataPlugin).unwrap();
        // ToDo(ap59): What should happen if the person doesn't have an itinerary?
        match container.calculate_infectiousness_multiplier_vector_for_person(person_id) {
            Some(v) => v.iter().sum(),
            None => 0.0,
        }
    }

    fn draw_contact_from_itinerary(&self, person_id: PersonId) -> Option<(PersonId, SettingId)> {
        let container = self.get_data_container(SettingDataPlugin).unwrap();
        self.sample(SettingsRng, |rng| {
            let setting_id = container.draw_setting_from_itinerary(person_id, rng)?;
            let contact_id = container.draw_contact_from_itinerary(person_id, setting_id, rng)?;
            Some((contact_id, setting_id))
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::settings::{Itinerary, ItineraryEntry};
    use crate::{assert_almost_eq, Context, ContextPeopleExt, ContextRandomExt};
    use ixa_fips::{FIPSCode, SettingCategoryCode, USState};

    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    #[repr(u8)]
    #[allow(dead_code)]
    enum SettingCategory {
        Unspecified = 0,
        Home,
        Workplace,
        PublicSchool,
        PrivateSchool,
        CensusTract,
    }

    impl From<SettingCategory> for SettingCategoryCode {
        fn from(value: SettingCategory) -> Self {
            value as SettingCategoryCode
        }
    }

    #[test]
    fn test_setting_type_creation() {
        let mut context = Context::new();
        // Set new values
        assert!(context
            .set_alpha_for_setting_category(SettingCategory::Home.into(), 0.1)
            .is_none());
        assert!(context
            .set_alpha_for_setting_category(SettingCategory::CensusTract.into(), 0.001)
            .is_none());

        // Assert that:
        //   1) the old value is returned when a new value is set
        //   2) the original value was inserted correctly to begin with
        assert_almost_eq!(
            context
                .set_alpha_for_setting_category(SettingCategory::Home.into(), 0.9)
                .unwrap(),
            0.1,
            0.0
        );
        assert_almost_eq!(
            context
                .set_alpha_for_setting_category(SettingCategory::CensusTract.into(), 0.5)
                .unwrap(),
            0.001,
            0.0
        );
    }

    #[test]
    fn test_setting_multiplier() {
        // TODO: if setting not registered, shouldn't be able to register people to setting
        let mut context = Context::new();
        context.init_random(42);
        context.set_alpha_for_setting_category(SettingCategory::Home.into(), 0.1);

        let setting_prototype =
            FIPSCode::with_category(USState::AK.into(), 0, 0, SettingCategory::Home.into())
                .unwrap();

        for s in 0..5 {
            let setting = setting_prototype.set_id(s).unwrap();
            let itinerary_prototype =
                Itinerary::from_vec(vec![ItineraryEntry::new(setting, 0.5)]).unwrap();
            // Create 5 people
            for _ in 0..5 {
                let person = context.add_person(()).unwrap();
                let _ = context.set_itinerary_for_person(person, itinerary_prototype.clone());
            }
        }

        // Has id = 0.
        let itinerary =
            Itinerary::from_vec(vec![ItineraryEntry::new(setting_prototype, 0.5)]).unwrap();
        let person = context.add_person(()).unwrap();
        let _ = context.set_itinerary_for_person(person, itinerary);

        let inf_multiplier = context.calculate_total_infectiousness_multiplier_for_person(person);

        // This is assuming we know what the function for Home is (N - 1) ^ alpha
        assert_almost_eq!(inf_multiplier, 0.5 * f64::from(6 - 1).powf(0.1), 0.0);
    }

    #[test]
    fn test_total_infectiousness_multiplier() {
        // Go through all the settings and compute infectiousness multiplier
        let mut context = Context::new();
        context.set_alpha_for_setting_category(SettingCategory::Home.into(), 0.1);
        context.set_alpha_for_setting_category(SettingCategory::CensusTract.into(), 0.01);

        let home_prototype =
            FIPSCode::with_category(USState::AK.into(), 0, 0, SettingCategory::Home.into())
                .unwrap();
        let tract_prototype = FIPSCode::with_category(
            USState::AK.into(),
            0,
            0,
            SettingCategory::CensusTract.into(),
        )
        .unwrap();

        // Create 5 homes and census tracts with 5 people each.
        for s in 0..5 {
            let itinerary_prototype = Itinerary::from_vec(vec![
                ItineraryEntry::new(home_prototype.set_id(s).unwrap(), 0.5),
                ItineraryEntry::new(tract_prototype.set_id(s).unwrap(), 0.5),
            ])
            .unwrap();
            for _ in 0..5 {
                let person = context.add_person(()).unwrap();
                let _ = context.set_itinerary_for_person(person, itinerary_prototype.clone());
            }
        }

        // Create a new person and register to home 0
        let itinerary = Itinerary::from_vec(vec![ItineraryEntry::new(
            home_prototype.set_id(0).unwrap(),
            1.0,
        )])
        .unwrap();
        let person = context.add_person(()).unwrap();
        let _ = context.set_itinerary_for_person(person, itinerary);

        // If only registered at home, total infectiousness multiplier should be (6 - 1) ^ (alpha)
        let inf_multiplier = context.calculate_total_infectiousness_multiplier_for_person(person);
        assert_almost_eq!(inf_multiplier, f64::from(6 - 1).powf(0.1), 0.0);

        // If person's itinerary is changed for two settings,
        // CensusTract 0 should have 6 members, Home 0 should have 7 members
        // the total infectiousness should be the sum of infs * proportion
        let person = context.add_person(()).unwrap();
        let itinerary_complete = Itinerary::from_vec(vec![
            ItineraryEntry::new(home_prototype.set_id(0).unwrap(), 0.5),
            ItineraryEntry::new(tract_prototype.set_id(0).unwrap(), 0.5),
        ])
        .unwrap();

        let _ = context.set_itinerary_for_person(person, itinerary_complete);
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
        // Register two people to a setting and make sure that the person chosen is the other one.
        // Attempt to draw a contact from a setting with only the person trying to get a contact
        let mut context = Context::new();
        context.init_random(42);
        context.set_alpha_for_setting_category(SettingCategory::Home.into(), 0.1);
        context.set_alpha_for_setting_category(SettingCategory::CensusTract.into(), 0.01);

        let person_a = context.add_person(()).unwrap();
        let person_b = context.add_person(()).unwrap();

        let home_prototype =
            FIPSCode::with_category(USState::AK.into(), 0, 0, SettingCategory::Home.into())
                .unwrap();
        let tract_prototype = FIPSCode::with_category(
            USState::AK.into(),
            0,
            0,
            SettingCategory::CensusTract.into(),
        )
        .unwrap();

        let itinerary_a = Itinerary::from_vec(vec![
            ItineraryEntry::new(home_prototype, 0.5),
            ItineraryEntry::new(tract_prototype, 0.5),
        ])
        .unwrap();

        let itinerary_b =
            Itinerary::from_vec(vec![ItineraryEntry::new(home_prototype, 1.0)]).unwrap();

        let _ = context.set_itinerary_for_person(person_a, itinerary_a);
        let _ = context.set_itinerary_for_person(person_b, itinerary_b);
        let (contact_a, _) = context.draw_contact_from_itinerary(person_a).unwrap();
        let (contact_b, _) = context.draw_contact_from_itinerary(person_b).unwrap();

        assert_eq!(person_b, contact_a);
        assert_eq!(person_a, contact_b);
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
        // We keep track of the people at home and at census tract
        let mut people_at_home = vec![];
        let mut people_at_tract = vec![];

        for seed in 0..100 {
            let mut context = Context::new();
            context.init_random(seed);
            context.set_alpha_for_setting_category(SettingCategory::Home.into(), 0.1);
            context.set_alpha_for_setting_category(SettingCategory::CensusTract.into(), 0.01);

            let home_prototype =
                FIPSCode::with_category(USState::AK.into(), 0, 0, SettingCategory::Home.into())
                    .unwrap();
            let tract_prototype = FIPSCode::with_category(
                USState::AK.into(),
                0,
                0,
                SettingCategory::CensusTract.into(),
            )
            .unwrap();

            {
                let itinerary_home =
                    Itinerary::from_vec(vec![ItineraryEntry::new(home_prototype, 1.0)]).unwrap();
                for _ in 0..3 {
                    let person = context.add_person(()).unwrap();
                    people_at_home.push(person);
                    let _ = context.set_itinerary_for_person(person, itinerary_home.clone());
                }
            }

            {
                let itinerary_tract =
                    Itinerary::from_vec(vec![ItineraryEntry::new(tract_prototype, 1.0)]).unwrap();
                for _ in 0..3 {
                    let person = context.add_person(()).unwrap();
                    people_at_tract.push(person);
                    let _ = context.set_itinerary_for_person(person, itinerary_tract.clone());
                }
            }

            // The 7th person whose contact we shall draw
            let person = context.add_person(()).unwrap();

            // An itinerary with proportion 1 at home and 0 at census tract
            let itinerary_home = Itinerary::from_vec(vec![
                ItineraryEntry::new(home_prototype, 1.0),
                ItineraryEntry::new(tract_prototype, 0.0),
            ])
            .unwrap();
            // An itinerary with proportion 0 at home and 1 at census tract
            let itinerary_tract = Itinerary::from_vec(vec![
                ItineraryEntry::new(home_prototype, 0.0),
                ItineraryEntry::new(tract_prototype, 1.0),
            ])
            .unwrap();

            // First draw a contact from the itinerary with 1 at home and 0 at census tract
            let _ = context.set_itinerary_for_person(person, itinerary_home);
            let (contact_id_home, _) = context.draw_contact_from_itinerary(person).unwrap();
            assert!(people_at_home.contains(&contact_id_home));

            // Now draw a contact from the itinerary with 0 at home and 1 at census tract
            let _ = context.set_itinerary_for_person(person, itinerary_tract);
            let (contact_id_tract, _) = context.draw_contact_from_itinerary(person).unwrap();
            assert!(people_at_tract.contains(&contact_id_tract));
        }
    }

    #[test]
    #[should_panic(expected = "setting category 1 was not assigned an alpha value")]
    fn test_failure_if_alpha_not_initialized() {
        let mut context = Context::new();
        context.init_random(42);
        // Commenting out this line should result in a panic.
        // context.set_alpha_for_setting_category(SettingCategory::Home.into(), 0.1);

        let person = context.add_person(()).unwrap();

        let home = FIPSCode::with_category(USState::AK.into(), 0, 0, SettingCategory::Home.into())
            .unwrap();
        let tract = FIPSCode::with_category(
            USState::AK.into(),
            0,
            0,
            SettingCategory::CensusTract.into(),
        )
        .unwrap();

        let itinerary_a = Itinerary::from_vec(vec![
            ItineraryEntry::new(home, 0.5),
            ItineraryEntry::new(tract, 0.5),
        ])
        .unwrap();

        let _ = context.set_itinerary_for_person(person, itinerary_a);

        // Should panic
        let _contact = context.draw_contact_from_itinerary(person);
    }
}
