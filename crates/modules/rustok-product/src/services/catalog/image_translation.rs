use super::*;

use rustok_api::TenantLocale;
use sea_orm::sea_query::ExprTrait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProductImageTranslationExactLocaleError {
    #[error(transparent)]
    Commerce(#[from] CommerceError),

    #[error("Product image not found: {0}")]
    ImageNotFound(Uuid),

    #[error("Product image translation source locale not found: {locale} for image {image_id}")]
    SourceLocaleNotFound { image_id: Uuid, locale: String },

    #[error(
        "Product image translation target locale missing after apply: {locale} for image {image_id}"
    )]
    TargetLocaleMissingAfterApply { image_id: Uuid, locale: String },

    #[error("Product image translation {revision} revision conflict")]
    RevisionConflict { revision: &'static str },
}

impl From<sea_orm::DbErr> for ProductImageTranslationExactLocaleError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::Commerce(CommerceError::Database(error))
    }
}

pub type ProductImageTranslationExactLocaleResult<T> =
    Result<T, ProductImageTranslationExactLocaleError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductImageTranslationExactLocaleRecord {
    pub locale: String,
    pub alt_text: Option<String>,
}

impl From<entities::product_image_translation::Model> for ProductImageTranslationExactLocaleRecord {
    fn from(value: entities::product_image_translation::Model) -> Self {
        Self {
            locale: value.locale,
            alt_text: value.alt_text,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductImageTranslationExactLocaleSnapshot {
    pub product_id: Uuid,
    pub image_id: Uuid,
    pub product_status: entities::product::ProductStatus,
    pub source_locale: String,
    pub target_locale: String,
    pub resource_revision: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub exact_locales: Vec<String>,
    pub source: ProductImageTranslationExactLocaleRecord,
    pub target: Option<ProductImageTranslationExactLocaleRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductImageTranslationExactLocaleApply {
    pub source_locale: String,
    pub target_locale: String,
    pub alt_text: Option<String>,
    pub expected_resource_revision: String,
    pub expected_source_revision: String,
    pub expected_target_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductImageTranslationExactLocaleApplyReceipt {
    pub operation_id: Option<Uuid>,
    pub product_id: Uuid,
    pub image_id: Uuid,
    pub resource_revision: String,
    pub target_revision: String,
    pub target: ProductImageTranslationExactLocaleRecord,
}

impl CatalogService {
    /// Lists non-archived Product Images that own the requested exact source locale.
    ///
    /// Ordering and pagination use Image UUIDs. The page is assembled with
    /// bounded bulk owner reads so every summary uses the same semantic revision
    /// algorithm as exact read/apply without provider-owned SQL or N+1 queries.
    pub async fn list_product_image_translation_exact_resources(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
        after: Option<Uuid>,
        limit: u16,
    ) -> ProductImageTranslationExactLocaleResult<(
        Vec<ProductImageTranslationExactLocaleSnapshot>,
        Option<Uuid>,
    )> {
        let source_locale = canonical_image_translation_locale(source_locale)?;
        let target_locale = canonical_image_translation_locale(target_locale)?;
        validate_image_locale_pair(&source_locale, &target_locale)?;
        if limit == 0 {
            return Err(CommerceError::Validation(
                "Product image translation resource page limit must be positive".to_string(),
            )
            .into());
        }

        let source_image_ids = sea_orm::sea_query::Query::select()
            .column(entities::product_image_translation::Column::ImageId)
            .from(entities::product_image_translation::Entity)
            .and_where(
                sea_orm::sea_query::Expr::col(entities::product_image_translation::Column::Locale)
                    .eq(source_locale.clone()),
            )
            .to_owned();
        let mut query = entities::product_image::Entity::find()
            .inner_join(entities::product::Entity)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .filter(
                entities::product::Column::Status.ne(entities::product::ProductStatus::Archived),
            )
            .filter(entities::product_image::Column::Id.in_subquery(source_image_ids))
            .order_by_asc(entities::product_image::Column::Id);
        if let Some(after) = after {
            query = query.filter(entities::product_image::Column::Id.gt(after));
        }

        let mut images = query.limit(u64::from(limit) + 1).all(&self.db).await?;
        let has_more = images.len() > usize::from(limit);
        if has_more {
            images.truncate(usize::from(limit));
        }
        let next_after = has_more
            .then(|| images.last().map(|image| image.id))
            .flatten();
        if images.is_empty() {
            return Ok((Vec::new(), None));
        }

        let image_ids = images.iter().map(|image| image.id).collect::<Vec<_>>();
        let product_ids = images
            .iter()
            .map(|image| image.product_id)
            .collect::<HashSet<_>>();
        let products = entities::product::Entity::find()
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .filter(entities::product::Column::Id.is_in(product_ids))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|product| (product.id, product))
            .collect::<HashMap<_, _>>();
        let mut translations = load_image_translations_for_images(&self.db, &image_ids)
            .await?
            .into_iter()
            .fold(
                HashMap::<Uuid, Vec<entities::product_image_translation::Model>>::new(),
                |mut grouped, translation| {
                    grouped
                        .entry(translation.image_id)
                        .or_default()
                        .push(translation);
                    grouped
                },
            );

        let mut resources = Vec::with_capacity(images.len());
        for image in images {
            let product = products
                .get(&image.product_id)
                .cloned()
                .ok_or(CommerceError::ProductNotFound(image.product_id))?;
            let exact = translations.remove(&image.id).ok_or_else(|| {
                ProductImageTranslationExactLocaleError::SourceLocaleNotFound {
                    image_id: image.id,
                    locale: source_locale.clone(),
                }
            })?;
            resources.push(build_image_exact_locale_snapshot(
                product,
                image,
                exact,
                source_locale.clone(),
                target_locale.clone(),
            )?);
        }

        Ok((resources, next_after))
    }

    /// Reads one exact Product Image source locale and one exact target locale
    /// without consulting storefront fallback.
    ///
    /// The resource revision covers Product lifecycle, Image owner semantics,
    /// and every exact Image alt-text locale. Locale revisions cover only the
    /// selected owner row, including the distinction between a missing row and
    /// a present row whose `alt_text` is NULL.
    pub async fn read_product_image_translation_exact_locale(
        &self,
        tenant_id: Uuid,
        image_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> ProductImageTranslationExactLocaleResult<ProductImageTranslationExactLocaleSnapshot> {
        let source_locale = canonical_image_translation_locale(source_locale)?;
        let target_locale = canonical_image_translation_locale(target_locale)?;
        validate_image_locale_pair(&source_locale, &target_locale)?;

        let image = load_image(&self.db, tenant_id, image_id).await?;
        let product = load_image_product(&self.db, tenant_id, image.product_id).await?;
        let translations = load_image_translations(&self.db, image_id).await?;

        build_image_exact_locale_snapshot(
            product,
            image,
            translations,
            source_locale,
            target_locale,
        )
    }

    /// Applies one exact Product Image locale under resource/source/target CAS.
    ///
    /// The parent Product row is locked before the Image row so lifecycle
    /// changes and Image translation writes serialize in canonical owner order.
    /// Only the requested `product_image_translations` row is inserted or
    /// updated; sibling locale rows are preserved. Applying `alt_text = None`
    /// keeps an explicit exact-locale row with NULL rather than deleting it.
    /// `ProductUpdated` publication and any active Product owner-operation receipt
    /// complete in the same write transaction before commit.
    pub async fn apply_product_image_translation_exact_locale(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        image_id: Uuid,
        request: ProductImageTranslationExactLocaleApply,
    ) -> ProductImageTranslationExactLocaleResult<ProductImageTranslationExactLocaleApplyReceipt>
    {
        validate_image_translation_alt_text(request.alt_text.as_deref())?;

        let source_locale = canonical_image_translation_locale(&request.source_locale)?;
        let target_locale = canonical_image_translation_locale(&request.target_locale)?;
        validate_image_locale_pair(&source_locale, &target_locale)?;

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

        // Discover the immutable parent identity through the tenant-owned Product,
        // then lock parent before child to match Product lifecycle write ordering.
        let discovered_image = load_image(&txn, tenant_id, image_id).await?;
        let product = entities::product::Entity::find_by_id(discovered_image.product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or(CommerceError::ProductNotFound(discovered_image.product_id))?;
        let image = entities::product_image::Entity::find_by_id(image_id)
            .filter(entities::product_image::Column::ProductId.eq(product.id))
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or(ProductImageTranslationExactLocaleError::ImageNotFound(
                image_id,
            ))?;

        let translations = load_image_translations(&txn, image_id).await?;
        let source = exact_image_locale_row(&translations, &source_locale).ok_or_else(|| {
            ProductImageTranslationExactLocaleError::SourceLocaleNotFound {
                image_id,
                locale: source_locale.clone(),
            }
        })?;
        let target = exact_image_locale_row(&translations, &target_locale);

        let current_resource_revision =
            product_image_translation_resource_revision(&product, &image, &translations);
        let current_source_revision = product_image_translation_locale_revision(source);
        let current_target_revision = target.map(product_image_translation_locale_revision);

        ensure_image_revision(
            "resource",
            &request.expected_resource_revision,
            &current_resource_revision,
        )?;
        ensure_image_revision(
            "source",
            &request.expected_source_revision,
            &current_source_revision,
        )?;
        if request.expected_target_revision != current_target_revision {
            return Err(ProductImageTranslationExactLocaleError::RevisionConflict {
                revision: "target",
            });
        }

        if let Some(existing) = target.cloned() {
            let mut active: entities::product_image_translation::ActiveModel = existing.into();
            active.alt_text = Set(request.alt_text.clone());
            active.update(&txn).await?;
        } else {
            entities::product_image_translation::ActiveModel {
                id: Set(generate_id()),
                image_id: Set(image_id),
                locale: Set(target_locale.clone()),
                alt_text: Set(request.alt_text.clone()),
            }
            .insert(&txn)
            .await?;
        }

        let translations_after = load_image_translations(&txn, image_id).await?;
        let target_after =
            exact_image_locale_row(&translations_after, &target_locale).ok_or_else(|| {
                ProductImageTranslationExactLocaleError::TargetLocaleMissingAfterApply {
                    image_id,
                    locale: target_locale.clone(),
                }
            })?;
        let resource_revision =
            product_image_translation_resource_revision(&product, &image, &translations_after);
        let target_revision = product_image_translation_locale_revision(target_after);
        let target = ProductImageTranslationExactLocaleRecord::from(target_after.clone());

        txn.publish(
            tenant_id,
            actor_user_id,
            DomainEvent::ProductUpdated {
                product_id: product.id,
            },
        )
        .await?;
        let receipt = ProductImageTranslationExactLocaleApplyReceipt {
            operation_id: current_product_operation_id(),
            product_id: product.id,
            image_id,
            resource_revision,
            target_revision,
            target,
        };
        record_product_operation_result(&receipt)?;
        txn.commit().await?;

        Ok(receipt)
    }
}

async fn load_image<C>(
    db: &C,
    tenant_id: Uuid,
    image_id: Uuid,
) -> ProductImageTranslationExactLocaleResult<entities::product_image::Model>
where
    C: ConnectionTrait,
{
    entities::product_image::Entity::find_by_id(image_id)
        .inner_join(entities::product::Entity)
        .filter(entities::product::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or(ProductImageTranslationExactLocaleError::ImageNotFound(
            image_id,
        ))
}

async fn load_image_product<C>(
    db: &C,
    tenant_id: Uuid,
    product_id: Uuid,
) -> ProductImageTranslationExactLocaleResult<entities::product::Model>
where
    C: ConnectionTrait,
{
    entities::product::Entity::find_by_id(product_id)
        .filter(entities::product::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| CommerceError::ProductNotFound(product_id).into())
}

async fn load_image_translations<C>(
    db: &C,
    image_id: Uuid,
) -> ProductImageTranslationExactLocaleResult<Vec<entities::product_image_translation::Model>>
where
    C: ConnectionTrait,
{
    Ok(entities::product_image_translation::Entity::find()
        .filter(entities::product_image_translation::Column::ImageId.eq(image_id))
        .order_by_asc(entities::product_image_translation::Column::Locale)
        .all(db)
        .await?)
}

async fn load_image_translations_for_images<C>(
    db: &C,
    image_ids: &[Uuid],
) -> ProductImageTranslationExactLocaleResult<Vec<entities::product_image_translation::Model>>
where
    C: ConnectionTrait,
{
    if image_ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(entities::product_image_translation::Entity::find()
        .filter(entities::product_image_translation::Column::ImageId.is_in(image_ids.to_vec()))
        .order_by_asc(entities::product_image_translation::Column::ImageId)
        .order_by_asc(entities::product_image_translation::Column::Locale)
        .all(db)
        .await?)
}

fn build_image_exact_locale_snapshot(
    product: entities::product::Model,
    image: entities::product_image::Model,
    translations: Vec<entities::product_image_translation::Model>,
    source_locale: String,
    target_locale: String,
) -> ProductImageTranslationExactLocaleResult<ProductImageTranslationExactLocaleSnapshot> {
    let source = exact_image_locale_row(&translations, &source_locale)
        .cloned()
        .ok_or_else(
            || ProductImageTranslationExactLocaleError::SourceLocaleNotFound {
                image_id: image.id,
                locale: source_locale.clone(),
            },
        )?;
    let target = exact_image_locale_row(&translations, &target_locale).cloned();

    let resource_revision =
        product_image_translation_resource_revision(&product, &image, &translations);
    let source_revision = product_image_translation_locale_revision(&source);
    let target_revision = target
        .as_ref()
        .map(product_image_translation_locale_revision);
    let exact_locales = translations
        .iter()
        .map(|translation| translation.locale.clone())
        .collect();

    Ok(ProductImageTranslationExactLocaleSnapshot {
        product_id: product.id,
        image_id: image.id,
        product_status: product.status,
        source_locale,
        target_locale,
        resource_revision,
        source_revision,
        target_revision,
        exact_locales,
        source: ProductImageTranslationExactLocaleRecord::from(source),
        target: target.map(ProductImageTranslationExactLocaleRecord::from),
    })
}

fn exact_image_locale_row<'a>(
    translations: &'a [entities::product_image_translation::Model],
    locale: &str,
) -> Option<&'a entities::product_image_translation::Model> {
    translations
        .iter()
        .find(|translation| translation.locale == locale)
}

fn canonical_image_translation_locale(
    locale: &str,
) -> ProductImageTranslationExactLocaleResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|error| CommerceError::Validation(error.to_string()).into())
}

fn validate_image_locale_pair(
    source_locale: &str,
    target_locale: &str,
) -> ProductImageTranslationExactLocaleResult<()> {
    if source_locale == target_locale {
        return Err(CommerceError::Validation(
            "Product image translation source and target locale must differ".to_string(),
        )
        .into());
    }
    Ok(())
}

fn validate_image_translation_alt_text(
    alt_text: Option<&str>,
) -> ProductImageTranslationExactLocaleResult<()> {
    if alt_text.is_some_and(|alt_text| alt_text.chars().count() > 255) {
        return Err(CommerceError::Validation(
            "Product image translation alt text must be at most 255 characters".to_string(),
        )
        .into());
    }
    Ok(())
}

fn ensure_image_revision(
    revision: &'static str,
    expected: &str,
    current: &str,
) -> ProductImageTranslationExactLocaleResult<()> {
    if expected != current {
        return Err(ProductImageTranslationExactLocaleError::RevisionConflict { revision });
    }
    Ok(())
}

fn product_image_translation_resource_revision(
    product: &entities::product::Model,
    image: &entities::product_image::Model,
    translations: &[entities::product_image_translation::Model],
) -> String {
    let mut hasher = Sha256::new();
    digest_image_text(&mut hasher, "rustok-product/image-translation-resource/v1");
    digest_image_text(&mut hasher, &product.id.to_string());
    digest_image_text(&mut hasher, &product.tenant_id.to_string());
    digest_image_text(&mut hasher, &product.status.to_string());
    digest_image_text(&mut hasher, &image.id.to_string());
    digest_image_text(&mut hasher, &image.product_id.to_string());
    digest_image_text(&mut hasher, &image.media_id.to_string());
    digest_image_text(&mut hasher, &image.position.to_string());

    let mut exact = translations.iter().collect::<Vec<_>>();
    exact.sort_by(|left, right| left.locale.cmp(&right.locale));
    for translation in exact {
        digest_image_translation(&mut hasher, translation);
    }
    finish_image_revision(hasher)
}

fn product_image_translation_locale_revision(
    translation: &entities::product_image_translation::Model,
) -> String {
    let mut hasher = Sha256::new();
    digest_image_text(&mut hasher, "rustok-product/image-translation-locale/v1");
    digest_image_translation(&mut hasher, translation);
    finish_image_revision(hasher)
}

fn digest_image_translation(
    hasher: &mut Sha256,
    translation: &entities::product_image_translation::Model,
) {
    digest_image_text(hasher, &translation.image_id.to_string());
    digest_image_text(hasher, &translation.locale);
    digest_image_optional_text(hasher, translation.alt_text.as_deref());
}

fn digest_image_text(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn digest_image_optional_text(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            digest_image_text(hasher, value);
        }
        None => hasher.update([0]),
    }
}

fn finish_image_revision(hasher: Sha256) -> String {
    format!("sha256:{}", hex::encode(hasher.finalize()))
}
