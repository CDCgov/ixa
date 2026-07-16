use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ixa::prelude::*;
use ixa::{define_derived_property, impl_property, ContextPopulationExt};
use tempfile::tempdir;

define_entity!(EmptyPopulation);
define_entity!(Household);
define_entity!(Person);

define_property!(struct HouseholdSize(u8), Household);
define_property!(struct Age(u8), Person);
define_property!(struct Home(HouseholdId), Person);
define_property!(struct Vaccinated(bool), Person, default_const = Vaccinated(false));
define_property!(
    enum InfectionStatus {
        Susceptible,
        Infected,
        Recovered,
    },
    Person,
    default_const = InfectionStatus::Susceptible
);
define_property!(
    struct OptionalCode(Option<u16>),
    Person,
    default_const = OptionalCode(None)
);

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct Score(u16);
impl_property!(Score, Person, default_const = Score(0), persist = true);

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, serde::Serialize, serde::Deserialize)]
enum Region {
    Local,
    #[serde(rename = "north,west\nward")]
    NorthWest,
}
impl_property!(
    Region,
    Person,
    default_const = Region::Local,
    persist = true
);

define_derived_property!(struct IsAdult(bool), Person, [Age], |age| IsAdult(age.0 >= 18));

fn entity_file<E: Entity>(directory: &Path) -> PathBuf {
    let mut reader = csv::Reader::from_path(directory.join("manifest.csv")).unwrap();
    for record in reader.records() {
        let record = record.unwrap();
        if record.get(0) == Some("entity") && record.get(2) == Some(std::any::type_name::<E>()) {
            return directory.join(record.get(3).unwrap());
        }
    }
    panic!("entity type was not present in the population manifest");
}

fn replace_first_property_value<E: Entity, P>(directory: &Path, replacement: &str) {
    let file = entity_file::<E>(directory);
    let mut reader = csv::Reader::from_path(&file).unwrap();
    let headers = reader.headers().unwrap().clone();
    let property_column = headers
        .iter()
        .position(|header| header == std::any::type_name::<P>())
        .unwrap();
    let mut records: Vec<Vec<String>> = reader
        .records()
        .map(|record| record.unwrap().iter().map(str::to_owned).collect())
        .collect();
    records[0][property_column] = replacement.to_owned();
    drop(reader);

    let mut writer = csv::Writer::from_path(&file).unwrap();
    writer.write_record(&headers).unwrap();
    for record in records {
        writer.write_record(record).unwrap();
    }
    writer.flush().unwrap();
}

