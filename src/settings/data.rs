use crate::settings::itinerary::{Itinerary, ItineraryEntry};
use crate::settings::SettingId;
use crate::PersonId;
use ixa_fips::SettingCategoryCode;
use rand::distributions::{Distribution, WeightedIndex};
use rand::Rng;
use rustc_hash::FxHashMap as HashMap;
use std::collections::hash_map::Entry;

#[derive(Default)]
pub(super) struct SettingsDataContainer {
    /// Each `SettingCategoryCode` has an alpha of type `f64`
    ///
    /// In a setting with `n` people (including the source of infection), the rate of the total
    /// infectiousness process is computed as
    ///      (intrinsic infectiousness) × (n - 1)ᵅ
    /// where 0 ≤ 𝛼 ≤ 1. This interpolates between having the total hazard _distributed_ equally and
    /// the total hazard applying equally to the nonsources.
    pub(super) alpha_for_setting_category: HashMap<SettingCategoryCode, f64>,
    /// Each `PersonId` has an `Itinerary` of `SettingId`s
    pub(super) itineraries: HashMap<PersonId, Itinerary>,
    /// Each `SettingId` has a list of members
    pub(super) members: HashMap<SettingId, Vec<PersonId>>,
}

impl SettingsDataContainer {
    pub(super) fn new() -> Self {
        SettingsDataContainer::default()
    }

    /// Adds an `Itinerary` for the given person, inserting the person as a member of the settings
    /// in the given `Itinerary`. Returns the old `Itinerary` if the method replaced an existing
    /// itinerary (i.e. an itinerary was already set for this person), `None` otherwise.
    pub(super) fn add_itinerary_for_person(
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
    ///     - alpha - the alpha value for the `SettingCategoryCode` associated to the `SettingId` in
    ///       the `ItineraryEntry`
    ///     - `member_count`: the length of the list of members of the setting
    ///
    /// If there is no itinerary for the person, this method is a no-op.
    pub(super) fn with_itinerary<F>(&self, person_id: PersonId, mut callback: F)
    where
        // f(entry: ItineraryEntry, alpha_for_setting: f64, member_count: usize)
        F: FnMut(ItineraryEntry, f64, usize),
    {
        if let Some(itinerary) = self.itineraries.get(&person_id) {
            for entry in itinerary {
                let alpha = match self
                    .alpha_for_setting_category
                    .get(&entry.setting_id.category_code())
                {
                    Some(alpha) => *alpha,
                    None => {
                        panic!(
                            "setting category {} was not assigned an alpha value",
                            entry.setting_id.category_code()
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
