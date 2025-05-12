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
use crate::{define_data_plugin, define_rng, Context, ContextRandomExt, HashMap, IxaError};
use ixa_fips::aspr::SettingCategory;
use ixa_fips::FIPSCode;
use rand::distributions::Distribution;
use rand::distributions::WeightedIndex;
use rand::Rng;
use std::collections::hash_map::Entry;
use std::ops::Index;

define_rng!(SettingsRng);

pub type SettingId = FIPSCode;

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct ItineraryEntry {
    setting_id: SettingId,
    ratio: f64,
}

#[allow(dead_code)]
impl ItineraryEntry {
    fn new(setting_id: SettingId, ratio: f64) -> ItineraryEntry {
        ItineraryEntry { setting_id, ratio }
    }
}

/// A convenience wrapper for a vector of `ItineraryEntry`s that enforces the constraint that there
/// is at most one instance of any given `SettingCategory` represented in the `Itinerary`.
// ToDo(ap59): This should be a small_vec or equivalent, as many of these will have a single entry.
#[derive(Debug, Clone, Default)]
pub struct Itinerary(Vec<ItineraryEntry>);

impl Itinerary {
    /// Creates an empty `Itinerary`.
    #[must_use]
    pub fn new() -> Itinerary {
        Itinerary::default()
    }

    /// Creates an `Itinerary` from a vector of `ItineraryEntry`'s. Returns an `IxaError` if there
    /// are two `ItineraryEntry`s with the same `SettingId`.
    pub fn from_vec(entries: Vec<ItineraryEntry>) -> Result<Self, IxaError> {
        let mut itinerary = Itinerary::default();

        for entry in entries {
            itinerary.add(entry.setting_id, entry.ratio)?;
        }

        Ok(itinerary)
    }

    /// Convenience wrapper over `add_itinerary_entry` that creates the `ItineraryEntry` for you.
    pub fn add(&mut self, setting_id: SettingId, ratio: f64) -> Result<(), IxaError> {
        self.add_itinerary_entry(ItineraryEntry::new(setting_id, ratio))
    }

    /// Adds the `ItineraryEntry` to the `Itinerary`. Returns an `IxaError` if an `ItineraryEntry`
    /// with the same setting already exists in the `Itinerary`.
    pub fn add_itinerary_entry(&mut self, entry: ItineraryEntry) -> Result<(), IxaError> {
        // Check if there is already an entry for this setting
        if self.0.iter().any(
            // ToDo(ap59): Should the "data" field be included in the comparison?
            |other| other.setting_id.compare_non_data(entry.setting_id).is_eq(),
        ) {
            return Err(IxaError::IxaError(format!(
                "Duplicated setting in itinerary when adding setting_id {:?}",
                entry.setting_id
            )));
        }

        self.0.push(entry);
        Ok(())
    }

    /// Returns an iterator over the `ItineraryEntry`s in this `Itinerary`.
    pub fn iter(&self) -> std::slice::Iter<ItineraryEntry> {
        self.0.iter()
    }
}

impl<'this> IntoIterator for &'this Itinerary {
    type Item = &'this ItineraryEntry;
    type IntoIter = std::slice::Iter<'this, ItineraryEntry>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl Index<usize> for Itinerary {
    type Output = ItineraryEntry;
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

#[derive(Default)]
pub struct SettingsDataContainer {
    /// Each `SettingCategory` has an alpha of type `f64`
    ///
    /// In a setting with `n` people (including the source of infection), the rate of the total
    /// infectiousness process is computed as
    ///      (intrinsic infectiousness) × (n - 1)ᵅ
    /// where 0 ≤ 𝛼 ≤ 1. This interpolates between having the total hazard _distributed_ equally and
    /// the total hazard applying equally to the nonsources.
    alpha_for_setting_category: HashMap<SettingCategory, f64>,
    /// Each `PersonId` has an `Itinerary` of `SettingId`s
    itineraries: HashMap<PersonId, Itinerary>,
    /// Each `SettingId` has a list of members
    members: HashMap<SettingId, Vec<PersonId>>,
}

impl SettingsDataContainer {
    fn new() -> Self {
        SettingsDataContainer::default()
    }

