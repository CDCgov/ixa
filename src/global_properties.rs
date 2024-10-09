use crate::context::Context;
use serde::de::DeserializeOwned;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::{self, Debug, Display};
use std::fs;
use std::io;
use std::path::Path;

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

#[derive(Debug)]
pub enum IxaError {
    IoError(io::Error),
    JsonError(serde_json::Error),
}

impl From<io::Error> for IxaError {
    fn from(error: io::Error) -> Self {
        IxaError::IoError(error)
    }
}

impl From<serde_json::Error> for IxaError {
    fn from(error: serde_json::Error) -> Self {
        IxaError::JsonError(error)
    }
}

impl std::error::Error for IxaError {}

impl Display for IxaError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error: {self:?}")?;
        Ok(())
    }
}

pub trait GlobalProperty: Any {
    type Value: Any;
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
    fn set_global_property_value<T: GlobalProperty + 'static>(
        &mut self,
        property: T,
        value: T::Value,
    );
    fn get_global_property_value<T: GlobalProperty + 'static>(&self, _property: T) -> &T::Value;

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
        let config_file = fs::read_to_string(file_name)?;
        let config = serde_json::from_str(&config_file)?;
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
