use super::*;

use sea_orm::{DatabaseTransaction, DbBackend, QueryResult};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

pub const MAX_PRODUCT_IMAGE_TRANSLATION_CHANGE_PAGE: u16 = 200;
const DELETED_REVISION_PREFIX: &str = "image-deleted-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductImageTranslationChangeLifecycle {
    Active,
    Archived,
    Deleted,
}

impl ProductImageTranslationChangeLifecycle {
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
                "Product Image translation change journal returned an invalid lifecycle"
                    .to_owned(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductImageTranslationChangeRecord {
    pub change_seq: u64,
    pub product_id: Uuid,
    pub image_id: Uuid,
    pub resource_revision: String,
    pub lifecycle: ProductImageTranslationChangeLifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreviousImageTranslationChange {
    resource_revision: String,
    lifecycle: ProductImageTranslationChangeLifecycle,
}

impl CatalogService {
    pub async fn product_image_translation_change_highwater(
        &self,
        tenant_id: Uuid,
    ) -> CommerceResult<Option<u64>> {
        validate_tenant(tenant_id)?;
        ensure_postgres(self.database())?;
        let row = self
            .database()
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT MAX(change_seq) AS highwater FROM product_image_translation_change_journal WHERE tenant_id = $1",
                vec![tenant_id.into()],
            ))
            .await?
            .ok_or_else(|| {
                CommerceError::Validation(
                    "Product Image translation change high-water query returned no row".to_owned(),
                )
            })?;
        optional_positive_sequence(row.try_get("", "highwater")?, "high-water")
    }

    pub async fn read_product_image_translation_changes(
        &self,
        tenant_id: Uuid,
        after_seq: u64,
        through_seq: u64,
        limit: u16,
    ) -> CommerceResult<Vec<ProductImageTranslationChangeRecord>> {
        validate_tenant(tenant_id)?;
        if through_seq == 0 || after_seq > through_seq {
            return Err(CommerceError::Validation(
                "Product Image translation change cursor bounds are invalid".to_owned(),
            ));
        }
        if limit == 0 || limit > MAX_PRODUCT_IMAGE_TRANSLATION_CHANGE_PAGE {
            return Err(CommerceError::Validation(format!(
                "Product Image translation change page size must be between 1 and {MAX_PRODUCT_IMAGE_TRANSLATION_CHANGE_PAGE}"
            )));
        }
        ensure_postgres(self.database())?;

        let rows = self
            .database()
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
SELECT change_seq, product_id, image_id, resource_revision, lifecycle
FROM product_image_translation_change_journal
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

/// Captures durable Product-owned Image Translation change evidence from the
/// exact post-command owner state under the same outbox envelope transaction.
///
/// Product lifecycle and ProductUpdated events fan out across live Images because
/// the Image translation resource revision includes parent Product lifecycle.
/// Semantic dedupe suppresses unchanged Images. Missing Images that were previously
/// journaled become deleted on live Product updates. Full Product deletion supplies
/// the exact pre-delete Image identities so first-delete evidence also works for
/// Images that predate this journal.
pub(crate) async fn record_product_image_translation_changes_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    root_event_id: Uuid,
    deleted_image_ids: Option<&[Uuid]>,
) -> CommerceResult<()> {
    if txn.get_database_backend() != DbBackend::Postgres {
        return Ok(());
    }
    if tenant_id.is_nil() || product_id.is_nil() || root_event_id.is_nil() {
        return Err(CommerceError::Validation(
            "Product Image translation change journal identity must not be nil".to_owned(),
        ));
    }

    let previous = load_previous_changes(txn, tenant_id, product_id).await?;
    let product = entities::product::Entity::find_by_id(product_id)
        .filter(entities::product::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?;

    match (product, deleted_image_ids) {
        (Some(product), None) => {
            record_live_product_images(txn, &product, root_event_id, &previous).await
        }
        (None, Some(image_ids)) => {
            record_deleted_product_images(
                txn,
                tenant_id,
                product_id,
                root_event_id,
                image_ids,
                &previous,
            )
            .await
        }
        (Some(_), Some(_)) => Err(CommerceError::Validation(
            "Product Image translation delete targets were supplied while Product is still live"
                .to_owned(),
        )),
        (None, None) => Err(CommerceError::Validation(
            "Product Image translation delete evidence requires pre-delete Image identities"
                .to_owned(),
        )),
    }
}

async fn record_live_product_images(
    txn: &DatabaseTransaction,
    product: &entities::product::Model,
    root_event_id: Uuid,
    previous: &HashMap<Uuid, PreviousImageTranslationChange>,
) -> CommerceResult<()> {
    let images = entities::product_image::Entity::find()
        .filter(entities::product_image::Column::ProductId.eq(product.id))
        .order_by_asc(entities::product_image::Column::Id)
        .all(txn)
        .await?;
    let current_ids = images.iter().map(|image| image.id).collect::<HashSet<_>>();
    let image_ids = images.iter().map(|image| image.id).collect::<Vec<_>>();
    let translations = if image_ids.is_empty() {
        Vec::new()
    } else {
        entities::product_image_translation::Entity::find()
            .filter(entities::product_image_translation::Column::ImageId.is_in(image_ids))
            .order_by_asc(entities::product_image_translation::Column::ImageId)
            .order_by_asc(entities::product_image_translation::Column::Locale)
            .all(txn)
            .await?
    };
    let mut translations_by_image = translations.into_iter().fold(
        HashMap::<Uuid, Vec<entities::product_image_translation::Model>>::new(),
        |mut grouped, translation| {
            grouped
                .entry(translation.image_id)
                .or_default()
                .push(translation);
            grouped
        },
    );
    let lifecycle = product_lifecycle(&product.status);

    for image in images {
        let translations = translations_by_image.remove(&image.id).unwrap_or_default();
        let resource_revision =
            image_translation_resource_revision(product, &image, &translations);
        if previous.get(&image.id).is_some_and(|previous| {
            previous.resource_revision == resource_revision && previous.lifecycle == lifecycle
        }) {
            continue;
        }
        insert_change(
            txn,
            root_event_id,
            product.tenant_id,
            product.id,
            image.id,
            &resource_revision,
            lifecycle,
        )
        .await?;
    }

    // Product remains live, but a previously journaled Image can disappear through
    // an owner mutation that publishes ProductUpdated. Emit its delete exactly once.
    let mut removed_ids = previous
        .iter()
        .filter_map(|(image_id, previous)| {
            (!current_ids.contains(image_id)
                && previous.lifecycle != ProductImageTranslationChangeLifecycle::Deleted)
                .then_some(*image_id)
        })
        .collect::<Vec<_>>();
    removed_ids.sort_unstable();
    for image_id in removed_ids {
        let resource_revision = deleted_revision(root_event_id, image_id);
        insert_change(
            txn,
            root_event_id,
            product.tenant_id,
            product.id,
            image_id,
            &resource_revision,
            ProductImageTranslationChangeLifecycle::Deleted,
        )
        .await?;
    }

    Ok(())
}

async fn record_deleted_product_images(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    root_event_id: Uuid,
    image_ids: &[Uuid],
    previous: &HashMap<Uuid, PreviousImageTranslationChange>,
) -> CommerceResult<()> {
    let mut seen = HashSet::with_capacity(image_ids.len());
    let mut ordered = Vec::with_capacity(image_ids.len());
    for image_id in image_ids {
        if image_id.is_nil() || !seen.insert(*image_id) {
            return Err(CommerceError::Validation(
                "Product Image translation delete targets must be unique non-nil UUIDs".to_owned(),
            ));
        }
        ordered.push(*image_id);
    }
    ordered.sort_unstable();

    for image_id in ordered {
        if previous.get(&image_id).is_some_and(|previous| {
            previous.lifecycle == ProductImageTranslationChangeLifecycle::Deleted
        }) {
            continue;
        }
        let resource_revision = deleted_revision(root_event_id, image_id);
        insert_change(
            txn,
            root_event_id,
            tenant_id,
            product_id,
            image_id,
            &resource_revision,
            ProductImageTranslationChangeLifecycle::Deleted,
        )
        .await?;
    }
    Ok(())
}

async fn load_previous_changes(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
) -> CommerceResult<HashMap<Uuid, PreviousImageTranslationChange>> {
    let rows = txn
        .query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT DISTINCT ON (image_id)
    image_id,
    resource_revision,
    lifecycle
FROM product_image_translation_change_journal
WHERE tenant_id = $1
  AND product_id = $2
ORDER BY image_id ASC, change_seq DESC
"#,
            vec![tenant_id.into(), product_id.into()],
        ))
        .await?;

    let mut previous = HashMap::with_capacity(rows.len());
    for row in rows {
        let image_id: Uuid = row.try_get("", "image_id")?;
        let resource_revision: String = row.try_get("", "resource_revision")?;
        let lifecycle: String = row.try_get("", "lifecycle")?;
        if image_id.is_nil() || resource_revision.trim().is_empty() {
            return Err(CommerceError::Validation(
                "Product Image translation change journal returned an invalid previous row"
                    .to_owned(),
            ));
        }
        previous.insert(
            image_id,
            PreviousImageTranslationChange {
                resource_revision,
                lifecycle: ProductImageTranslationChangeLifecycle::parse(&lifecycle)?,
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
    image_id: Uuid,
    resource_revision: &str,
    lifecycle: ProductImageTranslationChangeLifecycle,
) -> CommerceResult<()> {
    txn.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
INSERT INTO product_image_translation_change_journal (
    root_event_id,
    tenant_id,
    product_id,
    image_id,
    resource_revision,
    lifecycle,
    created_at
) VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP)
ON CONFLICT (root_event_id, image_id) DO NOTHING
"#,
        vec![
            root_event_id.into(),
            tenant_id.into(),
            product_id.into(),
            image_id.into(),
            resource_revision.to_owned().into(),
            lifecycle.as_str().into(),
        ],
    ))
    .await?;
    Ok(())
}

fn change_record_from_row(row: QueryResult) -> CommerceResult<ProductImageTranslationChangeRecord> {
    let change_seq = positive_sequence(row.try_get("", "change_seq")?, "change")?;
    let product_id: Uuid = row.try_get("", "product_id")?;
    let image_id: Uuid = row.try_get("", "image_id")?;
    let resource_revision: String = row.try_get("", "resource_revision")?;
    let lifecycle: String = row.try_get("", "lifecycle")?;
    if product_id.is_nil() || image_id.is_nil() || resource_revision.trim().is_empty() {
        return Err(CommerceError::Validation(
            "Product Image translation change journal returned an invalid row".to_owned(),
        ));
    }
    Ok(ProductImageTranslationChangeRecord {
        change_seq,
        product_id,
        image_id,
        resource_revision,
        lifecycle: ProductImageTranslationChangeLifecycle::parse(&lifecycle)?,
    })
}

fn product_lifecycle(
    status: &entities::product::ProductStatus,
) -> ProductImageTranslationChangeLifecycle {
    match status {
        entities::product::ProductStatus::Draft | entities::product::ProductStatus::Active => {
            ProductImageTranslationChangeLifecycle::Active
        }
        entities::product::ProductStatus::Archived => {
            ProductImageTranslationChangeLifecycle::Archived
        }
    }
}

/// Keep this algorithm semantically identical to
/// `image_translation::product_image_translation_resource_revision`.
fn image_translation_resource_revision(
    product: &entities::product::Model,
    image: &entities::product_image::Model,
    translations: &[entities::product_image_translation::Model],
) -> String {
    let mut hasher = Sha256::new();
    digest_text(
        &mut hasher,
        "rustok-product/image-translation-resource/v1",
    );
    digest_text(&mut hasher, &product.id.to_string());
    digest_text(&mut hasher, &product.tenant_id.to_string());
    digest_text(&mut hasher, &product.status.to_string());
    digest_text(&mut hasher, &image.id.to_string());
    digest_text(&mut hasher, &image.product_id.to_string());
    digest_text(&mut hasher, &image.media_id.to_string());
    digest_text(&mut hasher, &image.position.to_string());

    let mut exact = translations.iter().collect::<Vec<_>>();
    exact.sort_by(|left, right| left.locale.cmp(&right.locale));
    for translation in exact {
        digest_text(&mut hasher, &translation.image_id.to_string());
        digest_text(&mut hasher, &translation.locale);
        digest_optional_text(&mut hasher, translation.alt_text.as_deref());
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

fn deleted_revision(root_event_id: Uuid, image_id: Uuid) -> String {
    format!("{DELETED_REVISION_PREFIX}:{root_event_id}:{image_id}")
}

fn validate_tenant(tenant_id: Uuid) -> CommerceResult<()> {
    if tenant_id.is_nil() {
        return Err(CommerceError::Validation(
            "Product Image translation change tenant must not be nil".to_owned(),
        ));
    }
    Ok(())
}

fn ensure_postgres(db: &DatabaseConnection) -> CommerceResult<()> {
    if db.get_database_backend() != DbBackend::Postgres {
        return Err(CommerceError::Validation(
            "Product Image translation change source requires PostgreSQL".to_owned(),
        ));
    }
    Ok(())
}

fn optional_positive_sequence(value: Option<i64>, field: &str) -> CommerceResult<Option<u64>> {
    value.map(|value| positive_sequence(value, field)).transpose()
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
        "Product Image translation change {field} sequence must be positive"
    ))
}