    /// Adds an `Itinerary` for the given person, inserting the person as a member of the settings
    /// in the given `Itinerary`. Returns the old `Itinerary` if the method replaced an existing
    /// itinerary (i.e. an itinerary was already set for this person), `None` otherwise.
    fn add_itinerary_for_person(
        &mut self,
        person_id: PersonId,
        itinerary: Itinerary,
    ) -> Option<Itinerary> {
        match self.itineraries.entry(person_id) {
            Entry::Vacant(vacant_entry) => {
                // An itinerary was not previously set for this person.
                let new_it = vacant_entry.insert(itinerary);
                // Add the person to each setting in the *new* itinerary:
                for entry in new_it.iter() {
                    let setting = entry.setting_id;
                    self.members
                        .entry(setting)
                        .and_modify(|members| members.push(person_id))
                        .or_insert_with(|| vec![person_id]);
                }
                None
            }
            Entry::Occupied(mut occupied_entry) => {
                // Replace the old itinerary, taking ownership of it
                let old_it = occupied_entry.insert(itinerary);
                // Remove the person from each setting in the old itinerary
                for entry in &old_it {
                    let setting = entry.setting_id;
                    self.members.entry(setting).and_modify(|members| {
                        if let Some(idx) = members.iter().position(|&pid| pid == person_id) {
                            members.swap_remove(idx);
                        }
                    });
                }
                // Now add the person to each setting in the new itinerary
                let new_it = occupied_entry.get();
                for entry in new_it {
                    let setting = entry.setting_id;
                    self.members
                        .entry(setting)
                        .and_modify(|members| members.push(person_id))
                        .or_insert_with(|| vec![person_id]);
                }
                Some(old_it)
            }
        }
    }

    /// Looks up the itinerary for the given person and for each of its `ItineraryEntry`s calls
    /// `callback` with
    ///     - the `ItineraryEntry` (contains `SettingId` and `ratio`)
    ///     - alpha - the alpha value for the `SettingCategory` associated to the `SettingId` in
    ///       the `ItineraryEntry`
    ///     - `member_count`: the length of the list of members of the setting
    ///
    /// If there is no itinerary for the person, this method is a no-op.
    fn with_itinerary<F>(&self, person_id: PersonId, mut callback: F)
    where
        // f(entry: ItineraryEntry, alpha_for_setting: f64, member_count: usize)
        F: FnMut(ItineraryEntry, f64, usize),
    {
        if let Some(itinerary) = self.itineraries.get(&person_id) {
            for entry in itinerary {
                let alpha = match self
                    .alpha_for_setting_category
                    .get(&entry.setting_id.category())
                {
                    Some(alpha) => *alpha,
                    None => {
                        panic!(
                            "setting category {} was not assigned an alpha value",
                            entry.setting_id.category()
                        );
                    }
                };

                // Unwrap guaranteed to succeed since `itinerary` above succeeded.
                let members = self.members.get(&entry.setting_id).unwrap();
                callback(*entry, alpha, members.len());
            }
        }
    }

    /// For a given person, compute the element-wise product `R ⊗ M` where `R` is the vector of
    /// ratios for each setting and `M` is the vector of multipliers for each setting.
    pub(crate) fn calculate_infectiousness_multiplier_vector_for_person(
        &self,
        person_id: PersonId,
    ) -> Option<Vec<f64>> {
        let mut multiplier_vector = vec![];
        self.with_itinerary(person_id, |entry, alpha, member_count| {
            #[allow(clippy::cast_precision_loss)]
            let multiplier = ((member_count - 1) as f64).powf(alpha);
            multiplier_vector.push(entry.ratio * multiplier);
        });

        if multiplier_vector.is_empty() {
            None
        } else {
            Some(multiplier_vector)
        }
    }

    /// For a given person, use the person's itinerary and associated setting properties to
    /// sample a contact from one of the person's settings.
    pub(crate) fn draw_contact_from_itinerary<R: Rng + ?Sized>(
        &self,
        person_id: PersonId,
        rng: &mut R,
    ) -> Option<PersonId> {
        // Compute the element-wise product `R ⊗ M` where `R` is the vector of ratios for each
        // setting and `M` is the vector of multipliers for each setting.
        let itinerary_multiplier =
            self.calculate_infectiousness_multiplier_vector_for_person(person_id)?;

        // Use the resulting vector as weights to sample a setting index.
        let index = WeightedIndex::new(&itinerary_multiplier).unwrap();
        let setting_index = index.sample(rng);

        // Unwrap guaranteed to succeed since `itinerary_multiplier` succeeded.
        let itinerary = self.itineraries.get(&person_id).unwrap();
        let itinerary_entry = itinerary[setting_index];

        // Unwrap guaranteed to succeed since `itinerary_multiplier` succeeded.
        let members = self.members.get(&itinerary_entry.setting_id).unwrap();
        if members.len() == 1 {
            // The person is isolated alone in this setting; there is no other contact.
            return None;
        }

        // Sample a contact from the setting different from `person_id`.
        let mut contact_id = person_id;
        while contact_id == person_id {
            contact_id = members[rng.gen_range(0..members.len())];
        }
        Some(contact_id)
    }
}

define_data_plugin!(
    SettingDataPlugin,
    SettingsDataContainer,
    SettingsDataContainer::new()
);

#[allow(dead_code)]
pub trait ContextSettingExt {
    /// Associates an alpha value to a `SettingCategory`. If a value of alpha was already set for
    /// the given category, returns the previous value.
    fn set_alpha_for_setting_category(
        &mut self,
        setting_category: SettingCategory,
        alpha: f64,
    ) -> Option<f64>;

