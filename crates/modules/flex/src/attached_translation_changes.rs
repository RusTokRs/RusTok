use async_trait::async_trait;
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction, FromQueryResult,
    Statement,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexAttachedTranslationChangeLifecycle {
    Active,
    Deleted,
}

impl FlexAttachedTranslationChangeLifecycle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Deleted => "deleted",
        }
    }

    pub fn parse(value: &str) -> FlexAttachedTranslationResult<Self> {
        match value {
            "active" => Ok(Self::Active),
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

impl FlexAttachedTranslationChangeRecord {
    pub fn validate(&self) -> FlexAttachedTranslationResult<()> {
        if self.change_seq == 0 {
            return Err(FlexAttachedTranslationError::OwnerInvariant(
                "attached Translation change cursor must be positive".to_string(),
            ));
        }
        if !is_valid_flex_entity_type(&self.entity_type) {
            return Err(FlexAttachedTranslationError::OwnerInvariant(format!(
                "attached Translation change has invalid entity type `{}`",
                self.entity_type
            )));
        }
        if self.entity_id.is_nil() {
            return Err(FlexAttachedTranslationError::OwnerInvariant(
                "attached Translation change must contain a non-nil entity id".to_string(),
            ));
        }
        if self.resource_revision.trim().is_empty() {
            return Err(FlexAttachedTranslationError::OwnerInvariant(
                "attached Translation change must contain a resource revision".to_string(),
            ));
        }
        Ok(())
    }
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

/// Flex-owned PostgreSQL read model for a single registered attached donor.
///
/// Storage and SQL stay inside Flex; the host only composes this capability with the donor
/// inventory/progress owner and the neutral TranslationTarget provider.
#[derive(Clone)]
pub struct FlexAttachedTranslationChangeReader {
    db: DatabaseConnection,
    entity_type: String,
}

impl FlexAttachedTranslationChangeReader {
    pub fn new(
        db: DatabaseConnection,
        entity_type: impl Into<String>,
    ) -> FlexAttachedTranslationResult<Self> {
        let entity_type = entity_type.into();
        if !is_valid_flex_entity_type(&entity_type) {
            return Err(FlexAttachedTranslationError::Invalid(format!(
                "invalid attached Translation change entity type `{entity_type}`"
            )));
        }
        Ok(Self { db, entity_type })
    }
}

#[derive(Debug, FromQueryResult)]
struct ChangeHighwaterRow {
    highwater: Option<i64>,
}

#[derive(Debug, FromQueryResult)]
struct ChangeRow {
    change_seq: i64,
    entity_type: String,
    entity_id: Uuid,
    resource_revision: String,
    lifecycle: String,
}

#[async_trait]
impl FlexAttachedTranslationChangeOwnerPort for FlexAttachedTranslationChangeReader {
    fn entity_type(&self) -> &str {
        &self.entity_type
    }

    async fn read_change_highwater(
        &self,
        tenant_id: Uuid,
    ) -> FlexAttachedTranslationResult<Option<u64>> {
        validate_tenant_id(tenant_id)?;
        ensure_change_postgres(&self.db)?;
        let row = ChangeHighwaterRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            format!(
                "SELECT MAX(change_seq) AS highwater FROM {FLEX_ATTACHED_TRANSLATION_CHANGE_JOURNAL_TABLE} WHERE tenant_id = $1 AND entity_type = $2"
            ),
            vec![tenant_id.into(), self.entity_type.clone().into()],
        ))
        .one(&self.db)
        .await
        .map_err(change_database_error)?
        .ok_or_else(|| {
            FlexAttachedTranslationError::OwnerInvariant(
                "attached Translation change high-water query returned no row".to_string(),
            )
        })?;
        row.highwater
            .map(|value| positive_sequence(value, "high-water"))
            .transpose()
    }

    async fn read_changes(
        &self,
        tenant_id: Uuid,
        after_seq: u64,
        through_seq: u64,
        limit: u16,
    ) -> FlexAttachedTranslationResult<Vec<FlexAttachedTranslationChangeRecord>> {
        validate_tenant_id(tenant_id)?;
        validate_flex_attached_translation_change_page(after_seq, through_seq, limit)?;
        if through_seq == 0 {
            return Ok(Vec::new());
        }
        ensure_change_postgres(&self.db)?;

        let rows = ChangeRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            format!(
                r#"
SELECT change_seq, entity_type, entity_id, resource_revision, lifecycle
FROM {FLEX_ATTACHED_TRANSLATION_CHANGE_JOURNAL_TABLE}
WHERE tenant_id = $1
  AND entity_type = $2
  AND change_seq > $3
  AND change_seq <= $4
ORDER BY change_seq ASC
LIMIT $5
"#
            ),
            vec![
                tenant_id.into(),
                self.entity_type.clone().into(),
                i64::try_from(after_seq)
                    .map_err(|_| invalid_sequence("after"))?
                    .into(),
                i64::try_from(through_seq)
                    .map_err(|_| invalid_sequence("through"))?
                    .into(),
                i64::from(limit).into(),
            ],
        ))
        .all(&self.db)
        .await
        .map_err(change_database_error)?;

        rows.into_iter().map(change_record_from_row).collect()
    }
}

