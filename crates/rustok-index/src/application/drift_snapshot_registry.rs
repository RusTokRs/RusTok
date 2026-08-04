use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

use rustok_core::ModuleRuntimeExtensions;
use thiserror::Error;

use crate::domain::{SchemaIdentity, SchemaRef};

use super::{
    IndexDriftDependencyFailure, IndexDriftDigestRequest, IndexDriftSnapshotPair,
    IndexDriftSnapshotReader, IndexSchemaSourceCatalog,
};

const MAX_READER_NAME_BYTES: usize = 128;

#[derive(Clone)]
pub struct IndexDriftSnapshotReaderDescriptor {
    owner_module: String,
    reader_name: String,
    schemas: Vec<SchemaRef>,
    reader: Arc<dyn IndexDriftSnapshotReader>,
}

impl IndexDriftSnapshotReaderDescriptor {
    pub fn owner_module(&self) -> &str {
        &self.owner_module
    }

    pub fn reader_name(&self) -> &str {
        &self.reader_name
    }

    pub fn schemas(&self) -> &[SchemaRef] {
        &self.schemas
    }
}

impl fmt::Debug for IndexDriftSnapshotReaderDescriptor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDriftSnapshotReaderDescriptor")
            .field("owner_module", &self.owner_module)
            .field("reader_name", &self.reader_name)
            .field("schemas", &self.schemas)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Default)]
pub struct IndexDriftSnapshotReaderCatalog {
    readers: BTreeMap<String, IndexDriftSnapshotReaderDescriptor>,
    schema_readers: BTreeMap<SchemaRef, String>,
    identity_readers: BTreeMap<SchemaIdentity, (String, String)>,
}

impl fmt::Debug for IndexDriftSnapshotReaderCatalog {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexDriftSnapshotReaderCatalog")
            .field("readers", &self.readers)
            .field("schema_readers", &self.schema_readers)
            .finish()
    }
}

impl IndexDriftSnapshotReaderCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.readers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.readers.is_empty()
    }

    pub fn get(&self, reader_name: &str) -> Option<&IndexDriftSnapshotReaderDescriptor> {
        self.readers.get(reader_name)
    }

    pub fn reader_for_schema(
        &self,
        schema: &SchemaRef,
    ) -> Option<&IndexDriftSnapshotReaderDescriptor> {
        self.schema_readers
            .get(schema)
            .and_then(|reader_name| self.readers.get(reader_name))
    }

    pub fn iter(&self) -> impl Iterator<Item = &IndexDriftSnapshotReaderDescriptor> {
        self.readers.values()
    }

    pub fn register<R>(
        &mut self,
        owner_module: impl Into<String>,
        reader_name: impl Into<String>,
        schemas: impl IntoIterator<Item = SchemaRef>,
        reader: R,
    ) -> Result<(), IndexDriftSnapshotRegistryError>
    where
        R: IndexDriftSnapshotReader + 'static,
    {
        let owner_module = owner_module.into();
        let reader_name = reader_name.into();
        if !valid_owner_module(&owner_module) {
            return Err(IndexDriftSnapshotRegistryError::InvalidOwnerModule(
                owner_module,
            ));
        }
        if !valid_reader_name(&reader_name) {
            return Err(IndexDriftSnapshotRegistryError::InvalidReaderName(
                reader_name,
            ));
        }
        if self.readers.contains_key(&reader_name) {
            return Err(IndexDriftSnapshotRegistryError::DuplicateReaderName(
                reader_name,
            ));
        }

        let mut unique_schemas = BTreeSet::new();
        for schema in schemas {
            if !unique_schemas.insert(schema.clone()) {
                return Err(
                    IndexDriftSnapshotRegistryError::DuplicateSchemaDeclaration {
                        reader_name,
                        schema,
                    },
                );
            }
        }
        if unique_schemas.is_empty() {
            return Err(IndexDriftSnapshotRegistryError::EmptySchemaSet(
                reader_name,
            ));
        }

        for schema in &unique_schemas {
            if let Some(existing_reader) = self.schema_readers.get(schema) {
                return Err(IndexDriftSnapshotRegistryError::SchemaReaderConflict {
                    schema: schema.clone(),
                    existing_reader: existing_reader.clone(),
                    incoming_reader: reader_name.clone(),
                });
            }
            if let Some((existing_owner, existing_reader)) =
                self.identity_readers.get(&schema.identity())
            {
                if existing_owner != &owner_module || existing_reader != &reader_name {
                    return Err(
                        IndexDriftSnapshotRegistryError::SchemaIdentityReaderConflict {
                            identity: schema.identity(),
                            existing_owner: existing_owner.clone(),
                            existing_reader: existing_reader.clone(),
                            incoming_owner: owner_module.clone(),
                            incoming_reader: reader_name.clone(),
                        },
                    );
                }
            }
        }

        let schemas = unique_schemas.into_iter().collect::<Vec<_>>();
        for schema in &schemas {
            self.schema_readers
                .insert(schema.clone(), reader_name.clone());
            self.identity_readers.insert(
                schema.identity(),
                (owner_module.clone(), reader_name.clone()),
            );
        }
        self.readers.insert(
            reader_name.clone(),
            IndexDriftSnapshotReaderDescriptor {
                owner_module,
                reader_name,
                schemas,
                reader: Arc::new(reader),
            },
        );
        Ok(())
    }

    pub fn materialize(
        &self,
        schema_catalog: &IndexSchemaSourceCatalog,
    ) -> Result<SharedIndexDriftSnapshotReaderRegistry, IndexDriftSnapshotRegistryError> {
        for descriptor in self.readers.values() {
            for schema in &descriptor.schemas {
                let published = schema_catalog.get(schema).ok_or_else(|| {
                    IndexDriftSnapshotRegistryError::UnpublishedReaderSchema {
                        reader_name: descriptor.reader_name.clone(),
                        schema: schema.clone(),
                    }
                })?;
                if published.owner_module != descriptor.owner_module {
                    return Err(IndexDriftSnapshotRegistryError::ReaderSchemaOwnerMismatch {
                        reader_name: descriptor.reader_name.clone(),
                        schema: schema.clone(),
                        schema_owner: published.owner_module.clone(),
                        reader_owner: descriptor.owner_module.clone(),
                    });
                }
            }
        }

        Ok(SharedIndexDriftSnapshotReaderRegistry(Arc::new(
            IndexDriftSnapshotReaderRegistry {
                readers: self.readers.clone(),
                schema_readers: self.schema_readers.clone(),
            },
        )))
    }
}

#[derive(Clone)]
struct IndexDriftSnapshotReaderRegistry {
    readers: BTreeMap<String, IndexDriftSnapshotReaderDescriptor>,
    schema_readers: BTreeMap<SchemaRef, String>,
}

#[derive(Clone)]
pub struct SharedIndexDriftSnapshotReaderRegistry(Arc<IndexDriftSnapshotReaderRegistry>);

impl fmt::Debug for SharedIndexDriftSnapshotReaderRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedIndexDriftSnapshotReaderRegistry")
            .field("reader_count", &self.len())
            .finish()
    }
}

impl SharedIndexDriftSnapshotReaderRegistry {
    pub fn len(&self) -> usize {
        self.0.readers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.readers.is_empty()
    }

    pub fn reader_for_schema(
        &self,
        schema: &SchemaRef,
    ) -> Option<&IndexDriftSnapshotReaderDescriptor> {
        self.0
            .schema_readers
            .get(schema)
            .and_then(|reader_name| self.0.readers.get(reader_name))
    }

