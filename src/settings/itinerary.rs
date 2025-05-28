//! An `Itinerary` is a vector of `ItineraryEntry`s that enforces the constraint that there
//! is at most one instance of any given `SettingCategoryCode` represented in the `Itinerary`.

use crate::settings::SettingId;
use crate::IxaError;
use std::ops::Index;

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct ItineraryEntry {
    pub setting_id: SettingId,
    pub ratio: f64,
}

#[allow(dead_code)]
impl ItineraryEntry {
    pub fn new(setting_id: SettingId, ratio: f64) -> ItineraryEntry {
        ItineraryEntry { setting_id, ratio }
    }
}

/// A convenience wrapper for a vector of `ItineraryEntry`s that enforces the constraint that there
/// is at most one instance of any given `SettingCategoryCode` represented in the `Itinerary`.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::ContextSettingExt;
    use crate::{Context, ContextPeopleExt, ContextRandomExt};
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
    fn test_duplicated_itinerary() {
        // Different homes is ok. (Also, different person from person1 with same home is ok.)
        let itinerary0 = Itinerary::from_vec(vec![
            ItineraryEntry::new(
                FIPSCode::with_category(USState::AK.into(), 0, 0, SettingCategory::Home.into())
                    .unwrap(),
                0.5,
            ),
            ItineraryEntry::new(
                FIPSCode::with_category(USState::TN.into(), 0, 0, SettingCategory::Home.into())
                    .unwrap(),
                0.5,
            ),
        ]);
        assert!(itinerary0.is_ok());

        // Same value in "id" field but different setting type is ok.
        let itinerary1 = Itinerary::from_vec(vec![
            ItineraryEntry::new(
                FIPSCode::new(
                    USState::AK.into(),
                    0,
                    0,
                    SettingCategory::Home.into(),
                    314,
                    0,
                )
                .unwrap(),
                0.5,
            ),
            ItineraryEntry::new(
                FIPSCode::new(
                    USState::AK.into(),
                    0,
                    0,
                    SettingCategory::CensusTract.into(),
                    314,
                    0,
                )
                .unwrap(),
                0.5,
            ),
        ]);
        assert!(itinerary1.is_ok());

        // Differing only in "data field" should be an error.
        let itinerary2 = Itinerary::from_vec(vec![
            ItineraryEntry::new(
                FIPSCode::new(
                    USState::AK.into(),
                    0,
                    0,
                    SettingCategory::Home.into(),
                    314,
                    1,
                )
                .unwrap(),
                0.5,
            ),
            ItineraryEntry::new(
                FIPSCode::new(
                    USState::AK.into(),
                    0,
                    0,
                    SettingCategory::Home.into(),
                    314,
                    0,
                )
                .unwrap(),
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
        context.set_alpha_for_setting_category(SettingCategory::Home.into(), 0.1);

        let common_setting =
            FIPSCode::with_category(USState::AK.into(), 0, 0, SettingCategory::Home.into())
                .unwrap();

        let person = context.add_person(()).unwrap();
        let itinerary =
            Itinerary::from_vec(vec![ItineraryEntry::new(common_setting, 1.0)]).unwrap();
        assert!(context
            .set_itinerary_for_person(person, itinerary)
            .is_none());

        let person2 = context.add_person(()).unwrap();
        let itinerary2 =
            Itinerary::from_vec(vec![ItineraryEntry::new(common_setting, 1.0)]).unwrap();
        assert!(context
            .set_itinerary_for_person(person2, itinerary2)
            .is_none());

        let (sampled_person, _) = context.draw_contact_from_itinerary(person).unwrap();
        assert_eq!(sampled_person, person2);
    }
}
