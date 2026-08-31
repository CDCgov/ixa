use ixa::prelude::*;
use ixa::{impl_property, ContextPopulationExt};
use tempfile::tempdir;

define_entity!(Person);

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
struct ManualValue(u8);
impl_property!(ManualValue, Person, default_const = ManualValue(0));

#[test]
fn export_rejects_a_manual_property_that_did_not_opt_in() {
    let temp_dir = tempdir().unwrap();
    let directory = temp_dir.path().join("population");
    let context = Context::new();

    let error = context.export_population(&directory).unwrap_err();

    assert!(matches!(
        error,
        IxaError::PopulationUnsupportedProperty { .. }
    ));
    assert!(!directory.exists());
}
