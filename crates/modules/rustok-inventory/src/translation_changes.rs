use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseTransaction, FromQueryResult, Statement,
};
use uuid::Uuid;

use crate::{
    StockLocationTranslationExactLocaleError, StockLocationTranslationExactLocaleResult,
    StockLocationTranslationService,
};

pub const MAX_STOCK_LOCATION_TRANSLATION_CHANGE_PAGE: u16 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StockLocationTranslationChangeLifecycle {
    Active,
    Deleted,
}

impl StockLocationTranslationChangeLifecycle {
    fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Deleted => "deleted",
        }
    }

    fn parse(value: &str) -> StockLocationTranslationExactLocaleResult<Self> {
        match value {
            "active" => Ok(Self::Active),
            "deleted" => Ok(Self::Deleted),
            _ => Err(StockLocationTranslationExactLocaleError::Validation(
                "Inventory translation change journal returned an invalid lifecycle".to_string(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockLocationTranslationChangeRecord {
    pub change_seq: u64,
    pub stock_location_id: Uuid,
    pub resource_revision: String,
    pub lifecycle: StockLocationTranslationChangeLifecycle,
}

#[derive(Debug, FromQueryResult)]
struct HighwaterRow {
    highwater: Option<i64>,
}

#[derive(Debug, FromQueryResult)]
struct ChangeRow {
    change_seq: i64,
    stock_location_id: Uuid,
    resource_revision: String,
    lifecycle: String,
}

#[derive(Debug, FromQueryResult)]
struct PreviousChangeRow {
    resource_revision: String,
    lifecycle: String,
}

impl StockLocationTranslationService {
    pub async fn stock_location_translation_change_highwater(
        &self,
        tenant_id: Uuid,
    ) -> StockLocationTranslationExactLocaleResult<Option<u64>> {
        validate_tenant(tenant_id)?;
        let backend = self.database().get_database_backend();
        let sql = match backend {
            DatabaseBackend::Postgres => {
                "SELECT MAX(change_seq) AS highwater FROM stock_location_translation_change_journal WHERE tenant_id = $1"
            }
            _ => {
                "SELECT MAX(change_seq) AS highwater FROM stock_location_translation_change_journal WHERE tenant_id = ?"
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
            StockLocationTranslationExactLocaleError::Validation(
                "Inventory translation change high-water query returned no row".to_string(),
            )
        })?;
        optional_positive_sequence(row.highwater, "high-water")
    }

    pub async fn read_stock_location_translation_changes(
        &self,
        tenant_id: Uuid,
        after_seq: u64,
        through_seq: u64,
        limit: u16,
    ) -> StockLocationTranslationExactLocaleResult<Vec<StockLocationTranslationChangeRecord>> {
        validate_tenant(tenant_id)?;
        if through_seq == 0 || after_seq > through_seq {
            return Err(StockLocationTranslationExactLocaleError::Validation(
                "Inventory translation change cursor bounds are invalid".to_string(),
            ));
        }
        if limit == 0 || limit > MAX_STOCK_LOCATION_TRANSLATION_CHANGE_PAGE {
            return Err(StockLocationTranslationExactLocaleError::Validation(format!(
                "Inventory translation change page size must be between 1 and {MAX_STOCK_LOCATION_TRANSLATION_CHANGE_PAGE}"
            )));
        }

        let backend = self.database().get_database_backend();
        let sql = match backend {
            DatabaseBackend::Postgres => {
                r#"
SELECT change_seq, stock_location_id, resource_revision, lifecycle
FROM stock_location_translation_change_journal
WHERE tenant_id = $1
  AND change_seq > $2
  AND change_seq <= $3
ORDER BY change_seq ASC
LIMIT $4
"#
            }
            _ => {
                r#"
SELECT change_seq, stock_location_id, resource_revision, lifecycle
FROM stock_location_translation_change_journal
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

pub(crate) async fn record_stock_location_translation_change_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    stock_location_id: Uuid,
    operation_id: Uuid,
    resource_revision: &str,
    lifecycle: StockLocationTranslationChangeLifecycle,
) -> StockLocationTranslationExactLocaleResult<()> {
    validate_identity(tenant_id, stock_location_id, operation_id)?;
    if resource_revision.trim().is_empty() {
        return Err(StockLocationTranslationExactLocaleError::Validation(
            "Inventory translation change revision must not be blank".to_string(),
        ));
    }

    let backend = txn.get_database_backend();
    let previous_sql = match backend {
        DatabaseBackend::Postgres => {
            r#"
SELECT resource_revision, lifecycle
FROM stock_location_translation_change_journal
WHERE tenant_id = $1 AND stock_location_id = $2
ORDER BY change_seq DESC
LIMIT 1
"#
        }
        _ => {
            r#"
SELECT resource_revision, lifecycle
FROM stock_location_translation_change_journal
WHERE tenant_id = ? AND stock_location_id = ?
ORDER BY change_seq DESC
LIMIT 1
"#
        }
    };
    if let Some(previous) = PreviousChangeRow::find_by_statement(Statement::from_sql_and_values(
        backend,
        previous_sql,
        vec![tenant_id.into(), stock_location_id.into()],
    ))
    .one(txn)
    .await?
    {
        if previous.resource_revision == resource_revision
            && StockLocationTranslationChangeLifecycle::parse(&previous.lifecycle)? == lifecycle
        {
            return Ok(());
        }
    }

    let insert_sql = match backend {
        DatabaseBackend::Postgres => {
            r#"
INSERT INTO stock_location_translation_change_journal (
    operation_id, tenant_id, stock_location_id, resource_revision, lifecycle
) VALUES ($1, $2, $3, $4, $5)
"#
        }
        _ => {
            r#"
INSERT INTO stock_location_translation_change_journal (
    operation_id, tenant_id, stock_location_id, resource_revision, lifecycle
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
            stock_location_id.into(),
            resource_revision.to_string().into(),
            lifecycle.as_str().into(),
        ],
    ))
    .await?;
    Ok(())
}

fn change_record_from_row(
    row: ChangeRow,
) -> StockLocationTranslationExactLocaleResult<StockLocationTranslationChangeRecord> {
    let change_seq = positive_sequence(row.change_seq, "change")?;
    if row.stock_location_id.is_nil() || row.resource_revision.trim().is_empty() {
        return Err(StockLocationTranslationExactLocaleError::Validation(
            "Inventory translation change journal returned an invalid row".to_string(),
        ));
    }
    Ok(StockLocationTranslationChangeRecord {
        change_seq,
        stock_location_id: row.stock_location_id,
        resource_revision: row.resource_revision,
        lifecycle: StockLocationTranslationChangeLifecycle::parse(&row.lifecycle)?,
    })
}

fn validate_tenant(tenant_id: Uuid) -> StockLocationTranslationExactLocaleResult<()> {
    if tenant_id.is_nil() {
        return Err(StockLocationTranslationExactLocaleError::Validation(
            "Inventory translation change tenant must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn validate_identity(
    tenant_id: Uuid,
    stock_location_id: Uuid,
    operation_id: Uuid,
) -> StockLocationTranslationExactLocaleResult<()> {
    validate_tenant(tenant_id)?;
    if stock_location_id.is_nil() || operation_id.is_nil() {
        return Err(StockLocationTranslationExactLocaleError::Validation(
            "Inventory translation change identity must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn optional_positive_sequence(
    value: Option<i64>,
    field: &str,
) -> StockLocationTranslationExactLocaleResult<Option<u64>> {
    value.map(|value| positive_sequence(value, field)).transpose()
}

fn positive_sequence(
    value: i64,
    field: &str,
) -> StockLocationTranslationExactLocaleResult<u64> {
    let value = u64::try_from(value).map_err(|_| invalid_sequence(field))?;
    if value == 0 {
        return Err(invalid_sequence(field));
    }
    Ok(value)
}

fn invalid_sequence(field: &str) -> StockLocationTranslationExactLocaleError {
    StockLocationTranslationExactLocaleError::Validation(format!(
        "Inventory translation change {field} sequence must be positive"
    ))
}
