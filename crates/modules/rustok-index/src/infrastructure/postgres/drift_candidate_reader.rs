use std::{fmt, str::FromStr};

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend,
    IsolationLevel, QueryResult, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    EntityKey, EntityName, IndexDriftCandidate, IndexDriftCandidateCursor,
    IndexDriftCandidateFailure, IndexDriftCandidateFence, IndexDriftCandidatePage,
    IndexDriftCandidateReader, IndexDriftCandidateRequest, IndexDriftCandidateScope, LinkName,
    LinkedEntityKey, LocaleKey, ModuleName, SchemaRef, SchemaVersion,
};

const WIRE_VERSION: u8 = 1;
const MAX_SNAPSHOT_TOKEN_BYTES: usize = 256;
const SCOPE_DIGEST_DOMAIN: &[u8] = b"index_drift_candidate_scope_v1";
const BACKEND_UNSUPPORTED: &str = "index_drift_candidate_backend_unsupported";
const STORAGE_UNAVAILABLE: &str = "index_drift_candidate_storage_unavailable";
const CURSOR_INVALID: &str = "index_drift_candidate_cursor_invalid";
const FENCE_INVALID: &str = "index_drift_candidate_fence_invalid";
const MATERIALIZED_INVALID: &str = "index_drift_candidate_materialized_invalid";
const CONTRACT_INVALID: &str = "index_drift_candidate_contract_invalid";

#[derive(Clone)]
pub struct PostgresIndexDriftCandidateReader {
    db: DatabaseConnection,
}

impl PostgresIndexDriftCandidateReader {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn read_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        request: &IndexDriftCandidateRequest,
    ) -> Result<IndexDriftCandidatePage, IndexDriftCandidateFailure> {
        let (fence, snapshot_token) = resolve_fence(transaction, request).await?;
        let phase = decode_request_phase(request)?;
        let mut candidates = Vec::with_capacity(request.limit());
        let mut next_cursor = None;

        match phase {
            ReadPhase::Stale(after) => {
                let stale_rows = load_stale_rows(
                    transaction,
                    request.scope(),
                    snapshot_token.as_str(),
                    after.as_ref(),
                    request.limit() + 1,
                )
                .await?;
                let stale_has_more = stale_rows.len() > request.limit();
                let stale_take = stale_rows.len().min(request.limit());
                let mut last_stale = None;
                for row in stale_rows.into_iter().take(stale_take) {
                    last_stale = Some(row.position.clone());
                    candidates.push(row.candidate);
                }

                if stale_has_more {
                    next_cursor = Some(encode_cursor(
                        request.scope(),
                        CursorPhaseWire::Stale {
                            after: last_stale.ok_or_else(|| permanent_failure(CONTRACT_INVALID))?,
                        },
                    )?);
                } else {
                    let remaining = request.limit() - candidates.len();
                    if remaining == 0 {
                        let orphan_lookahead = load_orphan_rows(
                            transaction,
                            request.scope(),
                            snapshot_token.as_str(),
                            None,
                            1,
                        )
                        .await?;
                        if !orphan_lookahead.is_empty() {
                            next_cursor = Some(encode_cursor(
                                request.scope(),
                                CursorPhaseWire::Orphan { after: None },
                            )?);
                        }
                    } else {
                        append_orphan_rows(
                            transaction,
                            request,
                            snapshot_token.as_str(),
                            None,
                            remaining,
                            &mut candidates,
                            &mut next_cursor,
                        )
                        .await?;
                    }
                }
            }
            ReadPhase::Orphan(after) => {
                append_orphan_rows(
                    transaction,
                    request,
                    snapshot_token.as_str(),
                    after.as_ref(),
                    request.limit(),
                    &mut candidates,
                    &mut next_cursor,
                )
                .await?;
            }
        }

        IndexDriftCandidatePage::new(request, fence, candidates, next_cursor)
            .map_err(|_| permanent_failure(CONTRACT_INVALID))
    }
}

