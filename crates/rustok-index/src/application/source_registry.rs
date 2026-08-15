use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

use async_trait::async_trait;
use rustok_core::ModuleRuntimeExtensions;
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use serde_json::Value as JsonValue;
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{EntityKey, IndexMutation, LocaleKey, SchemaIdentity, SchemaRef};

use super::IndexSchemaSourceCatalog;

const MAX_SOURCE_NAME_BYTES: usize = 128;
const MAX_CURSOR_BYTES: usize = 8 * 1024;
const MAX_SCAN_BATCH_SIZE: usize = 1_000;
const MAX_LOAD_KEYS: usize = 256;
const MAX_FAILURE_CODE_BYTES: usize = 128;

/// Opaque, source-owned continuation state with a stable JSON persistence boundary.
///
/// Construction and deserialization both reject JSON null and encoded values above
/// 8 KiB so a durable worker cannot bypass the bound by restoring a checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct IndexSourceCursor(JsonValue);

impl IndexSourceCursor {
    pub fn new(value: JsonValue) -> Result<Self, IndexSourceError> {
        if value.is_null() {
            return Err(IndexSourceError::NullCursor);
        }
        let encoded =
            serde_json::to_vec(&value).map_err(|_| IndexSourceError::CursorSerializationFailed)?;
        if encoded.len() > MAX_CURSOR_BYTES {
            return Err(IndexSourceError::CursorTooLarge {
                actual: encoded.len(),
                max: MAX_CURSOR_BYTES,
            });
        }
        Ok(Self(value))
    }

    pub fn value(&self) -> &JsonValue {
        &self.0
    }

    pub fn into_value(self) -> JsonValue {
        self.0
    }
}

impl<'de> Deserialize<'de> for IndexSourceCursor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSourceScanRequest {
    tenant_id: Uuid,
    schema: SchemaRef,
    locale: Option<LocaleKey>,
    cursor: Option<IndexSourceCursor>,
    limit: usize,
}

impl IndexSourceScanRequest {
    /// Construct the existing schema-wide scan request.
    pub fn new(
        tenant_id: Uuid,
        schema: SchemaRef,
        cursor: Option<IndexSourceCursor>,
        limit: usize,
    ) -> Result<Self, IndexSourceError> {
        Self::new_scoped(tenant_id, schema, None, cursor, limit)
    }

    /// Construct an exact-locale scan request.
    ///
    /// The caller owns schema `LocaleMode` admission. Once this request reaches a source, the
    /// returned page is fail-closed: every mutation must carry exactly this canonical locale.
    pub fn for_locale(
        tenant_id: Uuid,
        schema: SchemaRef,
        locale: LocaleKey,
        cursor: Option<IndexSourceCursor>,
        limit: usize,
    ) -> Result<Self, IndexSourceError> {
        Self::new_scoped(tenant_id, schema, Some(locale), cursor, limit)
    }

