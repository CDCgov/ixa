//! The population and its settings, loaded from a line list: each row is one
//! person with their household, and optionally school and workplace, by id.
//! The community is the population group everyone is in.
use std::path::Path;

use ixa::core_epi::{Itinerary, SettingsExt};
use ixa::define_settings;
use ixa::prelude::*;

use crate::parameters::ParametersExt;

define_entity!(Person);

define_settings!(pub Setting for Person);

define_property!(struct Age(u8), Person);

/// Register the settings, then create one person per row of the line list,
/// creating each setting group the first time its id appears.
pub fn setup(context: &mut Context) {
    let parameters = context.get_parameters().clone();
    let household_kind =
        context.add_setting_kind::<Setting>("household", parameters.household_alpha);
    let school_kind = context.add_setting_kind::<Setting>("school", parameters.school_alpha);
    let workplace_kind =
        context.add_setting_kind::<Setting>("workplace", parameters.workplace_alpha);
    let community_kind =
        context.add_setting_kind::<Setting>("community", parameters.community_alpha);
    context.create_population_setting_group(community_kind);
    context.set_itinerary_assignment(Itinerary::new([
        (household_kind, parameters.household_ratio),
        (school_kind, parameters.school_ratio),
        (workplace_kind, parameters.workplace_ratio),
        (community_kind, parameters.community_ratio),
    ]));

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(&parameters.population_file);
    let mut reader = csv::Reader::from_path(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let mut row = csv::ByteRecord::new();
    while reader.read_byte_record(&mut row).unwrap() {
        let age: u8 = std::str::from_utf8(&row[0]).unwrap().parse().unwrap();
        let person = context.add_entity(with!(Person, Age(age))).unwrap();
        // Ids are hashed as bytes straight from the row, so nothing is allocated.
        let household = context.setting_group_for_key(household_kind, &row[1]);
        context.add_setting_member(household, person);
        if !row[2].is_empty() {
            let school = context.setting_group_for_key(school_kind, &row[2]);
            context.add_setting_member(school, person);
        }
        if !row[3].is_empty() {
            let workplace = context.setting_group_for_key(workplace_kind, &row[3]);
            context.add_setting_member(workplace, person);
        }
    }
}