impl fmt::Debug for PostgresIndexDriftCandidateReader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresIndexDriftCandidateReader")
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl IndexDriftCandidateReader for PostgresIndexDriftCandidateReader {
    async fn read_candidate_page(
        &self,
        request: IndexDriftCandidateRequest,
    ) -> Result<IndexDriftCandidatePage, IndexDriftCandidateFailure> {
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(permanent_failure(BACKEND_UNSUPPORTED));
        }
        let transaction = self
            .db
            .begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadOnly),
            )
            .await
            .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
        let result = self.read_in_transaction(&transaction, &request).await;
        match result {
            Ok(page) => {
                transaction
                    .commit()
                    .await
                    .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
                Ok(page)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexDriftCandidateCompositionError {
    #[error("PostgreSQL Index drift candidate reader does not support this database backend")]
    UnsupportedBackend,
}

/// Constructs the production reader without executing SQL or publishing a transport capability.
pub fn materialize_postgres_index_drift_candidate_reader(
    db: DatabaseConnection,
) -> Result<PostgresIndexDriftCandidateReader, IndexDriftCandidateCompositionError> {
    if db.get_database_backend() != DbBackend::Postgres {
        return Err(IndexDriftCandidateCompositionError::UnsupportedBackend);
    }
    Ok(PostgresIndexDriftCandidateReader::new(db))
}

async fn append_orphan_rows(
    transaction: &DatabaseTransaction,
    request: &IndexDriftCandidateRequest,
    snapshot_token: &str,
    after: Option<&OrphanPositionWire>,
    remaining: usize,
    candidates: &mut Vec<IndexDriftCandidate>,
    next_cursor: &mut Option<IndexDriftCandidateCursor>,
) -> Result<(), IndexDriftCandidateFailure> {
    let rows = load_orphan_rows(
        transaction,
        request.scope(),
        snapshot_token,
        after,
        remaining + 1,
    )
    .await?;
    let has_more = rows.len() > remaining;
    let take = rows.len().min(remaining);
    let mut last = None;
    for row in rows.into_iter().take(take) {
        last = Some(row.position.clone());
        candidates.push(row.candidate);
    }
    if has_more {
        *next_cursor = Some(encode_cursor(
            request.scope(),
            CursorPhaseWire::Orphan {
                after: Some(last.ok_or_else(|| permanent_failure(CONTRACT_INVALID))?),
            },
        )?);
    }
    Ok(())
}

async fn resolve_fence(
    transaction: &DatabaseTransaction,
    request: &IndexDriftCandidateRequest,
) -> Result<(IndexDriftCandidateFence, String), IndexDriftCandidateFailure> {
    if let Some(fence) = request.fence() {
        let wire: FenceWire = decode_wire(fence.as_str(), FENCE_INVALID)?;
        if wire.version != WIRE_VERSION || wire.scope_digest != scope_digest(request.scope()) {
            return Err(permanent_failure(FENCE_INVALID));
        }
        validate_snapshot_token(wire.snapshot.as_str())?;
        return Ok((fence.clone(), wire.snapshot));
    }

    let snapshot = transaction
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT txid_current_snapshot()::text AS snapshot_token".to_owned(),
        ))
        .await
        .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?
        .ok_or_else(|| retryable_failure(STORAGE_UNAVAILABLE))?
        .try_get::<String>("", "snapshot_token")
        .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
    validate_snapshot_token(snapshot.as_str())?;
    let encoded = encode_wire(
        &FenceWire {
            version: WIRE_VERSION,
            scope_digest: scope_digest(request.scope()),
            snapshot: snapshot.clone(),
        },
        CONTRACT_INVALID,
    )?;
    let fence =
        IndexDriftCandidateFence::new(encoded).map_err(|_| permanent_failure(CONTRACT_INVALID))?;
    Ok((fence, snapshot))
}

fn decode_request_phase(
    request: &IndexDriftCandidateRequest,
) -> Result<ReadPhase, IndexDriftCandidateFailure> {
    let Some(cursor) = request.cursor() else {
        return Ok(ReadPhase::Stale(None));
    };
    let wire: CursorWire = decode_wire(cursor.as_str(), CURSOR_INVALID)?;
    validate_wire_scope(
        wire.version,
        wire.tenant_id,
        &wire.schema,
        request.scope(),
        CURSOR_INVALID,
    )?;
    match wire.phase {
        CursorPhaseWire::Stale { after } => {
            validate_stale_position(&after)?;
            Ok(ReadPhase::Stale(Some(after)))
        }
        CursorPhaseWire::Orphan { after } => {
            if let Some(position) = &after {
                validate_orphan_position(position)?;
            }
            Ok(ReadPhase::Orphan(after))
        }
    }
}

