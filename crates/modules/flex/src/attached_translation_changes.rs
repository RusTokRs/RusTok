use std::collections::BTreeMap;

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder, Statement,
};
use uuid::Uuid;

use rustok_core::field_schema::FlexError;

use crate::{
    FlexAttachedTranslationError, FlexAttachedTranslationResult, is_valid_flex_entity_type,
};

pub const FLEX_ATTACHED_TRANSLATION_RESOURCE_STATE_TABLE: &str =
    "flex_attached_translation_resource_state";
pub const FLEX_ATTACHED_TRANSLATION_CHANGE_JOURNAL_TABLE: &str =
    "flex_attached_translation_change_journal";
pub const MAX_FLEX_ATTACHED_TRANSLATION_CHANGE_PAGE: u16 = 200;
pub const MAX_FLEX_ATTACHED_TRANSLATION_REVISION_BATCH: usize = 200;
const RESOURCE_REVISION_PREFIX: &str = "flex-attached-resource:";

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "flex_attached_translation_resource_state")]
struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    tenant_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    entity_type: String,
    #[sea_orm(primary_key, auto_increment = false)]
    entity_id: Uuid,
    revision: i64,
    updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexAttachedTranslationChangeLifecycle {
    Active,
    Unavailable,
    Deleted,
}

