//! A generic mechanism for storing context-wide data.
//!
//! Global properties are not mutable and represent variables that are
//! required in a global scope during the simulation, such as
//! simulation parameters.
//! A global property can be of any type, and is is just a value
//! stored in the context. Global properties are defined by the
//! [`define_global_property!()`] macro and can then be
//! set in one of two ways:
//!
//! * Directly by using [`Context::set_global_property_value()`]
//! * Loaded from a configuration file using [`Context::load_global_properties()`]
//!
//! Attempting to change a global property which has been set already
//! will result in an error.
//!
//! Global properties can be read with [`Context::get_global_property_value()`]
use crate::context::Context;
use crate::error::IxaError;
use serde::de::DeserializeOwned;
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::{hash_map::Entry, HashMap};
use std::fmt::Debug;
use std::fs;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;

type PropertySetterFn =
    dyn Fn(&mut Context, &str, serde_json::Value) -> Result<(), IxaError> + Send + Sync;

type PropertyGetterFn = dyn Fn(&Context) -> Result<Option<String>, IxaError> + Send + Sync;

pub struct PropertyAccessors {
    setter: Box<PropertySetterFn>,
    getter: Box<PropertyGetterFn>,
}

#[allow(clippy::type_complexity)]
// This is a global list of all the global properties that
// are compiled in. Fundamentally it's a HashMap of property
// names to the setter function, but it's wrapped in the
// RefCell/Mutex/LazyLock combo to allow it to be globally
// shared and initialized at startup time while still being
// safe.
#[doc(hidden)]
pub static GLOBAL_PROPERTIES: LazyLock<Mutex<RefCell<HashMap<String, Arc<PropertyAccessors>>>>> =
    LazyLock::new(|| Mutex::new(RefCell::new(HashMap::new())));

#[allow(clippy::missing_panics_doc)]
pub fn add_global_property<T: GlobalProperty>(name: &str)
where
    for<'de> <T as GlobalProperty>::Value: serde::Deserialize<'de> + serde::Serialize,
{
    let properties = GLOBAL_PROPERTIES.lock().unwrap();
    assert!(properties
        .borrow_mut()
        .insert(
            name.to_string(),
            Arc::new(PropertyAccessors {
                setter: Box::new(
                    |context: &mut Context, name, value| -> Result<(), IxaError> {
                        let val: T::Value = serde_json::from_value(value)?;
                        T::validate(&val)?;
                        if context.get_global_property_value(T::new()).is_some() {
                            return Err(IxaError::IxaError(format!("Duplicate property {name}")));
                        }
                        context.set_global_property_value(T::new(), val)?;
                        Ok(())
                    }
                ),
                getter: Box::new(|context: &Context| -> Result<Option<String>, IxaError> {
                    let value = context.get_global_property_value(T::new());
                    match value {
                        Some(val) => Ok(Some(serde_json::to_string(val)?)),
                        None => Ok(None),
                    }
                }),
            })
        )
        .is_none());
}

fn get_global_property_accessor(name: &str) -> Option<Arc<PropertyAccessors>> {
    let properties = GLOBAL_PROPERTIES.lock().unwrap();
    let tmp = properties.borrow();
    tmp.get(name).map(Arc::clone)
}

/// Defines a global property with the following parameters:
/// * `$global_property`: Name for the identifier type of the global property
/// * `$value`: The type of the property's value
/// * `$validate`: A function (or closure) that checks the validity of the property (optional)
#[macro_export]
macro_rules! define_global_property {
    ($global_property:ident, $value:ty, $validate: expr) => {
        #[derive(Copy, Clone)]
        pub struct $global_property;

        impl $crate::global_properties::GlobalProperty for $global_property {
            type Value = $value;

            fn new() -> Self {
                $global_property
            }

            fn validate(val: &$value) -> Result<(), $crate::error::IxaError> {
                $validate(val)
            }
        }

        paste::paste! {
            #[ctor::ctor]
            fn [<$global_property:snake _register>]() {
                let module = module_path!();
                let mut name = module.split("::").next().unwrap().to_string();
                name += ".";
                name += stringify!($global_property);
                $crate::global_properties::add_global_property::<$global_property>(&name);
            }
        }
    };

    ($global_property: ident, $value: ty) => {
        define_global_property!($global_property, $value, |_| { Ok(()) });
    };
}

