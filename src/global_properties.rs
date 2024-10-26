use crate::context::Context;
use crate::error::IxaError;
use serde::de::DeserializeOwned;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs;
use std::io::BufReader;
use std::path::Path;

/// Defines a global property with the following parameters:
/// * `$global_property`: Name for the identifier type of the global property
/// * `$value`: The type of the property's value
#[macro_export]
macro_rules! define_global_property {
    ($global_property:ident, $value:ty) => {
        #[derive(Copy, Clone)]
        pub struct $global_property;

        impl $crate::global_properties::GlobalProperty for $global_property {
            type Value = $value;
        }
    };
}

/// Global properties are not mutable and represent variables that are required
/// in a global scope during the simulation, such as simulation parameters.
pub trait GlobalProperty: Any {
    type Value: Any;
}

pub use define_global_property;

#[allow(clippy::module_name_repetitions)]
pub struct GlobalPropertiesDataContainer {
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
    fn set_global_property_value<T: GlobalProperty + 'static>(
        &mut self,
        property: T,
        value: T::Value,
    );

    /// Return value of global property T
    fn get_global_property_value<T: GlobalProperty + 'static>(&self, _property: T) -> &T::Value;

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
}

impl GlobalPropertiesDataContainer {
    fn set_global_property_value<T: GlobalProperty + 'static>(
        &mut self,
        _property: &T,
        value: T::Value,
    ) {
        let _data_container = self
            .global_property_container
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(value));
    }

    fn get_global_property_value<T: GlobalProperty + 'static>(&self) -> &T::Value {
        let data_container = self
            .global_property_container
            .get(&TypeId::of::<T>())
            .expect("Global property not initialized");
        data_container.downcast_ref::<T::Value>().unwrap()
    }
}

impl ContextGlobalPropertiesExt for Context {
    fn set_global_property_value<T: GlobalProperty + 'static>(
        &mut self,
        property: T,
        value: T::Value,
    ) {
        let data_container = self.get_data_container_mut(GlobalPropertiesPlugin);
        data_container.set_global_property_value(&property, value);
    }

    #[allow(unused_variables)]
    fn get_global_property_value<T: GlobalProperty + 'static>(&self, _property: T) -> &T::Value {
        let data_container = self.get_data_container(GlobalPropertiesPlugin).unwrap();
        data_container.get_global_property_value::<T>()
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
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::context::Context;
    use serde::{Deserialize, Serialize};
    use std::path::PathBuf;
    use tempfile::tempdir;
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ParamType {
        pub days: usize,
        pub diseases: usize,
    }

    define_global_property!(DiseaseParams, ParamType);
    //Since global properties aren't mutable right now, only
    // check that they are properly set
    #[test]
    fn set_get_global_property() {
        let params: ParamType = ParamType {
            days: 10,
            diseases: 2,
        };
        let mut context = Context::new();
        context.set_global_property_value(DiseaseParams, params.clone());
        let global_params = context.get_global_property_value(DiseaseParams).clone();
        assert_eq!(global_params.days, params.days);
        assert_eq!(global_params.diseases, params.diseases);
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

        context.set_global_property_value(Parameters, params_json);

        let params_read = context.get_global_property_value(Parameters).clone();
        assert_eq!(params_read.days, params.days);
        assert_eq!(params_read.diseases, params.diseases);
    }
}
