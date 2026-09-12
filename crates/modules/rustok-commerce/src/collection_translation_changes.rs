use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseTransaction, FromQueryResult, Statement};
use uuid::Uuid;

use crate::{
    CommerceError, CommerceResult, services::collection_translation::CollectionTranslationService,
};

pub const MAX_COLLECTION_TRANSLATION_CHANGE_PAGE: u16 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionTranslationChangeLifecycle {
    Active,
    Deleted,
}

impl CollectionTranslationChangeLifecycle {
    fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Deleted => "deleted",
        }
    }

    fn parse(value: &str) -> CommerceResult<Self> {
        match value {
            "active" => Ok(Self::Active),
            "deleted" => Ok(Self::Deleted),
            _ => Err(CommerceError::Validation(
                "Commerce Collection translation change journal returned an invalid lifecycle"
                    .to_owned(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionTranslationChangeRecord {
    pub change_seq: u64,
    pub collection_id: Uuid,
    pub resource_revision: String,
    pub lifecycle: CollectionTranslationChangeLifecycle,
}

#[derive(Debug, FromQueryResult)]
struct HighwaterRow {
    highwater: Option<i64>,
}

#[derive(Debug, FromQueryResult)]
struct ChangeRow {
    change_seq: i64,
    collection_id: Uuid,
    resource_revision: String,
    lifecycle: String,
}

#[derive(Debug, FromQueryResult)]
struct PreviousChangeRow {
    resource_revision: String,
    lifecycle: String,
}

impl CollectionTranslationService {
    pub async fn collection_translation_change_highwater(
        &self,
        tenant_id: Uuid,
    ) -> CommerceResult<Option<u64>> {
        validate_tenant(tenant_id)?;
        ensure_postgres(self.database())?;
        let row = HighwaterRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT MAX(change_seq) AS highwater FROM commerce_collection_translation_change_journal WHERE tenant_id = $1",
            vec![tenant_id.into()],
        ))
        .one(self.database())
        .await?
        .ok_or_else(|| {
            CommerceError::Validation(
                "Commerce Collection translation change high-water query returned no row".to_owned(),
            )
        })?;
        optional_positive_sequence(row.highwater, "high-water")
    }

    pub async fn read_collection_translation_changes(
        &self,
        tenant_id: Uuid,
        after_seq: u64,
        through_seq: u64,
        limit: u16,
    ) -> CommerceResult<Vec<CollectionTranslationChangeRecord>> {
        validate_tenant(tenant_id)?;
        if through_seq == 0 || after_seq > through_seq {
            return Err(CommerceError::Validation(
                "Commerce Collection translation change cursor bounds are invalid".to_owned(),
            ));
        }
        if limit == 0 || limit > MAX_COLLECTION_TRANSLATION_CHANGE_PAGE {
            return Err(CommerceError::Validation(format!(
                "Commerce Collection translation change page size must be between 1 and {MAX_COLLECTION_TRANSLATION_CHANGE_PAGE}"
            )));
        }
        ensure_postgres(self.database())?;

        let rows = ChangeRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
SELECT change_seq, collection_id, resource_revision, lifecycle
FROM commerce_collection_translation_change_journal
WHERE tenant_id = $1
  AND change_seq > $2
  AND change_seq <= $3
ORDER BY change_seq ASC
LIMIT $4
"#,
            vec![
                tenant_id.into(),
                i64::try_from(after_seq)
                    .map_err(|_| invalid_sequence("after"))?
                    .into(),
                i64::try_from(through_seq)
                    .map_err(|_| invalid_sequence("through"))?
                    .into(),
                i64::from(limit).into(),
            ],
        ))
        .all(self.database())
        .await?;

        rows.into_iter().map(change_record_from_row).collect()
    }
}

/// Records the exact post-command active Collection translation state under the same
/// root outbox envelope and owner transaction that made the semantic change.
/// Repeated envelopes or unrelated owner events are harmless: root/target
/// uniqueness and semantic revision dedupe suppress duplicate evidence.
pub(crate) async fn record_collection_translation_change_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    collection_id: Uuid,
    root_event_id: Uuid,
    resource_revision: &str,
) -> CommerceResult<()> {
    record_collection_translation_lifecycle_change_in_tx(
        txn,
        tenant_id,
        collection_id,
        root_event_id,
        resource_revision,
        CollectionTranslationChangeLifecycle::Active,
    )
    .await
}

