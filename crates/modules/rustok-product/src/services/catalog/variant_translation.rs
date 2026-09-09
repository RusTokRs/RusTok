use super::*;

use rustok_api::TenantLocale;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProductVariantTranslationExactLocaleError {
    #[error(transparent)]
    Commerce(#[from] CommerceError),

    #[error("Product variant not found: {0}")]
    VariantNotFound(Uuid),

    #[error(
        "Product variant translation source locale not found: {locale} for variant {variant_id}"
    )]
    SourceLocaleNotFound { variant_id: Uuid, locale: String },

    #[error(
        "Product variant translation target locale missing after apply: {locale} for variant {variant_id}"
    )]
    TargetLocaleMissingAfterApply { variant_id: Uuid, locale: String },

    #[error("Product variant translation {revision} revision conflict")]
    RevisionConflict { revision: &'static str },
}

impl From<sea_orm::DbErr> for ProductVariantTranslationExactLocaleError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::Commerce(CommerceError::Database(error))
    }
}

pub type ProductVariantTranslationExactLocaleResult<T> =
    Result<T, ProductVariantTranslationExactLocaleError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductVariantTranslationExactLocaleRecord {
    pub locale: String,
    pub title: Option<String>,
}

impl From<entities::variant_translation::Model> for ProductVariantTranslationExactLocaleRecord {
    fn from(value: entities::variant_translation::Model) -> Self {
        Self {
            locale: value.locale,
            title: value.title,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductVariantTranslationExactLocaleSnapshot {
    pub product_id: Uuid,
    pub variant_id: Uuid,
    pub product_status: entities::product::ProductStatus,
    pub source_locale: String,
    pub target_locale: String,
    pub resource_revision: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub exact_locales: Vec<String>,
    pub source: ProductVariantTranslationExactLocaleRecord,
    pub target: Option<ProductVariantTranslationExactLocaleRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductVariantTranslationExactLocaleApply {
    pub source_locale: String,
    pub target_locale: String,
    pub title: Option<String>,
    pub expected_resource_revision: String,
    pub expected_source_revision: String,
    pub expected_target_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductVariantTranslationExactLocaleApplyReceipt {
    pub operation_id: Option<Uuid>,
    pub product_id: Uuid,
    pub variant_id: Uuid,
    pub resource_revision: String,
    pub target_revision: String,
    pub target: ProductVariantTranslationExactLocaleRecord,
}

impl CatalogService {
    /// Lists non-archived Product Variants that own the requested exact source locale.
    ///
    /// Ordering and pagination use Variant UUIDs. The page is assembled with
    /// bounded bulk owner reads so every summary uses the same semantic revision
    /// algorithm as exact read/apply without provider-owned SQL or N+1 queries.
    pub async fn list_product_variant_translation_exact_resources(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
        after: Option<Uuid>,
        limit: u16,
    ) -> ProductVariantTranslationExactLocaleResult<(
        Vec<ProductVariantTranslationExactLocaleSnapshot>,
        Option<Uuid>,
    )> {
        let source_locale = canonical_variant_translation_locale(source_locale)?;
        let target_locale = canonical_variant_translation_locale(target_locale)?;
        validate_variant_locale_pair(&source_locale, &target_locale)?;
        if limit == 0 {
            return Err(CommerceError::Validation(
                "Product variant translation resource page limit must be positive".to_string(),
            )
            .into());
        }

        let mut query = entities::product_variant::Entity::find()
            .inner_join(entities::product::Entity)
            .inner_join(entities::variant_translation::Entity)
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .filter(
                entities::product::Column::Status.ne(entities::product::ProductStatus::Archived),
            )
            .filter(entities::variant_translation::Column::Locale.eq(source_locale.clone()))
            .order_by_asc(entities::product_variant::Column::Id);
        if let Some(after) = after {
            query = query.filter(entities::product_variant::Column::Id.gt(after));
        }

        let mut variants = query.limit(u64::from(limit) + 1).all(&self.db).await?;
        let has_more = variants.len() > usize::from(limit);
        if has_more {
            variants.truncate(usize::from(limit));
        }
        let next_after = has_more
            .then(|| variants.last().map(|variant| variant.id))
            .flatten();
        if variants.is_empty() {
            return Ok((Vec::new(), None));
        }

        let variant_ids = variants
            .iter()
            .map(|variant| variant.id)
            .collect::<Vec<_>>();
        let product_ids = variants
            .iter()
            .map(|variant| variant.product_id)
            .collect::<HashSet<_>>();
        let products = entities::product::Entity::find()
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .filter(entities::product::Column::Id.is_in(product_ids))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|product| (product.id, product))
            .collect::<HashMap<_, _>>();
        let mut translations = load_variant_translations_for_variants(&self.db, &variant_ids)
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

        let mut resources = Vec::with_capacity(variants.len());
        for variant in variants {
            let product = products
                .get(&variant.product_id)
                .cloned()
                .ok_or(CommerceError::ProductNotFound(variant.product_id))?;
            let exact = translations.remove(&variant.id).ok_or_else(|| {
                ProductVariantTranslationExactLocaleError::SourceLocaleNotFound {
                    variant_id: variant.id,
                    locale: source_locale.clone(),
                }
            })?;
            resources.push(build_variant_exact_locale_snapshot(
                product,
                variant,
                exact,
                source_locale.clone(),
                target_locale.clone(),
            )?);
        }

        Ok((resources, next_after))
    }

    /// Reads one exact Product Variant source locale and one exact target locale
    /// without consulting storefront fallback.
    ///
    /// The resource revision covers Product lifecycle plus every exact Variant
    /// title locale, while locale revisions cover only the selected owner row.
    pub async fn read_product_variant_translation_exact_locale(
        &self,
        tenant_id: Uuid,
        variant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> ProductVariantTranslationExactLocaleResult<ProductVariantTranslationExactLocaleSnapshot>
    {
        let source_locale = canonical_variant_translation_locale(source_locale)?;
        let target_locale = canonical_variant_translation_locale(target_locale)?;
        validate_variant_locale_pair(&source_locale, &target_locale)?;

        let variant = load_variant(&self.db, tenant_id, variant_id).await?;
        let product = load_variant_product(&self.db, tenant_id, variant.product_id).await?;
        let translations = load_variant_translations(&self.db, variant_id).await?;

        build_variant_exact_locale_snapshot(
            product,
            variant,
            translations,
            source_locale,
            target_locale,
        )
    }

    /// Applies one exact Product Variant locale under resource/source/target CAS.
    ///
    /// The parent Product row is locked before the Variant row so lifecycle
    /// changes and Variant translation writes serialize in canonical owner order.
    /// Only the requested `product_variant_translations` row is inserted or
    /// updated; sibling locale rows are preserved. `ProductUpdated` publication
    /// and any active Product owner-operation receipt complete in the same write
    /// transaction before commit.
    pub async fn apply_product_variant_translation_exact_locale(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        variant_id: Uuid,
        request: ProductVariantTranslationExactLocaleApply,
    ) -> ProductVariantTranslationExactLocaleResult<ProductVariantTranslationExactLocaleApplyReceipt>
    {
        validate_variant_translation_title(request.title.as_deref())?;

        let source_locale = canonical_variant_translation_locale(&request.source_locale)?;
        let target_locale = canonical_variant_translation_locale(&request.target_locale)?;
        validate_variant_locale_pair(&source_locale, &target_locale)?;

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

        // Read once to discover the immutable parent identity, then lock parent
        // before child to stay compatible with Product lifecycle write ordering.
        let discovered_variant = load_variant(&txn, tenant_id, variant_id).await?;
        let product = entities::product::Entity::find_by_id(discovered_variant.product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or(CommerceError::ProductNotFound(
                discovered_variant.product_id,
            ))?;
        let variant = entities::product_variant::Entity::find_by_id(variant_id)
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .filter(entities::product_variant::Column::ProductId.eq(product.id))
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or(ProductVariantTranslationExactLocaleError::VariantNotFound(
                variant_id,
            ))?;

        let translations = load_variant_translations(&txn, variant_id).await?;
        let source = exact_variant_locale_row(&translations, &source_locale).ok_or_else(|| {
            ProductVariantTranslationExactLocaleError::SourceLocaleNotFound {
                variant_id,
                locale: source_locale.clone(),
            }
        })?;
        let target = exact_variant_locale_row(&translations, &target_locale);

        let current_resource_revision =
            product_variant_translation_resource_revision(&product, &variant, &translations);
        let current_source_revision = product_variant_translation_locale_revision(source);
        let current_target_revision = target.map(product_variant_translation_locale_revision);

        ensure_variant_revision(
            "resource",
            &request.expected_resource_revision,
            &current_resource_revision,
        )?;
        ensure_variant_revision(
            "source",
            &request.expected_source_revision,
            &current_source_revision,
        )?;
        if request.expected_target_revision != current_target_revision {
            return Err(
                ProductVariantTranslationExactLocaleError::RevisionConflict { revision: "target" },
            );
        }

        if let Some(existing) = target.cloned() {
            let mut active: entities::variant_translation::ActiveModel = existing.into();
            active.title = Set(request.title.clone());
            active.update(&txn).await?;
        } else {
            entities::variant_translation::ActiveModel {
                id: Set(generate_id()),
                variant_id: Set(variant_id),
                locale: Set(target_locale.clone()),
                title: Set(request.title.clone()),
            }
            .insert(&txn)
            .await?;
        }

        let translations_after = load_variant_translations(&txn, variant_id).await?;
        let target_after = exact_variant_locale_row(&translations_after, &target_locale)
            .ok_or_else(|| {
                ProductVariantTranslationExactLocaleError::TargetLocaleMissingAfterApply {
                    variant_id,
                    locale: target_locale.clone(),
                }
            })?;
        let resource_revision =
            product_variant_translation_resource_revision(&product, &variant, &translations_after);
        let target_revision = product_variant_translation_locale_revision(target_after);
        let target = ProductVariantTranslationExactLocaleRecord::from(target_after.clone());

        txn.publish(
            tenant_id,
            actor_user_id,
            DomainEvent::ProductUpdated {
                product_id: product.id,
            },
        )
        .await?;
        let receipt = ProductVariantTranslationExactLocaleApplyReceipt {
            operation_id: current_product_operation_id(),
            product_id: product.id,
            variant_id,
            resource_revision,
            target_revision,
            target,
        };
        record_product_operation_result(&receipt)?;
        txn.commit().await?;

        Ok(receipt)
    }
}

async fn load_variant<C>(
    db: &C,
    tenant_id: Uuid,
    variant_id: Uuid,
) -> ProductVariantTranslationExactLocaleResult<entities::product_variant::Model>
where
    C: ConnectionTrait,
{
    entities::product_variant::Entity::find_by_id(variant_id)
        .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or(ProductVariantTranslationExactLocaleError::VariantNotFound(
            variant_id,
        ))
}

async fn load_variant_product<C>(
    db: &C,
    tenant_id: Uuid,
    product_id: Uuid,
) -> ProductVariantTranslationExactLocaleResult<entities::product::Model>
where
    C: ConnectionTrait,
{
    entities::product::Entity::find_by_id(product_id)
        .filter(entities::product::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| CommerceError::ProductNotFound(product_id).into())
}

async fn load_variant_translations<C>(
    db: &C,
    variant_id: Uuid,
) -> ProductVariantTranslationExactLocaleResult<Vec<entities::variant_translation::Model>>
where
    C: ConnectionTrait,
{
    Ok(entities::variant_translation::Entity::find()
        .filter(entities::variant_translation::Column::VariantId.eq(variant_id))
        .order_by_asc(entities::variant_translation::Column::Locale)
        .all(db)
        .await?)
}

async fn load_variant_translations_for_variants<C>(
    db: &C,
    variant_ids: &[Uuid],
) -> ProductVariantTranslationExactLocaleResult<Vec<entities::variant_translation::Model>>
where
    C: ConnectionTrait,
{
    if variant_ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(entities::variant_translation::Entity::find()
        .filter(entities::variant_translation::Column::VariantId.is_in(variant_ids.to_vec()))
        .order_by_asc(entities::variant_translation::Column::VariantId)
        .order_by_asc(entities::variant_translation::Column::Locale)
        .all(db)
        .await?)
}

fn build_variant_exact_locale_snapshot(
    product: entities::product::Model,
    variant: entities::product_variant::Model,
    translations: Vec<entities::variant_translation::Model>,
    source_locale: String,
    target_locale: String,
) -> ProductVariantTranslationExactLocaleResult<ProductVariantTranslationExactLocaleSnapshot> {
    let source = exact_variant_locale_row(&translations, &source_locale)
        .cloned()
        .ok_or_else(
            || ProductVariantTranslationExactLocaleError::SourceLocaleNotFound {
                variant_id: variant.id,
                locale: source_locale.clone(),
            },
        )?;
    let target = exact_variant_locale_row(&translations, &target_locale).cloned();

    let resource_revision =
        product_variant_translation_resource_revision(&product, &variant, &translations);
    let source_revision = product_variant_translation_locale_revision(&source);
    let target_revision = target
        .as_ref()
        .map(product_variant_translation_locale_revision);
    let exact_locales = translations
        .iter()
        .map(|translation| translation.locale.clone())
        .collect();

    Ok(ProductVariantTranslationExactLocaleSnapshot {
        product_id: product.id,
        variant_id: variant.id,
        product_status: product.status,
        source_locale,
        target_locale,
        resource_revision,
        source_revision,
        target_revision,
        exact_locales,
        source: ProductVariantTranslationExactLocaleRecord::from(source),
        target: target.map(ProductVariantTranslationExactLocaleRecord::from),
    })
}

fn exact_variant_locale_row<'a>(
    translations: &'a [entities::variant_translation::Model],
    locale: &str,
) -> Option<&'a entities::variant_translation::Model> {
    translations
        .iter()
        .find(|translation| translation.locale == locale)
}

fn canonical_variant_translation_locale(
    locale: &str,
) -> ProductVariantTranslationExactLocaleResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|error| CommerceError::Validation(error.to_string()).into())
}

fn validate_variant_locale_pair(
    source_locale: &str,
    target_locale: &str,
) -> ProductVariantTranslationExactLocaleResult<()> {
    if source_locale == target_locale {
        return Err(CommerceError::Validation(
            "Product variant translation source and target locale must differ".to_string(),
        )
        .into());
    }
    Ok(())
}

fn validate_variant_translation_title(
    title: Option<&str>,
) -> ProductVariantTranslationExactLocaleResult<()> {
    if title.is_some_and(|title| title.chars().count() > 255) {
        return Err(CommerceError::Validation(
            "Product variant translation title must be at most 255 characters".to_string(),
        )
        .into());
    }
    Ok(())
}

fn ensure_variant_revision(
    revision: &'static str,
    expected: &str,
    current: &str,
) -> ProductVariantTranslationExactLocaleResult<()> {
    if expected != current {
        return Err(ProductVariantTranslationExactLocaleError::RevisionConflict { revision });
    }
    Ok(())
}

fn product_variant_translation_resource_revision(
    product: &entities::product::Model,
    variant: &entities::product_variant::Model,
    translations: &[entities::variant_translation::Model],
) -> String {
    let mut hasher = Sha256::new();
    digest_variant_text(
        &mut hasher,
        "rustok-product/variant-translation-resource/v1",
    );
    digest_variant_text(&mut hasher, &product.id.to_string());
    digest_variant_text(&mut hasher, &product.tenant_id.to_string());
    digest_variant_text(&mut hasher, &product.status.to_string());
    digest_variant_text(&mut hasher, &variant.id.to_string());
    digest_variant_text(&mut hasher, &variant.product_id.to_string());
    digest_variant_text(&mut hasher, &variant.tenant_id.to_string());

    let mut exact = translations.iter().collect::<Vec<_>>();
    exact.sort_by(|left, right| left.locale.cmp(&right.locale));
    for translation in exact {
        digest_variant_translation(&mut hasher, translation);
    }
    finish_variant_revision(hasher)
}

fn product_variant_translation_locale_revision(
    translation: &entities::variant_translation::Model,
) -> String {
    let mut hasher = Sha256::new();
    digest_variant_text(&mut hasher, "rustok-product/variant-translation-locale/v1");
    digest_variant_translation(&mut hasher, translation);
    finish_variant_revision(hasher)
}

fn digest_variant_translation(
    hasher: &mut Sha256,
    translation: &entities::variant_translation::Model,
) {
    digest_variant_text(hasher, &translation.variant_id.to_string());
    digest_variant_text(hasher, &translation.locale);
    digest_variant_optional_text(hasher, translation.title.as_deref());
}

fn digest_variant_text(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn digest_variant_optional_text(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            digest_variant_text(hasher, value);
        }
        None => hasher.update([0]),
    }
}

fn finish_variant_revision(hasher: Sha256) -> String {
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn product(status: entities::product::ProductStatus) -> entities::product::Model {
        entities::product::Model {
            id: Uuid::from_u128(1),
            tenant_id: Uuid::from_u128(2),
            status,
            seller_id: None,
            vendor: None,
            product_type: None,
            shipping_profile_slug: None,
            primary_category_id: None,
            metadata: serde_json::json!({}),
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
            published_at: None,
        }
    }

    fn variant() -> entities::product_variant::Model {
        entities::product_variant::Model {
            id: Uuid::from_u128(3),
            product_id: Uuid::from_u128(1),
            tenant_id: Uuid::from_u128(2),
            sku: None,
            barcode: None,
            shipping_profile_slug: None,
            ean: None,
            upc: None,
            inventory_policy: "deny".to_string(),
            inventory_management: "manual".to_string(),
            inventory_quantity: 0,
            weight: None,
            weight_unit: None,
            option1: None,
            option2: None,
            option3: None,
            position: 0,
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        }
    }

    fn translation(locale: &str, title: Option<&str>) -> entities::variant_translation::Model {
        entities::variant_translation::Model {
            id: Uuid::new_v4(),
            variant_id: Uuid::from_u128(3),
            locale: locale.to_string(),
            title: title.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn locale_revision_is_semantic_and_ignores_storage_identity() {
        let first = translation("en", Some("Small / Blue"));
        let mut rewritten = first.clone();
        rewritten.id = Uuid::new_v4();

        assert_eq!(
            product_variant_translation_locale_revision(&first),
            product_variant_translation_locale_revision(&rewritten)
        );
    }

    #[test]
    fn resource_revision_tracks_lifecycle_and_every_exact_locale() {
        let variant = variant();
        let en = translation("en", Some("Small / Blue"));
        let fr = translation("fr", Some("Petit / Bleu"));
        let draft = product(entities::product::ProductStatus::Draft);

        let ordered = product_variant_translation_resource_revision(
            &draft,
            &variant,
            &[en.clone(), fr.clone()],
        );
        let reversed = product_variant_translation_resource_revision(
            &draft,
            &variant,
            &[fr.clone(), en.clone()],
        );
        assert_eq!(ordered, reversed);

        let mut changed = fr;
        changed.title = Some("Petit / Marine".to_string());
        let changed_revision =
            product_variant_translation_resource_revision(&draft, &variant, &[en, changed]);
        assert_ne!(ordered, changed_revision);

        let archived = product(entities::product::ProductStatus::Archived);
        let archived_revision = product_variant_translation_resource_revision(
            &archived,
            &variant,
            &[
                translation("en", Some("Small / Blue")),
                translation("fr", Some("Petit / Bleu")),
            ],
        );
        assert_ne!(ordered, archived_revision);
    }

    #[test]
    fn locale_and_title_validation_follow_owner_storage_contract() {
        assert_eq!(
            canonical_variant_translation_locale(" pt_br ").expect("canonical locale"),
            "pt-BR"
        );
        assert!(canonical_variant_translation_locale("und").is_err());
        assert!(validate_variant_locale_pair("en", "en").is_err());
        assert!(validate_variant_translation_title(Some(&"x".repeat(255))).is_ok());
        assert!(validate_variant_translation_title(Some(&"x".repeat(256))).is_err());
    }
}