#[test]
fn whole_population_round_trips_with_indexes_derived_values_and_events() {
    let temp_dir = tempdir().unwrap();
    let directory = temp_dir.path().join("population");
    let mut source = Context::new();
    let household_0 = source
        .add_entity(with!(Household, HouseholdSize(2)))
        .unwrap();
    let household_1 = source
        .add_entity(with!(Household, HouseholdSize(1)))
        .unwrap();
    let person_0 = source
        .add_entity(with!(
            Person,
            Age(17),
            Home(household_0),
            OptionalCode(Some(12))
        ))
        .unwrap();
    let person_1 = source
        .add_entity(with!(Person, Age(42), Home(household_1)))
        .unwrap();
    source.set_property(person_0, InfectionStatus::Infected);
    source.set_property(person_0, Vaccinated(true));
    source.set_property(person_0, Region::NorthWest);
    source.set_property(person_0, Score(8));
    source.set_property(person_1, Score(13));

    source.export_population(&directory).unwrap();

    let person_csv = std::fs::read_to_string(entity_file::<Person>(&directory)).unwrap();
    assert!(person_csv.contains("\"north,west\nward\""));

    let manifest = std::fs::read_to_string(directory.join("manifest.csv")).unwrap();
    assert!(manifest.contains(std::any::type_name::<EmptyPopulation>()));
    assert!(manifest.contains(std::any::type_name::<Household>()));
    assert!(manifest.contains(std::any::type_name::<Person>()));
    let mut manifest_reader = csv::Reader::from_reader(manifest.as_bytes());
    let manifest_rows: Vec<_> = manifest_reader
        .records()
        .map(|record| record.unwrap())
        .filter(|record| record.get(0) == Some("entity"))
        .collect();
    let entity_names: Vec<_> = manifest_rows
        .iter()
        .map(|record| record.get(2).unwrap())
        .collect();
    let mut sorted_entity_names = entity_names.clone();
    sorted_entity_names.sort_unstable();
    assert_eq!(entity_names, sorted_entity_names);
    for (index, record) in manifest_rows.iter().enumerate() {
        assert_eq!(record.get(3).unwrap(), format!("entity_{index:04}.csv"));
    }

    let event_order = Rc::new(RefCell::new(Vec::new()));
    let mut target = Context::new();
    target.index_property::<Person, Age>();
    let household_events = Rc::clone(&event_order);
    target.subscribe_to_event(move |_context, event: EntityCreatedEvent<Household>| {
        household_events
            .borrow_mut()
            .push(format!("household:{}", event.entity_id));
    });
    let person_events = Rc::clone(&event_order);
    target.subscribe_to_event(move |_context, event: EntityCreatedEvent<Person>| {
        person_events
            .borrow_mut()
            .push(format!("person:{}", event.entity_id));
    });

    target.import_population(&directory).unwrap();

    assert_eq!(target.get_entity_count::<EmptyPopulation>(), 0);
    assert_eq!(target.get_entity_count::<Household>(), 2);
    assert_eq!(target.get_entity_count::<Person>(), 2);
    assert_eq!(target.get_property::<Person, Age>(person_0), Age(17));
    assert_eq!(target.get_property::<Person, Age>(person_1), Age(42));
    assert_eq!(
        target.get_property::<Person, InfectionStatus>(person_0),
        InfectionStatus::Infected
    );
    assert_eq!(target.get_property::<Person, Score>(person_0), Score(8));
    assert_eq!(target.get_property::<Person, Score>(person_1), Score(13));
    assert_eq!(
        target.get_property::<Person, Vaccinated>(person_0),
        Vaccinated(true)
    );
    assert_eq!(
        target.get_property::<Person, Vaccinated>(person_1),
        Vaccinated(false)
    );
    assert_eq!(
        target.get_property::<Person, Region>(person_0),
        Region::NorthWest
    );
    assert_eq!(
        target.get_property::<Person, Region>(person_1),
        Region::Local
    );
    assert_eq!(
        target.get_property::<Person, OptionalCode>(person_0),
        OptionalCode(Some(12))
    );
    assert_eq!(
        target.get_property::<Person, OptionalCode>(person_1),
        OptionalCode(None)
    );
    assert_eq!(
        target.get_property::<Person, Home>(person_0),
        Home(household_0)
    );
    assert_eq!(
        target.get_property::<Person, IsAdult>(person_0),
        IsAdult(false)
    );
    assert_eq!(
        target.get_property::<Person, IsAdult>(person_1),
        IsAdult(true)
    );
    assert_eq!(
        target.query_entity_count(with!(Person, Age(42))),
        1,
        "the pre-existing index should be caught up after import"
    );

    target.execute();
    assert_eq!(
        *event_order.borrow(),
        ["household:0", "household:1", "person:0", "person:1"]
    );
}

#[test]
fn export_refuses_to_overwrite_an_existing_directory() {
    let directory = tempdir().unwrap();
    let context = Context::new();

    let error = context.export_population(directory.path()).unwrap_err();

    assert!(matches!(
        error,
        IxaError::PopulationDestinationExists { .. }
    ));
}

#[test]
fn import_refuses_a_nonempty_target() {
    let temp_dir = tempdir().unwrap();
    let directory = temp_dir.path().join("population");
    let source = Context::new();
    source.export_population(&directory).unwrap();

    let mut target = Context::new();
    target
        .add_entity(with!(Household, HouseholdSize(1)))
        .unwrap();
    let error = target.import_population(&directory).unwrap_err();

    assert!(matches!(error, IxaError::PopulationNotEmpty { .. }));
    assert_eq!(target.get_entity_count::<Household>(), 1);
}

