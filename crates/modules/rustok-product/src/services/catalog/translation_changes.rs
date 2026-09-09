use super::translation::product_translation_resource_revision;
use super::*;

use sea_orm::{DatabaseTransaction, DbBackend, QueryResult};

pub const MAX_PRODUCT_TRANSLATION_CHANGE_PAGE: u16 = 200;
const DELETED_REVISION_PREFIX: &str = "product-deleted-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductTranslationChangeLifecycle {
    Active,
    Archived,
    Deleted,
}

impl ProductTranslationChangeLifecycle {
    fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
            Self::Deleted => "deleted",
        }
    }

    fn parse(value: &str) -> CommerceResult<Self> {
        match value {
            "active" => Ok(Self::Active),
            "archived" => Ok(Self::Archived),
            "deleted" => Ok(Self::Deleted),
            _ => Err(CommerceError::Validation(
                "Product translation change journal returned an invalid lifecycle".to_owned(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductTranslationChangeRecord {
    pub change_seq: u64,
    pub product_id: Uuid,
    pub resource_revision: String,
    pub lifecycle: ProductTranslationChangeLifecycle,
}

impl CatalogService {
    pub async fn product_translation_change_highwater(
        &self,
        tenant_id: Uuid,
    ) -> CommerceResult<Option<u64>> {
        validate_tenant(tenant_id)?;
        ensure_postgres(self.database())?;
        let row = self
            .database()
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT MAX(change_seq) AS highwater FROM product_translation_change_journal WHERE tenant_id = $1",
                vec![tenant_id.into()],
            ))
            .await?
            .ok_or_else(|| {
                CommerceError::Validation(
                    "Product translation change high-water query returned no row".to_owned(),
                )
            })?;
        optional_positive_sequence(row.try_get("", "highwater")?, "high-water")
    }

    pub async fn read_product_translation_changes(
        &self,
        tenant_id: Uuid,
        after_seq: u64,
        through_seq: u64,
        limit: u16,
    ) -> CommerceResult<Vec<ProductTranslationChangeRecord>> {
        validate_tenant(tenant_id)?;
        if through_seq == 0 || after_seq > through_seq {
            return Err(CommerceError::Validation(
                "Product translation change cursor bounds are invalid".to_owned(),
            ));
        }
        if limit == 0 || limit > MAX_PRODUCT_TRANSLATION_CHANGE_PAGE {
            return Err(CommerceError::Validation(format!(
                "Product translation change page size must be between 1 and {MAX_PRODUCT_TRANSLATION_CHANGE_PAGE}"
            )));
        }
        ensure_postgres(self.database())?;

        let rows = self
            .database()
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
SELECT change_seq, product_id, resource_revision, lifecycle
FROM product_translation_change_journal
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
            .await?;

        rows.into_iter().map(change_record_from_row).collect()
    }
}

pub(crate) async fn record_product_translation_change_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    root_event_id: Uuid,
) -> CommerceResult<()> {
    if txn.get_database_backend() != DbBackend::Postgres {
        return Ok(());
    }
    if tenant_id.is_nil() || product_id.is_nil() || root_event_id.is_nil() {
        return Err(CommerceError::Validation(
            "Product translation change journal identity must not be nil".to_owned(),
        ));
    }

    let product = entities::product::Entity::find_by_id(product_id)
        .filter(entities::product::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?;
    let (resource_revision, lifecycle) = match product {
        Some(product) => {
            let translations = entities::product_translation::Entity::find()
                .filter(entities::product_translation::Column::TenantId.eq(tenant_id))
                .filter(entities::product_translation::Column::ProductId.eq(product_id))
                .order_by_asc(entities::product_translation::Column::Locale)
                .all(txn)
                .await?;
            let lifecycle = if product.status == entities::product::ProductStatus::Archived {
                ProductTranslationChangeLifecycle::Archived
            } else {
                ProductTranslationChangeLifecycle::Active
            };
            (
                product_translation_resource_revision(&product, &translations),
                lifecycle,
            )
        }
        None => (
            format!("{DELETED_REVISION_PREFIX}:{root_event_id}"),
            ProductTranslationChangeLifecycle::Deleted,
        ),
    };

    let previous = txn
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT resource_revision, lifecycle
FROM product_translation_change_journal
WHERE tenant_id = $1 AND product_id = $2
ORDER BY change_seq DESC
LIMIT 1
"#,
            vec![tenant_id.into(), product_id.into()],
        ))
        .await?;
    if previous.as_ref().is_some_and(|row| {
        let previous_revision = row.try_get::<String>("", "resource_revision").ok();
        let previous_lifecycle = row.try_get::<String>("", "lifecycle").ok();
        previous_revision.as_deref() == Some(resource_revision.as_str())
            && previous_lifecycle.as_deref() == Some(lifecycle.as_str())
    }) {
        return Ok(());
    }

    txn.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
INSERT INTO product_translation_change_journal (
    root_event_id,
    tenant_id,
    product_id,
    resource_revision,
    lifecycle,
    created_at
) VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP)
"#,
        vec![
            root_event_id.into(),
            tenant_id.into(),
            product_id.into(),
            resource_revision.into(),
            lifecycle.as_str().into(),
        ],
    ))
    .await?;

    Ok(())
}

fn change_record_from_row(row: QueryResult) -> CommerceResult<ProductTranslationChangeRecord> {
    let change_seq = positive_sequence(row.try_get("", "change_seq")?, "change")?;
    let product_id: Uuid = row.try_get("", "product_id")?;
    let resource_revision: String = row.try_get("", "resource_revision")?;
    let lifecycle: String = row.try_get("", "lifecycle")?;
    if product_id.is_nil() || resource_revision.trim().is_empty() {
        return Err(CommerceError::Validation(
            "Product translation change journal returned an invalid row".to_owned(),
        ));
    }
    Ok(ProductTranslationChangeRecord {
        change_seq,
        product_id,
        resource_revision,
        lifecycle: ProductTranslationChangeLifecycle::parse(&lifecycle)?,
    })
}

fn validate_tenant(tenant_id: Uuid) -> CommerceResult<()> {
    if tenant_id.is_nil() {
        return Err(CommerceError::Validation(
            "Product translation change tenant must not be nil".to_owned(),
        ));
    }
    Ok(())
}

fn ensure_postgres(db: &DatabaseConnection) -> CommerceResult<()> {
    if db.get_database_backend() != DbBackend::Postgres {
        return Err(CommerceError::Validation(
            "Product translation change source requires PostgreSQL".to_owned(),
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
        "Product translation change {field} sequence must be positive"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_storage_contract_is_exact() {
        assert_eq!(ProductTranslationChangeLifecycle::Active.as_str(), "active");
        assert_eq!(
            ProductTranslationChangeLifecycle::Archived.as_str(),
            "archived"
        );
        assert_eq!(
            ProductTranslationChangeLifecycle::Deleted.as_str(),
            "deleted"
        );
        assert_eq!(
            ProductTranslationChangeLifecycle::parse("archived").expect("lifecycle"),
            ProductTranslationChangeLifecycle::Archived
        );
        assert!(ProductTranslationChangeLifecycle::parse("draft").is_err());
    }

    #[test]
    fn sequence_contract_rejects_zero_and_negative_values() {
        assert_eq!(positive_sequence(1, "change").expect("positive"), 1);
        assert!(positive_sequence(0, "change").is_err());
        assert!(positive_sequence(-1, "change").is_err());
    }
}