    fn new_scoped(
        tenant_id: Uuid,
        schema: SchemaRef,
        locale: Option<LocaleKey>,
        cursor: Option<IndexSourceCursor>,
        limit: usize,
    ) -> Result<Self, IndexSourceError> {
        if tenant_id.is_nil() {
            return Err(IndexSourceError::NilTenantId);
        }
        if !(1..=MAX_SCAN_BATCH_SIZE).contains(&limit) {
            return Err(IndexSourceError::InvalidScanLimit {
                actual: limit,
                max: MAX_SCAN_BATCH_SIZE,
            });
        }
        Ok(Self {
            tenant_id,
            schema,
            locale,
            cursor,
            limit,
        })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    pub fn locale(&self) -> Option<&LocaleKey> {
        self.locale.as_ref()
    }

    pub fn cursor(&self) -> Option<&IndexSourceCursor> {
        self.cursor.as_ref()
    }

    pub fn limit(&self) -> usize {
        self.limit
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSourceLoadRequest {
    tenant_id: Uuid,
    schema: SchemaRef,
    keys: Vec<EntityKey>,
}

impl IndexSourceLoadRequest {
    pub fn new(keys: Vec<EntityKey>) -> Result<Self, IndexSourceError> {
        let Some(first) = keys.first() else {
            return Err(IndexSourceError::EmptyLoadKeys);
        };
        if keys.len() > MAX_LOAD_KEYS {
            return Err(IndexSourceError::TooManyLoadKeys {
                actual: keys.len(),
                max: MAX_LOAD_KEYS,
            });
        }
        if first.tenant_id.is_nil() {
            return Err(IndexSourceError::NilTenantId);
        }

        let tenant_id = first.tenant_id;
        let schema = first.schema.clone();
        let mut unique = BTreeSet::new();
        for (position, key) in keys.iter().enumerate() {
            if key.tenant_id != tenant_id || key.schema != schema {
                return Err(IndexSourceError::MixedLoadScope { position });
            }
            if !unique.insert(key.clone()) {
                return Err(IndexSourceError::DuplicateLoadKey { position });
            }
        }

        Ok(Self {
            tenant_id,
            schema,
            keys,
        })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    pub fn keys(&self) -> &[EntityKey] {
        &self.keys
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSourcePage {
    mutations: Vec<IndexMutation>,
    next_cursor: Option<IndexSourceCursor>,
}

impl IndexSourcePage {
    pub fn new(
        request: &IndexSourceScanRequest,
        mutations: Vec<IndexMutation>,
        next_cursor: Option<IndexSourceCursor>,
    ) -> Result<Self, IndexSourceError> {
        let page = Self {
            mutations,
            next_cursor,
        };
        validate_scan_page(request, &page)?;
        Ok(page)
    }

    pub fn mutations(&self) -> &[IndexMutation] {
        &self.mutations
    }

    pub fn next_cursor(&self) -> Option<&IndexSourceCursor> {
        self.next_cursor.as_ref()
    }

    pub fn is_complete(&self) -> bool {
        self.next_cursor.is_none()
    }

    pub fn into_parts(self) -> (Vec<IndexMutation>, Option<IndexSourceCursor>) {
        (self.mutations, self.next_cursor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSourceLoadBatch {
    mutations: Vec<IndexMutation>,
}

impl IndexSourceLoadBatch {
    pub fn new(
        request: &IndexSourceLoadRequest,
        mutations: Vec<IndexMutation>,
    ) -> Result<Self, IndexSourceError> {
        let batch = Self { mutations };
        validate_load_batch(request, &batch)?;
        Ok(batch)
    }

    pub fn mutations(&self) -> &[IndexMutation] {
        &self.mutations
    }

    pub fn into_mutations(self) -> Vec<IndexMutation> {
        self.mutations
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexSourceFailureKind {
    Retryable,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("Index source reported a {kind:?} failure ({code})")]
pub struct IndexSourceFailure {
    kind: IndexSourceFailureKind,
    code: String,
}

impl IndexSourceFailure {
    pub fn retryable(code: impl Into<String>) -> Result<Self, IndexSourceError> {
        Self::new(IndexSourceFailureKind::Retryable, code)
    }

    pub fn permanent(code: impl Into<String>) -> Result<Self, IndexSourceError> {
        Self::new(IndexSourceFailureKind::Permanent, code)
    }

    fn new(
        kind: IndexSourceFailureKind,
        code: impl Into<String>,
    ) -> Result<Self, IndexSourceError> {
        let code = code.into();
        if !valid_machine_name(&code, MAX_FAILURE_CODE_BYTES) {
            return Err(IndexSourceError::InvalidFailureCode(code));
        }
        Ok(Self { kind, code })
    }

    pub fn kind(&self) -> IndexSourceFailureKind {
        self.kind
    }

    pub fn is_retryable(&self) -> bool {
        self.kind == IndexSourceFailureKind::Retryable
    }

    pub fn code(&self) -> &str {
        &self.code
    }
}

#[async_trait]
pub trait IndexSource: Send + Sync {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure>;

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure>;
}

#[derive(Clone)]
pub struct IndexSourceDescriptor {
    owner_module: String,
    source_name: String,
    schemas: Vec<SchemaRef>,
    source: Arc<dyn IndexSource>,
}

impl IndexSourceDescriptor {
    pub fn owner_module(&self) -> &str {
        &self.owner_module
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn schemas(&self) -> &[SchemaRef] {
        &self.schemas
    }
}

impl fmt::Debug for IndexSourceDescriptor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexSourceDescriptor")
            .field("owner_module", &self.owner_module)
            .field("source_name", &self.source_name)
            .field("schemas", &self.schemas)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Default)]
pub struct IndexSourceCatalog {
    sources: BTreeMap<String, IndexSourceDescriptor>,
    schema_sources: BTreeMap<SchemaRef, String>,
    identity_sources: BTreeMap<SchemaIdentity, (String, String)>,
}

impl fmt::Debug for IndexSourceCatalog {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexSourceCatalog")
            .field("sources", &self.sources)
            .field("schema_sources", &self.schema_sources)
            .finish()
    }
}

impl IndexSourceCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.sources.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    pub fn get(&self, source_name: &str) -> Option<&IndexSourceDescriptor> {
        self.sources.get(source_name)
    }

    pub fn source_for_schema(&self, schema: &SchemaRef) -> Option<&IndexSourceDescriptor> {
        self.schema_sources
            .get(schema)
            .and_then(|source_name| self.sources.get(source_name))
    }

    pub fn iter(&self) -> impl Iterator<Item = &IndexSourceDescriptor> {
        self.sources.values()
    }

    pub fn register<S>(
        &mut self,
        owner_module: impl Into<String>,
        source_name: impl Into<String>,
        schemas: impl IntoIterator<Item = SchemaRef>,
        source: S,
    ) -> Result<(), IndexSourceError>
    where
        S: IndexSource + 'static,
    {
        self.register_boxed(owner_module, source_name, schemas, Arc::new(source))
    }

    pub fn register_boxed(
        &mut self,
        owner_module: impl Into<String>,
        source_name: impl Into<String>,
        schemas: impl IntoIterator<Item = SchemaRef>,
        source: Arc<dyn IndexSource>,
    ) -> Result<(), IndexSourceError> {
        let owner_module = owner_module.into();
        let source_name = source_name.into();
        if !valid_owner_module(&owner_module) {
            return Err(IndexSourceError::InvalidOwnerModule(owner_module));
        }
        if !valid_machine_name(&source_name, MAX_SOURCE_NAME_BYTES) {
            return Err(IndexSourceError::InvalidSourceName(source_name));
        }
        if self.sources.contains_key(&source_name) {
            return Err(IndexSourceError::DuplicateSourceName(source_name));
        }

        let mut unique_schemas = BTreeSet::new();
        for schema in schemas {
            if !unique_schemas.insert(schema.clone()) {
                return Err(IndexSourceError::DuplicateSchemaDeclaration {
                    source_name,
                    schema,
                });
            }
        }
        if unique_schemas.is_empty() {
            return Err(IndexSourceError::EmptySchemaSet(source_name));
        }

        for schema in &unique_schemas {
            if let Some(existing_source) = self.schema_sources.get(schema) {
                return Err(IndexSourceError::SchemaSourceConflict {
                    schema: schema.clone(),
                    existing_source: existing_source.clone(),
                    incoming_source: source_name.clone(),
                });
            }
            if let Some((existing_owner, existing_source)) = self
                .identity_sources
                .get(&schema.identity())
                .filter(|(existing_owner, existing_source)| {
                    existing_owner.as_str() != owner_module.as_str()
                        || existing_source.as_str() != source_name.as_str()
                })
            {
                return Err(IndexSourceError::SchemaIdentitySourceConflict {
                    identity: schema.identity(),
                    existing_owner: existing_owner.clone(),
                    existing_source: existing_source.clone(),
                    incoming_owner: owner_module.clone(),
                    incoming_source: source_name.clone(),
                });
            }
        }

        let schemas = unique_schemas.into_iter().collect::<Vec<_>>();
        for schema in &schemas {
            self.schema_sources
                .insert(schema.clone(), source_name.clone());
            self.identity_sources.insert(
                schema.identity(),
                (owner_module.clone(), source_name.clone()),
            );
        }
        self.sources.insert(
            source_name.clone(),
            IndexSourceDescriptor {
                owner_module,
                source_name,
                schemas,
                source,
            },
        );
        Ok(())
    }

    pub fn materialize(
        &self,
        schema_catalog: &IndexSchemaSourceCatalog,
    ) -> Result<SharedIndexSourceRegistry, IndexSourceError> {
        for descriptor in self.sources.values() {
            for schema in &descriptor.schemas {
                let published = schema_catalog.get(schema).ok_or_else(|| {
                    IndexSourceError::UnpublishedSourceSchema {
                        source_name: descriptor.source_name.clone(),
                        schema: schema.clone(),
                    }
                })?;
                if published.owner_module != descriptor.owner_module {
                    return Err(IndexSourceError::SourceSchemaOwnerMismatch {
                        source_name: descriptor.source_name.clone(),
                        schema: schema.clone(),
                        schema_owner: published.owner_module.clone(),
                        source_owner: descriptor.owner_module.clone(),
                    });
                }
            }
        }

        Ok(SharedIndexSourceRegistry(Arc::new(IndexSourceRegistry {
            sources: self.sources.clone(),
            schema_sources: self.schema_sources.clone(),
        })))
    }
}

#[derive(Clone)]
struct IndexSourceRegistry {
    sources: BTreeMap<String, IndexSourceDescriptor>,
    schema_sources: BTreeMap<SchemaRef, String>,
}

#[derive(Clone)]
pub struct SharedIndexSourceRegistry(Arc<IndexSourceRegistry>);

impl fmt::Debug for SharedIndexSourceRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedIndexSourceRegistry")
            .field("source_count", &self.len())
            .finish()
    }
}

impl SharedIndexSourceRegistry {
    pub fn len(&self) -> usize {
        self.0.sources.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.sources.is_empty()
    }

    pub fn get(&self, source_name: &str) -> Option<&IndexSourceDescriptor> {
        self.0.sources.get(source_name)
    }

    pub fn source_for_schema(&self, schema: &SchemaRef) -> Option<&IndexSourceDescriptor> {
        self.0
            .schema_sources
            .get(schema)
            .and_then(|source_name| self.0.sources.get(source_name))
    }

    pub async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceError> {
        let descriptor = self
            .source_for_schema(request.schema())
            .ok_or_else(|| IndexSourceError::UnknownSchemaSource(request.schema().clone()))?;
        let page = descriptor
            .source
            .scan(request.clone())
            .await
            .map_err(|failure| IndexSourceError::SourceFailure {
                source_name: descriptor.source_name.clone(),
                failure,
            })?;
        validate_scan_page(&request, &page)?;
        Ok(page)
    }

    pub async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceError> {
        let descriptor = self
            .source_for_schema(request.schema())
            .ok_or_else(|| IndexSourceError::UnknownSchemaSource(request.schema().clone()))?;
        let batch = descriptor
            .source
            .load(request.clone())
            .await
            .map_err(|failure| IndexSourceError::SourceFailure {
                source_name: descriptor.source_name.clone(),
                failure,
            })?;
        validate_load_batch(&request, &batch)?;
        Ok(batch)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexSourceError {
    #[error("Index source owner module is invalid: {0}")]
    InvalidOwnerModule(String),
    #[error("Index source name is invalid: {0}")]
    InvalidSourceName(String),
    #[error("Index source failure code is invalid: {0}")]
    InvalidFailureCode(String),
    #[error("Index source {0} declares no schemas")]
    EmptySchemaSet(String),
    #[error("Index source name is already registered: {0}")]
    DuplicateSourceName(String),
    #[error("Index source {source_name} declares schema {schema} more than once")]
    DuplicateSchemaDeclaration {
        source_name: String,
        schema: SchemaRef,
    },
    #[error(
        "Index schema {schema} has multiple replay sources: existing={existing_source}, incoming={incoming_source}"
    )]
    SchemaSourceConflict {
        schema: SchemaRef,
        existing_source: String,
        incoming_source: String,
    },
    #[error(
        "Index schema identity {identity} changes replay source: existing={existing_owner}/{existing_source}, incoming={incoming_owner}/{incoming_source}"
    )]
    SchemaIdentitySourceConflict {
        identity: SchemaIdentity,
        existing_owner: String,
        existing_source: String,
        incoming_owner: String,
        incoming_source: String,
    },
    #[error("Index source catalog exists without an Index schema source catalog")]
    MissingSchemaCatalog,
    #[error("Index source {source_name} declares unpublished schema {schema}")]
    UnpublishedSourceSchema {
        source_name: String,
        schema: SchemaRef,
    },
    #[error(
        "Index source {source_name} owner does not match schema {schema}: schema={schema_owner}, source={source_owner}"
    )]
    SourceSchemaOwnerMismatch {
        source_name: String,
        schema: SchemaRef,
        schema_owner: String,
        source_owner: String,
    },
    #[error("Index source cursor cannot be JSON null")]
    NullCursor,
    #[error("Index source cursor serialization failed")]
    CursorSerializationFailed,
    #[error("Index source cursor is too large: actual={actual}, max={max}")]
    CursorTooLarge { actual: usize, max: usize },
    #[error("Index source request tenant cannot be nil")]
    NilTenantId,
    #[error("Index source scan limit is invalid: actual={actual}, max={max}")]
    InvalidScanLimit { actual: usize, max: usize },
    #[error("Index source targeted load requires at least one entity key")]
    EmptyLoadKeys,
    #[error("Index source targeted load has too many keys: actual={actual}, max={max}")]
    TooManyLoadKeys { actual: usize, max: usize },
    #[error("Index source targeted load key at position {position} has another tenant or schema")]
    MixedLoadScope { position: usize },
    #[error("Index source targeted load key at position {position} is duplicated")]
    DuplicateLoadKey { position: usize },
    #[error("No Index source owns schema {0}")]
    UnknownSchemaSource(SchemaRef),
    #[error("Index source scan batch exceeds its request: actual={actual}, max={max}")]
    ScanBatchTooLarge { actual: usize, max: usize },
    #[error(
        "Index source scan mutation at position {position} escapes the requested tenant/schema"
    )]
    ScanMutationScopeMismatch { position: usize },
    #[error("Index source scan mutation at position {position} escapes the requested locale")]
    ScanMutationLocaleMismatch { position: usize },
    #[error("Index source scan mutation at position {position} duplicates an entity key")]
    DuplicateScanMutationKey { position: usize },
    #[error("Index source scan returned an empty page with a continuation cursor")]
    EmptyScanContinuation,
    #[error("Index source scan continuation cursor did not advance")]
    ScanCursorDidNotAdvance,
    #[error("Index source targeted load returned too many mutations: actual={actual}, max={max}")]
    LoadBatchTooLarge { actual: usize, max: usize },
    #[error("Index source targeted load mutation at position {position} was not requested")]
    LoadMutationNotRequested { position: usize },
    #[error("Index source targeted load mutation at position {position} duplicates an entity key")]
    DuplicateLoadMutationKey { position: usize },
    #[error("Index source {source_name} failed")]
    SourceFailure {
        source_name: String,
        #[source]
        failure: IndexSourceFailure,
    },
}