    pub async fn capture(
        &self,
        request: &IndexDriftDigestRequest,
    ) -> Result<IndexDriftSnapshotPair, IndexDriftSnapshotRegistryError> {
        let descriptor = self
            .reader_for_schema(&request.key().schema)
            .ok_or_else(|| {
                IndexDriftSnapshotRegistryError::UnknownSchemaReader(
                    request.key().schema.clone(),
                )
            })?;
        descriptor
            .reader
            .capture_entity_snapshot(request)
            .await
            .map_err(|failure| IndexDriftSnapshotRegistryError::ReaderFailure {
                reader_name: descriptor.reader_name.clone(),
                failure,
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexDriftSnapshotRegistryError {
    #[error("Index drift snapshot reader owner module is invalid: {0}")]
    InvalidOwnerModule(String),
    #[error("Index drift snapshot reader name is invalid: {0}")]
    InvalidReaderName(String),
    #[error("Index drift snapshot reader {0} declares no schemas")]
    EmptySchemaSet(String),
    #[error("Index drift snapshot reader name is already registered: {0}")]
    DuplicateReaderName(String),
    #[error("Index drift snapshot reader {reader_name} declares schema {schema} more than once")]
    DuplicateSchemaDeclaration {
        reader_name: String,
        schema: SchemaRef,
    },
    #[error(
        "Index schema {schema} has multiple drift snapshot readers: existing={existing_reader}, incoming={incoming_reader}"
    )]
    SchemaReaderConflict {
        schema: SchemaRef,
        existing_reader: String,
        incoming_reader: String,
    },
    #[error(
        "Index schema identity {identity} changes drift snapshot reader: existing={existing_owner}/{existing_reader}, incoming={incoming_owner}/{incoming_reader}"
    )]
    SchemaIdentityReaderConflict {
        identity: SchemaIdentity,
        existing_owner: String,
        existing_reader: String,
        incoming_owner: String,
        incoming_reader: String,
    },
    #[error("Index drift snapshot reader catalog exists without an Index schema source catalog")]
    MissingSchemaCatalog,
    #[error("Index drift snapshot reader {reader_name} declares unpublished schema {schema}")]
    UnpublishedReaderSchema {
        reader_name: String,
        schema: SchemaRef,
    },
    #[error(
        "Index drift snapshot reader {reader_name} owner does not match schema {schema}: schema={schema_owner}, reader={reader_owner}"
    )]
    ReaderSchemaOwnerMismatch {
        reader_name: String,
        schema: SchemaRef,
        schema_owner: String,
        reader_owner: String,
    },
    #[error("No Index drift snapshot reader owns schema {0}")]
    UnknownSchemaReader(SchemaRef),
    #[error("Index drift snapshot reader {reader_name} failed")]
    ReaderFailure {
        reader_name: String,
        #[source]
        failure: IndexDriftDependencyFailure,
    },
}

pub fn register_index_drift_snapshot_reader<R>(
    extensions: &mut ModuleRuntimeExtensions,
    owner_module: impl Into<String>,
    reader_name: impl Into<String>,
    schemas: impl IntoIterator<Item = SchemaRef>,
    reader: R,
) -> Result<(), IndexDriftSnapshotRegistryError>
where
    R: IndexDriftSnapshotReader + 'static,
{
    extensions
        .get_or_insert_with::<IndexDriftSnapshotReaderCatalog, _>(
            IndexDriftSnapshotReaderCatalog::new,
        )
        .register(owner_module, reader_name, schemas, reader)
}

pub fn materialize_index_drift_snapshot_reader_registry(
    extensions: &ModuleRuntimeExtensions,
) -> Result<Option<SharedIndexDriftSnapshotReaderRegistry>, IndexDriftSnapshotRegistryError> {
    let Some(catalog) = extensions.get::<IndexDriftSnapshotReaderCatalog>() else {
        return Ok(None);
    };
    if catalog.is_empty() {
        return Ok(None);
    }
    let schema_catalog = extensions
        .get::<IndexSchemaSourceCatalog>()
        .ok_or(IndexDriftSnapshotRegistryError::MissingSchemaCatalog)?;
    catalog.materialize(schema_catalog).map(Some)
}

fn valid_owner_module(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_READER_NAME_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'-' | b'_')
        })
}

fn valid_reader_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_READER_NAME_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'-' | b'_' | b'.')
        })
}
