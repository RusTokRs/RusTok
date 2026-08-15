use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

use async_trait::async_trait;
use rustok_core::ModuleRuntimeExtensions;
use thiserror::Error;

use crate::domain::{EntityKey, SchemaIdentity, SchemaRef};

use super::{IndexSourceFailure, SharedIndexSourceRegistry};

const MAX_PROVIDER_NAME_BYTES: usize = 128;

/// Positive, owner-retained proof that one exact entity is absent at a source version.
///
/// A watermark is not inferred from an empty targeted load. The source owner must retain and
/// return it explicitly, for example from a tombstone or another durable high-watermark record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSourceAbsenceWatermark {
    key: EntityKey,
    source_version: u64,
}

impl IndexSourceAbsenceWatermark {
    pub fn new(key: EntityKey, source_version: u64) -> Result<Self, IndexSourceAbsenceError> {
        validate_key(&key)?;
        if source_version == 0 {
            return Err(IndexSourceAbsenceError::ZeroSourceVersion);
        }
        Ok(Self {
            key,
            source_version,
        })
    }

    pub fn key(&self) -> &EntityKey {
        &self.key
    }

    pub fn source_version(&self) -> u64 {
        self.source_version
    }
}

/// Source-owner adapter for one exact retained absence watermark.
///
/// Returning `None` means that the owner cannot currently prove absence. Callers must remain
/// fail-closed and must not reinterpret `None` or an empty ordinary targeted load as `Missing`.
#[async_trait]
pub trait IndexSourceAbsenceProvider: Send + Sync {
    async fn load_absence_watermark(
        &self,
        key: EntityKey,
    ) -> Result<Option<IndexSourceAbsenceWatermark>, IndexSourceFailure>;
}

#[derive(Clone)]
pub struct IndexSourceAbsenceDescriptor {
    owner_module: String,
    provider_name: String,
    schemas: Vec<SchemaRef>,
    provider: Arc<dyn IndexSourceAbsenceProvider>,
}

impl IndexSourceAbsenceDescriptor {
    pub fn owner_module(&self) -> &str {
        &self.owner_module
    }

    pub fn provider_name(&self) -> &str {
        &self.provider_name
    }

    pub fn schemas(&self) -> &[SchemaRef] {
        &self.schemas
    }
}

impl fmt::Debug for IndexSourceAbsenceDescriptor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexSourceAbsenceDescriptor")
            .field("owner_module", &self.owner_module)
            .field("provider_name", &self.provider_name)
            .field("schemas", &self.schemas)
            .finish_non_exhaustive()
    }
}

/// Mutable composition-time catalog for explicit owner-retained absence providers.
///
/// The catalog is independent from ordinary scan/load adapters so existing sources remain
/// source-compatible. Materialization later verifies that every absence provider has the same
/// owner as the canonical replay source for each exact schema.
#[derive(Clone, Default)]
pub struct IndexSourceAbsenceCatalog {
    providers: BTreeMap<String, IndexSourceAbsenceDescriptor>,
    schema_providers: BTreeMap<SchemaRef, String>,
    identity_providers: BTreeMap<SchemaIdentity, (String, String)>,
}

impl fmt::Debug for IndexSourceAbsenceCatalog {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexSourceAbsenceCatalog")
            .field("providers", &self.providers)
            .field("schema_providers", &self.schema_providers)
            .finish()
    }
}