async fn load_stale_rows(
    transaction: &DatabaseTransaction,
    scope: &IndexDriftCandidateScope,
    snapshot_token: &str,
    after: Option<&StalePositionWire>,
    limit: usize,
) -> Result<Vec<StaleCandidateRow>, IndexDriftCandidateFailure> {
    let (sql, values) = match after {
        None => (
            "SELECT entity_id, locale_key, CAST(source_version AS TEXT) AS source_version_text FROM index_entities e WHERE tenant_id = $1 AND module_name = $2 AND entity_name = $3 AND schema_version = $4 AND is_deleted = FALSE AND source_version > 0 AND txid_visible_in_snapshot((e.xmin::text)::bigint, $5::txid_snapshot) ORDER BY entity_id ASC, locale_key ASC LIMIT $6",
            vec![
                scope.tenant_id().into(),
                scope.schema().module.as_str().to_owned().into(),
                scope.schema().entity.as_str().to_owned().into(),
                i64::from(scope.schema().version.get()).into(),
                snapshot_token.to_owned().into(),
                limit_value(limit),
            ],
        ),
        Some(after) => (
            "SELECT entity_id, locale_key, CAST(source_version AS TEXT) AS source_version_text FROM index_entities e WHERE tenant_id = $1 AND module_name = $2 AND entity_name = $3 AND schema_version = $4 AND is_deleted = FALSE AND source_version > 0 AND txid_visible_in_snapshot((e.xmin::text)::bigint, $5::txid_snapshot) AND (entity_id, locale_key) > ($6::uuid, $7) ORDER BY entity_id ASC, locale_key ASC LIMIT $8",
            vec![
                scope.tenant_id().into(),
                scope.schema().module.as_str().to_owned().into(),
                scope.schema().entity.as_str().to_owned().into(),
                i64::from(scope.schema().version.get()).into(),
                snapshot_token.to_owned().into(),
                after.entity_id.into(),
                after.locale_key.clone().into(),
                limit_value(limit),
            ],
        ),
    };
    let rows = transaction
        .query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            values,
        ))
        .await
        .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
    rows.into_iter()
        .map(|row| decode_stale_row(scope, &row))
        .collect()
}