    /// Adds an `Itinerary` for the given person, inserting the person as a member of the settings
    /// in the given `Itinerary`. Returns the old `Itinerary` if the method replaced an existing
    /// itinerary (i.e. an itinerary was already set for this person), `None` otherwise.
    fn add_itinerary_for_person(
        &mut self,
        person_id: PersonId,
        itinerary: Itinerary,
    ) -> Option<Itinerary>;

    /// For the given person, computes the inner product $<R, M>$ where $R$ is the vector of ratios
    /// for each setting and $M$ is the vector of multipliers for each setting.
    ///
    /// Recall that the "multiplier" for a setting is computed as
    ///     `((n_members - 1) as f64).powf(alpha)`.
    fn calculate_total_infectiousness_multiplier_for_person(&self, person_id: PersonId) -> f64;

    /// For a given person, use the person's itinerary and associated setting properties to
    /// sample a contact from one of the person's settings.
    fn draw_contact_from_itinerary(&self, person_id: PersonId) -> Option<PersonId>;
}

impl ContextSettingExt for Context {
    fn set_alpha_for_setting_category(
        &mut self,
        setting_category: SettingCategory,
        alpha: f64,
    ) -> Option<f64> {
        let container = self.get_data_container_mut(SettingDataPlugin);
        container
            .alpha_for_setting_category
            .insert(setting_category, alpha)
    }

    fn add_itinerary_for_person(
        &mut self,
        person_id: PersonId,
        itinerary: Itinerary,
    ) -> Option<Itinerary> {
        let container = self.get_data_container_mut(SettingDataPlugin);
        container.add_itinerary_for_person(person_id, itinerary)
    }

    fn calculate_total_infectiousness_multiplier_for_person(&self, person_id: PersonId) -> f64 {
        let container = self.get_data_container(SettingDataPlugin).unwrap();
        // ToDo(ap59): What should happen if the person doesn't have an itinerary?
        match container.calculate_infectiousness_multiplier_vector_for_person(person_id) {
            Some(v) => v.iter().sum(),
            None => 0.0,
        }
    }