#[test]
fn invalid_entity_ids_are_rejected_before_context_mutation() {
    let temp_dir = tempdir().unwrap();
    let directory = temp_dir.path().join("population");
    let mut source = Context::new();
    source
        .add_entity(with!(Household, HouseholdSize(1)))
        .unwrap();
    source.export_population(&directory).unwrap();

    let file = entity_file::<Household>(&directory);
    let mut reader = csv::Reader::from_path(&file).unwrap();
    let headers = reader.headers().unwrap().clone();
    let mut records: Vec<Vec<String>> = reader
        .records()
        .map(|record| record.unwrap().iter().map(str::to_owned).collect())
        .collect();
    records[0][0] = "1".to_owned();
    drop(reader);
    let mut writer = csv::Writer::from_path(&file).unwrap();
    writer.write_record(&headers).unwrap();
    for record in records {
        writer.write_record(record).unwrap();
    }
    writer.flush().unwrap();

    let mut target = Context::new();
    let error = target.import_population(&directory).unwrap_err();

    assert!(matches!(error, IxaError::InvalidPopulationData { .. }));
    assert_eq!(target.get_entity_count::<Household>(), 0);
    assert_eq!(target.get_entity_count::<Person>(), 0);
}

#[test]
fn invalid_property_values_are_rejected_before_any_entity_type_is_imported() {
    let temp_dir = tempdir().unwrap();
    let directory = temp_dir.path().join("population");
    let mut source = Context::new();
    let household = source
        .add_entity(with!(Household, HouseholdSize(1)))
        .unwrap();
    source
        .add_entity(with!(Person, Age(27), Home(household)))
        .unwrap();
    source.export_population(&directory).unwrap();
    replace_first_property_value::<Person, Age>(&directory, "not-an-age");

    let mut target = Context::new();
    let error = target.import_population(&directory).unwrap_err();

    assert!(matches!(
        error,
        IxaError::InvalidPopulationPropertyValue { .. }
    ));
    assert_eq!(target.get_entity_count::<Household>(), 0);
    assert_eq!(target.get_entity_count::<Person>(), 0);
}

#[test]
fn unsupported_manifest_versions_are_rejected_before_context_mutation() {
    let temp_dir = tempdir().unwrap();
    let directory = temp_dir.path().join("population");
    Context::new().export_population(&directory).unwrap();

    let manifest_path = directory.join("manifest.csv");
    let mut reader = csv::Reader::from_path(&manifest_path).unwrap();
    let headers = reader.headers().unwrap().clone();
    let mut records: Vec<Vec<String>> = reader
        .records()
        .map(|record| record.unwrap().iter().map(str::to_owned).collect())
        .collect();
    records[0][1] = "999".to_owned();
    drop(reader);
    let mut writer = csv::Writer::from_path(&manifest_path).unwrap();
    writer.write_record(&headers).unwrap();
    for record in records {
        writer.write_record(record).unwrap();
    }
    writer.flush().unwrap();

    let mut target = Context::new();
    let error = target.import_population(&directory).unwrap_err();

    assert!(matches!(error, IxaError::InvalidPopulationData { .. }));
    assert_eq!(target.get_entity_count::<Person>(), 0);
}

#[test]
fn property_schema_mismatches_are_rejected_before_context_mutation() {
    let temp_dir = tempdir().unwrap();
    let directory = temp_dir.path().join("population");
    Context::new().export_population(&directory).unwrap();

    let file = entity_file::<Person>(&directory);
    let mut reader = csv::Reader::from_path(&file).unwrap();
    let mut headers: Vec<String> = reader
        .headers()
        .unwrap()
        .iter()
        .map(str::to_owned)
        .collect();
    let age_column = headers
        .iter()
        .position(|header| header == std::any::type_name::<Age>())
        .unwrap();
    headers[age_column] = "renamed::Age".to_owned();
    drop(reader);
    let mut writer = csv::Writer::from_path(&file).unwrap();
    writer.write_record(&headers).unwrap();
    writer.flush().unwrap();

    let mut target = Context::new();
    let error = target.import_population(&directory).unwrap_err();

    assert!(matches!(error, IxaError::PopulationSchemaMismatch { .. }));
    assert_eq!(target.get_entity_count::<Person>(), 0);
}

#[test]
fn unexpected_files_are_rejected_before_context_mutation() {
    let temp_dir = tempdir().unwrap();
    let directory = temp_dir.path().join("population");
    Context::new().export_population(&directory).unwrap();
    std::fs::write(directory.join("unreferenced.csv"), "unexpected").unwrap();

    let mut target = Context::new();
    let error = target.import_population(&directory).unwrap_err();

    assert!(matches!(error, IxaError::InvalidPopulationData { .. }));
    assert_eq!(target.get_entity_count::<Person>(), 0);
}