async fn load_orphan_rows(
    transaction: &DatabaseTransaction,
    scope: &IndexDriftCandidateScope,
    snapshot_token: &str,
    after: Option<&OrphanPositionWire>,
    limit: usize,
) -> Result<Vec<OrphanCandidateRow>, IndexDriftCandidateFailure> {
    let base = "SELECT l.source_entity_id, l.source_locale_key, CAST(s.source_version AS TEXT) AS source_version_text, l.link_name, l.ordinal, l.target_module, l.target_entity, l.target_schema_version, l.target_entity_id, l.target_locale_key FROM index_links l JOIN index_entities s ON s.tenant_id = l.tenant_id AND s.module_name = l.source_module AND s.entity_name = l.source_entity AND s.schema_version = l.source_schema_version AND s.entity_id = l.source_entity_id AND s.locale_key = l.source_locale_key AND s.source_version = l.source_version LEFT JOIN index_entities t ON t.tenant_id = l.tenant_id AND t.module_name = l.target_module AND t.entity_name = l.target_entity AND t.schema_version = l.target_schema_version AND t.entity_id = l.target_entity_id AND t.locale_key = l.target_locale_key WHERE l.tenant_id = $1 AND l.source_module = $2 AND l.source_entity = $3 AND l.source_schema_version = $4 AND s.is_deleted = FALSE AND s.source_version > 0 AND txid_visible_in_snapshot((l.xmin::text)::bigint, $5::txid_snapshot) AND txid_visible_in_snapshot((s.xmin::text)::bigint, $5::txid_snapshot) AND (t.tenant_id IS NULL OR (t.is_deleted = TRUE AND txid_visible_in_snapshot((t.xmin::text)::bigint, $5::txid_snapshot)))";
    let (sql, values) = match after {
        None => (
            format!(
                "{base} ORDER BY l.source_entity_id ASC, l.source_locale_key ASC, l.link_name ASC, l.ordinal ASC, l.target_module ASC, l.target_entity ASC, l.target_schema_version ASC, l.target_entity_id ASC, l.target_locale_key ASC LIMIT $6"
            ),
            vec![
                scope.tenant_id().into(),
                scope.schema().module.as_str().to_owned().into(),
                scope.schema().entity.as_str().to_owned().into(),
                i64::from(scope.schema().version.get()).into(),
                snapshot_token.to_owned().into(),
                limit_value(limit),
            ],
        ),
        Some(after) => (
            format!(
                "{base} AND (l.source_entity_id, l.source_locale_key, l.link_name, l.ordinal, l.target_module, l.target_entity, l.target_schema_version, l.target_entity_id, l.target_locale_key) > ($6::uuid, $7, $8, $9, $10, $11, $12, $13::uuid, $14) ORDER BY l.source_entity_id ASC, l.source_locale_key ASC, l.link_name ASC, l.ordinal ASC, l.target_module ASC, l.target_entity ASC, l.target_schema_version ASC, l.target_entity_id ASC, l.target_locale_key ASC LIMIT $15"
            ),
            vec![
                scope.tenant_id().into(),
                scope.schema().module.as_str().to_owned().into(),
                scope.schema().entity.as_str().to_owned().into(),
                i64::from(scope.schema().version.get()).into(),
                snapshot_token.to_owned().into(),
                after.source_entity_id.into(),
                after.source_locale_key.clone().into(),
                after.link_name.clone().into(),
                i64::from(after.ordinal).into(),
                after.target_module.clone().into(),
                after.target_entity.clone().into(),
                i64::from(after.target_schema_version).into(),
                after.target_entity_id.into(),
                after.target_locale_key.clone().into(),
                limit_value(limit),
            ],
        ),
    };
    let rows = transaction
        .query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            values,
        ))
        .await
        .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
    rows.into_iter()
        .map(|row| decode_orphan_row(scope, &row))
        .collect()
}

fn decode_stale_row(
    scope: &IndexDriftCandidateScope,
    row: &QueryResult,
) -> Result<StaleCandidateRow, IndexDriftCandidateFailure> {
    let entity_id = row_uuid(row, "entity_id")?;
    let locale_key = row_string(row, "locale_key")?;
    let key = EntityKey {
        tenant_id: scope.tenant_id(),
        schema: scope.schema().clone(),
        entity_id,
        locale: decode_locale(locale_key.as_str())?,
    };
    let version = row_source_version(row)?;
    let candidate = IndexDriftCandidate::stale_entity(key, version)
        .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?;
    Ok(StaleCandidateRow {
        candidate,
        position: StalePositionWire {
            entity_id,
            locale_key,
        },
    })
}

fn decode_orphan_row(
    scope: &IndexDriftCandidateScope,
    row: &QueryResult,
) -> Result<OrphanCandidateRow, IndexDriftCandidateFailure> {
    let source_entity_id = row_uuid(row, "source_entity_id")?;
    let source_locale_key = row_string(row, "source_locale_key")?;
    let source_key = EntityKey {
        tenant_id: scope.tenant_id(),
        schema: scope.schema().clone(),
        entity_id: source_entity_id,
        locale: decode_locale(source_locale_key.as_str())?,
    };
    let source_version = row_source_version(row)?;
    let link_name_raw = row_string(row, "link_name")?;
    let link_name = LinkName::new(link_name_raw.clone())
        .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?;
    let ordinal = row_u32(row, "ordinal")?;
    let target_module_raw = row_string(row, "target_module")?;
    let target_entity_raw = row_string(row, "target_entity")?;
    let target_schema_version = row_positive_u32(row, "target_schema_version")?;
    let target_entity_id = row_uuid(row, "target_entity_id")?;
    let target_locale_key = row_string(row, "target_locale_key")?;
    let target = LinkedEntityKey {
        schema: SchemaRef {
            module: ModuleName::new(target_module_raw.clone())
                .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?,
            entity: EntityName::new(target_entity_raw.clone())
                .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?,
            version: SchemaVersion::new(target_schema_version),
        },
        entity_id: target_entity_id,
        locale: decode_locale(target_locale_key.as_str())?,
    };
    let candidate =
        IndexDriftCandidate::orphan_link(source_key, source_version, link_name, ordinal, target)
            .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?;
    Ok(OrphanCandidateRow {
        candidate,
        position: OrphanPositionWire {
            source_entity_id,
            source_locale_key,
            link_name: link_name_raw,
            ordinal,
            target_module: target_module_raw,
            target_entity: target_entity_raw,
            target_schema_version,
            target_entity_id,
            target_locale_key,
        },
    })
}