impl IndexSourceAbsenceCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    pub fn get(&self, provider_name: &str) -> Option<&IndexSourceAbsenceDescriptor> {
        self.providers.get(provider_name)
    }

    pub fn provider_for_schema(&self, schema: &SchemaRef) -> Option<&IndexSourceAbsenceDescriptor> {
        self.schema_providers
            .get(schema)
            .and_then(|provider_name| self.providers.get(provider_name))
    }

    pub fn iter(&self) -> impl Iterator<Item = &IndexSourceAbsenceDescriptor> {
        self.providers.values()
    }

    pub fn register<P>(
        &mut self,
        owner_module: impl Into<String>,
        provider_name: impl Into<String>,
        schemas: impl IntoIterator<Item = SchemaRef>,
        provider: P,
    ) -> Result<(), IndexSourceAbsenceError>
    where
        P: IndexSourceAbsenceProvider + 'static,
    {
        self.register_boxed(owner_module, provider_name, schemas, Arc::new(provider))
    }

    pub fn register_boxed(
        &mut self,
        owner_module: impl Into<String>,
        provider_name: impl Into<String>,
        schemas: impl IntoIterator<Item = SchemaRef>,
        provider: Arc<dyn IndexSourceAbsenceProvider>,
    ) -> Result<(), IndexSourceAbsenceError> {
        let owner_module = owner_module.into();
        let provider_name = provider_name.into();
        if !valid_owner_module(&owner_module) {
            return Err(IndexSourceAbsenceError::InvalidOwnerModule(owner_module));
        }
        if !valid_machine_name(&provider_name) {
            return Err(IndexSourceAbsenceError::InvalidProviderName(provider_name));
        }
        if self.providers.contains_key(&provider_name) {
            return Err(IndexSourceAbsenceError::DuplicateProviderName(
                provider_name,
            ));
        }

        let mut unique_schemas = BTreeSet::new();
        for schema in schemas {
            if !unique_schemas.insert(schema.clone()) {
                return Err(IndexSourceAbsenceError::DuplicateSchemaDeclaration {
                    provider_name,
                    schema,
                });
            }
        }
        if unique_schemas.is_empty() {
            return Err(IndexSourceAbsenceError::EmptySchemaSet(provider_name));
        }

        for schema in &unique_schemas {
            if let Some(existing_provider) = self.schema_providers.get(schema) {
                return Err(IndexSourceAbsenceError::SchemaProviderConflict {
                    schema: schema.clone(),
                    existing_provider: existing_provider.clone(),
                    incoming_provider: provider_name.clone(),
                });
            }
            if let Some((existing_owner, existing_provider)) = self
                .identity_providers
                .get(&schema.identity())
                .filter(|(existing_owner, existing_provider)| {
                    existing_owner.as_str() != owner_module.as_str()
                        || existing_provider.as_str() != provider_name.as_str()
                })
            {
                return Err(IndexSourceAbsenceError::SchemaIdentityProviderConflict {
                    identity: schema.identity(),
                    existing_owner: existing_owner.clone(),
                    existing_provider: existing_provider.clone(),
                    incoming_owner: owner_module.clone(),
                    incoming_provider: provider_name.clone(),
                });
            }
        }

        let schemas = unique_schemas.into_iter().collect::<Vec<_>>();
        for schema in &schemas {
            self.schema_providers
                .insert(schema.clone(), provider_name.clone());
            self.identity_providers.insert(
                schema.identity(),
                (owner_module.clone(), provider_name.clone()),
            );
        }
        self.providers.insert(
            provider_name.clone(),
            IndexSourceAbsenceDescriptor {
                owner_module,
                provider_name,
                schemas,
                provider,
            },
        );
        Ok(())
    }

    pub fn materialize(
        &self,
        sources: &SharedIndexSourceRegistry,
    ) -> Result<SharedIndexSourceAbsenceRegistry, IndexSourceAbsenceError> {
        for descriptor in self.providers.values() {
            for schema in &descriptor.schemas {
                let source = sources.source_for_schema(schema).ok_or_else(|| {
                    IndexSourceAbsenceError::UnpublishedReplaySource {
                        provider_name: descriptor.provider_name.clone(),
                        schema: schema.clone(),
                    }
                })?;
                if source.owner_module() != descriptor.owner_module {
                    return Err(IndexSourceAbsenceError::ReplaySourceOwnerMismatch {
                        provider_name: descriptor.provider_name.clone(),
                        schema: schema.clone(),
                        source_name: source.source_name().to_owned(),
                        source_owner: source.owner_module().to_owned(),
                        provider_owner: descriptor.owner_module.clone(),
                    });
                }
            }
        }

        Ok(SharedIndexSourceAbsenceRegistry(Arc::new(
            IndexSourceAbsenceRegistry {
                providers: self.providers.clone(),
                schema_providers: self.schema_providers.clone(),
            },
        )))
    }
}

#[derive(Clone)]
struct IndexSourceAbsenceRegistry {
    providers: BTreeMap<String, IndexSourceAbsenceDescriptor>,
    schema_providers: BTreeMap<SchemaRef, String>,
}

