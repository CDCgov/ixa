//! Export and import complete entity populations as directories of CSV files.
//!
//! Population persistence includes entity IDs and non-derived properties. It does not include
//! networks, global properties, random-number-generator state, plans, simulation time, data
//! plugins, event handlers, or indexes.

pub(crate) mod scalar;

use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path};
use std::sync::{LazyLock, Mutex, OnceLock};

use crate::entity::events::EntityCreatedEvent;
use crate::entity::property_store::{persisted_properties, PropertyPersistence};
use crate::entity::{Entity, EntityId};
use crate::{warn, Context, HashMap, IxaError};

const MANIFEST_FILE_NAME: &str = "manifest.csv";
const POPULATION_FORMAT_VERSION: &str = "1";
const MANIFEST_HEADER: [&str; 4] = ["record_type", "format_version", "entity_type", "file_name"];
const ENTITY_ID_HEADER: &str = "entity_id";

#[derive(Clone, Copy)]
struct EntityPersistenceMetadata {
    entity_id: usize,
    type_name: &'static str,
    count: fn(&Context) -> usize,
    write: fn(&Context, &Path) -> Result<(), IxaError>,
    read: fn(&Path) -> Result<Box<dyn PendingEntityImport>, IxaError>,
}

static ENTITY_PERSISTENCE_BUILDER: LazyLock<Mutex<HashMap<usize, EntityPersistenceMetadata>>> =
    LazyLock::new(|| Mutex::new(HashMap::default()));
static ENTITY_PERSISTENCE: OnceLock<Box<[EntityPersistenceMetadata]>> = OnceLock::new();

pub(crate) fn register_entity<E: Entity>() {
    let entity_id = E::id();
    if let Some(metadata) = ENTITY_PERSISTENCE.get() {
        debug_assert!(metadata
            .iter()
            .any(|entry| entry.entity_id == entity_id
                && entry.type_name == std::any::type_name::<E>()));
        return;
    }

    ENTITY_PERSISTENCE_BUILDER
        .lock()
        .expect("entity persistence registry lock was poisoned")
        .entry(entity_id)
        .or_insert(EntityPersistenceMetadata {
            entity_id,
            type_name: std::any::type_name::<E>(),
            count: entity_count::<E>,
            write: write_entity::<E>,
            read: read_entity::<E>,
        });
}

fn registered_entities() -> &'static [EntityPersistenceMetadata] {
    ENTITY_PERSISTENCE.get_or_init(|| {
        let mut builder = ENTITY_PERSISTENCE_BUILDER
            .lock()
            .expect("entity persistence registry lock was poisoned");
        let mut metadata: Vec<_> = builder.drain().map(|(_, metadata)| metadata).collect();
        metadata.sort_unstable_by_key(|entry| entry.entity_id);
        metadata.into_boxed_slice()
    })
}

fn entities_by_name() -> Vec<EntityPersistenceMetadata> {
    let mut entities = registered_entities().to_vec();
    entities.sort_unstable_by_key(|entry| entry.type_name);
    entities
}

fn entity_count<E: Entity>(context: &Context) -> usize {
    context.entity_store.get_entity_count::<E>()
}

/// Extension trait for exporting and importing all entity populations in a [`Context`].
pub trait ContextPopulationExt {
    /// Exports all registered entity types and their non-derived properties to `directory`.
    ///
    /// The destination must not already exist. The export contains a `manifest.csv` and one CSV
    /// file per registered entity type.
    ///
    /// # Errors
    ///
    /// Returns an error when the destination exists, filesystem or CSV output fails, a
    /// non-derived property has not opted into persistence, or a property value cannot be encoded
    /// as a supported scalar CSV cell.
    fn export_population(&self, directory: &Path) -> Result<(), IxaError>;

    /// Imports every registered entity population from `directory`.
    ///
    /// All registered entity populations in the target context must be empty. The complete export
    /// is read and validated before the context is mutated.
    ///
    /// # Errors
    ///
    /// Returns an error when an entity population is nonempty, the directory or CSV input is
    /// invalid, the persisted schema does not exactly match the current model, a property has not
    /// opted into persistence, or a scalar property value cannot be decoded.
    fn import_population(&mut self, directory: &Path) -> Result<(), IxaError>;
}

