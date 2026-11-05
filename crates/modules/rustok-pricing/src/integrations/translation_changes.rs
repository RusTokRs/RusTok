use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseTransaction, FromQueryResult, Statement};
use uuid::Uuid;

use crate::{
    PriceListTranslationExactLocaleError, PriceListTranslationExactLocaleResult,
    PriceListTranslationService,
};

pub const MAX_PRICE_LIST_TRANSLATION_CHANGE_PAGE: u16 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceListTranslationChangeLifecycle {
    Active,
    Deleted,
}

impl PriceListTranslationChangeLifecycle {
    fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Deleted => "deleted",
        }
    }

    fn parse(value: &str) -> PriceListTranslationExactLocaleResult<Self> {
        match value {
            "active" => Ok(Self::Active),
            "deleted" => Ok(Self::Deleted),
            _ => Err(PriceListTranslationExactLocaleError::Validation(
                "Pricing translation change journal returned an invalid lifecycle".to_string(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceListTranslationChangeRecord {
    pub change_seq: u64,
    pub price_list_id: Uuid,
    pub resource_revision: String,
    pub lifecycle: PriceListTranslationChangeLifecycle,
}

#[derive(Debug, FromQueryResult)]
struct HighwaterRow {
    highwater: Option<i64>,
}

#[derive(Debug, FromQueryResult)]
struct ChangeRow {
    change_seq: i64,
    price_list_id: Uuid,
    resource_revision: String,
    lifecycle: String,
}

#[derive(Debug, FromQueryResult)]
struct PreviousChangeRow {
    resource_revision: String,
    lifecycle: String,
}

impl PriceListTranslationService {
    pub async fn price_list_translation_change_highwater(
        &self,
        tenant_id: Uuid,
    ) -> PriceListTranslationExactLocaleResult<Option<u64>> {
        validate_tenant(tenant_id)?;
        let backend = self.database().get_database_backend();
        let sql = match backend {
            DatabaseBackend::Postgres => {
                "SELECT MAX(change_seq) AS highwater FROM price_list_translation_change_journal WHERE tenant_id = $1"
            }
            _ => {
                "SELECT MAX(change_seq) AS highwater FROM price_list_translation_change_journal WHERE tenant_id = ?"
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
            PriceListTranslationExactLocaleError::Validation(
                "Pricing translation change high-water query returned no row".to_string(),
            )
        })?;
        optional_positive_sequence(row.highwater, "high-water")
    }

    pub async fn read_price_list_translation_changes(
        &self,
        tenant_id: Uuid,
        after_seq: u64,
        through_seq: u64,
        limit: u16,
    ) -> PriceListTranslationExactLocaleResult<Vec<PriceListTranslationChangeRecord>> {
        validate_tenant(tenant_id)?;
        if through_seq == 0 || after_seq > through_seq {
            return Err(PriceListTranslationExactLocaleError::Validation(
                "Pricing translation change cursor bounds are invalid".to_string(),
            ));
        }
        if limit == 0 || limit > MAX_PRICE_LIST_TRANSLATION_CHANGE_PAGE {
            return Err(PriceListTranslationExactLocaleError::Validation(format!(
                "Pricing translation change page size must be between 1 and {MAX_PRICE_LIST_TRANSLATION_CHANGE_PAGE}"
            )));
        }

        let backend = self.database().get_database_backend();
        let sql = match backend {
            DatabaseBackend::Postgres => {
                r#"
SELECT change_seq, price_list_id, resource_revision, lifecycle
FROM price_list_translation_change_journal
WHERE tenant_id = $1
  AND change_seq > $2
  AND change_seq <= $3
ORDER BY change_seq ASC
LIMIT $4
"#
            }
            _ => {
                r#"
SELECT change_seq, price_list_id, resource_revision, lifecycle
FROM price_list_translation_change_journal
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

#[allow(clippy::collapsible_if)]
pub(crate) async fn record_price_list_translation_change_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    price_list_id: Uuid,
    operation_id: Uuid,
    resource_revision: &str,
    lifecycle: PriceListTranslationChangeLifecycle,
) -> PriceListTranslationExactLocaleResult<()> {
    validate_identity(tenant_id, price_list_id, operation_id)?;
    if resource_revision.trim().is_empty() {
        return Err(PriceListTranslationExactLocaleError::Validation(
            "Pricing translation change revision must not be blank".to_string(),
        ));
    }

    let backend = txn.get_database_backend();
    let previous_sql = match backend {
        DatabaseBackend::Postgres => {
            r#"
SELECT resource_revision, lifecycle
FROM price_list_translation_change_journal
WHERE tenant_id = $1 AND price_list_id = $2
ORDER BY change_seq DESC
LIMIT 1
"#
        }
        _ => {
            r#"
SELECT resource_revision, lifecycle
FROM price_list_translation_change_journal
WHERE tenant_id = ? AND price_list_id = ?
ORDER BY change_seq DESC
LIMIT 1
"#
        }
    };
    if let Some(previous) = PreviousChangeRow::find_by_statement(Statement::from_sql_and_values(
        backend,
        previous_sql,
        vec![tenant_id.into(), price_list_id.into()],
    ))
    .one(txn)
    .await?
    {
        if previous.resource_revision == resource_revision
            && PriceListTranslationChangeLifecycle::parse(&previous.lifecycle)? == lifecycle
        {
            return Ok(());
        }
    }

    let insert_sql = match backend {
        DatabaseBackend::Postgres => {
            r#"
INSERT INTO price_list_translation_change_journal (
    operation_id, tenant_id, price_list_id, resource_revision, lifecycle
) VALUES ($1, $2, $3, $4, $5)
"#
        }
        _ => {
            r#"
INSERT INTO price_list_translation_change_journal (
    operation_id, tenant_id, price_list_id, resource_revision, lifecycle
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
            price_list_id.into(),
            resource_revision.to_string().into(),
            lifecycle.as_str().into(),
        ],
    ))
    .await?;
    Ok(())
}

fn change_record_from_row(
    row: ChangeRow,
) -> PriceListTranslationExactLocaleResult<PriceListTranslationChangeRecord> {
    let change_seq = positive_sequence(row.change_seq, "change")?;
    if row.price_list_id.is_nil() || row.resource_revision.trim().is_empty() {
        return Err(PriceListTranslationExactLocaleError::Validation(
            "Pricing translation change journal returned an invalid row".to_string(),
        ));
    }
    Ok(PriceListTranslationChangeRecord {
        change_seq,
        price_list_id: row.price_list_id,
        resource_revision: row.resource_revision,
        lifecycle: PriceListTranslationChangeLifecycle::parse(&row.lifecycle)?,
    })
}

fn validate_tenant(tenant_id: Uuid) -> PriceListTranslationExactLocaleResult<()> {
    if tenant_id.is_nil() {
        return Err(PriceListTranslationExactLocaleError::Validation(
            "Pricing translation change tenant must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn validate_identity(
    tenant_id: Uuid,
    price_list_id: Uuid,
    operation_id: Uuid,
) -> PriceListTranslationExactLocaleResult<()> {
    validate_tenant(tenant_id)?;
    if price_list_id.is_nil() || operation_id.is_nil() {
        return Err(PriceListTranslationExactLocaleError::Validation(
            "Pricing translation change identity must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn optional_positive_sequence(
    value: Option<i64>,
    field: &str,
) -> PriceListTranslationExactLocaleResult<Option<u64>> {
    value
        .map(|value| positive_sequence(value, field))
        .transpose()
}

fn positive_sequence(value: i64, field: &str) -> PriceListTranslationExactLocaleResult<u64> {
    let value = u64::try_from(value).map_err(|_| invalid_sequence(field))?;
    if value == 0 {
        return Err(invalid_sequence(field));
    }
    Ok(value)
}

fn invalid_sequence(field: &str) -> PriceListTranslationExactLocaleError {
    PriceListTranslationExactLocaleError::Validation(format!(
        "Pricing translation change {field} sequence must be positive"
    ))
}
