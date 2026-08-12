use std::{collections::BTreeMap, fmt};

use async_trait::async_trait;
use rustok_core::ModuleRuntimeExtensions;
use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend,
    IsolationLevel, QueryResult, Statement, TransactionTrait,
};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    EntityKey, EntityName, FieldName, IndexDriftDependencyFailure, IndexDriftDigestRequest,
    IndexDriftEntityState, IndexDriftSnapshotBoundary, IndexDriftSnapshotPair,
    IndexDriftSnapshotReader, IndexDriftSnapshotView, IndexLinkValue, IndexRecord,
    IndexSourceAbsenceError, IndexSourceError, IndexSourceFailureKind, IndexSourceLoadRequest,
    IndexValue, LinkName, LinkedEntityKey, LocaleKey, ModuleName, SchemaRef, SchemaVersion,
    SharedIndexSchemaRegistry, SharedIndexSourceAbsenceRegistry, SharedIndexSourceRegistry,
};

const SNAPSHOT_BOUNDARY_DOMAIN: &[u8] = b"index_drift_postgres_source_version_boundary_v1";
const ABSENCE_WATERMARK_BOUNDARY_TAG: &[u8] = b"explicit_source_absence_watermark_v1";
const SOURCE_UNAVAILABLE: &str = "index_drift_source_unavailable";
const SOURCE_REJECTED: &str = "index_drift_source_rejected";
const SOURCE_CONTRACT_INVALID: &str = "index_drift_source_contract_invalid";
const SOURCE_WATERMARK_MISSING: &str = "index_drift_source_watermark_missing";
const SOURCE_CHANGED: &str = "index_drift_source_changed_during_capture";
const BACKEND_UNSUPPORTED: &str = "index_drift_snapshot_backend_unsupported";
const STORAGE_UNAVAILABLE: &str = "index_drift_snapshot_storage_unavailable";
const MATERIALIZED_INVALID: &str = "index_drift_materialized_state_invalid";
const SCHEMA_UNAVAILABLE: &str = "index_drift_snapshot_schema_unavailable";
const BOUNDARY_INVALID: &str = "index_drift_snapshot_boundary_invalid";

#[derive(Clone, PartialEq, Eq)]
struct IndexDriftSourceObservation {
    state: IndexDriftEntityState,
    absence_source_version: Option<u64>,
}

impl IndexDriftSourceObservation {
    fn retained(state: IndexDriftEntityState) -> Self {
        Self {
            state,
            absence_source_version: None,
        }
    }

    fn missing(key: EntityKey, source_version: u64) -> Self {
        Self {
            state: IndexDriftEntityState::missing(key),
            absence_source_version: Some(source_version),
        }
    }
}

/// PostgreSQL materialized-state reader fenced by one exact owner source version.
///
/// The reader loads one retained source mutation or one explicit positive-version absence
/// watermark, opens a `REPEATABLE READ READ ONLY` PostgreSQL transaction, captures the
/// materialized entity and links, then reloads the same owner evidence while the transaction
/// remains open. Only an identical typed state and version are accepted. An empty targeted load
/// without an admitted watermark remains fail-closed.
#[derive(Clone)]
pub struct PostgresIndexDriftSnapshotReader {
    db: DatabaseConnection,
    sources: SharedIndexSourceRegistry,
    schemas: SharedIndexSchemaRegistry,
    absence: Option<SharedIndexSourceAbsenceRegistry>,
}

impl PostgresIndexDriftSnapshotReader {
    pub fn new(
        db: DatabaseConnection,
        sources: SharedIndexSourceRegistry,
        schemas: SharedIndexSchemaRegistry,
    ) -> Self {
        Self {
            db,
            sources,
            schemas,
            absence: None,
        }
    }

    pub fn with_absence_registry(mut self, absence: SharedIndexSourceAbsenceRegistry) -> Self {
        self.absence = Some(absence);
        self
    }