    fn draw_contact_from_itinerary(&self, person_id: PersonId) -> Option<PersonId> {
        let container = self.get_data_container(SettingDataPlugin).unwrap();
        self.sample(SettingsRng, |rng| {
            container.draw_contact_from_itinerary(person_id, rng)
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::assert_almost_eq;
    use crate::settings::ContextSettingExt;
    use crate::ContextPeopleExt;
    use ixa_fips::{FIPSCode, USState};
    #[test]
    fn test_setting_type_creation() {
        let mut context = Context::new();
        // Set new values
        assert!(context
            .set_alpha_for_setting_category(SettingCategory::Home, 0.1)
            .is_none());
        assert!(context
            .set_alpha_for_setting_category(SettingCategory::CensusTract, 0.001)
            .is_none());

        // Assert that:
        //   1) the old value is returned when a new value is set
        //   2) the original value was inserted correctly to begin with
        assert_almost_eq!(
            context
                .set_alpha_for_setting_category(SettingCategory::Home, 0.9)
                .unwrap(),
            0.1,
            0.0
        );
        assert_almost_eq!(
            context
                .set_alpha_for_setting_category(SettingCategory::CensusTract, 0.5)
                .unwrap(),
            0.001,
            0.0
        );
    }

    #[test]
    fn test_duplicated_itinerary() {
        // Different homes is ok. (Also, different person from person1 with same home is ok.)
        let itinerary0 = Itinerary::from_vec(vec![
            ItineraryEntry::new(
                FIPSCode::with_category(USState::AK, 0, 0, SettingCategory::Home),
                0.5,
            ),
            ItineraryEntry::new(
                FIPSCode::with_category(USState::TN, 0, 0, SettingCategory::Home),
                0.5,
            ),
        ]);
        assert!(itinerary0.is_ok());

        // Same value in "id" field but different setting type is ok.
        let itinerary1 = Itinerary::from_vec(vec![
            ItineraryEntry::new(
                FIPSCode::new(USState::AK, 0, 0, SettingCategory::Home, 314, 0),
                0.5,
            ),
            ItineraryEntry::new(
                FIPSCode::new(USState::AK, 0, 0, SettingCategory::CensusTract, 314, 0),
                0.5,
            ),
        ]);
        assert!(itinerary1.is_ok());

        // Differing only in "data field" should be an error.
        let itinerary2 = Itinerary::from_vec(vec![
            ItineraryEntry::new(
                FIPSCode::new(USState::AK, 0, 0, SettingCategory::Home, 314, 1),
                0.5,
            ),
            ItineraryEntry::new(
                FIPSCode::new(USState::AK, 0, 0, SettingCategory::Home, 314, 0),
                0.5,
            ),
        ]);
        assert!(itinerary2.is_err());
    }

    #[test]
    fn test_add_itinerary() {
        // Put two people into the same setting and sample one from the other's settings.
        let mut context = Context::new();
        context.init_random(42);
        context.set_alpha_for_setting_category(SettingCategory::Home, 0.1);

        let common_setting = FIPSCode::with_category(USState::AK, 0, 0, SettingCategory::Home);

        let person = context.add_person(()).unwrap();
        let itinerary =
            Itinerary::from_vec(vec![ItineraryEntry::new(common_setting, 1.0)]).unwrap();
        assert!(context
            .add_itinerary_for_person(person, itinerary)
            .is_none());

        let person2 = context.add_person(()).unwrap();
        let itinerary2 =
            Itinerary::from_vec(vec![ItineraryEntry::new(common_setting, 1.0)]).unwrap();
        assert!(context
            .add_itinerary_for_person(person2, itinerary2)
            .is_none());

        let sampled_person = context.draw_contact_from_itinerary(person);
        let sampled_person = sampled_person.unwrap();
        assert_eq!(sampled_person, person2);
    }

    #[test]
    fn test_setting_multiplier() {
        // TODO: if setting not registered, shouldn't be able to register people to setting
        let mut context = Context::new();
        context.init_random(42);
        context.set_alpha_for_setting_category(SettingCategory::Home, 0.1);

        let setting_prototype = FIPSCode::with_category(USState::AK, 0, 0, SettingCategory::Home);

        for s in 0..5 {
            let itinerary_prototype =
                Itinerary::from_vec(vec![ItineraryEntry::new(setting_prototype.set_id(s), 0.5)])
                    .unwrap();
            // Create 5 people
            for _ in 0..5 {
                let person = context.add_person(()).unwrap();
                let _ = context.add_itinerary_for_person(person, itinerary_prototype.clone());
            }
        }

        // Has id = 0.
        let itinerary =
            Itinerary::from_vec(vec![ItineraryEntry::new(setting_prototype, 0.5)]).unwrap();
        let person = context.add_person(()).unwrap();
        let _ = context.add_itinerary_for_person(person, itinerary);

        let inf_multiplier = context.calculate_total_infectiousness_multiplier_for_person(person);

        // This is assuming we know what the function for Home is (N - 1) ^ alpha
        assert_almost_eq!(inf_multiplier, 0.5 * f64::from(6 - 1).powf(0.1), 0.0);
    }

    #[test]
    fn test_total_infectiousness_multiplier() {
        // Go through all the settings and compute infectiousness multiplier
        let mut context = Context::new();
        context.set_alpha_for_setting_category(SettingCategory::Home, 0.1);
        context.set_alpha_for_setting_category(SettingCategory::CensusTract, 0.01);

        let home_prototype = FIPSCode::with_category(USState::AK, 0, 0, SettingCategory::Home);
        let tract_prototype =
            FIPSCode::with_category(USState::AK, 0, 0, SettingCategory::CensusTract);

        // Create 5 homes and census tracts with 5 people each.
        for s in 0..5 {
            let itinerary_prototype = Itinerary::from_vec(vec![
                ItineraryEntry::new(home_prototype.set_id(s), 0.5),
                ItineraryEntry::new(tract_prototype.set_id(s), 0.5),
            ])
            .unwrap();
            for _ in 0..5 {
                let person = context.add_person(()).unwrap();
                let _ = context.add_itinerary_for_person(person, itinerary_prototype.clone());
            }
        }

        // Create a new person and register to home 0
        let itinerary =
            Itinerary::from_vec(vec![ItineraryEntry::new(home_prototype.set_id(0), 1.0)]).unwrap();
        let person = context.add_person(()).unwrap();
        let _ = context.add_itinerary_for_person(person, itinerary);

        // If only registered at home, total infectiousness multiplier should be (6 - 1) ^ (alpha)
        let inf_multiplier = context.calculate_total_infectiousness_multiplier_for_person(person);
        assert_almost_eq!(inf_multiplier, f64::from(6 - 1).powf(0.1), 0.0);

        // If person's itinerary is changed for two settings,
        // CensusTract 0 should have 6 members, Home 0 should have 7 members
        // the total infectiousness should be the sum of infs * proportion
        let person = context.add_person(()).unwrap();
        let itinerary_complete = Itinerary::from_vec(vec![
            ItineraryEntry::new(home_prototype.set_id(0), 0.5),
            ItineraryEntry::new(tract_prototype.set_id(0), 0.5),
        ])
        .unwrap();

        let _ = context.add_itinerary_for_person(person, itinerary_complete);
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
        context.set_alpha_for_setting_category(SettingCategory::Home, 0.1);
        context.set_alpha_for_setting_category(SettingCategory::CensusTract, 0.01);

        let person_a = context.add_person(()).unwrap();
        let person_b = context.add_person(()).unwrap();

        let home_prototype = FIPSCode::with_category(USState::AK, 0, 0, SettingCategory::Home);
        let tract_prototype =
            FIPSCode::with_category(USState::AK, 0, 0, SettingCategory::CensusTract);

        let itinerary_a = Itinerary::from_vec(vec![
            ItineraryEntry::new(home_prototype, 0.5),
            ItineraryEntry::new(tract_prototype, 0.5),
        ])
        .unwrap();

        let itinerary_b =
            Itinerary::from_vec(vec![ItineraryEntry::new(home_prototype, 1.0)]).unwrap();

        let _ = context.add_itinerary_for_person(person_a, itinerary_a);
        let _ = context.add_itinerary_for_person(person_b, itinerary_b);

        assert_eq!(
            person_b,
            context.draw_contact_from_itinerary(person_a).unwrap()
        );
        assert_eq!(
            person_a,
            context.draw_contact_from_itinerary(person_b).unwrap()
        );
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
            context.set_alpha_for_setting_category(SettingCategory::Home, 0.1);
            context.set_alpha_for_setting_category(SettingCategory::CensusTract, 0.01);

            let home_prototype = FIPSCode::with_category(USState::AK, 0, 0, SettingCategory::Home);
            let tract_prototype =
                FIPSCode::with_category(USState::AK, 0, 0, SettingCategory::CensusTract);

            {
                let itinerary_home =
                    Itinerary::from_vec(vec![ItineraryEntry::new(home_prototype, 1.0)]).unwrap();
                for _ in 0..3 {
                    let person = context.add_person(()).unwrap();
                    people_at_home.push(person);
                    let _ = context.add_itinerary_for_person(person, itinerary_home.clone());
                }
            }

            {
                let itinerary_tract =
                    Itinerary::from_vec(vec![ItineraryEntry::new(tract_prototype, 1.0)]).unwrap();
                for _ in 0..3 {
                    let person = context.add_person(()).unwrap();
                    people_at_tract.push(person);
                    let _ = context.add_itinerary_for_person(person, itinerary_tract.clone());
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
            let _ = context.add_itinerary_for_person(person, itinerary_home);
            let contact_id_home = context.draw_contact_from_itinerary(person).unwrap();
            assert!(people_at_home.contains(&contact_id_home));

            // Now draw a contact from the itinerary with 0 at home and 1 at census tract
            let _ = context.add_itinerary_for_person(person, itinerary_tract);
            let contact_id_tract = context.draw_contact_from_itinerary(person).unwrap();
            assert!(people_at_tract.contains(&contact_id_tract));
        }
    }

    // TODO: Test failure of getting properties if not initialized
    // TODO: Test that proportions either add to 1 or that they are weighted based on proportion
}