/// The trait representing a global property. Do not use this
/// directly, but instead define global properties with
/// [`define_global_property()`]
pub trait GlobalProperty: Any {
    type Value: Any; // The actual type of the data.

    fn new() -> Self;
    #[allow(clippy::missing_errors_doc)]
    // A function which validates the global property.
    fn validate(value: &Self::Value) -> Result<(), IxaError>;
}

pub use define_global_property;

struct GlobalPropertiesDataContainer {
    global_property_container: HashMap<TypeId, Box<dyn Any>>,
}

crate::context::define_data_plugin!(
    GlobalPropertiesPlugin,
    GlobalPropertiesDataContainer,
    GlobalPropertiesDataContainer {
        global_property_container: HashMap::default(),
    }
);

pub trait ContextGlobalPropertiesExt {
    /// Set the value of a global property of type T
    ///
    /// # Errors
    /// Will return an error if an attempt is made to change a value.
    fn set_global_property_value<T: GlobalProperty + 'static>(
        &mut self,
        property: T,
        value: T::Value,
    ) -> Result<(), IxaError>;

    /// Return value of global property T
    fn get_global_property_value<T: GlobalProperty + 'static>(
        &self,
        _property: T,
    ) -> Option<&T::Value>;

    fn list_registered_global_properties(&self) -> Vec<String>;

    /// Return the serialized value of a global property by fully qualified name
    ///
    /// # Errors
    ///
    /// Will return an `IxaError` if the property does not exist
    fn get_serialized_value_by_string(&self, name: &str) -> Result<Option<String>, IxaError>;

    /// Given a file path for a valid json file, deserialize parameter values
    /// for a given struct T
    ///
    /// # Errors
    ///
    /// Will return an `IxaError` if the `file_path` does not exist or
    /// cannot be deserialized
    fn load_parameters_from_json<T: 'static + Debug + DeserializeOwned>(
        &mut self,
        file_path: &Path,
    ) -> Result<T, IxaError>;

    /// Load global properties from a JSON file.
    ///
    /// The expected structure is a dictionary with each name being
    /// the name of the struct prefixed with the crate name, as in:
    /// `ixa.NumFluVariants` and the value being an object which can
    /// serde deserialize into the relevant struct.
    ///
    /// # Errors
    /// Will return an `IxaError` if:
    /// * The `file_path` doesn't exist
    /// * The file isn't valid JSON
    /// * A specified object doesn't correspond to an existing global property.
    /// * There are two values for the same object.
    ///
    /// Ixa automatically knows about any property defined with
    /// [`define_global_property!()`] so you don't need to register them
    /// explicitly.
    ///
    /// It is possible to call [`Context::load_global_properties()`] multiple
    /// times with different files as long as the files have disjoint
    /// sets of properties.
    fn load_global_properties(&mut self, file_name: &Path) -> Result<(), IxaError>;
}

impl GlobalPropertiesDataContainer {
    fn set_global_property_value<T: GlobalProperty + 'static>(
        &mut self,
        _property: &T,
        value: T::Value,
    ) -> Result<(), IxaError> {
        match self.global_property_container.entry(TypeId::of::<T>()) {
            Entry::Vacant(entry) => {
                entry.insert(Box::new(value));
                Ok(())
            }
            // Note: If we change global properties to be mutable, we'll need to
            // update define_derived_property to either handle updates or only
            // allow immutable properties.
            Entry::Occupied(_) => Err(IxaError::from("Entry already exists")),
        }
    }

    #[must_use]
    fn get_global_property_value<T: GlobalProperty + 'static>(&self) -> Option<&T::Value> {
        let data_container = self.global_property_container.get(&TypeId::of::<T>());

        match data_container {
            Some(property) => Some(property.downcast_ref::<T::Value>().unwrap()),
            None => None,
        }
    }
}