impl FlexAttachedTranslationChangeLifecycle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Unavailable => "unavailable",
            Self::Deleted => "deleted",
        }
    }

    pub fn parse(value: &str) -> FlexAttachedTranslationResult<Self> {
        match value {
            "active" => Ok(Self::Active),
            "unavailable" => Ok(Self::Unavailable),
            "deleted" => Ok(Self::Deleted),
            other => Err(FlexAttachedTranslationError::OwnerInvariant(format!(
                "attached Translation change journal contains invalid lifecycle `{other}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlexAttachedTranslationChangeRecord {
    pub change_seq: u64,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub resource_revision: String,
    pub lifecycle: FlexAttachedTranslationChangeLifecycle,
}

#[async_trait]
pub trait FlexAttachedTranslationChangeOwnerPort: Send + Sync {
    fn entity_type(&self) -> &str;

    async fn read_change_highwater(
        &self,
        tenant_id: Uuid,
    ) -> FlexAttachedTranslationResult<Option<u64>>;

    async fn read_changes(
        &self,
        tenant_id: Uuid,
        after_seq: u64,
        through_seq: u64,
        limit: u16,
    ) -> FlexAttachedTranslationResult<Vec<FlexAttachedTranslationChangeRecord>>;
}

pub fn flex_attached_translation_resource_revision(
    revision: u64,
) -> FlexAttachedTranslationResult<String> {
    if revision == 0 {
        return Err(FlexAttachedTranslationError::OwnerInvariant(
            "attached Translation resource revision must be positive".to_string(),
        ));
    }
    Ok(format!("{RESOURCE_REVISION_PREFIX}{revision}"))
}

pub async fn load_attached_translation_resource_revisions<C>(
    connection: &C,
    tenant_id: Uuid,
    entity_type: &str,
    entity_ids: &[Uuid],
) -> Result<BTreeMap<Uuid, u64>, FlexError>
where
    C: ConnectionTrait,
{
    if !is_valid_flex_entity_type(entity_type) {
        return Err(FlexError::UnknownEntityType(entity_type.to_string()));
    }
    if entity_ids.len() > MAX_FLEX_ATTACHED_TRANSLATION_REVISION_BATCH {
        return Err(FlexError::Database(format!(
            "attached Translation revision batch exceeds {MAX_FLEX_ATTACHED_TRANSLATION_REVISION_BATCH} entities"
        )));
    }
    if entity_ids.is_empty() {
        return Ok(BTreeMap::new());
    }

    let rows = Entity::find()
        .filter(Column::TenantId.eq(tenant_id))
        .filter(Column::EntityType.eq(entity_type))
        .filter(Column::EntityId.is_in(entity_ids.to_vec()))
        .order_by_asc(Column::EntityId)
        .all(connection)
        .await
        .map_err(database_error)?;

    let mut revisions = BTreeMap::new();
    for row in rows {
        let revision = positive_revision(row.entity_id, row.revision)?;
        if revisions.insert(row.entity_id, revision).is_some() {
            return Err(FlexError::Database(format!(
                "duplicate attached Translation revision state for {entity_type}/{}",
                row.entity_id
            )));
        }
    }
    Ok(revisions)
}

/// Finalize the Translation lifecycle of an attached resource during the donor-owned delete
/// transaction. Localized-value DELETE triggers may already have coalesced an `unavailable`
/// journal row in this transaction; this helper overwrites that same transaction entry with
/// the final `deleted` tombstone and then removes the live revision state.
pub async fn record_attached_translation_deleted_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
) -> Result<Option<String>, FlexError> {
    if !is_valid_flex_entity_type(entity_type) {
        return Err(FlexError::UnknownEntityType(entity_type.to_string()));
    }

    let row = Entity::find()
        .filter(Column::TenantId.eq(tenant_id))
        .filter(Column::EntityType.eq(entity_type))
        .filter(Column::EntityId.eq(entity_id))
        .lock_exclusive()
        .one(txn)
        .await
        .map_err(database_error)?;
    let Some(row) = row else {
        return Ok(None);
    };

    let current = positive_revision(entity_id, row.revision)?;
    let next = current.checked_add(1).ok_or_else(|| {
        FlexError::Database(format!(
            "attached Translation resource revision is exhausted for {entity_type}/{entity_id}"
        ))
    })?;
    let next_i64 = i64::try_from(next).map_err(|_| {
        FlexError::Database(format!(
            "attached Translation resource revision exceeds i64 for {entity_type}/{entity_id}"
        ))
    })?;

    let mut active: ActiveModel = row.into();
    active.revision = Set(next_i64);
    active.updated_at = Set(Utc::now().fixed_offset());
    active.update(txn).await.map_err(database_error)?;

    let revision = flex_attached_translation_resource_revision(next)
        .map_err(|error| FlexError::Database(error.to_string()))?;
    if txn.get_database_backend() == DatabaseBackend::Postgres {
        txn.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            format!(
                "INSERT INTO {FLEX_ATTACHED_TRANSLATION_CHANGE_JOURNAL_TABLE} \
                 (root_txid, tenant_id, entity_type, entity_id, resource_revision, lifecycle, created_at) \
                 VALUES (txid_current(), $1, $2, $3, $4, 'deleted', CURRENT_TIMESTAMP) \
                 ON CONFLICT (root_txid, tenant_id, entity_type, entity_id) DO UPDATE SET \
                 resource_revision = EXCLUDED.resource_revision, lifecycle = 'deleted', created_at = CURRENT_TIMESTAMP"
            ),
            vec![
                tenant_id.into(),
                entity_type.to_string().into(),
                entity_id.into(),
                revision.clone().into(),
            ],
        ))
        .await
        .map_err(database_error)?;
    }

    Entity::delete_many()
        .filter(Column::TenantId.eq(tenant_id))
        .filter(Column::EntityType.eq(entity_type))
        .filter(Column::EntityId.eq(entity_id))
        .exec(txn)
        .await
        .map_err(database_error)?;

    Ok(Some(revision))
}

fn positive_revision(entity_id: Uuid, revision: i64) -> Result<u64, FlexError> {
    let revision = u64::try_from(revision).map_err(|_| {
        FlexError::Database(format!(
            "attached Translation resource {entity_id} has a negative revision"
        ))
    })?;
    if revision == 0 {
        return Err(FlexError::Database(format!(
            "attached Translation resource {entity_id} has a zero revision"
        )));
    }
    Ok(revision)
}

fn database_error(error: sea_orm::DbErr) -> FlexError {
    FlexError::Database(error.to_string())
}