impl ContextPopulationExt for Context {
    fn export_population(&self, directory: &Path) -> Result<(), IxaError> {
        if directory.try_exists()? {
            return Err(IxaError::PopulationDestinationExists {
                path: directory.to_path_buf(),
            });
        }

        fs::create_dir(directory)?;
        let result = export_population(self, directory);
        if result.is_err() {
            if let Err(cleanup_error) = fs::remove_dir_all(directory) {
                warn!(
                    "Failed to clean up incomplete population export at {:?}: {}",
                    directory, cleanup_error
                );
            }
        }
        result
    }

    fn import_population(&mut self, directory: &Path) -> Result<(), IxaError> {
        let entities = entities_by_name();
        for entity in &entities {
            let count = (entity.count)(self);
            if count != 0 {
                return Err(IxaError::PopulationNotEmpty {
                    entity_type: entity.type_name.to_owned(),
                    count,
                });
            }
        }

        let manifest = read_manifest(directory)?;
        validate_manifest_schema(&entities, &manifest)?;
        validate_directory_contents(directory, &manifest)?;

        let entities_by_name: BTreeMap<_, _> = entities
            .iter()
            .map(|entity| (entity.type_name, *entity))
            .collect();
        let mut pending = Vec::with_capacity(manifest.len());
        for entry in manifest {
            let metadata = entities_by_name
                .get(entry.entity_type.as_str())
                .expect("validated manifest entity type is missing from the registry");
            pending.push((metadata.read)(&directory.join(entry.file_name))?);
        }

        for entity in pending {
            entity.apply(self);
        }
        Ok(())
    }
}

fn export_population(context: &Context, directory: &Path) -> Result<(), IxaError> {
    let entities = entities_by_name();
    let mut manifest = Vec::with_capacity(entities.len());
    for (index, entity) in entities.iter().enumerate() {
        let file_name = entity_file_name(index);
        (entity.write)(context, &directory.join(&file_name))?;
        manifest.push(ManifestEntry {
            entity_type: entity.type_name.to_owned(),
            file_name,
        });
    }
    write_manifest(directory, &manifest)
}

fn entity_file_name(index: usize) -> String {
    format!("entity_{index:04}.csv")
}

#[derive(Debug)]
struct ManifestEntry {
    entity_type: String,
    file_name: String,
}

fn write_manifest(directory: &Path, entries: &[ManifestEntry]) -> Result<(), IxaError> {
    let path = directory.join(MANIFEST_FILE_NAME);
    let mut writer = csv::Writer::from_path(path)?;
    writer.write_record(MANIFEST_HEADER)?;
    writer.write_record(["manifest", POPULATION_FORMAT_VERSION, "", ""])?;
    for entry in entries {
        writer.write_record([
            "entity",
            POPULATION_FORMAT_VERSION,
            entry.entity_type.as_str(),
            entry.file_name.as_str(),
        ])?;
    }
    writer.flush()?;
    Ok(())
}

fn read_manifest(directory: &Path) -> Result<Vec<ManifestEntry>, IxaError> {
    let path = directory.join(MANIFEST_FILE_NAME);
    let mut reader = csv::Reader::from_path(&path)
        .map_err(|error| invalid_population_data(&path, error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| invalid_population_data(&path, error.to_string()))?;
    if headers.iter().ne(MANIFEST_HEADER) {
        return Err(invalid_population_data(
            &path,
            format!(
                "expected manifest header {:?}, found {headers:?}",
                MANIFEST_HEADER
            ),
        ));
    }

    let mut records = reader.records();
    let metadata = records
        .next()
        .ok_or_else(|| invalid_population_data(&path, "manifest metadata row is missing"))?
        .map_err(|error| invalid_population_data(&path, error.to_string()))?;
    if metadata.get(0) != Some("manifest")
        || metadata.get(1) != Some(POPULATION_FORMAT_VERSION)
        || metadata.get(2) != Some("")
        || metadata.get(3) != Some("")
    {
        return Err(invalid_population_data(
            &path,
            format!(
                "expected format version {POPULATION_FORMAT_VERSION} metadata row, found {metadata:?}"
            ),
        ));
    }

    let mut entries = Vec::new();
    let mut entity_types = BTreeSet::new();
    let mut file_names = BTreeSet::new();
    for record in records {
        let record = record.map_err(|error| invalid_population_data(&path, error.to_string()))?;
        if record.get(0) != Some("entity") || record.get(1) != Some(POPULATION_FORMAT_VERSION) {
            return Err(invalid_population_data(
                &path,
                format!("invalid entity manifest row: {record:?}"),
            ));
        }
        let entity_type = required_manifest_field(&path, &record, 2, "entity_type")?;
        let file_name = required_manifest_field(&path, &record, 3, "file_name")?;
        validate_file_name(&path, file_name)?;
        if !entity_types.insert(entity_type.to_owned()) {
            return Err(invalid_population_data(
                &path,
                format!("duplicate entity type `{entity_type}`"),
            ));
        }
        if !file_names.insert(file_name.to_owned()) {
            return Err(invalid_population_data(
                &path,
                format!("duplicate entity file name `{file_name}`"),
            ));
        }
        entries.push(ManifestEntry {
            entity_type: entity_type.to_owned(),
            file_name: file_name.to_owned(),
        });
    }
    Ok(entries)
}

fn required_manifest_field<'a>(
    path: &Path,
    record: &'a csv::StringRecord,
    index: usize,
    name: &str,
) -> Result<&'a str, IxaError> {
    record
        .get(index)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid_population_data(path, format!("manifest field `{name}` is empty")))
}

