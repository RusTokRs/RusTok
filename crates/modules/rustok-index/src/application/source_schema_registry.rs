use std::{collections::BTreeMap, sync::Arc};

use rustok_core::ModuleRuntimeExtensions;
use thiserror::Error;

use crate::domain::{DomainError, IndexSchema, SchemaFingerprint, SchemaIdentity, SchemaRef};

use super::{SchemaRegistry, SchemaRegistryError};

/// One source-module-owned schema contribution to the generic Index engine.
///
/// `owner_module` is the platform module slug that owns publication and replay of
/// the schema. It is intentionally separate from `schema.reference.module`, which
/// is the stable Index contract namespace and may use a different naming grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSchemaSourceDescriptor {
    pub owner_module: String,
    pub schema: IndexSchema,
    pub fingerprint: SchemaFingerprint,
}

/// Deterministic source-owned schema catalog collected during module registration.
///
/// The catalog is mutable only while `ModuleRuntimeExtensions` are being built.
/// The distribution materializes one immutable [`SchemaRegistry`] from the complete
/// catalog after every compiled source module has registered its contracts.
#[derive(Debug, Clone, Default)]
pub struct IndexSchemaSourceCatalog {
    sources: BTreeMap<SchemaRef, IndexSchemaSourceDescriptor>,
}

impl IndexSchemaSourceCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.sources.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    pub fn get(&self, reference: &SchemaRef) -> Option<&IndexSchemaSourceDescriptor> {
        self.sources.get(reference)
    }

    pub fn iter(&self) -> impl Iterator<Item = &IndexSchemaSourceDescriptor> {
        self.sources.values()
    }

    /// Registers exactly one source owner for an exact schema reference and its
    /// complete module/entity identity across versions.
    ///
    /// Duplicate exact references are rejected even when the semantic contract is
    /// equal. Different source owners also cannot split versions of one schema
    /// identity, because that would make replay and drift repair ownership ambiguous.
    pub fn register(
        &mut self,
        owner_module: impl Into<String>,
        schema: IndexSchema,
    ) -> Result<(), IndexSchemaSourceError> {
        let owner_module = owner_module.into();
        validate_owner_module(&owner_module)?;
        let fingerprint = schema.fingerprint()?;
        let reference = schema.reference.clone();

        if let Some(existing) = self.sources.get(&reference) {
            return Err(IndexSchemaSourceError::DuplicateSchemaOwner {
                reference,
                existing_owner: existing.owner_module.clone(),
                incoming_owner: owner_module,
            });
        }

        let identity = reference.identity();
        if let Some(existing) = self.sources.values().find(|existing| {
            existing.schema.reference.identity() == identity
                && existing.owner_module.as_str() != owner_module.as_str()
        }) {
            return Err(IndexSchemaSourceError::SchemaIdentityOwnerConflict {
                identity,
                existing_owner: existing.owner_module.clone(),
                incoming_owner: owner_module,
            });
        }

        self.sources.insert(
            reference,
            IndexSchemaSourceDescriptor {
                owner_module,
                schema,
                fingerprint,
            },
        );
        Ok(())
    }

    /// Builds the complete immutable query registry as one atomic batch.
    ///
    /// Batch materialization allows links to target schemas contributed by another
    /// source module while retaining the registry's validation and monotonicity
    /// guarantees. An empty catalog is never promoted into a query runtime.
    pub fn materialize(&self) -> Result<SharedIndexSchemaRegistry, IndexSchemaSourceError> {
        if self.sources.is_empty() {
            return Err(IndexSchemaSourceError::EmptyCatalog);
        }

        let mut registry = SchemaRegistry::new();
        registry.register_batch(
            self.sources
                .values()
                .map(|descriptor| descriptor.schema.clone()),
        )?;
        Ok(SharedIndexSchemaRegistry::new(Arc::new(registry)))
    }
}

#[derive(Debug, Clone)]
pub struct SharedIndexSchemaRegistry(Arc<SchemaRegistry>);

impl SharedIndexSchemaRegistry {
    fn new(registry: Arc<SchemaRegistry>) -> Self {
        Self(registry)
    }

    pub fn registry(&self) -> &SchemaRegistry {
        self.0.as_ref()
    }