    async fn load_source_observation(
        &self,
        request: &IndexDriftDigestRequest,
    ) -> Result<IndexDriftSourceObservation, IndexDriftDependencyFailure> {
        let load = IndexSourceLoadRequest::new(vec![request.key().clone()])
            .map_err(|_| permanent_failure(SOURCE_CONTRACT_INVALID))?;
        let batch = self.sources.load(load).await.map_err(map_source_error)?;
        let mut mutations = batch.into_mutations();
        if mutations.is_empty() {
            let Some(absence) = &self.absence else {
                return Err(permanent_failure(SOURCE_WATERMARK_MISSING));
            };
            if absence.provider_for_schema(&request.key().schema).is_none() {
                return Err(permanent_failure(SOURCE_WATERMARK_MISSING));
            }
            let watermark = absence
                .load(request.key().clone())
                .await
                .map_err(map_absence_error)?
                .ok_or_else(|| permanent_failure(SOURCE_WATERMARK_MISSING))?;
            return Ok(IndexDriftSourceObservation::missing(
                request.key().clone(),
                watermark.source_version(),
            ));
        }
        if mutations.len() != 1 {
            return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
        }
        let state = IndexDriftEntityState::from_mutation(mutations.remove(0));
        if state.key() != request.key() || source_version(&state).is_none_or(|version| version == 0)
        {
            return Err(permanent_failure(SOURCE_CONTRACT_INVALID));
        }
        Ok(IndexDriftSourceObservation::retained(state))
    }

    async fn capture_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        request: &IndexDriftDigestRequest,
        source: &IndexDriftSourceObservation,
    ) -> Result<IndexDriftSnapshotPair, IndexDriftDependencyFailure> {
        let snapshot = transaction
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                "SELECT txid_current_snapshot()::text AS snapshot_token".to_owned(),
            ))
            .await
            .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?
            .ok_or_else(|| retryable_failure(STORAGE_UNAVAILABLE))?
            .try_get::<String>("", "snapshot_token")
            .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;

        let materialized = self
            .load_materialized_state(transaction, request.key())
            .await?;
        let observed_again = match self.load_source_observation(request).await {
            Ok(observation) => observation,
            Err(error)
                if source.absence_source_version.is_some()
                    && error.code() == SOURCE_WATERMARK_MISSING =>
            {
                return Err(retryable_failure(SOURCE_CHANGED));
            }
            Err(error) => return Err(error),
        };
        if &observed_again != source {
            return Err(retryable_failure(SOURCE_CHANGED));
        }

        let boundary = derive_boundary(&snapshot, source)?;
        IndexDriftSnapshotPair::new(
            IndexDriftSnapshotView::new(boundary.clone(), source.state.clone()),
            IndexDriftSnapshotView::new(boundary, materialized),
        )
        .map_err(|_| permanent_failure(BOUNDARY_INVALID))
    }

    async fn load_materialized_state(
        &self,
        transaction: &DatabaseTransaction,
        key: &EntityKey,
    ) -> Result<IndexDriftEntityState, IndexDriftDependencyFailure> {
        let registered = self
            .schemas
            .registry()
            .get(&key.schema)
            .ok_or_else(|| permanent_failure(SCHEMA_UNAVAILABLE))?;
        let locale = persisted_locale(key.locale.as_ref());
        let row = transaction
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT CAST(source_version AS TEXT) AS source_version_text, schema_fingerprint, payload, is_deleted FROM index_entities WHERE tenant_id = $1 AND module_name = $2 AND entity_name = $3 AND schema_version = $4 AND entity_id = $5 AND locale_key = $6 LIMIT 1",
                vec![
                    key.tenant_id.into(),
                    key.schema.module.as_str().to_owned().into(),
                    key.schema.entity.as_str().to_owned().into(),
                    i64::from(key.schema.version.get()).into(),
                    key.entity_id.into(),
                    locale.clone().into(),
                ],
            ))
            .await
            .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
        let Some(row) = row else {
            return Ok(IndexDriftEntityState::missing(key.clone()));
        };

        let version = stored_source_version(&row)?;
        let fingerprint: String = row
            .try_get("", "schema_fingerprint")
            .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?;
        if fingerprint != registered.fingerprint.to_string() {
            return Err(permanent_failure(MATERIALIZED_INVALID));
        }
        let is_deleted: bool = row
            .try_get("", "is_deleted")
            .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?;
        let payload: Option<JsonValue> = row
            .try_get("", "payload")
            .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?;
        let links = load_links(transaction, key, version).await?;

        if is_deleted {
            if payload.is_some() || !links.is_empty() {
                return Err(permanent_failure(MATERIALIZED_INVALID));
            }
            return Ok(IndexDriftEntityState::delete(key.clone(), version));
        }

        let fields = serde_json::from_value::<BTreeMap<FieldName, IndexValue>>(
            payload.ok_or_else(|| permanent_failure(MATERIALIZED_INVALID))?,
        )
        .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?;
        let links = order_links(&registered.schema.links, links)?;
        Ok(IndexDriftEntityState::upsert(IndexRecord {
            key: key.clone(),
            source_version: version,
            fields,
            links,
        }))
    }
}

