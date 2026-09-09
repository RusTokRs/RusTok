use super::*;

use sea_orm::{DatabaseTransaction, DbBackend, QueryResult};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub const MAX_PRODUCT_VARIANT_TRANSLATION_CHANGE_PAGE: u16 = 200;
const DELETED_REVISION_PREFIX: &str = "variant-deleted-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductVariantTranslationChangeLifecycle {
    Active,
    Archived,
    Deleted,
}

impl ProductVariantTranslationChangeLifecycle {
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
                "Product Variant translation change journal returned an invalid lifecycle"
                    .to_owned(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductVariantTranslationChangeRecord {
    pub change_seq: u64,
    pub product_id: Uuid,
    pub variant_id: Uuid,
    pub resource_revision: String,
    pub lifecycle: ProductVariantTranslationChangeLifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreviousVariantTranslationChange {
    resource_revision: String,
    lifecycle: ProductVariantTranslationChangeLifecycle,
}

impl CatalogService {
    pub async fn product_variant_translation_change_highwater(
        &self,
        tenant_id: Uuid,
    ) -> CommerceResult<Option<u64>> {
        validate_tenant(tenant_id)?;
        ensure_postgres(self.database())?;
        let row = self
            .database()
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT MAX(change_seq) AS highwater FROM product_variant_translation_change_journal WHERE tenant_id = $1",
                vec![tenant_id.into()],
            ))
            .await?
            .ok_or_else(|| {
                CommerceError::Validation(
                    "Product Variant translation change high-water query returned no row"
                        .to_owned(),
                )
            })?;
        optional_positive_sequence(row.try_get("", "highwater")?, "high-water")
    }

    pub async fn read_product_variant_translation_changes(
        &self,
        tenant_id: Uuid,
        after_seq: u64,
        through_seq: u64,
        limit: u16,
    ) -> CommerceResult<Vec<ProductVariantTranslationChangeRecord>> {
        validate_tenant(tenant_id)?;
        if through_seq == 0 || after_seq > through_seq {
            return Err(CommerceError::Validation(
                "Product Variant translation change cursor bounds are invalid".to_owned(),
            ));
        }
        if limit == 0 || limit > MAX_PRODUCT_VARIANT_TRANSLATION_CHANGE_PAGE {
            return Err(CommerceError::Validation(format!(
                "Product Variant translation change page size must be between 1 and {MAX_PRODUCT_VARIANT_TRANSLATION_CHANGE_PAGE}"
            )));
        }
        ensure_postgres(self.database())?;

        let rows = self
            .database()
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
SELECT change_seq, product_id, variant_id, resource_revision, lifecycle
FROM product_variant_translation_change_journal
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

/// Captures durable Product-owned Variant Translation change evidence from the
/// exact post-command owner state.
///
/// Product lifecycle and ProductUpdated commands are product-scoped because the
/// Variant resource revision includes parent lifecycle. A Variant event may pass
/// `variant_hint` to narrow the capture to one target and to preserve a delete
/// after the live row has disappeared. Product delete fan-out is reconstructed
/// from the already-retained ProductVariant Index tombstones, so no cross-owner
/// persistence or pre-delete Translation shadow state is required.
pub(crate) async fn record_product_variant_translation_changes_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    root_event_id: Uuid,
    variant_hint: Option<Uuid>,
) -> CommerceResult<()> {
    if txn.get_database_backend() != DbBackend::Postgres {
        return Ok(());
    }
    if tenant_id.is_nil()
        || product_id.is_nil()
        || root_event_id.is_nil()
        || variant_hint.is_some_and(|variant_id| variant_id.is_nil())
    {
        return Err(CommerceError::Validation(
            "Product Variant translation change journal identity must not be nil".to_owned(),
        ));
    }

    let previous = load_previous_changes(txn, tenant_id, product_id, variant_hint).await?;
    let product = entities::product::Entity::find_by_id(product_id)
        .filter(entities::product::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?;

    match product {
        Some(product) => {
            record_live_product_variants(
                txn,
                tenant_id,
                &product,
                root_event_id,
                variant_hint,
                &previous,
            )
            .await
        }
        None => {
            record_deleted_product_variants(
                txn,
                tenant_id,
                product_id,
                root_event_id,
                variant_hint,
                &previous,
            )
            .await
        }
    }
}

async fn record_live_product_variants(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product: &entities::product::Model,
    root_event_id: Uuid,
    variant_hint: Option<Uuid>,
    previous: &HashMap<Uuid, PreviousVariantTranslationChange>,
) -> CommerceResult<()> {
    let mut query = entities::product_variant::Entity::find()
        .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
        .filter(entities::product_variant::Column::ProductId.eq(product.id))
        .order_by_asc(entities::product_variant::Column::Id);
    if let Some(variant_id) = variant_hint {
        query = query.filter(entities::product_variant::Column::Id.eq(variant_id));
    }
    let variants = query.all(txn).await?;

    // A hinted Variant that is no longer live is a retained delete beneath a
    // still-live Product. Product-scoped lifecycle events instead enumerate only
    // current live Variants here.
    if variants.is_empty() {
        if let Some(variant_id) = variant_hint {
            return record_deleted_variant_if_retained(
                txn,
                tenant_id,
                product.id,
                variant_id,
                root_event_id,
                previous.get(&variant_id),
            )
            .await;
        }
        return Ok(());
    }

    let variant_ids = variants
        .iter()
        .map(|variant| variant.id)
        .collect::<Vec<_>>();
    let mut translations = entities::variant_translation::Entity::find()
        .filter(entities::variant_translation::Column::VariantId.is_in(variant_ids))
        .order_by_asc(entities::variant_translation::Column::VariantId)
        .order_by_asc(entities::variant_translation::Column::Locale)
        .all(txn)
        .await?
        .into_iter()
        .fold(
            HashMap::<Uuid, Vec<entities::variant_translation::Model>>::new(),
            |mut grouped, translation| {
                grouped
                    .entry(translation.variant_id)
                    .or_default()
                    .push(translation);
                grouped
            },
        );
    let lifecycle = product_lifecycle(&product.status);

    for variant in variants {
        let exact = translations.remove(&variant.id).unwrap_or_default();
        let resource_revision = variant_translation_resource_revision(product, &variant, &exact);
        if previous.get(&variant.id).is_some_and(|previous| {
            previous.resource_revision == resource_revision && previous.lifecycle == lifecycle
        }) {
            continue;
        }
        insert_change(
            txn,
            root_event_id,
            tenant_id,
            product.id,
            variant.id,
            &resource_revision,
            lifecycle,
        )
        .await?;
    }

    Ok(())
}

async fn record_deleted_product_variants(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    root_event_id: Uuid,
    variant_hint: Option<Uuid>,
    previous: &HashMap<Uuid, PreviousVariantTranslationChange>,
) -> CommerceResult<()> {
    let rows = if let Some(variant_id) = variant_hint {
        txn.query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT variant_id
FROM product_variant_index_tombstones
WHERE tenant_id = $1
  AND product_id = $2
  AND variant_id = $3
ORDER BY variant_id ASC
"#,
            vec![tenant_id.into(), product_id.into(), variant_id.into()],
        ))
        .await?
    } else {
        txn.query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT variant_id
FROM product_variant_index_tombstones
WHERE tenant_id = $1
  AND product_id = $2
ORDER BY variant_id ASC
"#,
            vec![tenant_id.into(), product_id.into()],
        ))
        .await?
    };

    for row in rows {
        let variant_id: Uuid = row.try_get("", "variant_id")?;
        if variant_id.is_nil() {
            return Err(CommerceError::Validation(
                "Product Variant translation tombstone returned an invalid Variant identity"
                    .to_owned(),
            ));
        }
        record_deleted_variant(
            txn,
            tenant_id,
            product_id,
            variant_id,
            root_event_id,
            previous.get(&variant_id),
        )
        .await?;
    }
    Ok(())
}