fn validate_wire_scope(
    version: u8,
    tenant_id: Uuid,
    schema: &SchemaRef,
    expected: &IndexDriftCandidateScope,
    code: &'static str,
) -> Result<(), IndexDriftCandidateFailure> {
    if version != WIRE_VERSION || tenant_id != expected.tenant_id() || schema != expected.schema() {
        return Err(permanent_failure(code));
    }
    Ok(())
}

fn scope_digest(scope: &IndexDriftCandidateScope) -> String {
    let mut digest = Sha256::new();
    digest.update(SCOPE_DIGEST_DOMAIN);
    digest.update(scope.tenant_id().as_bytes());
    digest_part(&mut digest, scope.schema().module.as_str().as_bytes());
    digest_part(&mut digest, scope.schema().entity.as_str().as_bytes());
    digest.update(scope.schema().version.get().to_be_bytes());
    hex::encode(digest.finalize())
}

fn digest_part(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u32).to_be_bytes());
    digest.update(value);
}

fn validate_snapshot_token(value: &str) -> Result<(), IndexDriftCandidateFailure> {
    if value.is_empty()
        || value.len() > MAX_SNAPSHOT_TOKEN_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b':' | b','))
    {
        return Err(permanent_failure(FENCE_INVALID));
    }
    let mut parts = value.split(':');
    let xmin = parts
        .next()
        .and_then(|value| u64::from_str(value).ok())
        .ok_or_else(|| permanent_failure(FENCE_INVALID))?;
    let xmax = parts
        .next()
        .and_then(|value| u64::from_str(value).ok())
        .ok_or_else(|| permanent_failure(FENCE_INVALID))?;
    let active = parts
        .next()
        .ok_or_else(|| permanent_failure(FENCE_INVALID))?;
    if parts.next().is_some() || xmin > xmax {
        return Err(permanent_failure(FENCE_INVALID));
    }
    let mut previous = None;
    if !active.is_empty() {
        for value in active.split(',') {
            let xid = u64::from_str(value).map_err(|_| permanent_failure(FENCE_INVALID))?;
            if xid < xmin || xid >= xmax || previous.is_some_and(|previous| previous >= xid) {
                return Err(permanent_failure(FENCE_INVALID));
            }
            previous = Some(xid);
        }
    }
    Ok(())
}

fn validate_stale_position(position: &StalePositionWire) -> Result<(), IndexDriftCandidateFailure> {
    if position.entity_id.is_nil() {
        return Err(permanent_failure(CURSOR_INVALID));
    }
    validate_persisted_locale(position.locale_key.as_str(), CURSOR_INVALID)
}

fn validate_orphan_position(
    position: &OrphanPositionWire,
) -> Result<(), IndexDriftCandidateFailure> {
    if position.source_entity_id.is_nil() || position.target_entity_id.is_nil() {
        return Err(permanent_failure(CURSOR_INVALID));
    }
    validate_persisted_locale(position.source_locale_key.as_str(), CURSOR_INVALID)?;
    validate_persisted_locale(position.target_locale_key.as_str(), CURSOR_INVALID)?;
    LinkName::new(position.link_name.clone()).map_err(|_| permanent_failure(CURSOR_INVALID))?;
    ModuleName::new(position.target_module.clone())
        .map_err(|_| permanent_failure(CURSOR_INVALID))?;
    EntityName::new(position.target_entity.clone())
        .map_err(|_| permanent_failure(CURSOR_INVALID))?;
    if position.target_schema_version == 0 {
        return Err(permanent_failure(CURSOR_INVALID));
    }
    Ok(())
}

