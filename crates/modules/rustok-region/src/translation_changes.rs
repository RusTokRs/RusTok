use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseTransaction, EntityTrait, FromQueryResult,
    QueryFilter, QueryOrder, Statement,
};
use uuid::Uuid;

use rustok_commerce_foundation::entities;

use crate::{RegionError, RegionResult, RegionTranslationService};

pub const MAX_REGION_TRANSLATION_CHANGE_PAGE: u16 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionTranslationChangeLifecycle {
    Active,
    Deleted,
}

impl RegionTranslationChangeLifecycle {
    fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Deleted => "deleted",
        }
    }

    fn parse(value: &str) -> RegionResult<Self> {
        match value {
            "active" => Ok(Self::Active),
            "deleted" => Ok(Self::Deleted),
            _ => Err(RegionError::Validation(
                "Region translation change journal returned an invalid lifecycle".to_string(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionTranslationChangeRecord {
    pub change_seq: u64,
    pub region_id: Uuid,
    pub resource_revision: String,
    pub lifecycle: RegionTranslationChangeLifecycle,
}

#[derive(Debug, FromQueryResult)]
struct HighwaterRow {
    highwater: Option<i64>,
}

#[derive(Debug, FromQueryResult)]
struct ChangeRow {
    change_seq: i64,
    region_id: Uuid,
    resource_revision: String,
    lifecycle: String,
}

#[derive(Debug, FromQueryResult)]
struct PreviousChangeRow {
    resource_revision: String,
    lifecycle: String,
}

impl RegionTranslationService {
    pub async fn region_translation_change_highwater(
        &self,
        tenant_id: Uuid,
    ) -> RegionResult<Option<u64>> {
        validate_tenant(tenant_id)?;
        let backend = self.database().get_database_backend();
        let sql = match backend {
            DatabaseBackend::Postgres => {
                "SELECT MAX(change_seq) AS highwater FROM region_translation_change_journal WHERE tenant_id = $1"
            }
            _ => {
                "SELECT MAX(change_seq) AS highwater FROM region_translation_change_journal WHERE tenant_id = ?"
            }
        };
        let row = HighwaterRow::find_by_statement(Statement::from_sql_and_values(
            backend,
            sql,
            vec![tenant_id.into()],
        ))
        .one(self.database())
        .await?
        .ok_or_else(|| {
            RegionError::Validation(
                "Region translation change high-water query returned no row".to_string(),
            )
        })?;
        optional_positive_sequence(row.highwater, "high-water")
    }

    pub async fn read_region_translation_changes(
        &self,
        tenant_id: Uuid,
        after_seq: u64,
        through_seq: u64,
        limit: u16,
    ) -> RegionResult<Vec<RegionTranslationChangeRecord>> {
        validate_tenant(tenant_id)?;
        if through_seq == 0 || after_seq > through_seq {
            return Err(RegionError::Validation(
                "Region translation change cursor bounds are invalid".to_string(),
            ));
        }
        if limit == 0 || limit > MAX_REGION_TRANSLATION_CHANGE_PAGE {
            return Err(RegionError::Validation(format!(
                "Region translation change page size must be between 1 and {MAX_REGION_TRANSLATION_CHANGE_PAGE}"
            )));
        }

        let backend = self.database().get_database_backend();
        let sql = match backend {
            DatabaseBackend::Postgres => {
                r#"
SELECT change_seq, region_id, resource_revision, lifecycle
FROM region_translation_change_journal
WHERE tenant_id = $1
  AND change_seq > $2
  AND change_seq <= $3
ORDER BY change_seq ASC
LIMIT $4
"#
            }
            _ => {
                r#"
SELECT change_seq, region_id, resource_revision, lifecycle
FROM region_translation_change_journal
WHERE tenant_id = ?
  AND change_seq > ?
  AND change_seq <= ?
ORDER BY change_seq ASC
LIMIT ?
"#
            }
        };
        let rows = ChangeRow::find_by_statement(Statement::from_sql_and_values(
            backend,
            sql,
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

pub(crate) async fn record_current_region_translation_change_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    region_id: Uuid,
    operation_id: Uuid,
) -> RegionResult<()> {
    validate_identity(tenant_id, region_id, operation_id)?;
    let region = entities::region::Entity::find_by_id(region_id)
        .filter(entities::region::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or(RegionError::RegionNotFound(region_id))?;
    let translations = entities::region_translation::Entity::find()
        .filter(entities::region_translation::Column::RegionId.eq(region_id))
        .order_by_asc(entities::region_translation::Column::Locale)
        .all(txn)
        .await?;
    let revision = crate::services::translation::resource_revision(&region, &translations);
    record_region_translation_change_in_tx(
        txn,
        tenant_id,
        region_id,
        operation_id,
        &revision,
        RegionTranslationChangeLifecycle::Active,
    )
    .await
}

pub(crate) async fn record_region_translation_change_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    region_id: Uuid,
    operation_id: Uuid,
    resource_revision: &str,
    lifecycle: RegionTranslationChangeLifecycle,
) -> RegionResult<()> {
    validate_identity(tenant_id, region_id, operation_id)?;
    if resource_revision.trim().is_empty() {
        return Err(RegionError::Validation(
            "Region translation change revision must not be blank".to_string(),
        ));
    }

    let backend = txn.get_database_backend();
    let previous_sql = match backend {
        DatabaseBackend::Postgres => {
            r#"
SELECT resource_revision, lifecycle
FROM region_translation_change_journal
WHERE tenant_id = $1 AND region_id = $2
ORDER BY change_seq DESC
LIMIT 1
"#
        }
        _ => {
            r#"
SELECT resource_revision, lifecycle
FROM region_translation_change_journal
WHERE tenant_id = ? AND region_id = ?
ORDER BY change_seq DESC
LIMIT 1
"#
        }
    };
    if let Some(previous) = PreviousChangeRow::find_by_statement(Statement::from_sql_and_values(
        backend,
        previous_sql,
        vec![tenant_id.into(), region_id.into()],
    ))
    .one(txn)
    .await?
    {
        if previous.resource_revision == resource_revision
            && RegionTranslationChangeLifecycle::parse(&previous.lifecycle)? == lifecycle
        {
            return Ok(());
        }
    }

    let insert_sql = match backend {
        DatabaseBackend::Postgres => {
            r#"
INSERT INTO region_translation_change_journal (
    operation_id, tenant_id, region_id, resource_revision, lifecycle
) VALUES ($1, $2, $3, $4, $5)
"#
        }
        _ => {
            r#"
INSERT INTO region_translation_change_journal (
    operation_id, tenant_id, region_id, resource_revision, lifecycle
) VALUES (?, ?, ?, ?, ?)
"#
        }
    };
    txn.execute_raw(Statement::from_sql_and_values(
        backend,
        insert_sql,
        vec![
            operation_id.into(),
            tenant_id.into(),
            region_id.into(),
            resource_revision.to_string().into(),
            lifecycle.as_str().into(),
        ],
    ))
    .await?;
    Ok(())
}

fn change_record_from_row(row: ChangeRow) -> RegionResult<RegionTranslationChangeRecord> {
    let change_seq = positive_sequence(row.change_seq, "change")?;
    if row.region_id.is_nil() || row.resource_revision.trim().is_empty() {
        return Err(RegionError::Validation(
            "Region translation change journal returned an invalid row".to_string(),
        ));
    }
    Ok(RegionTranslationChangeRecord {
        change_seq,
        region_id: row.region_id,
        resource_revision: row.resource_revision,
        lifecycle: RegionTranslationChangeLifecycle::parse(&row.lifecycle)?,
    })
}

fn validate_tenant(tenant_id: Uuid) -> RegionResult<()> {
    if tenant_id.is_nil() {
        return Err(RegionError::Validation(
            "Region translation change tenant must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn validate_identity(tenant_id: Uuid, region_id: Uuid, operation_id: Uuid) -> RegionResult<()> {
    validate_tenant(tenant_id)?;
    if region_id.is_nil() || operation_id.is_nil() {
        return Err(RegionError::Validation(
            "Region translation change identity must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn optional_positive_sequence(value: Option<i64>, field: &str) -> RegionResult<Option<u64>> {
    value.map(|value| positive_sequence(value, field)).transpose()
}

fn positive_sequence(value: i64, field: &str) -> RegionResult<u64> {
    let value = u64::try_from(value).map_err(|_| invalid_sequence(field))?;
    if value == 0 {
        return Err(invalid_sequence(field));
    }
    Ok(value)
}

fn invalid_sequence(field: &str) -> RegionError {
    RegionError::Validation(format!(
        "Region translation change {field} sequence must be positive"
    ))
}