async fn record_deleted_variant_if_retained(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    variant_id: Uuid,
    root_event_id: Uuid,
    previous: Option<&PreviousVariantTranslationChange>,
) -> CommerceResult<()> {
    let retained = txn
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT variant_id
FROM product_variant_index_tombstones
WHERE tenant_id = $1
  AND product_id = $2
  AND variant_id = $3
"#,
            vec![tenant_id.into(), product_id.into(), variant_id.into()],
        ))
        .await?;
    if retained.is_none() {
        return Ok(());
    }
    record_deleted_variant(
        txn,
        tenant_id,
        product_id,
        variant_id,
        root_event_id,
        previous,
    )
    .await
}

async fn record_deleted_variant(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    variant_id: Uuid,
    root_event_id: Uuid,
    previous: Option<&PreviousVariantTranslationChange>,
) -> CommerceResult<()> {
    if previous.is_some_and(|previous| {
        previous.lifecycle == ProductVariantTranslationChangeLifecycle::Deleted
    }) {
        return Ok(());
    }
    let resource_revision = deleted_revision(root_event_id, variant_id);
    insert_change(
        txn,
        root_event_id,
        tenant_id,
        product_id,
        variant_id,
        &resource_revision,
        ProductVariantTranslationChangeLifecycle::Deleted,
    )
    .await
}

