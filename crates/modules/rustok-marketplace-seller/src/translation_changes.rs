use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, FromQueryResult, QueryFilter,
    QueryOrder, Statement,
};
use uuid::Uuid;

use crate::entities::{seller, seller_translation};
use crate::error::{MarketplaceSellerError, MarketplaceSellerResult};
use crate::translation::resource_revision;

pub const MAX_MARKETPLACE_SELLER_TRANSLATION_CHANGE_PAGE: u16 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketplaceSellerTranslationChangeLifecycle {
    Active,
    Archived,
    Deleted,
}

impl MarketplaceSellerTranslationChangeLifecycle {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
            Self::Deleted => "deleted",
        }
    }

    fn parse(value: &str) -> MarketplaceSellerResult<Self> {
        match value {
            "active" => Ok(Self::Active),
            "archived" => Ok(Self::Archived),
            "deleted" => Ok(Self::Deleted),
            _ => Err(MarketplaceSellerError::Validation(
                "marketplace seller Translation change journal returned an invalid lifecycle"
                    .to_string(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketplaceSellerTranslationChangeRecord {
    pub change_seq: u64,
    pub seller_id: Uuid,
    pub resource_revision: String,
    pub lifecycle: MarketplaceSellerTranslationChangeLifecycle,
}

#[derive(Debug, FromQueryResult)]
struct HighwaterRow {
    highwater: Option<i64>,
}

#[derive(Debug, FromQueryResult)]
struct ChangeRow {
    change_seq: i64,
    seller_id: Uuid,
    resource_revision: String,
    lifecycle: String,
}

#[derive(Debug, FromQueryResult)]
struct PreviousChangeRow {
    resource_revision: String,
    lifecycle: String,
}

impl crate::MarketplaceSellerTranslationService {
    pub async fn seller_translation_change_highwater(
        &self,
        tenant_id: Uuid,
    ) -> MarketplaceSellerResult<Option<u64>> {
        validate_tenant(tenant_id)?;
        let backend = self.database().get_database_backend();
        let sql = match backend {
            DatabaseBackend::Postgres => {
                "SELECT MAX(change_seq) AS highwater FROM marketplace_seller_translation_change_journal WHERE tenant_id = $1"
            }
            _ => {
                "SELECT MAX(change_seq) AS highwater FROM marketplace_seller_translation_change_journal WHERE tenant_id = ?"
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
            MarketplaceSellerError::Validation(
                "marketplace seller Translation high-water query returned no row".to_string(),
            )
        })?;
        optional_positive_sequence(row.highwater, "high-water")
    }

    pub async fn read_seller_translation_changes(
        &self,
        tenant_id: Uuid,
        after_seq: u64,
        through_seq: u64,
        limit: u16,
    ) -> MarketplaceSellerResult<Vec<MarketplaceSellerTranslationChangeRecord>> {
        validate_tenant(tenant_id)?;
        if through_seq == 0 || after_seq > through_seq {
            return Err(MarketplaceSellerError::Validation(
                "marketplace seller Translation change cursor bounds are invalid".to_string(),
            ));
        }
        if limit == 0 || limit > MAX_MARKETPLACE_SELLER_TRANSLATION_CHANGE_PAGE {
            return Err(MarketplaceSellerError::Validation(format!(
                "marketplace seller Translation change page size must be between 1 and {MAX_MARKETPLACE_SELLER_TRANSLATION_CHANGE_PAGE}"
            )));
        }

        let backend = self.database().get_database_backend();
        let sql = match backend {
            DatabaseBackend::Postgres => {
                r#"
SELECT change_seq, seller_id, resource_revision, lifecycle
FROM marketplace_seller_translation_change_journal
WHERE tenant_id = $1
  AND change_seq > $2
  AND change_seq <= $3
ORDER BY change_seq ASC
LIMIT $4
"#
            }
            _ => {
                r#"
SELECT change_seq, seller_id, resource_revision, lifecycle
FROM marketplace_seller_translation_change_journal
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

pub(crate) fn translation_lifecycle_for_status(
    status: &str,
) -> MarketplaceSellerResult<MarketplaceSellerTranslationChangeLifecycle> {
    match status {
        "draft" | "active" | "suspended" => {
            Ok(MarketplaceSellerTranslationChangeLifecycle::Active)
        }
        "closed" => Ok(MarketplaceSellerTranslationChangeLifecycle::Archived),
        _ => Err(MarketplaceSellerError::Validation(format!(
            "unknown marketplace seller status `{status}`"
        ))),
    }
}

pub(crate) async fn record_current_seller_translation_change_in_tx<C>(
    connection: &C,
    tenant_id: Uuid,
    seller_id: Uuid,
    operation_id: Uuid,
) -> MarketplaceSellerResult<()>
where
    C: ConnectionTrait,
{
    validate_identity(tenant_id, seller_id, operation_id)?;
    let seller = seller::Entity::find_by_id(seller_id)
        .filter(seller::Column::TenantId.eq(tenant_id))
        .one(connection)
        .await?
        .ok_or(MarketplaceSellerError::SellerNotFound(seller_id))?;
    let translations = seller_translation::Entity::find()
        .filter(seller_translation::Column::TenantId.eq(tenant_id))
        .filter(seller_translation::Column::SellerId.eq(seller_id))
        .order_by_asc(seller_translation::Column::Locale)
        .all(connection)
        .await?;
    if translations.is_empty() {
        return Ok(());
    }
    let revision = resource_revision(&seller, &translations);
    let lifecycle = translation_lifecycle_for_status(&seller.status)?;
    record_seller_translation_change_in_tx(
        connection,
        tenant_id,
        seller_id,
        operation_id,
        &revision,
        lifecycle,
    )
    .await
}

#[allow(clippy::collapsible_if)]
pub(crate) async fn record_seller_translation_change_in_tx<C>(
    connection: &C,
    tenant_id: Uuid,
    seller_id: Uuid,
    operation_id: Uuid,
    resource_revision: &str,
    lifecycle: MarketplaceSellerTranslationChangeLifecycle,
) -> MarketplaceSellerResult<()>
where
    C: ConnectionTrait,
{
    validate_identity(tenant_id, seller_id, operation_id)?;
    if resource_revision.trim().is_empty() {
        return Err(MarketplaceSellerError::Validation(
            "marketplace seller Translation change revision must not be blank".to_string(),
        ));
    }

    let backend = connection.get_database_backend();
    let previous_sql = match backend {
        DatabaseBackend::Postgres => {
            r#"
SELECT resource_revision, lifecycle
FROM marketplace_seller_translation_change_journal
WHERE tenant_id = $1 AND seller_id = $2
ORDER BY change_seq DESC
LIMIT 1
"#
        }
        _ => {
            r#"
SELECT resource_revision, lifecycle
FROM marketplace_seller_translation_change_journal
WHERE tenant_id = ? AND seller_id = ?
ORDER BY change_seq DESC
LIMIT 1
"#
        }
    };
    if let Some(previous) = PreviousChangeRow::find_by_statement(Statement::from_sql_and_values(
        backend,
        previous_sql,
        vec![tenant_id.into(), seller_id.into()],
    ))
    .one(connection)
    .await?
    {
        if previous.resource_revision == resource_revision
            && MarketplaceSellerTranslationChangeLifecycle::parse(&previous.lifecycle)? == lifecycle
        {
            return Ok(());
        }
    }

    let insert_sql = match backend {
        DatabaseBackend::Postgres => {
            r#"
INSERT INTO marketplace_seller_translation_change_journal (
    operation_id, tenant_id, seller_id, resource_revision, lifecycle
) VALUES ($1, $2, $3, $4, $5)
ON CONFLICT (operation_id, seller_id) DO NOTHING
"#
        }
        _ => {
            r#"
INSERT INTO marketplace_seller_translation_change_journal (
    operation_id, tenant_id, seller_id, resource_revision, lifecycle
) VALUES (?, ?, ?, ?, ?)
ON CONFLICT (operation_id, seller_id) DO NOTHING
"#
        }
    };
    connection
        .execute_raw(Statement::from_sql_and_values(
            backend,
            insert_sql,
            vec![
                operation_id.into(),
                tenant_id.into(),
                seller_id.into(),
                resource_revision.to_string().into(),
                lifecycle.as_str().into(),
            ],
        ))
        .await?;
    Ok(())
}

fn change_record_from_row(
    row: ChangeRow,
) -> MarketplaceSellerResult<MarketplaceSellerTranslationChangeRecord> {
    let change_seq = positive_sequence(row.change_seq, "change")?;
    if row.seller_id.is_nil() || row.resource_revision.trim().is_empty() {
        return Err(MarketplaceSellerError::Validation(
            "marketplace seller Translation change journal returned an invalid row".to_string(),
        ));
    }
    Ok(MarketplaceSellerTranslationChangeRecord {
        change_seq,
        seller_id: row.seller_id,
        resource_revision: row.resource_revision,
        lifecycle: MarketplaceSellerTranslationChangeLifecycle::parse(&row.lifecycle)?,
    })
}

fn validate_tenant(tenant_id: Uuid) -> MarketplaceSellerResult<()> {
    if tenant_id.is_nil() {
        return Err(MarketplaceSellerError::Validation(
            "marketplace seller Translation tenant must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn validate_identity(
    tenant_id: Uuid,
    seller_id: Uuid,
    operation_id: Uuid,
) -> MarketplaceSellerResult<()> {
    validate_tenant(tenant_id)?;
    if seller_id.is_nil() || operation_id.is_nil() {
        return Err(MarketplaceSellerError::Validation(
            "marketplace seller Translation change identity must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn optional_positive_sequence(
    value: Option<i64>,
    field: &str,
) -> MarketplaceSellerResult<Option<u64>> {
    value
        .map(|value| positive_sequence(value, field))
        .transpose()
}

fn positive_sequence(value: i64, field: &str) -> MarketplaceSellerResult<u64> {
    let value = u64::try_from(value).map_err(|_| invalid_sequence(field))?;
    if value == 0 {
        return Err(invalid_sequence(field));
    }
    Ok(value)
}

fn invalid_sequence(field: &str) -> MarketplaceSellerError {
    MarketplaceSellerError::Validation(format!(
        "marketplace seller Translation change {field} sequence must be positive"
    ))
}