pub fn validate_flex_attached_translation_change_page(
    after_seq: u64,
    through_seq: u64,
    limit: u16,
) -> FlexAttachedTranslationResult<()> {
    if through_seq < after_seq {
        return Err(FlexAttachedTranslationError::Invalid(
            "attached Translation change highwater must not precede the cursor".to_string(),
        ));
    }
    if limit == 0 || limit > MAX_FLEX_ATTACHED_TRANSLATION_CHANGE_PAGE {
        return Err(FlexAttachedTranslationError::Invalid(format!(
            "attached Translation change page limit must be between 1 and {MAX_FLEX_ATTACHED_TRANSLATION_CHANGE_PAGE}"
        )));
    }
    Ok(())
}

pub fn flex_attached_translation_deleted_revision(
    entity_type: &str,
    entity_id: Uuid,
) -> Result<String, FlexError> {
    if !is_valid_flex_entity_type(entity_type) {
        return Err(FlexError::UnknownEntityType(entity_type.to_string()));
    }
    if entity_id.is_nil() {
        return Err(FlexError::Database(
            "attached Translation deleted resource id must not be nil".to_string(),
        ));
    }
    Ok(format!("deleted:{entity_type}:{entity_id}"))
}

/// Append the final donor-owned deletion tombstone in the same transaction that deletes
/// the canonical resource. A tombstone is emitted only for a resource that previously had
/// attached Translation state; unrelated Category deletions do not pollute the cursor.
/// If localized-value triggers already recorded this resource in the transaction, the
/// transaction-scoped journal row is converted to the final deletion tombstone in place.
pub async fn record_flex_attached_translation_deleted_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
) -> Result<Option<String>, FlexError> {
    if tenant_id.is_nil() {
        return Err(FlexError::Database(
            "attached Translation deleted tenant id must not be nil".to_string(),
        ));
    }
    if !is_valid_flex_entity_type(entity_type) {
        return Err(FlexError::UnknownEntityType(entity_type.to_string()));
    }
    if entity_id.is_nil() {
        return Err(FlexError::Database(
            "attached Translation deleted resource id must not be nil".to_string(),
        ));
    }
    if txn.get_database_backend() != DatabaseBackend::Postgres {
        return Ok(None);
    }

    let state = txn
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            format!(
                "SELECT revision FROM {FLEX_ATTACHED_TRANSLATION_RESOURCE_STATE_TABLE} WHERE tenant_id = $1 AND entity_type = $2 AND entity_id = $3 FOR UPDATE"
            ),
            vec![
                tenant_id.into(),
                entity_type.to_string().into(),
                entity_id.into(),
            ],
        ))
        .await
        .map_err(database_error)?;
    let Some(_) = state else {
        return Ok(None);
    };

    let revision = flex_attached_translation_deleted_revision(entity_type, entity_id)?;
    txn.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        format!(
            "INSERT INTO {FLEX_ATTACHED_TRANSLATION_CHANGE_JOURNAL_TABLE} (tx_id, tenant_id, entity_type, entity_id, resource_revision, lifecycle) VALUES (txid_current(), $1, $2, $3, $4, 'deleted') ON CONFLICT (tx_id, tenant_id, entity_type, entity_id) DO UPDATE SET resource_revision = EXCLUDED.resource_revision, lifecycle = 'deleted'"
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
    txn.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        format!(
            "DELETE FROM {FLEX_ATTACHED_TRANSLATION_RESOURCE_STATE_TABLE} WHERE tenant_id = $1 AND entity_type = $2 AND entity_id = $3"
        ),
        vec![
            tenant_id.into(),
            entity_type.to_string().into(),
            entity_id.into(),
        ],
    ))
    .await
    .map_err(database_error)?;

    Ok(Some(revision))
}