fn validate_persisted_locale(
    value: &str,
    code: &'static str,
) -> Result<(), IndexDriftCandidateFailure> {
    if value.is_empty() {
        return Ok(());
    }
    LocaleKey::new(value).map_err(|_| permanent_failure(code))?;
    Ok(())
}

fn decode_locale(value: &str) -> Result<Option<LocaleKey>, IndexDriftCandidateFailure> {
    if value.is_empty() {
        Ok(None)
    } else {
        LocaleKey::new(value)
            .map(Some)
            .map_err(|_| permanent_failure(MATERIALIZED_INVALID))
    }
}

fn row_uuid(row: &QueryResult, column: &str) -> Result<Uuid, IndexDriftCandidateFailure> {
    let value = row
        .try_get::<Uuid>("", column)
        .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?;
    if value.is_nil() {
        return Err(permanent_failure(MATERIALIZED_INVALID));
    }
    Ok(value)
}

fn row_string(row: &QueryResult, column: &str) -> Result<String, IndexDriftCandidateFailure> {
    row.try_get::<String>("", column)
        .map_err(|_| permanent_failure(MATERIALIZED_INVALID))
}

fn row_u32(row: &QueryResult, column: &str) -> Result<u32, IndexDriftCandidateFailure> {
    let value = row
        .try_get::<i64>("", column)
        .map_err(|_| permanent_failure(MATERIALIZED_INVALID))?;
    u32::try_from(value).map_err(|_| permanent_failure(MATERIALIZED_INVALID))
}

fn row_positive_u32(row: &QueryResult, column: &str) -> Result<u32, IndexDriftCandidateFailure> {
    row_u32(row, column).and_then(|value| {
        if value == 0 {
            Err(permanent_failure(MATERIALIZED_INVALID))
        } else {
            Ok(value)
        }
    })
}

fn row_source_version(row: &QueryResult) -> Result<u64, IndexDriftCandidateFailure> {
    let raw = row_string(row, "source_version_text")?;
    u64::from_str(raw.as_str())
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| permanent_failure(MATERIALIZED_INVALID))
}

fn limit_value(limit: usize) -> sea_orm::Value {
    i64::try_from(limit)
        .expect("candidate limits are bounded to at most 33")
        .into()
}

fn encode_cursor(
    scope: &IndexDriftCandidateScope,
    phase: CursorPhaseWire,
) -> Result<IndexDriftCandidateCursor, IndexDriftCandidateFailure> {
    let encoded = encode_wire(
        &CursorWire {
            version: WIRE_VERSION,
            tenant_id: scope.tenant_id(),
            schema: scope.schema().clone(),
            phase,
        },
        CONTRACT_INVALID,
    )?;
    IndexDriftCandidateCursor::new(encoded).map_err(|_| permanent_failure(CONTRACT_INVALID))
}

fn encode_wire<T: Serialize>(
    value: &T,
    code: &'static str,
) -> Result<String, IndexDriftCandidateFailure> {
    serde_json::to_vec(value)
        .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        .map_err(|_| permanent_failure(code))
}

fn decode_wire<T: DeserializeOwned>(
    value: &str,
    code: &'static str,
) -> Result<T, IndexDriftCandidateFailure> {
    let bytes = URL_SAFE_NO_PAD.decode(value)
        .map_err(|_| permanent_failure(code))?;
    serde_json::from_slice(bytes.as_slice()).map_err(|_| permanent_failure(code))
}

fn retryable_failure(code: &'static str) -> IndexDriftCandidateFailure {
    IndexDriftCandidateFailure::retryable(code).expect("static candidate failure code is valid")
}

