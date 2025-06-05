//! Settings (locations) represent places that are contexts in which transmission takes place. A
//! setting determines which other people are in contact with a particular person and for how long.
//! The `SettingProperties::alpha` parameter determines how the hazard is distributed among an
//! infected person's contacts within a setting.
//!
//! Data related to settings is managed by the [`SettingsDataContainer`]. A
//! `SettingId` is just an alias for `FIPSCode`, a FIPS code. A setting category
//! is represented by a `SettingCategoryCode`, which is an alias for a `u8`.
//!
//!

mod context_extension;
mod data;
mod itinerary;

use crate::{define_data_plugin, define_rng};
use ixa_fips::FIPSCode;

pub use context_extension::ContextSettingExt;
use data::SettingsDataContainer;
pub use itinerary::{Itinerary, ItineraryEntry};

pub type SettingId = FIPSCode;

define_rng!(SettingsRng);

define_data_plugin!(
    SettingDataPlugin,
    SettingsDataContainer,
    SettingsDataContainer::new()
);