impl ContextGlobalPropertiesExt for Context {
    fn set_global_property_value<T: GlobalProperty + 'static>(
        &mut self,
        property: T,
        value: T::Value,
    ) -> Result<(), IxaError> {
        T::validate(&value)?;
        let data_container = self.get_data_container_mut(GlobalPropertiesPlugin);
        data_container.set_global_property_value(&property, value)
    }

    #[allow(unused_variables)]
    fn get_global_property_value<T: GlobalProperty + 'static>(
        &self,
        _property: T,
    ) -> Option<&T::Value> {
        if let Some(data_container) = self.get_data_container(GlobalPropertiesPlugin) {
            return data_container.get_global_property_value::<T>();
        };
        None
    }

    fn list_registered_global_properties(&self) -> Vec<String> {
        let properties = GLOBAL_PROPERTIES.lock().unwrap();
        let tmp = properties.borrow();
        tmp.keys().cloned().collect()
    }

    fn get_serialized_value_by_string(&self, name: &str) -> Result<Option<String>, IxaError> {
        let accessor = get_global_property_accessor(name);
        match accessor {
            Some(accessor) => (accessor.getter)(self),
            None => Err(IxaError::from(format!("No global property: {name}"))),
        }
    }

    fn load_parameters_from_json<T: 'static + Debug + DeserializeOwned>(
        &mut self,
        file_name: &Path,
    ) -> Result<T, IxaError> {
        let config_file = fs::File::open(file_name)?;
        let reader = BufReader::new(config_file);
        let config = serde_json::from_reader(reader)?;
        Ok(config)
    }

    fn load_global_properties(&mut self, file_name: &Path) -> Result<(), IxaError> {
        let config_file = fs::File::open(file_name)?;
        let reader = BufReader::new(config_file);
        let val: serde_json::Map<String, serde_json::Value> = serde_json::from_reader(reader)?;

        for (k, v) in val {
            if let Some(accessor) = get_global_property_accessor(&k) {
                (accessor.setter)(self, &k, v)?;
            } else {
                return Err(IxaError::from(format!("No global property: {k}")));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::context::Context;
    use crate::error::IxaError;
    use serde::{Deserialize, Serialize};
    use std::path::PathBuf;
    use tempfile::tempdir;
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ParamType {
        pub days: usize,
        pub diseases: usize,
    }

    define_global_property!(DiseaseParams, ParamType);

    #[test]
    fn set_get_global_property() {
        let params: ParamType = ParamType {
            days: 10,
            diseases: 2,
        };
        let params2: ParamType = ParamType {
            days: 11,
            diseases: 3,
        };

        let mut context = Context::new();

        // Set and check the stored value.
        context
            .set_global_property_value(DiseaseParams, params.clone())
            .unwrap();
        let global_params = context
            .get_global_property_value(DiseaseParams)
            .unwrap()
            .clone();
        assert_eq!(global_params.days, params.days);
        assert_eq!(global_params.diseases, params.diseases);

        // Setting again should fail because global properties are immutable.
        assert!(context
            .set_global_property_value(DiseaseParams, params2.clone())
            .is_err());

        // Check that the value is unchanged.
        let global_params = context
            .get_global_property_value(DiseaseParams)
            .unwrap()
            .clone();
        assert_eq!(global_params.days, params.days);
        assert_eq!(global_params.diseases, params.diseases);
    }

    #[test]
    fn get_global_propert_missing() {
        let context = Context::new();
        let global_params = context.get_global_property_value(DiseaseParams);
        assert!(global_params.is_none());
    }

    #[test]
    fn set_parameters() {
        let mut context = Context::new();
        let temp_dir = tempdir().unwrap();
        let config_path = PathBuf::from(&temp_dir.path());
        let file_name = "test.json";
        let file_path = config_path.join(file_name);
        let config = fs::File::create(config_path.join(file_name)).unwrap();

        let params: ParamType = ParamType {
            days: 10,
            diseases: 2,
        };

        define_global_property!(Parameters, ParamType);

        let _ = serde_json::to_writer(config, &params);
        let params_json = context
            .load_parameters_from_json::<ParamType>(&file_path)
            .unwrap();

        context
            .set_global_property_value(Parameters, params_json)
            .unwrap();

        let params_read = context
            .get_global_property_value(Parameters)
            .unwrap()
            .clone();
        assert_eq!(params_read.days, params.days);
        assert_eq!(params_read.diseases, params.diseases);
    }

    #[derive(Serialize, Deserialize)]
    pub struct Property1Type {
        field_int: u32,
        field_str: String,
    }
    define_global_property!(Property1, Property1Type);

    #[derive(Serialize, Deserialize)]
    pub struct Property2Type {
        field_int: u32,
    }
    define_global_property!(Property2, Property2Type);

    #[test]
    fn read_global_properties() {
        let mut context = Context::new();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/global_properties_test1.json");
        context.load_global_properties(&path).unwrap();
        let p1 = context.get_global_property_value(Property1).unwrap();
        assert_eq!(p1.field_int, 1);
        assert_eq!(p1.field_str, "test");
        let p2 = context.get_global_property_value(Property2).unwrap();
        assert_eq!(p2.field_int, 2);
    }

    #[test]
    fn read_unknown_property() {
        let mut context = Context::new();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/global_properties_missing.json");
        match context.load_global_properties(&path) {
            Err(IxaError::IxaError(msg)) => {
                assert_eq!(msg, "No global property: ixa.PropertyUnknown");
            }
            _ => panic!("Unexpected error type"),
        }
    }

    #[test]
    fn read_malformed_property() {
        let mut context = Context::new();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/global_properties_malformed.json");
        let error = context.load_global_properties(&path);
        println!("Error {error:?}");
        match error {
            Err(IxaError::JsonError(_)) => {}
            _ => panic!("Unexpected error type"),
        }
    }

    #[test]
    fn read_duplicate_property() {
        let mut context = Context::new();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/global_properties_test1.json");
        context.load_global_properties(&path).unwrap();
        let error = context.load_global_properties(&path);
        match error {
            Err(IxaError::IxaError(_)) => {}
            _ => panic!("Unexpected error type"),
        }
    }

    #[derive(Serialize, Deserialize)]
    pub struct Property3Type {
        field_int: u32,
    }
    define_global_property!(Property3, Property3Type, |v: &Property3Type| {
        match v.field_int {
            0 => Ok(()),
            _ => Err(IxaError::IxaError(format!(
                "Illegal value for `field_int`: {}",
                v.field_int
            ))),
        }
    });

    #[test]
    fn validate_property_set_success() {
        let mut context = Context::new();
        context
            .set_global_property_value(Property3, Property3Type { field_int: 0 })
            .unwrap();
    }

    #[test]
    fn validate_property_set_failure() {
        let mut context = Context::new();
        assert!(matches!(
            context.set_global_property_value(Property3, Property3Type { field_int: 1 }),
            Err(IxaError::IxaError(_))
        ));
    }

    #[test]
    fn validate_property_load_success() {
        let mut context = Context::new();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/global_properties_valid.json");
        context.load_global_properties(&path).unwrap();
    }

    #[test]
    fn validate_property_load_failure() {
        let mut context = Context::new();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/global_properties_invalid.json");
        assert!(matches!(
            context.load_global_properties(&path),
            Err(IxaError::IxaError(_))
        ));
    }

    #[test]
    fn list_registered_global_properties() {
        let context = Context::new();
        let properties = context.list_registered_global_properties();
        assert!(properties.contains(&"ixa.DiseaseParams".to_string()));
    }

    #[test]
    fn get_serialized_value_by_string() {
        let mut context = Context::new();
        context
            .set_global_property_value(
                DiseaseParams,
                ParamType {
                    days: 10,
                    diseases: 2,
                },
            )
            .unwrap();
        let serialized = context
            .get_serialized_value_by_string("ixa.DiseaseParams")
            .unwrap();
        assert_eq!(serialized, Some("{\"days\":10,\"diseases\":2}".to_string()));
    }
}