fn change_record_from_row(
    row: ChangeRow,
) -> FlexAttachedTranslationResult<FlexAttachedTranslationChangeRecord> {
    let record = FlexAttachedTranslationChangeRecord {
        change_seq: positive_sequence(row.change_seq, "change")?,
        entity_type: row.entity_type,
        entity_id: row.entity_id,
        resource_revision: row.resource_revision,
        lifecycle: FlexAttachedTranslationChangeLifecycle::parse(&row.lifecycle)?,
    };
    record.validate()?;
    Ok(record)
}

fn ensure_change_postgres(db: &DatabaseConnection) -> FlexAttachedTranslationResult<()> {
    if db.get_database_backend() != DatabaseBackend::Postgres {
        return Err(FlexAttachedTranslationError::Invalid(
            "attached Translation ChangeCursor requires PostgreSQL".to_string(),
        ));
    }
    Ok(())
}

fn validate_tenant_id(tenant_id: Uuid) -> FlexAttachedTranslationResult<()> {
    if tenant_id.is_nil() {
        return Err(FlexAttachedTranslationError::Invalid(
            "attached Translation tenant_id must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn positive_sequence(value: i64, field: &str) -> FlexAttachedTranslationResult<u64> {
    let value = u64::try_from(value).map_err(|_| invalid_sequence(field))?;
    if value == 0 {
        return Err(invalid_sequence(field));
    }
    Ok(value)
}

fn invalid_sequence(field: &str) -> FlexAttachedTranslationError {
    FlexAttachedTranslationError::OwnerInvariant(format!(
        "attached Translation change {field} sequence must be positive"
    ))
}

fn change_database_error(error: sea_orm::DbErr) -> FlexAttachedTranslationError {
    FlexAttachedTranslationError::Storage(error.to_string())
}

fn database_error(error: sea_orm::DbErr) -> FlexError {
    FlexError::Database(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_round_trips() {
        for lifecycle in [
            FlexAttachedTranslationChangeLifecycle::Active,
            FlexAttachedTranslationChangeLifecycle::Deleted,
        ] {
            assert_eq!(
                FlexAttachedTranslationChangeLifecycle::parse(lifecycle.as_str()).unwrap(),
                lifecycle
            );
        }
    }

    #[test]
    fn record_requires_event_time_revision() {
        let record = FlexAttachedTranslationChangeRecord {
            change_seq: 1,
            entity_type: "taxonomy.category".to_string(),
            entity_id: Uuid::new_v4(),
            resource_revision: "attached:7".to_string(),
            lifecycle: FlexAttachedTranslationChangeLifecycle::Active,
        };
        assert!(record.validate().is_ok());
    }

    #[test]
    fn deleted_revision_is_stable_and_scoped() {
        let entity_id = Uuid::new_v4();
        assert_eq!(
            flex_attached_translation_deleted_revision("taxonomy.category", entity_id).unwrap(),
            format!("deleted:taxonomy.category:{entity_id}")
        );
    }
}