#[derive(Clone)]
pub struct SharedIndexSourceAbsenceRegistry(Arc<IndexSourceAbsenceRegistry>);

impl fmt::Debug for SharedIndexSourceAbsenceRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedIndexSourceAbsenceRegistry")
            .field("provider_count", &self.len())
            .finish()
    }
}

impl SharedIndexSourceAbsenceRegistry {
    pub fn len(&self) -> usize {
        self.0.providers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.providers.is_empty()
    }

    pub fn get(&self, provider_name: &str) -> Option<&IndexSourceAbsenceDescriptor> {
        self.0.providers.get(provider_name)
    }

    pub fn provider_for_schema(&self, schema: &SchemaRef) -> Option<&IndexSourceAbsenceDescriptor> {
        self.0
            .schema_providers
            .get(schema)
            .and_then(|provider_name| self.0.providers.get(provider_name))
    }

    /// Loads one exact absence watermark without scanning or collecting identifiers.
    pub async fn load(
        &self,
        key: EntityKey,
    ) -> Result<Option<IndexSourceAbsenceWatermark>, IndexSourceAbsenceError> {
        validate_key(&key)?;
        let descriptor = self
            .provider_for_schema(&key.schema)
            .ok_or_else(|| IndexSourceAbsenceError::UnknownSchemaProvider(key.schema.clone()))?;
        let expected = key.clone();
        let watermark = descriptor
            .provider
            .load_absence_watermark(key)
            .await
            .map_err(|failure| IndexSourceAbsenceError::ProviderFailure {
                provider_name: descriptor.provider_name.clone(),
                failure,
            })?;
        if let Some(watermark) = &watermark {
            if watermark.key() != &expected {
                return Err(IndexSourceAbsenceError::WatermarkScopeMismatch);
            }
            if watermark.source_version() == 0 {
                return Err(IndexSourceAbsenceError::ZeroSourceVersion);
            }
        }
        Ok(watermark)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexSourceAbsenceError {
    #[error("Index source absence owner module is invalid: {0}")]
    InvalidOwnerModule(String),
    #[error("Index source absence provider name is invalid: {0}")]
    InvalidProviderName(String),
    #[error("Index source absence provider {0} declares no schemas")]
    EmptySchemaSet(String),
    #[error("Index source absence provider name is already registered: {0}")]
    DuplicateProviderName(String),
    #[error(
        "Index source absence provider {provider_name} declares schema {schema} more than once"
    )]
    DuplicateSchemaDeclaration {
        provider_name: String,
        schema: SchemaRef,
    },
    #[error(
        "Index schema {schema} has multiple absence providers: existing={existing_provider}, incoming={incoming_provider}"
    )]
    SchemaProviderConflict {
        schema: SchemaRef,
        existing_provider: String,
        incoming_provider: String,
    },
    #[error(
        "Index schema identity {identity} changes absence provider: existing={existing_owner}/{existing_provider}, incoming={incoming_owner}/{incoming_provider}"
    )]
    SchemaIdentityProviderConflict {
        identity: SchemaIdentity,
        existing_owner: String,
        existing_provider: String,
        incoming_owner: String,
        incoming_provider: String,
    },
    #[error(
        "Index source absence provider catalog exists without the shared replay source registry"
    )]
    MissingSourceRegistry,
    #[error("Index absence provider {provider_name} has no replay source for schema {schema}")]
    UnpublishedReplaySource {
        provider_name: String,
        schema: SchemaRef,
    },
    #[error(
        "Index absence provider {provider_name} owner does not match replay source {source_name} for {schema}: source={source_owner}, provider={provider_owner}"
    )]
    ReplaySourceOwnerMismatch {
        provider_name: String,
        schema: SchemaRef,
        source_name: String,
        source_owner: String,
        provider_owner: String,
    },
    #[error("No Index absence provider owns schema {0}")]
    UnknownSchemaProvider(SchemaRef),
    #[error("Index absence watermark tenant id cannot be nil")]
    NilTenantId,
    #[error("Index absence watermark entity id cannot be nil")]
    NilEntityId,
    #[error("Index absence watermark schema version must be positive")]
    ZeroSchemaVersion,
    #[error("Index absence watermark source version must be positive")]
    ZeroSourceVersion,
    #[error("Index absence provider returned a watermark for another entity key")]
    WatermarkScopeMismatch,
    #[error("Index source absence provider {provider_name} failed")]
    ProviderFailure {
        provider_name: String,
        #[source]
        failure: IndexSourceFailure,
    },
}