pub fn register_index_source<S>(
    extensions: &mut ModuleRuntimeExtensions,
    owner_module: impl Into<String>,
    source_name: impl Into<String>,
    schemas: impl IntoIterator<Item = SchemaRef>,
    source: S,
) -> Result<(), IndexSourceError>
where
    S: IndexSource + 'static,
{
    extensions
        .get_or_insert_with::<IndexSourceCatalog, _>(IndexSourceCatalog::new)
        .register(owner_module, source_name, schemas, source)
}

pub fn materialize_index_source_registry(
    extensions: &ModuleRuntimeExtensions,
) -> Result<Option<SharedIndexSourceRegistry>, IndexSourceError> {
    let Some(catalog) = extensions.get::<IndexSourceCatalog>() else {
        return Ok(None);
    };
    if catalog.is_empty() {
        return Ok(None);
    }
    let schema_catalog = extensions
        .get::<IndexSchemaSourceCatalog>()
        .ok_or(IndexSourceError::MissingSchemaCatalog)?;
    catalog.materialize(schema_catalog).map(Some)
}

fn validate_scan_page(
    request: &IndexSourceScanRequest,
    page: &IndexSourcePage,
) -> Result<(), IndexSourceError> {
    if page.mutations.len() > request.limit {
        return Err(IndexSourceError::ScanBatchTooLarge {
            actual: page.mutations.len(),
            max: request.limit,
        });
    }

    let mut keys = BTreeSet::new();
    for (position, mutation) in page.mutations.iter().enumerate() {
        let key = mutation.key();
        if key.tenant_id != request.tenant_id || key.schema != request.schema {
            return Err(IndexSourceError::ScanMutationScopeMismatch { position });
        }
        if let Some(locale) = request.locale.as_ref()
            && key.locale.as_ref() != Some(locale)
        {
            return Err(IndexSourceError::ScanMutationLocaleMismatch { position });
        }
        if !keys.insert(key.clone()) {
            return Err(IndexSourceError::DuplicateScanMutationKey { position });
        }
    }

    if let Some(next_cursor) = &page.next_cursor {
        if page.mutations.is_empty() {
            return Err(IndexSourceError::EmptyScanContinuation);
        }
        if request.cursor.as_ref() == Some(next_cursor) {
            return Err(IndexSourceError::ScanCursorDidNotAdvance);
        }
    }
    Ok(())
}