fn validate_file_name(manifest_path: &Path, file_name: &str) -> Result<(), IxaError> {
    let path = Path::new(file_name);
    let mut components = path.components();
    let is_single_normal_component =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    if !is_single_normal_component
        || file_name == MANIFEST_FILE_NAME
        || path.extension().and_then(|extension| extension.to_str()) != Some("csv")
    {
        return Err(invalid_population_data(
            manifest_path,
            format!("unsafe or invalid entity file name `{file_name}`"),
        ));
    }
    Ok(())
}

fn validate_manifest_schema(
    entities: &[EntityPersistenceMetadata],
    manifest: &[ManifestEntry],
) -> Result<(), IxaError> {
    let expected_entity_types: Vec<_> = entities.iter().map(|entry| entry.type_name).collect();
    let actual_entity_types: Vec<_> = manifest
        .iter()
        .map(|entry| entry.entity_type.as_str())
        .collect();
    if actual_entity_types != expected_entity_types {
        return Err(IxaError::PopulationSchemaMismatch {
            reason: format!(
                "expected entity types {expected_entity_types:?}, found {actual_entity_types:?}"
            ),
        });
    }

    for (index, entry) in manifest.iter().enumerate() {
        let expected_file_name = entity_file_name(index);
        if entry.file_name != expected_file_name {
            return Err(IxaError::PopulationSchemaMismatch {
                reason: format!(
                    "entity `{}` should use file `{expected_file_name}`, found `{}`",
                    entry.entity_type, entry.file_name
                ),
            });
        }
    }
    Ok(())
}

fn validate_directory_contents(
    directory: &Path,
    manifest: &[ManifestEntry],
) -> Result<(), IxaError> {
    let mut expected: BTreeSet<String> = manifest
        .iter()
        .map(|entry| entry.file_name.clone())
        .collect();
    expected.insert(MANIFEST_FILE_NAME.to_owned());

    let mut actual = BTreeSet::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            return Err(invalid_population_data(
                directory,
                format!("unexpected non-file entry {:?}", entry.file_name()),
            ));
        }
        let file_name = entry.file_name().into_string().map_err(|file_name| {
            invalid_population_data(
                directory,
                format!("file name is not valid UTF-8: {file_name:?}"),
            )
        })?;
        actual.insert(file_name);
    }

    if actual != expected {
        return Err(invalid_population_data(
            directory,
            format!("expected files {expected:?}, found {actual:?}"),
        ));
    }
    Ok(())
}

fn write_entity<E: Entity>(context: &Context, path: &Path) -> Result<(), IxaError> {
    let properties = persisted_properties::<E>().map_err(|property_type| {
        unsupported_property::<E>(
            property_type,
            "property did not opt into population persistence",
        )
    })?;
    let property_store = context.entity_store.get_property_store::<E>();
    let mut writer = csv::Writer::from_path(path)?;

    let mut header = csv::StringRecord::new();
    header.push_field(ENTITY_ID_HEADER);
    for property in &properties {
        header.push_field(property.type_name);
    }
    writer.write_record(&header)?;

    for entity_id in context.entity_store.get_entity_iterator::<E>() {
        let mut record = csv::StringRecord::new();
        record.push_field(&entity_id.to_string());
        for property in &properties {
            let cell = (property.encode)(property_store, entity_id).map_err(|source| {
                IxaError::InvalidPopulationPropertyValue {
                    entity_type: std::any::type_name::<E>().to_owned(),
                    property_type: property.type_name.to_owned(),
                    entity_id: entity_id.0,
                    reason: source.to_string(),
                }
            })?;
            record.push_field(cell.as_deref().unwrap_or(""));
        }
        writer.write_record(&record)?;
    }
    writer.flush()?;
    Ok(())
}