/// Records owner lifecycle evidence in the same transaction as the Collection mutation.
/// Deleted rows deliberately retain the last active resource revision so bounded consumers can
/// invalidate exactly the state they observed before the soft delete.
pub(crate) async fn record_collection_translation_lifecycle_change_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    collection_id: Uuid,
    root_event_id: Uuid,
    resource_revision: &str,
    lifecycle: CollectionTranslationChangeLifecycle,
) -> CommerceResult<()> {
    if txn.get_database_backend() != DatabaseBackend::Postgres {
        return Ok(());
    }
    if tenant_id.is_nil() || collection_id.is_nil() || root_event_id.is_nil() {
        return Err(CommerceError::Validation(
            "Commerce Collection translation change journal identity must not be nil".to_owned(),
        ));
    }
    if resource_revision.trim().is_empty() {
        return Err(CommerceError::Validation(
            "Commerce Collection translation change revision must not be blank".to_owned(),
        ));
    }

    if let Some(previous) = PreviousChangeRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
SELECT resource_revision, lifecycle
FROM commerce_collection_translation_change_journal
WHERE tenant_id = $1
  AND collection_id = $2
ORDER BY change_seq DESC
LIMIT 1
"#,
        vec![tenant_id.into(), collection_id.into()],
    ))
    .one(txn)
    .await?
    {
        let previous_lifecycle = CollectionTranslationChangeLifecycle::parse(&previous.lifecycle)?;
        if previous.resource_revision == resource_revision && previous_lifecycle == lifecycle {
            return Ok(());
        }
    }

    txn.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
INSERT INTO commerce_collection_translation_change_journal (
    root_event_id,
    tenant_id,
    collection_id,
    resource_revision,
    lifecycle,
    created_at
) VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP)
ON CONFLICT (root_event_id, collection_id) DO NOTHING
"#,
        vec![
            root_event_id.into(),
            tenant_id.into(),
            collection_id.into(),
            resource_revision.to_owned().into(),
            lifecycle.as_str().into(),
        ],
    ))
    .await?;
    Ok(())
}

fn change_record_from_row(row: ChangeRow) -> CommerceResult<CollectionTranslationChangeRecord> {
    let change_seq = positive_sequence(row.change_seq, "change")?;
    if row.collection_id.is_nil() || row.resource_revision.trim().is_empty() {
        return Err(CommerceError::Validation(
            "Commerce Collection translation change journal returned an invalid row".to_owned(),
        ));
    }
    Ok(CollectionTranslationChangeRecord {
        change_seq,
        collection_id: row.collection_id,
        resource_revision: row.resource_revision,
        lifecycle: CollectionTranslationChangeLifecycle::parse(&row.lifecycle)?,
    })
}

fn validate_tenant(tenant_id: Uuid) -> CommerceResult<()> {
    if tenant_id.is_nil() {
        return Err(CommerceError::Validation(
            "Commerce Collection translation change tenant must not be nil".to_owned(),
        ));
    }
    Ok(())
}

fn ensure_postgres(service_db: &sea_orm::DatabaseConnection) -> CommerceResult<()> {
    if service_db.get_database_backend() != DatabaseBackend::Postgres {
        return Err(CommerceError::Validation(
            "Commerce Collection translation change source requires PostgreSQL".to_owned(),
        ));
    }
    Ok(())
}

fn optional_positive_sequence(value: Option<i64>, field: &str) -> CommerceResult<Option<u64>> {
    value
        .map(|value| positive_sequence(value, field))
        .transpose()
}

fn positive_sequence(value: i64, field: &str) -> CommerceResult<u64> {
    let value = u64::try_from(value).map_err(|_| invalid_sequence(field))?;
    if value == 0 {
        return Err(invalid_sequence(field));
    }
    Ok(value)
}

fn invalid_sequence(field: &str) -> CommerceError {
    CommerceError::Validation(format!(
        "Commerce Collection translation change {field} sequence must be positive"
    ))
}