impl fmt::Debug for PostgresIndexDriftSnapshotReader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresIndexDriftSnapshotReader")
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl IndexDriftSnapshotReader for PostgresIndexDriftSnapshotReader {
    async fn capture_entity_snapshot(
        &self,
        request: &IndexDriftDigestRequest,
    ) -> Result<IndexDriftSnapshotPair, IndexDriftDependencyFailure> {
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(permanent_failure(BACKEND_UNSUPPORTED));
        }
        let source = self.load_source_observation(request).await?;
        let transaction = self
            .db
            .begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadOnly),
            )
            .await
            .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
        let captured = self
            .capture_in_transaction(&transaction, request, &source)
            .await;
        match captured {
            Ok(pair) => {
                transaction
                    .commit()
                    .await
                    .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
                Ok(pair)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexDriftSnapshotCompositionError {
    #[error("PostgreSQL Index drift snapshot reader requires the shared schema registry")]
    MissingSchemaRegistry,
    #[error("PostgreSQL Index drift snapshot reader does not support this database backend")]
    UnsupportedBackend,
}

/// Constructs the production reader only after immutable source and schema registries exist.
///
/// The function performs no SQL and starts no task. An absent source registry remains an optional
/// capability. An optional already-materialized absence registry is attached without changing the
/// ordinary replay-source contract.
pub fn materialize_postgres_index_drift_snapshot_reader(
    extensions: &ModuleRuntimeExtensions,
    db: DatabaseConnection,
) -> Result<Option<PostgresIndexDriftSnapshotReader>, IndexDriftSnapshotCompositionError> {
    let Some(sources) = extensions.get::<SharedIndexSourceRegistry>().cloned() else {
        return Ok(None);
    };
    let schemas = extensions
        .get::<SharedIndexSchemaRegistry>()
        .cloned()
        .ok_or(IndexDriftSnapshotCompositionError::MissingSchemaRegistry)?;
    if db.get_database_backend() != DbBackend::Postgres {
        return Err(IndexDriftSnapshotCompositionError::UnsupportedBackend);
    }
    let reader = PostgresIndexDriftSnapshotReader::new(db, sources, schemas);
    Ok(Some(
        match extensions
            .get::<SharedIndexSourceAbsenceRegistry>()
            .cloned()
        {
            Some(absence) => reader.with_absence_registry(absence),
            None => reader,
        },
    ))
}

async fn load_links(
    transaction: &DatabaseTransaction,
    key: &EntityKey,
    source_version: u64,
) -> Result<BTreeMap<LinkName, Vec<(usize, LinkedEntityKey)>>, IndexDriftDependencyFailure> {
    let rows = transaction
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT link_name, ordinal, target_module, target_entity, target_schema_version, target_entity_id, target_locale_key FROM index_links WHERE tenant_id = $1 AND source_module = $2 AND source_entity = $3 AND source_schema_version = $4 AND source_entity_id = $5 AND source_locale_key = $6 AND CAST(source_version AS TEXT) = $7 ORDER BY link_name ASC, ordinal ASC",
            vec![
                key.tenant_id.into(),
                key.schema.module.as_str().to_owned().into(),
                key.schema.entity.as_str().to_owned().into(),
                i64::from(key.schema.version.get()).into(),
                key.entity_id.into(),
                persisted_locale(key.locale.as_ref()).into(),
                source_version.to_string().into(),
            ],
        ))
        .await
        .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;

    let mut links = BTreeMap::<LinkName, Vec<(usize, LinkedEntityKey)>>::new();
    for row in rows {
        let name = LinkName::new(
            row.try_get::<String>("", "link_name")
                .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?,
        )
        .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?;
        let ordinal = row
            .try_get::<i64>("", "ordinal")
            .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?;
        let ordinal =
            usize::try_from(ordinal).map_err(|_| permanent_failure(MATERIALIZED_INVALID))?;
        let target_version = row
            .try_get::<i64>("", "target_schema_version")
            .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?;
        let target_version = u32::try_from(target_version)
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| permanent_failure(MATERIALIZED_INVALID))?;
        let target_id = row
            .try_get::<Uuid>("", "target_entity_id")
            .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?;
        if target_id.is_nil() {
            return Err(permanent_failure(MATERIALIZED_INVALID));
        }
        let target_locale = row
            .try_get::<String>("", "target_locale_key")
            .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?;
        let target = LinkedEntityKey {
            schema: SchemaRef {
                module: ModuleName::new(
                    row.try_get::<String>("", "target_module")
                        .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?,
                )
                .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?,
                entity: EntityName::new(
                    row.try_get::<String>("", "target_entity")
                        .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?,
                )
                .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?,
                version: SchemaVersion::new(target_version),
            },
            entity_id: target_id,
            locale: if target_locale.is_empty() {
                None
            } else {
                Some(
                    LocaleKey::new(target_locale)
                        .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?,
                )
            },
        };
        links.entry(name).or_default().push((ordinal, target));
    }
    Ok(links)
}

