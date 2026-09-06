use super::*;

use rustok_api::TenantLocale;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductTranslationExactLocaleRecord {
    pub locale: String,
    pub title: String,
    pub handle: String,
    pub description: Option<String>,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
}

impl From<entities::product_translation::Model> for ProductTranslationExactLocaleRecord {
    fn from(value: entities::product_translation::Model) -> Self {
        Self {
            locale: value.locale,
            title: value.title,
            handle: value.handle,
            description: value.description,
            meta_title: value.meta_title,
            meta_description: value.meta_description,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductTranslationExactLocaleSnapshot {
    pub product_id: Uuid,
    pub product_status: entities::product::ProductStatus,
    pub source_locale: String,
    pub target_locale: String,
    pub resource_revision: String,
    pub source_revision: String,
    pub target_revision: Option<String>,
    pub exact_locales: Vec<String>,
    pub source: ProductTranslationExactLocaleRecord,
    pub target: Option<ProductTranslationExactLocaleRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductTranslationExactLocaleApply {
    pub source_locale: String,
    pub target: ProductTranslationInput,
    pub expected_resource_revision: String,
    pub expected_source_revision: String,
    pub expected_target_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductTranslationExactLocaleApplyReceipt {
    pub product_id: Uuid,
    pub resource_revision: String,
    pub target_revision: String,
    pub target: ProductTranslationExactLocaleRecord,
}

impl CatalogService {
    /// Reads one exact Product source locale and one exact target locale without
    /// consulting runtime fallback. Revisions are stable SHA-256 owner digests
    /// suitable for the neutral Translation target CAS contract.
    pub async fn read_product_translation_exact_locale(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> CommerceResult<ProductTranslationExactLocaleSnapshot> {
        let source_locale = canonical_translation_locale(source_locale)?;
        let target_locale = canonical_translation_locale(target_locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;

        let product = load_product(&self.db, tenant_id, product_id).await?;
        let translations = load_product_translations(&self.db, tenant_id, product_id).await?;
        build_exact_locale_snapshot(product, translations, source_locale, target_locale)
    }

    /// Applies one exact Product locale under resource/source/target CAS.
    ///
    /// The Product row is locked first so concurrent calls through this owner
    /// primitive serialize on one resource. Only the requested
    /// `product_translations` row is inserted or updated; sibling locale rows
    /// are never deleted or rewritten. The ProductUpdated outbox event is
    /// published through the normal Product write transaction before commit.
    pub async fn apply_product_translation_exact_locale(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Uuid,
        request: ProductTranslationExactLocaleApply,
    ) -> CommerceResult<ProductTranslationExactLocaleApplyReceipt> {
        request
            .target
            .validate()
            .map_err(|error| CommerceError::Validation(error.to_string()))?;

        let source_locale = canonical_translation_locale(&request.source_locale)?;
        let target_locale = canonical_translation_locale(&request.target.locale)?;
        validate_locale_pair(&source_locale, &target_locale)?;

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        let product = entities::product::Entity::find_by_id(product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or(CommerceError::ProductNotFound(product_id))?;

        let translations = load_product_translations(&txn, tenant_id, product_id).await?;
        let source = exact_locale_row(&translations, &source_locale).ok_or_else(|| {
            CommerceError::TranslationSourceLocaleNotFound {
                product_id,
                locale: source_locale.clone(),
            }
        })?;
        let target = exact_locale_row(&translations, &target_locale);

        let current_resource_revision = product_translation_resource_revision(&product, &translations);
        let current_source_revision = product_translation_locale_revision(source);
        let current_target_revision = target.map(product_translation_locale_revision);

        ensure_revision(
            "resource",
            &request.expected_resource_revision,
            &current_resource_revision,
        )?;
        ensure_revision(
            "source",
            &request.expected_source_revision,
            &current_source_revision,
        )?;
        if request.expected_target_revision != current_target_revision {
            return Err(CommerceError::TranslationRevisionConflict { revision: "target" });
        }

        let handle = request
            .target
            .handle
            .clone()
            .unwrap_or_else(|| slugify(&request.target.title));

        if let Some(existing) = target.cloned() {
            let mut active: entities::product_translation::ActiveModel = existing.into();
            active.title = Set(request.target.title.clone());
            active.handle = Set(handle.clone());
            active.description = Set(request.target.description.clone());
            active.meta_title = Set(request.target.meta_title.clone());
            active.meta_description = Set(request.target.meta_description.clone());
            active
                .update(&txn)
                .await
                .map_err(|error| map_product_unique_violation(error, &handle, &target_locale, None))?;
        } else {
            entities::product_translation::ActiveModel {
                id: Set(generate_id()),
                product_id: Set(product_id),
                tenant_id: Set(tenant_id),
                locale: Set(target_locale.clone()),
                title: Set(request.target.title.clone()),
                handle: Set(handle.clone()),
                description: Set(request.target.description.clone()),
                meta_title: Set(request.target.meta_title.clone()),
                meta_description: Set(request.target.meta_description.clone()),
            }
            .insert(&txn)
            .await
            .map_err(|error| map_product_unique_violation(error, &handle, &target_locale, None))?;
        }

        let translations_after = load_product_translations(&txn, tenant_id, product_id).await?;
        let target_after = exact_locale_row(&translations_after, &target_locale)
            .expect("exact Product target locale must exist after owner apply");
        let resource_revision = product_translation_resource_revision(&product, &translations_after);
        let target_revision = product_translation_locale_revision(target_after);
        let target = ProductTranslationExactLocaleRecord::from(target_after.clone());

        txn.publish(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProductUpdated { product_id },
        )
        .await?;
        txn.commit().await?;

        Ok(ProductTranslationExactLocaleApplyReceipt {
            product_id,
            resource_revision,
            target_revision,
            target,
        })
    }
}

async fn load_product<C>(
    db: &C,
    tenant_id: Uuid,
    product_id: Uuid,
) -> CommerceResult<entities::product::Model>
where
    C: ConnectionTrait,
{
    entities::product::Entity::find_by_id(product_id)
        .filter(entities::product::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or(CommerceError::ProductNotFound(product_id))
}

async fn load_product_translations<C>(
    db: &C,
    tenant_id: Uuid,
    product_id: Uuid,
) -> CommerceResult<Vec<entities::product_translation::Model>>
where
    C: ConnectionTrait,
{
    Ok(entities::product_translation::Entity::find()
        .filter(entities::product_translation::Column::TenantId.eq(tenant_id))
        .filter(entities::product_translation::Column::ProductId.eq(product_id))
        .order_by_asc(entities::product_translation::Column::Locale)
        .all(db)
        .await?)
}

fn build_exact_locale_snapshot(
    product: entities::product::Model,
    translations: Vec<entities::product_translation::Model>,
    source_locale: String,
    target_locale: String,
) -> CommerceResult<ProductTranslationExactLocaleSnapshot> {
    let source = exact_locale_row(&translations, &source_locale)
        .cloned()
        .ok_or_else(|| CommerceError::TranslationSourceLocaleNotFound {
            product_id: product.id,
            locale: source_locale.clone(),
        })?;
    let target = exact_locale_row(&translations, &target_locale).cloned();

    let resource_revision = product_translation_resource_revision(&product, &translations);
    let source_revision = product_translation_locale_revision(&source);
    let target_revision = target.as_ref().map(product_translation_locale_revision);
    let exact_locales = translations
        .iter()
        .map(|translation| translation.locale.clone())
        .collect();

    Ok(ProductTranslationExactLocaleSnapshot {
        product_id: product.id,
        product_status: product.status,
        source_locale,
        target_locale,
        resource_revision,
        source_revision,
        target_revision,
        exact_locales,
        source: ProductTranslationExactLocaleRecord::from(source),
        target: target.map(ProductTranslationExactLocaleRecord::from),
    })
}

fn exact_locale_row<'a>(
    translations: &'a [entities::product_translation::Model],
    locale: &str,
) -> Option<&'a entities::product_translation::Model> {
    translations
        .iter()
        .find(|translation| translation.locale == locale)
}

fn canonical_translation_locale(locale: &str) -> CommerceResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|error| CommerceError::Validation(error.to_string()))
}

fn validate_locale_pair(source_locale: &str, target_locale: &str) -> CommerceResult<()> {
    if source_locale == target_locale {
        return Err(CommerceError::Validation(
            "Product translation source and target locale must differ".to_string(),
        ));
    }
    Ok(())
}

fn ensure_revision(
    revision: &'static str,
    expected: &str,
    current: &str,
) -> CommerceResult<()> {
    if expected != current {
        return Err(CommerceError::TranslationRevisionConflict { revision });
    }
    Ok(())
}

fn product_translation_resource_revision(
    product: &entities::product::Model,
    translations: &[entities::product_translation::Model],
) -> String {
    let mut hasher = Sha256::new();
    digest_text(&mut hasher, "rustok-product/translation-resource/v1");
    digest_text(&mut hasher, &product.id.to_string());
    digest_text(&mut hasher, &product.tenant_id.to_string());
    digest_text(&mut hasher, &product.status.to_string());

    let mut exact = translations.iter().collect::<Vec<_>>();
    exact.sort_by(|left, right| left.locale.cmp(&right.locale));
    for translation in exact {
        digest_translation(&mut hasher, translation);
    }
    finish_revision(hasher)
}

fn product_translation_locale_revision(translation: &entities::product_translation::Model) -> String {
    let mut hasher = Sha256::new();
    digest_text(&mut hasher, "rustok-product/translation-locale/v1");
    digest_translation(&mut hasher, translation);
    finish_revision(hasher)
}

fn digest_translation(hasher: &mut Sha256, translation: &entities::product_translation::Model) {
    digest_text(hasher, &translation.product_id.to_string());
    digest_text(hasher, &translation.tenant_id.to_string());
    digest_text(hasher, &translation.locale);
    digest_text(hasher, &translation.title);
    digest_text(hasher, &translation.handle);
    digest_optional_text(hasher, translation.description.as_deref());
    digest_optional_text(hasher, translation.meta_title.as_deref());
    digest_optional_text(hasher, translation.meta_description.as_deref());
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

fn finish_revision(hasher: Sha256) -> String {
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn translation(locale: &str, title: &str) -> entities::product_translation::Model {
        entities::product_translation::Model {
            id: Uuid::new_v4(),
            product_id: Uuid::from_u128(1),
            tenant_id: Uuid::from_u128(2),
            locale: locale.to_string(),
            title: title.to_string(),
            handle: format!("{}-handle", locale.to_ascii_lowercase()),
            description: Some(format!("{title} description")),
            meta_title: Some(format!("{title} meta")),
            meta_description: None,
        }
    }

    #[test]
    fn locale_revision_is_semantic_and_ignores_storage_identity() {
        let first = translation("en", "Title");
        let mut rewritten = first.clone();
        rewritten.id = Uuid::new_v4();

        assert_eq!(
            product_translation_locale_revision(&first),
            product_translation_locale_revision(&rewritten)
        );
    }

    #[test]
    fn resource_revision_is_order_independent_and_tracks_every_exact_locale() {
        let product = entities::product::Model {
            id: Uuid::from_u128(1),
            tenant_id: Uuid::from_u128(2),
            status: entities::product::ProductStatus::Draft,
            seller_id: None,
            vendor: None,
            product_type: None,
            shipping_profile_slug: None,
            primary_category_id: None,
            metadata: serde_json::json!({}),
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
            published_at: None,
        };
        let en = translation("en", "Title");
        let fr = translation("fr", "Titre");

        let ordered = product_translation_resource_revision(&product, &[en.clone(), fr.clone()]);
        let reversed = product_translation_resource_revision(&product, &[fr.clone(), en.clone()]);
        assert_eq!(ordered, reversed);

        let mut changed = fr;
        changed.title = "Nouveau titre".to_string();
        let changed_revision = product_translation_resource_revision(&product, &[en, changed]);
        assert_ne!(ordered, changed_revision);
    }
}