pub fn register_index_source_absence_provider<P>(
    extensions: &mut ModuleRuntimeExtensions,
    owner_module: impl Into<String>,
    provider_name: impl Into<String>,
    schemas: impl IntoIterator<Item = SchemaRef>,
    provider: P,
) -> Result<(), IndexSourceAbsenceError>
where
    P: IndexSourceAbsenceProvider + 'static,
{
    extensions
        .get_or_insert_with::<IndexSourceAbsenceCatalog, _>(IndexSourceAbsenceCatalog::new)
        .register(owner_module, provider_name, schemas, provider)
}

pub fn materialize_index_source_absence_registry(
    extensions: &ModuleRuntimeExtensions,
) -> Result<Option<SharedIndexSourceAbsenceRegistry>, IndexSourceAbsenceError> {
    let Some(catalog) = extensions.get::<IndexSourceAbsenceCatalog>() else {
        return Ok(None);
    };
    if catalog.is_empty() {
        return Ok(None);
    }
    let sources = extensions
        .get::<SharedIndexSourceRegistry>()
        .ok_or(IndexSourceAbsenceError::MissingSourceRegistry)?;
    catalog.materialize(sources).map(Some)
}

fn validate_key(key: &EntityKey) -> Result<(), IndexSourceAbsenceError> {
    if key.tenant_id.is_nil() {
        return Err(IndexSourceAbsenceError::NilTenantId);
    }
    if key.entity_id.is_nil() {
        return Err(IndexSourceAbsenceError::NilEntityId);
    }
    if key.schema.version.get() == 0 {
        return Err(IndexSourceAbsenceError::ZeroSchemaVersion);
    }
    Ok(())
}