fn order_links(
    schema_links: &[crate::IndexLink],
    mut stored: BTreeMap<LinkName, Vec<(usize, LinkedEntityKey)>>,
) -> Result<Vec<IndexLinkValue>, IndexDriftDependencyFailure> {
    let mut links = Vec::new();
    for schema_link in schema_links {
        let Some(mut targets) = stored.remove(&schema_link.name) else {
            continue;
        };
        targets.sort_by_key(|(ordinal, _)| *ordinal);
        if targets
            .iter()
            .enumerate()
            .any(|(expected, (actual, _))| expected != *actual)
        {
            return Err(permanent_failure(MATERIALIZED_INVALID));
        }
        links.push(IndexLinkValue {
            name: schema_link.name.clone(),
            targets: targets.into_iter().map(|(_, target)| target).collect(),
        });
    }
    if !stored.is_empty() {
        return Err(permanent_failure(MATERIALIZED_INVALID));
    }
    Ok(links)
}

fn stored_source_version(row: &QueryResult) -> Result<u64, IndexDriftDependencyFailure> {
    let value = row
        .try_get::<String>("", "source_version_text")
        .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?;
    value
        .parse::<u64>()
        .ok()
        .filter(|version| *version > 0)
        .ok_or_else(|| permanent_failure(MATERIALIZED_INVALID))
}

fn persisted_locale(locale: Option<&LocaleKey>) -> String {
    locale.map_or_else(String::new, |locale| locale.as_str().to_owned())
}

fn source_version(state: &IndexDriftEntityState) -> Option<u64> {
    match state {
        IndexDriftEntityState::Missing { .. } => None,
        IndexDriftEntityState::Upsert { record } => Some(record.source_version),
        IndexDriftEntityState::Delete { source_version, .. } => Some(*source_version),
    }
}

fn derive_boundary(
    snapshot: &str,
    source: &IndexDriftSourceObservation,
) -> Result<IndexDriftSnapshotBoundary, IndexDriftDependencyFailure> {
    let encoded =
        postcard::to_allocvec(&source.state).map_err(|_| permanent_failure(BOUNDARY_INVALID))?;
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, SNAPSHOT_BOUNDARY_DOMAIN);
    hash_component(&mut hasher, snapshot.as_bytes());
    hash_component(&mut hasher, &encoded);
    if let Some(source_version) = source.absence_source_version {
        hash_component(&mut hasher, ABSENCE_WATERMARK_BOUNDARY_TAG);
        hash_component(&mut hasher, &source_version.to_be_bytes());
    }
    IndexDriftSnapshotBoundary::new(format!("pg:{}", hex::encode(hasher.finalize())))
        .map_err(|_| permanent_failure(BOUNDARY_INVALID))
}

fn hash_component(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn map_source_error(error: IndexSourceError) -> IndexDriftDependencyFailure {
    match error {
        IndexSourceError::SourceFailure { failure, .. }
            if failure.kind() == IndexSourceFailureKind::Retryable =>
        {
            retryable_failure(SOURCE_UNAVAILABLE)
        }
        IndexSourceError::SourceFailure { .. } => permanent_failure(SOURCE_REJECTED),
        _ => permanent_failure(SOURCE_CONTRACT_INVALID),
    }
}

fn map_absence_error(error: IndexSourceAbsenceError) -> IndexDriftDependencyFailure {
    match error {
        IndexSourceAbsenceError::ProviderFailure { failure, .. }
            if failure.kind() == IndexSourceFailureKind::Retryable =>
        {
            retryable_failure(SOURCE_UNAVAILABLE)
        }
        IndexSourceAbsenceError::ProviderFailure { .. } => permanent_failure(SOURCE_REJECTED),
        IndexSourceAbsenceError::UnknownSchemaProvider(_) => {
            permanent_failure(SOURCE_WATERMARK_MISSING)
        }
        _ => permanent_failure(SOURCE_CONTRACT_INVALID),
    }
}

fn retryable_failure(code: &'static str) -> IndexDriftDependencyFailure {
    IndexDriftDependencyFailure::retryable(code)
        .expect("static Index drift snapshot retry code is valid")
}

fn permanent_failure(code: &'static str) -> IndexDriftDependencyFailure {
    IndexDriftDependencyFailure::permanent(code)
        .expect("static Index drift snapshot permanent code is valid")
}
