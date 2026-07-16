use ixa::prelude::*;
use ixa::ContextPopulationExt;
use tempfile::tempdir;

define_entity!(Person);
define_property!(
    struct Coordinates {
        x: i16,
        y: i16,
    },
    Person
);

#[test]
fn export_rejects_a_structured_property_value() {
    let temp_dir = tempdir().unwrap();
    let directory = temp_dir.path().join("population");
    let mut context = Context::new();
    context
        .add_entity(with!(Person, Coordinates { x: 10, y: -4 }))
        .unwrap();

    let error = context.export_population(&directory).unwrap_err();

    assert!(matches!(
        error,
        IxaError::InvalidPopulationPropertyValue { .. }
    ));
    assert!(!directory.exists());
}
