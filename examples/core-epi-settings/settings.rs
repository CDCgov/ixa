//! The population and its settings, loaded from a line list: each row is one
//! person with their household, and optionally school and workplace, by id.
//! The community is the population group everyone is in.
use std::path::Path;

use ixa::core_epi::{Itinerary, Person, SettingsExt};
use ixa::define_settings;
use ixa::prelude::*;

use crate::parameters::ParametersExt;

define_settings!(pub Setting { Household, School, Workplace, Community });

define_property!(struct Age(u8), Person);

/// Register the settings, then create one person per row of the line list,
/// creating each setting group the first time its id appears.
pub fn setup(context: &mut Context) {
    let parameters = context.get_parameters().clone();
    context.register_setting(Setting::Household, parameters.household_alpha);
    context.register_setting(Setting::School, parameters.school_alpha);
    context.register_setting(Setting::Workplace, parameters.workplace_alpha);
    context.register_setting(Setting::Community, parameters.community_alpha);
    context.create_population_setting_group(Setting::Community);
    context.set_itinerary_assignment(Itinerary::new([
        (Setting::Household, parameters.household_ratio),
        (Setting::School, parameters.school_ratio),
        (Setting::Workplace, parameters.workplace_ratio),
        (Setting::Community, parameters.community_ratio),
    ]));

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(&parameters.population_file);
    let mut reader = csv::Reader::from_path(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let mut row = csv::ByteRecord::new();
    while reader.read_byte_record(&mut row).unwrap() {
        let age: u8 = std::str::from_utf8(&row[0]).unwrap().parse().unwrap();
        let person = context.add_entity(with!(Person, Age(age))).unwrap();
        // Ids are hashed as bytes straight from the row, so nothing is allocated.
        let household = context.setting_group_for_key(Setting::Household, &row[1]);
        context.add_setting_member(household, person);
        if !row[2].is_empty() {
            let school = context.setting_group_for_key(Setting::School, &row[2]);
            context.add_setting_member(school, person);
        }
        if !row[3].is_empty() {
            let workplace = context.setting_group_for_key(Setting::Workplace, &row[3]);
            context.add_setting_member(workplace, person);
        }
    }
}