    pub fn shared(&self) -> Arc<SchemaRegistry> {
        self.0.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexSchemaSourceError {
    #[error("Index schema source owner module is invalid: {0}")]
    InvalidOwnerModule(String),
    #[error(
        "Index schema {reference} has multiple source owners: existing={existing_owner}, incoming={incoming_owner}"
    )]
    DuplicateSchemaOwner {
        reference: SchemaRef,
        existing_owner: String,
        incoming_owner: String,
    },
    #[error(
        "Index schema identity {identity} changes source owner across versions: existing={existing_owner}, incoming={incoming_owner}"
    )]
    SchemaIdentityOwnerConflict {
        identity: SchemaIdentity,
        existing_owner: String,
        incoming_owner: String,
    },
    #[error("Index schema source catalog is empty")]
    EmptyCatalog,
    #[error(transparent)]
    InvalidSchema(#[from] DomainError),
    #[error(transparent)]
    Registry(#[from] SchemaRegistryError),
}

/// Publishes one source-owned schema into the module runtime extension catalog.
pub fn register_index_schema_source(
    extensions: &mut ModuleRuntimeExtensions,
    owner_module: impl Into<String>,
    schema: IndexSchema,
) -> Result<(), IndexSchemaSourceError> {
    extensions
        .get_or_insert_with::<IndexSchemaSourceCatalog, _>(IndexSchemaSourceCatalog::new)
        .register(owner_module, schema)
}

/// Materializes a query registry only when at least one source schema exists.
///
/// Returning `None` for an absent or empty catalog prevents a host from presenting
/// an empty registry as a completed query-port composition.
pub fn materialize_index_schema_registry(
    extensions: &ModuleRuntimeExtensions,
) -> Result<Option<SharedIndexSchemaRegistry>, IndexSchemaSourceError> {
    let Some(catalog) = extensions.get::<IndexSchemaSourceCatalog>() else {
        return Ok(None);
    };
    if catalog.is_empty() {
        return Ok(None);
    }
    catalog.materialize().map(Some)
}

fn validate_owner_module(value: &str) -> Result<(), IndexSchemaSourceError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        });
    if valid {
        Ok(())
    } else {
        Err(IndexSchemaSourceError::InvalidOwnerModule(value.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        EntityName, FieldCardinality, FieldName, IndexField, IndexLink, IndexValueType,
        LinkCardinality, LinkName, LocaleMode, ModuleName, SchemaVersion,
    };

    fn reference(module: &str, entity: &str) -> SchemaRef {
        SchemaRef {
            module: ModuleName::new(module).unwrap(),
            entity: EntityName::new(entity).unwrap(),
            version: SchemaVersion::INITIAL,
        }
    }

    fn uuid_field(name: &str) -> IndexField {
        IndexField {
            name: FieldName::new(name).unwrap(),
            value_type: IndexValueType::Uuid,
            cardinality: FieldCardinality::One,
            nullable: false,
            selectable: true,
            filterable: true,
            sortable: true,
        }
    }

    fn target_schema() -> IndexSchema {
        IndexSchema {
            reference: reference("test-owner", "user"),
            locale_mode: LocaleMode::None,
            fields: vec![uuid_field("id")],
            links: Vec::new(),
        }
    }

    fn target_schema_version(version: u32) -> IndexSchema {
        let mut schema = target_schema();
        schema.reference.version = SchemaVersion::new(version);
        schema
    }

    fn source_schema() -> IndexSchema {
        IndexSchema {
            reference: reference("test-owner", "post"),
            locale_mode: LocaleMode::None,
            fields: vec![uuid_field("author_id")],
            links: vec![IndexLink {
                name: LinkName::new("author").unwrap(),
                source_fields: vec![FieldName::new("author_id").unwrap()],
                target_schema: target_schema().reference,
                target_fields: vec![FieldName::new("id").unwrap()],
                cardinality: LinkCardinality::One,
            }],
        }
    }

    #[test]
    fn catalog_materializes_cross_source_links_as_one_batch() {
        let mut catalog = IndexSchemaSourceCatalog::new();
        catalog
            .register("posts", source_schema())
            .expect("source schema should register");
        catalog
            .register("profiles", target_schema())
            .expect("target schema should register");

        let shared = catalog.materialize().expect("catalog should materialize");
        assert_eq!(shared.registry().len(), 2);
        let path = shared
            .registry()
            .resolve_path(&source_schema().reference, &target_schema().reference)
            .expect("cross-source link should resolve");
        assert_eq!(path.len(), 1);
        assert_eq!(path[0].link, LinkName::new("author").unwrap());
        assert_eq!(
            catalog
                .get(&target_schema().reference)
                .expect("target descriptor")
                .owner_module,
            "profiles"
        );
    }

    #[test]
    fn duplicate_schema_reference_rejects_ambiguous_ownership() {
        let mut catalog = IndexSchemaSourceCatalog::new();
        catalog.register("profiles", target_schema()).unwrap();
        let error = catalog
            .register("accounts", target_schema())
            .expect_err("duplicate ownership must fail");

        assert!(matches!(
            error,
            IndexSchemaSourceError::DuplicateSchemaOwner {
                existing_owner,
                incoming_owner,
                ..
            } if existing_owner == "profiles" && incoming_owner == "accounts"
        ));
    }

    #[test]
    fn schema_identity_owner_is_stable_across_versions() {
        let mut catalog = IndexSchemaSourceCatalog::new();
        catalog.register("profiles", target_schema()).unwrap();
        catalog
            .register("profiles", target_schema_version(2))
            .expect("one owner may publish a later schema version");
        let error = catalog
            .register("accounts", target_schema_version(3))
            .expect_err("schema identity ownership must not move across versions");

        assert!(matches!(
            error,
            IndexSchemaSourceError::SchemaIdentityOwnerConflict {
                existing_owner,
                incoming_owner,
                ..
            } if existing_owner == "profiles" && incoming_owner == "accounts"
        ));
    }

    #[test]
    fn extensions_do_not_materialize_an_empty_registry() {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(IndexSchemaSourceCatalog::new());
        assert!(
            materialize_index_schema_registry(&extensions)
                .expect("empty catalog should be accepted")
                .is_none()
        );

        register_index_schema_source(&mut extensions, "profiles", target_schema())
            .expect("source registration should succeed");
        let shared = materialize_index_schema_registry(&extensions)
            .expect("catalog should materialize")
            .expect("non-empty catalog should publish a registry");
        assert!(shared.registry().get(&target_schema().reference).is_some());
    }

    #[test]
    fn owner_module_identity_is_bounded_lowercase_ascii() {
        let mut catalog = IndexSchemaSourceCatalog::new();
        assert!(matches!(
            catalog.register("Profiles", target_schema()),
            Err(IndexSchemaSourceError::InvalidOwnerModule(_))
        ));
        assert!(matches!(
            catalog.register("", target_schema()),
            Err(IndexSchemaSourceError::InvalidOwnerModule(_))
        ));
    }
}