async fn load_previous_changes(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    variant_hint: Option<Uuid>,
) -> CommerceResult<HashMap<Uuid, PreviousVariantTranslationChange>> {
    let rows = if let Some(variant_id) = variant_hint {
        txn.query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT variant_id, resource_revision, lifecycle
FROM product_variant_translation_change_journal
WHERE tenant_id = $1
  AND product_id = $2
  AND variant_id = $3
ORDER BY change_seq DESC
LIMIT 1
"#,
            vec![tenant_id.into(), product_id.into(), variant_id.into()],
        ))
        .await?
    } else {
        txn.query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT DISTINCT ON (variant_id)
    variant_id,
    resource_revision,
    lifecycle
FROM product_variant_translation_change_journal
WHERE tenant_id = $1
  AND product_id = $2
ORDER BY variant_id ASC, change_seq DESC
"#,
            vec![tenant_id.into(), product_id.into()],
        ))
        .await?
    };

    let mut previous = HashMap::with_capacity(rows.len());
    for row in rows {
        let variant_id: Uuid = row.try_get("", "variant_id")?;
        let resource_revision: String = row.try_get("", "resource_revision")?;
        let lifecycle: String = row.try_get("", "lifecycle")?;
        if variant_id.is_nil() || resource_revision.trim().is_empty() {
            return Err(CommerceError::Validation(
                "Product Variant translation change journal returned an invalid previous row"
                    .to_owned(),
            ));
        }
        previous.insert(
            variant_id,
            PreviousVariantTranslationChange {
                resource_revision,
                lifecycle: ProductVariantTranslationChangeLifecycle::parse(&lifecycle)?,
            },
        );
    }
    Ok(previous)
}

async fn insert_change(
    txn: &DatabaseTransaction,
    root_event_id: Uuid,
    tenant_id: Uuid,
    product_id: Uuid,
    variant_id: Uuid,
    resource_revision: &str,
    lifecycle: ProductVariantTranslationChangeLifecycle,
) -> CommerceResult<()> {
    txn.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
INSERT INTO product_variant_translation_change_journal (
    root_event_id,
    tenant_id,
    product_id,
    variant_id,
    resource_revision,
    lifecycle,
    created_at
) VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP)
ON CONFLICT (root_event_id, variant_id) DO NOTHING
"#,
        vec![
            root_event_id.into(),
            tenant_id.into(),
            product_id.into(),
            variant_id.into(),
            resource_revision.to_owned().into(),
            lifecycle.as_str().into(),
        ],
    ))
    .await?;
    Ok(())
}

fn change_record_from_row(
    row: QueryResult,
) -> CommerceResult<ProductVariantTranslationChangeRecord> {
    let change_seq = positive_sequence(row.try_get("", "change_seq")?, "change")?;
    let product_id: Uuid = row.try_get("", "product_id")?;
    let variant_id: Uuid = row.try_get("", "variant_id")?;
    let resource_revision: String = row.try_get("", "resource_revision")?;
    let lifecycle: String = row.try_get("", "lifecycle")?;
    if product_id.is_nil() || variant_id.is_nil() || resource_revision.trim().is_empty() {
        return Err(CommerceError::Validation(
            "Product Variant translation change journal returned an invalid row".to_owned(),
        ));
    }
    Ok(ProductVariantTranslationChangeRecord {
        change_seq,
        product_id,
        variant_id,
        resource_revision,
        lifecycle: ProductVariantTranslationChangeLifecycle::parse(&lifecycle)?,
    })
}