fn validate_load_batch(
    request: &IndexSourceLoadRequest,
    batch: &IndexSourceLoadBatch,
) -> Result<(), IndexSourceError> {
    if batch.mutations.len() > request.keys.len() {
        return Err(IndexSourceError::LoadBatchTooLarge {
            actual: batch.mutations.len(),
            max: request.keys.len(),
        });
    }

    let requested = request.keys.iter().cloned().collect::<BTreeSet<_>>();
    let mut returned = BTreeSet::new();
    for (position, mutation) in batch.mutations.iter().enumerate() {
        let key = mutation.key();
        if !requested.contains(key) {
            return Err(IndexSourceError::LoadMutationNotRequested { position });
        }
        if !returned.insert(key.clone()) {
            return Err(IndexSourceError::DuplicateLoadMutationKey { position });
        }
    }
    Ok(())
}

fn valid_owner_module(value: &str) -> bool {
    valid_bounded_ascii(value, MAX_SOURCE_NAME_BYTES, false)
}

fn valid_machine_name(value: &str, max_bytes: usize) -> bool {
    valid_bounded_ascii(value, max_bytes, true)
}

fn valid_bounded_ascii(value: &str, max_bytes: usize, allow_dot: bool) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'-' | b'_')
                || (allow_dot && byte == b'.')
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        EntityName, FieldCardinality, FieldName, IndexField, IndexSchema, IndexValueType,
        LocaleMode, ModuleName, SchemaVersion,
    };

    struct NoopSource;

    #[async_trait]
    impl IndexSource for NoopSource {
        async fn scan(
            &self,
            request: IndexSourceScanRequest,
        ) -> Result<IndexSourcePage, IndexSourceFailure> {
            Ok(IndexSourcePage::new(&request, Vec::new(), None).expect("valid empty final page"))
        }

        async fn load(
            &self,
            request: IndexSourceLoadRequest,
        ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
            Ok(IndexSourceLoadBatch::new(&request, Vec::new()).expect("valid empty load"))
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

    #[test]
    fn source_materialization_requires_exact_schema_owner() {
        let mut schemas = IndexSchemaSourceCatalog::new();
        schemas.register("product", schema(1)).unwrap();

        let mut sources = IndexSourceCatalog::new();
        sources
            .register("product", "product-primary", [schema_ref(1)], NoopSource)
            .unwrap();
        let shared = sources.materialize(&schemas).unwrap();

        assert_eq!(shared.len(), 1);
        assert_eq!(
            shared
                .source_for_schema(&schema_ref(1))
                .expect("source")
                .source_name(),
            "product-primary"
        );
    }

    #[test]
    fn schema_identity_cannot_move_between_replay_sources() {
        let mut sources = IndexSourceCatalog::new();
        sources
            .register("product", "product-primary", [schema_ref(1)], NoopSource)
            .unwrap();
        let error = sources
            .register("product", "product-secondary", [schema_ref(2)], NoopSource)
            .expect_err("schema identity replay source must stay stable");

        assert!(matches!(
            error,
            IndexSourceError::SchemaIdentitySourceConflict { .. }
        ));
    }

    #[test]
    fn cursor_and_scan_limits_are_bounded() {
        assert!(IndexSourceCursor::new(JsonValue::Null).is_err());
        assert!(IndexSourceCursor::new(JsonValue::String("x".repeat(MAX_CURSOR_BYTES))).is_err());
        assert!(serde_json::from_value::<IndexSourceCursor>(JsonValue::Null).is_err());
        assert!(
            serde_json::from_value::<IndexSourceCursor>(JsonValue::String(
                "x".repeat(MAX_CURSOR_BYTES)
            ))
            .is_err()
        );
        assert!(IndexSourceScanRequest::new(Uuid::new_v4(), schema_ref(1), None, 0).is_err());
        assert!(
            IndexSourceScanRequest::new(
                Uuid::new_v4(),
                schema_ref(1),
                None,
                MAX_SCAN_BATCH_SIZE + 1,
            )
            .is_err()
        );
    }

    #[test]
    fn locale_scan_request_preserves_exact_canonical_scope() {
        let locale = LocaleKey::new("EN-us").unwrap();
        let request = IndexSourceScanRequest::for_locale(
            Uuid::new_v4(),
            schema_ref(1),
            locale.clone(),
            None,
            10,
        )
        .unwrap();

        assert_eq!(request.locale(), Some(&locale));
        assert_eq!(request.locale().unwrap().as_str(), "en-US");
    }

    #[test]
    fn targeted_load_is_one_bounded_tenant_schema_scope() {
        let tenant_id = Uuid::new_v4();
        let key = EntityKey {
            tenant_id,
            schema: schema_ref(1),
            entity_id: Uuid::new_v4(),
            locale: None,
        };
        assert!(IndexSourceLoadRequest::new(vec![key.clone()]).is_ok());
        assert!(IndexSourceLoadRequest::new(vec![key.clone(), key]).is_err());
    }
}