fn valid_owner_module(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PROVIDER_NAME_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

fn valid_machine_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PROVIDER_NAME_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use uuid::Uuid;

    use super::*;
    use crate::{
        EntityName, FieldCardinality, FieldName, IndexField, IndexSchema, IndexSchemaSourceCatalog,
        IndexSource, IndexSourceCatalog, IndexSourceLoadBatch, IndexSourceLoadRequest,
        IndexSourcePage, IndexSourceScanRequest, IndexValueType, LocaleMode, ModuleName,
        SchemaVersion,
    };

    struct NoopSource;

    #[async_trait]
    impl IndexSource for NoopSource {
        async fn scan(
            &self,
            request: IndexSourceScanRequest,
        ) -> Result<IndexSourcePage, IndexSourceFailure> {
            Ok(IndexSourcePage::new(&request, Vec::new(), None)
                .expect("empty final page should be valid"))
        }

        async fn load(
            &self,
            request: IndexSourceLoadRequest,
        ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
            Ok(IndexSourceLoadBatch::new(&request, Vec::new())
                .expect("empty targeted load should be valid"))
        }
    }

    #[derive(Clone)]
    struct FixedProvider {
        watermark: Option<IndexSourceAbsenceWatermark>,
    }

    #[async_trait]
    impl IndexSourceAbsenceProvider for FixedProvider {
        async fn load_absence_watermark(
            &self,
            _key: EntityKey,
        ) -> Result<Option<IndexSourceAbsenceWatermark>, IndexSourceFailure> {
            Ok(self.watermark.clone())
        }
    }

    fn schema_ref(version: u32) -> SchemaRef {
        SchemaRef {
            module: ModuleName::new("rustok-product").unwrap(),
            entity: EntityName::new("product").unwrap(),
            version: SchemaVersion::new(version),
        }
    }

    fn schema(version: u32) -> IndexSchema {
        IndexSchema {
            reference: schema_ref(version),
            locale_mode: LocaleMode::None,
            fields: vec![IndexField {
                name: FieldName::new("id").unwrap(),
                value_type: IndexValueType::Uuid,
                cardinality: FieldCardinality::One,
                nullable: false,
                selectable: true,
                filterable: true,
                sortable: true,
            }],
            links: Vec::new(),
        }
    }

    fn key() -> EntityKey {
        EntityKey {
            tenant_id: Uuid::new_v4(),
            schema: schema_ref(1),
            entity_id: Uuid::new_v4(),
            locale: None,
        }
    }

    fn sources(owner: &str) -> SharedIndexSourceRegistry {
        let mut schemas = IndexSchemaSourceCatalog::new();
        schemas.register(owner, schema(1)).unwrap();
        let mut sources = IndexSourceCatalog::new();
        sources
            .register(owner, "product-primary", [schema_ref(1)], NoopSource)
            .unwrap();
        sources.materialize(&schemas).unwrap()
    }

    #[test]
    fn watermark_requires_exact_positive_identity_and_version() {
        let key = key();
        let watermark = IndexSourceAbsenceWatermark::new(key.clone(), 7).unwrap();
        assert_eq!(watermark.key(), &key);
        assert_eq!(watermark.source_version(), 7);
        assert!(matches!(
            IndexSourceAbsenceWatermark::new(key, 0),
            Err(IndexSourceAbsenceError::ZeroSourceVersion)
        ));
    }

    #[test]
    fn provider_owner_must_match_the_canonical_replay_source() {
        let mut catalog = IndexSourceAbsenceCatalog::new();
        catalog
            .register(
                "catalog",
                "product-absence",
                [schema_ref(1)],
                FixedProvider { watermark: None },
            )
            .unwrap();
        let error = catalog
            .materialize(&sources("product"))
            .expect_err("cross-owner absence proof must fail closed");
        assert!(matches!(
            error,
            IndexSourceAbsenceError::ReplaySourceOwnerMismatch { .. }
        ));
    }

    #[test]
    fn schema_identity_cannot_move_between_absence_providers() {
        let mut catalog = IndexSourceAbsenceCatalog::new();
        catalog
            .register(
                "product",
                "product-absence",
                [schema_ref(1)],
                FixedProvider { watermark: None },
            )
            .unwrap();
        let error = catalog
            .register(
                "product",
                "product-absence-v2",
                [schema_ref(2)],
                FixedProvider { watermark: None },
            )
            .expect_err("one schema identity must keep one provider");
        assert!(matches!(
            error,
            IndexSourceAbsenceError::SchemaIdentityProviderConflict { .. }
        ));
    }

    #[tokio::test]
    async fn shared_registry_returns_only_the_exact_registered_watermark() {
        let key = key();
        let watermark = IndexSourceAbsenceWatermark::new(key.clone(), 11).unwrap();
        let mut catalog = IndexSourceAbsenceCatalog::new();
        catalog
            .register(
                "product",
                "product-absence",
                [schema_ref(1)],
                FixedProvider {
                    watermark: Some(watermark.clone()),
                },
            )
            .unwrap();
        let shared = catalog.materialize(&sources("product")).unwrap();
        assert_eq!(shared.load(key).await.unwrap(), Some(watermark));
    }

    #[tokio::test]
    async fn cross_scope_provider_result_is_rejected() {
        let requested = key();
        let other = key();
        let mut catalog = IndexSourceAbsenceCatalog::new();
        catalog
            .register(
                "product",
                "product-absence",
                [schema_ref(1)],
                FixedProvider {
                    watermark: Some(IndexSourceAbsenceWatermark::new(other, 3).unwrap()),
                },
            )
            .unwrap();
        let shared = catalog.materialize(&sources("product")).unwrap();
        assert!(matches!(
            shared.load(requested).await,
            Err(IndexSourceAbsenceError::WatermarkScopeMismatch)
        ));
    }

    #[test]
    fn extension_materialization_requires_the_frozen_replay_registry() {
        let mut extensions = ModuleRuntimeExtensions::default();
        register_index_source_absence_provider(
            &mut extensions,
            "product",
            "product-absence",
            [schema_ref(1)],
            FixedProvider { watermark: None },
        )
        .unwrap();
        assert!(matches!(
            materialize_index_source_absence_registry(&extensions),
            Err(IndexSourceAbsenceError::MissingSourceRegistry)
        ));

        extensions.insert(sources("product"));
        assert!(
            materialize_index_source_absence_registry(&extensions)
                .unwrap()
                .is_some()
        );
    }
}