fn product_lifecycle(
    status: &entities::product::ProductStatus,
) -> ProductVariantTranslationChangeLifecycle {
    match status {
        entities::product::ProductStatus::Draft | entities::product::ProductStatus::Active => {
            ProductVariantTranslationChangeLifecycle::Active
        }
        entities::product::ProductStatus::Archived => {
            ProductVariantTranslationChangeLifecycle::Archived
        }
    }
}

/// Keep this byte-for-byte semantic algorithm aligned with
/// `variant_translation::product_variant_translation_resource_revision`.
/// The journal stores the same opaque revision returned by exact read/apply so a
/// future neutral ChangeCursor can hand consumers authoritative owner evidence.
fn variant_translation_resource_revision(
    product: &entities::product::Model,
    variant: &entities::product_variant::Model,
    translations: &[entities::variant_translation::Model],
) -> String {
    let mut hasher = Sha256::new();
    digest_text(
        &mut hasher,
        "rustok-product/variant-translation-resource/v1",
    );
    digest_text(&mut hasher, &product.id.to_string());
    digest_text(&mut hasher, &product.tenant_id.to_string());
    digest_text(&mut hasher, &product.status.to_string());
    digest_text(&mut hasher, &variant.id.to_string());
    digest_text(&mut hasher, &variant.product_id.to_string());
    digest_text(&mut hasher, &variant.tenant_id.to_string());

    let mut exact = translations.iter().collect::<Vec<_>>();
    exact.sort_by(|left, right| left.locale.cmp(&right.locale));
    for translation in exact {
        digest_text(&mut hasher, &translation.variant_id.to_string());
        digest_text(&mut hasher, &translation.locale);
        digest_optional_text(&mut hasher, translation.title.as_deref());
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn digest_text(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn digest_optional_text(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            digest_text(hasher, value);
        }
        None => hasher.update([0]),
    }
}

fn deleted_revision(root_event_id: Uuid, variant_id: Uuid) -> String {
    format!("{DELETED_REVISION_PREFIX}:{root_event_id}:{variant_id}")
}

fn validate_tenant(tenant_id: Uuid) -> CommerceResult<()> {
    if tenant_id.is_nil() {
        return Err(CommerceError::Validation(
            "Product Variant translation change tenant must not be nil".to_owned(),
        ));
    }
    Ok(())
}

fn ensure_postgres(db: &DatabaseConnection) -> CommerceResult<()> {
    if db.get_database_backend() != DbBackend::Postgres {
        return Err(CommerceError::Validation(
            "Product Variant translation change source requires PostgreSQL".to_owned(),
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
        "Product Variant translation change {field} sequence must be positive"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_storage_contract_is_exact() {
        assert_eq!(
            ProductVariantTranslationChangeLifecycle::Active.as_str(),
            "active"
        );
        assert_eq!(
            ProductVariantTranslationChangeLifecycle::Archived.as_str(),
            "archived"
        );
        assert_eq!(
            ProductVariantTranslationChangeLifecycle::Deleted.as_str(),
            "deleted"
        );
        assert_eq!(
            ProductVariantTranslationChangeLifecycle::parse("archived").expect("lifecycle"),
            ProductVariantTranslationChangeLifecycle::Archived
        );
        assert!(ProductVariantTranslationChangeLifecycle::parse("draft").is_err());
    }

    #[test]
    fn deleted_revision_stays_inside_storage_bound() {
        let revision = deleted_revision(Uuid::from_u128(1), Uuid::from_u128(2));
        assert!(revision.len() <= 96);
        assert!(revision.starts_with(DELETED_REVISION_PREFIX));
    }

    #[test]
    fn sequence_contract_rejects_zero_and_negative_values() {
        assert_eq!(positive_sequence(1, "change").expect("positive"), 1);
        assert!(positive_sequence(0, "change").is_err());
        assert!(positive_sequence(-1, "change").is_err());
    }
}