fn read_entity<E: Entity>(path: &Path) -> Result<Box<dyn PendingEntityImport>, IxaError> {
    let properties = persisted_properties::<E>().map_err(|property_type| {
        unsupported_property::<E>(
            property_type,
            "property did not opt into population persistence",
        )
    })?;
    let mut reader = csv::Reader::from_path(path)
        .map_err(|error| invalid_population_data(path, error.to_string()))?;
    let headers = reader
        .headers()
        .map_err(|error| invalid_population_data(path, error.to_string()))?;
    let expected_headers: Vec<_> = std::iter::once(ENTITY_ID_HEADER)
        .chain(properties.iter().map(|property| property.type_name))
        .collect();
    if headers.iter().ne(expected_headers.iter().copied()) {
        return Err(IxaError::PopulationSchemaMismatch {
            reason: format!(
                "entity `{}` expected columns {expected_headers:?}, found {headers:?}",
                std::any::type_name::<E>()
            ),
        });
    }

    let mut columns: Vec<Vec<String>> = (0..properties.len()).map(|_| Vec::new()).collect();
    let mut entity_count = 0usize;
    for record in reader.records() {
        let record = record.map_err(|error| invalid_population_data(path, error.to_string()))?;
        let entity_id = record
            .get(0)
            .ok_or_else(|| invalid_population_data(path, "entity ID column is missing"))?
            .parse::<usize>()
            .map_err(|error| {
                invalid_population_data(path, format!("invalid entity ID: {error}"))
            })?;
        if entity_id != entity_count {
            return Err(invalid_population_data(
                path,
                format!("expected entity ID {entity_count}, found {entity_id}"),
            ));
        }
        for (column_index, column) in columns.iter_mut().enumerate() {
            let value = record.get(column_index + 1).ok_or_else(|| {
                invalid_population_data(path, format!("column {} is missing", column_index + 1))
            })?;
            column.push(value.to_owned());
        }
        entity_count = entity_count.checked_add(1).ok_or_else(|| {
            invalid_population_data(path, "entity count exceeds the supported range")
        })?;
    }

    let mut pending_properties = Vec::with_capacity(properties.len());
    for (property, column) in properties.into_iter().zip(columns) {
        let values = (property.decode)(&column).map_err(|error| {
            IxaError::InvalidPopulationPropertyValue {
                entity_type: std::any::type_name::<E>().to_owned(),
                property_type: property.type_name.to_owned(),
                entity_id: error.entity_id,
                reason: error.source.to_string(),
            }
        })?;
        pending_properties.push((property, values));
    }

    Ok(Box::new(PendingEntityImportCore::<E> {
        entity_count,
        properties: pending_properties,
    }))
}

fn unsupported_property<E: Entity>(property_type: &str, reason: &str) -> IxaError {
    IxaError::PopulationUnsupportedProperty {
        entity_type: std::any::type_name::<E>().to_owned(),
        property_type: property_type.to_owned(),
        reason: reason.to_owned(),
    }
}

fn invalid_population_data(path: &Path, reason: impl Into<String>) -> IxaError {
    IxaError::InvalidPopulationData {
        path: path.to_path_buf(),
        reason: reason.into(),
    }
}

trait PendingEntityImport {
    fn apply(self: Box<Self>, context: &mut Context);
}

struct PendingEntityImportCore<E: Entity> {
    entity_count: usize,
    properties: Vec<(PropertyPersistence<E>, Box<dyn Any>)>,
}

impl<E: Entity> PendingEntityImport for PendingEntityImportCore<E> {
    fn apply(self: Box<Self>, context: &mut Context) {
        let Self {
            entity_count,
            properties,
        } = *self;

        let property_store = context.entity_store.get_property_store_mut::<E>();
        for (property, values) in properties {
            (property.apply)(property_store, values);
        }
        context
            .entity_store
            .set_entity_count_for_population_import::<E>(entity_count);

        let context_ptr: *const Context = context;
        let property_store = context.entity_store.get_property_store_mut::<E>();
        // SAFETY: This mirrors `ContextEntitiesExt::add_entity`. Index catch-up only reads
        // property values through the shared context while mutating index internals in the
        // exclusively borrowed property store.
        unsafe {
            property_store.index_unindexed_entities_for_all_properties(&*context_ptr);
        }

        for entity_id in 0..entity_count {
            context.emit_event(EntityCreatedEvent::<E>::new(EntityId::new(entity_id)));
        }
    }
}