fn permanent_failure(code: &'static str) -> IndexDriftCandidateFailure {
    IndexDriftCandidateFailure::permanent(code).expect("static candidate failure code is valid")
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ReadPhase {
    Stale(Option<StalePositionWire>),
    Orphan(Option<OrphanPositionWire>),
}

#[derive(Debug)]
struct StaleCandidateRow {
    candidate: IndexDriftCandidate,
    position: StalePositionWire,
}

#[derive(Debug)]
struct OrphanCandidateRow {
    candidate: IndexDriftCandidate,
    position: OrphanPositionWire,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FenceWire {
    version: u8,
    scope_digest: String,
    snapshot: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CursorWire {
    version: u8,
    tenant_id: Uuid,
    schema: SchemaRef,
    phase: CursorPhaseWire,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum CursorPhaseWire {
    Stale { after: StalePositionWire },
    Orphan { after: Option<OrphanPositionWire> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StalePositionWire {
    entity_id: Uuid,
    locale_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct OrphanPositionWire {
    source_entity_id: Uuid,
    source_locale_key: String,
    link_name: String,
    ordinal: u32,
    target_module: String,
    target_entity: String,
    target_schema_version: u32,
    target_entity_id: Uuid,
    target_locale_key: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(tenant_id: Uuid) -> IndexDriftCandidateScope {
        IndexDriftCandidateScope::new(
            tenant_id,
            SchemaRef {
                module: ModuleName::new("catalog").expect("module"),
                entity: EntityName::new("product").expect("entity"),
                version: SchemaVersion::new(1),
            },
        )
        .expect("scope")
    }

    #[test]
    fn snapshot_token_validation_is_bounded_and_canonical() {
        assert!(validate_snapshot_token("10:20:12,14").is_ok());
        assert!(validate_snapshot_token("10:20:").is_ok());
        assert!(validate_snapshot_token("10:10:").is_ok());
        assert!(validate_snapshot_token("20:10:").is_err());
        assert!(validate_snapshot_token("10:20:14,12").is_err());
        assert!(validate_snapshot_token("10:20:30").is_err());
        assert!(validate_snapshot_token("10:20:12;14").is_err());
    }

    #[test]
    fn compact_fence_remains_scope_bound() {
        let tenant_id = Uuid::new_v4();
        let candidate_scope = scope(tenant_id);
        let encoded = encode_wire(
            &FenceWire {
                version: WIRE_VERSION,
                scope_digest: scope_digest(&candidate_scope),
                snapshot: "10:20:12".to_owned(),
            },
            CONTRACT_INVALID,
        )
        .expect("fence wire");
        assert!(IndexDriftCandidateFence::new(encoded).is_ok());
        assert_ne!(
            scope_digest(&candidate_scope),
            scope_digest(&scope(Uuid::new_v4()))
        );
    }

    #[test]
    fn cursor_is_scope_bound_and_phase_typed() {
        let tenant_id = Uuid::new_v4();
        let candidate_scope = scope(tenant_id);
        let cursor = encode_cursor(
            &candidate_scope,
            CursorPhaseWire::Orphan {
                after: Some(OrphanPositionWire {
                    source_entity_id: Uuid::new_v4(),
                    source_locale_key: String::new(),
                    link_name: "variants".to_owned(),
                    ordinal: 0,
                    target_module: "catalog".to_owned(),
                    target_entity: "variant".to_owned(),
                    target_schema_version: 1,
                    target_entity_id: Uuid::new_v4(),
                    target_locale_key: String::new(),
                }),
            },
        )
        .expect("cursor");
        let request = IndexDriftCandidateRequest::new(
            candidate_scope.clone(),
            Some(
                IndexDriftCandidateFence::new(
                    encode_wire(
                        &FenceWire {
                            version: WIRE_VERSION,
                            scope_digest: scope_digest(&candidate_scope),
                            snapshot: "10:20:12".to_owned(),
                        },
                        CONTRACT_INVALID,
                    )
                    .expect("fence wire"),
                )
                .expect("fence"),
            ),
            Some(cursor),
            8,
        )
        .expect("request");
        assert!(matches!(
            decode_request_phase(&request).expect("phase"),
            ReadPhase::Orphan(Some(_))
        ));

        let foreign_scope = scope(Uuid::new_v4());
        let foreign_request = IndexDriftCandidateRequest::new(
            foreign_scope,
            request.fence().cloned(),
            request.cursor().cloned(),
            8,
        )
        .expect("foreign request");
        assert!(decode_request_phase(&foreign_request).is_err());
    }
}
