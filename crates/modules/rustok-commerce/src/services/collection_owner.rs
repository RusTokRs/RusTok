use std::collections::BTreeMap;

use chrono::Utc;
use rustok_api::TenantLocale;
use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    CommerceError, CommerceResult,
    collection_translation_changes::{
        CollectionTranslationChangeLifecycle, record_collection_translation_lifecycle_change_in_tx,
    },
    entities::{collection, collection_translation},
    services::collection_translation::{
        CollectionTranslationExactLocaleRecord, CollectionTranslationService,
    },
};

const COLLECTION_COPY_RESOURCE_REVISION_NAMESPACE: &str =
    "rustok-commerce/collection-copy-resource/v1";

/// Native Commerce owner lifecycle for Collection identity and localized copy.
///
/// Translation-target writes still go through `apply_exact_locale`; native create/update/delete
/// must go through these methods so the Collection row, exact-locale rows, and durable translation
/// ChangeCursor evidence are committed atomically.
impl CollectionTranslationService {
    pub async fn create_collection_owner(
        &self,
        tenant_id: Uuid,
        collection_type: String,
        conditions: Option<Value>,
        metadata: Value,
        translations: Vec<CollectionTranslationExactLocaleRecord>,
    ) -> CommerceResult<Uuid> {
        validate_tenant(tenant_id)?;
        let collection_type = normalize_collection_type(&collection_type)?;
        let translations = normalize_translations(translations)?;

        let txn = self.database().begin().await?;
        lock_active_tenant_collections(&txn, tenant_id).await?;
        let collection_id = generate_id();
        ensure_handles_available(&txn, tenant_id, collection_id, &translations).await?;

        let now = Utc::now();
        let model = collection::ActiveModel {
            id: Set(collection_id),
            tenant_id: Set(tenant_id),
            collection_type: Set(collection_type),
            conditions: Set(conditions),
            metadata: Set(metadata),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            deleted_at: Set(None),
        }
        .insert(&txn)
        .await?;
        insert_translations(&txn, collection_id, &translations).await?;
        let persisted = load_translations(&txn, collection_id).await?;
        let revision = collection_copy_revision(&model, &persisted);
        record_collection_translation_lifecycle_change_in_tx(
            &txn,
            tenant_id,
            collection_id,
            generate_id(),
            &revision,
            CollectionTranslationChangeLifecycle::Active,
        )
        .await?;
        txn.commit().await?;
        Ok(collection_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_collection_owner(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
        collection_type: Option<String>,
        conditions: Option<Option<Value>>,
        metadata: Option<Value>,
        translations: Option<Vec<CollectionTranslationExactLocaleRecord>>,
    ) -> CommerceResult<bool> {
        validate_tenant(tenant_id)?;
        validate_collection_id(collection_id)?;
        let collection_type = collection_type
            .as_deref()
            .map(normalize_collection_type)
            .transpose()?;
        let translations = translations.map(normalize_translations).transpose()?;

        let txn = self.database().begin().await?;
        let tenant_collections = lock_active_tenant_collections(&txn, tenant_id).await?;
        let current = tenant_collections
            .into_iter()
            .find(|model| model.id == collection_id)
            .ok_or_else(|| CommerceError::Validation("collection_id was not found".to_owned()))?;
        let current_translations = load_translations(&txn, collection_id).await?;

        if let Some(translations) = translations.as_ref() {
            ensure_handles_available(&txn, tenant_id, collection_id, translations).await?;
        }

        let owner_changed = collection_type
            .as_ref()
            .is_some_and(|value| value != &current.collection_type)
            || conditions
                .as_ref()
                .is_some_and(|value| value != &current.conditions)
            || metadata
                .as_ref()
                .is_some_and(|value| value != &current.metadata);
        let translations_changed = translations
            .as_ref()
            .is_some_and(|value| !translations_semantically_equal(&current_translations, value));
        if !owner_changed && !translations_changed {
            txn.commit().await?;
            return Ok(false);
        }

        let mut active: collection::ActiveModel = current.into();
        if let Some(collection_type) = collection_type {
            active.collection_type = Set(collection_type);
        }
        if let Some(conditions) = conditions {
            active.conditions = Set(conditions);
        }
        if let Some(metadata) = metadata {
            active.metadata = Set(metadata);
        }
        active.updated_at = Set(Utc::now().into());
        let model = active.update(&txn).await?;

        let persisted = if let Some(translations) = translations {
            if translations_changed {
                collection_translation::Entity::delete_many()
                    .filter(collection_translation::Column::CollectionId.eq(collection_id))
                    .exec(&txn)
                    .await?;
                insert_translations(&txn, collection_id, &translations).await?;
                load_translations(&txn, collection_id).await?
            } else {
                current_translations
            }
        } else {
            current_translations
        };

        let revision = collection_copy_revision(&model, &persisted);
        record_collection_translation_lifecycle_change_in_tx(
            &txn,
            tenant_id,
            collection_id,
            generate_id(),
            &revision,
            CollectionTranslationChangeLifecycle::Active,
        )
        .await?;
        txn.commit().await?;
        Ok(true)
    }

    pub async fn delete_collection_owner(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
    ) -> CommerceResult<()> {
        validate_tenant(tenant_id)?;
        validate_collection_id(collection_id)?;

        let txn = self.database().begin().await?;
        let tenant_collections = lock_active_tenant_collections(&txn, tenant_id).await?;
        let current = tenant_collections
            .into_iter()
            .find(|model| model.id == collection_id)
            .ok_or_else(|| CommerceError::Validation("collection_id was not found".to_owned()))?;
        let translations = load_translations(&txn, collection_id).await?;
        let revision = collection_copy_revision(&current, &translations);

        let now = Utc::now();
        let mut active: collection::ActiveModel = current.into();
        active.deleted_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());
        active.update(&txn).await?;
        record_collection_translation_lifecycle_change_in_tx(
            &txn,
            tenant_id,
            collection_id,
            generate_id(),
            &revision,
            CollectionTranslationChangeLifecycle::Deleted,
        )
        .await?;
        txn.commit().await?;
        Ok(())
    }
}

fn validate_tenant(tenant_id: Uuid) -> CommerceResult<()> {
    if tenant_id.is_nil() {
        return Err(CommerceError::Validation(
            "tenant_id must not be nil".to_owned(),
        ));
    }
    Ok(())
}

fn validate_collection_id(collection_id: Uuid) -> CommerceResult<()> {
    if collection_id.is_nil() {
        return Err(CommerceError::Validation(
            "collection_id must not be nil".to_owned(),
        ));
    }
    Ok(())
}

fn normalize_collection_type(value: &str) -> CommerceResult<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 64 {
        return Err(CommerceError::Validation(
            "collection type must contain between 1 and 64 characters".to_owned(),
        ));
    }
    Ok(value.to_owned())
}

fn normalize_translations(
    translations: Vec<CollectionTranslationExactLocaleRecord>,
) -> CommerceResult<Vec<CollectionTranslationExactLocaleRecord>> {
    if translations.is_empty() {
        return Err(CommerceError::Validation(
            "at least one Collection translation is required".to_owned(),
        ));
    }

    let mut normalized = BTreeMap::new();
    for mut translation in translations {
        let locale = TenantLocale::new(&translation.locale)
            .map(TenantLocale::into_inner)
            .map_err(|error| CommerceError::Validation(error.to_string()))?;
        validate_copy(&translation.title, &translation.handle)?;
        translation.locale = locale.clone();
        if normalized.insert(locale, translation).is_some() {
            return Err(CommerceError::Validation(
                "Collection translations must contain unique locales".to_owned(),
            ));
        }
    }
    Ok(normalized.into_values().collect())
}

fn validate_copy(title: &str, handle: &str) -> CommerceResult<()> {
    if title.trim().is_empty() || title.chars().count() > 255 {
        return Err(CommerceError::Validation(
            "Collection translation title must be nonblank and at most 255 characters".to_owned(),
        ));
    }
    if handle.trim().is_empty() || handle.chars().count() > 255 {
        return Err(CommerceError::Validation(
            "Collection translation handle must be nonblank and at most 255 characters".to_owned(),
        ));
    }
    Ok(())
}

async fn lock_active_tenant_collections(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> CommerceResult<Vec<collection::Model>> {
    Ok(collection::Entity::find()
        .filter(collection::Column::TenantId.eq(tenant_id))
        .filter(collection::Column::DeletedAt.is_null())
        .order_by_asc(collection::Column::Id)
        .lock_exclusive()
        .all(txn)
        .await?)
}

async fn ensure_handles_available(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    collection_id: Uuid,
    translations: &[CollectionTranslationExactLocaleRecord],
) -> CommerceResult<()> {
    let other_ids = collection::Entity::find()
        .filter(collection::Column::TenantId.eq(tenant_id))
        .filter(collection::Column::DeletedAt.is_null())
        .filter(collection::Column::Id.ne(collection_id))
        .all(txn)
        .await?
        .into_iter()
        .map(|model| model.id)
        .collect::<Vec<_>>();
    if other_ids.is_empty() {
        return Ok(());
    }

    for translation in translations {
        let collision = collection_translation::Entity::find()
            .filter(collection_translation::Column::CollectionId.is_in(other_ids.clone()))
            .filter(collection_translation::Column::Locale.eq(&translation.locale))
            .filter(collection_translation::Column::Handle.eq(&translation.handle))
            .one(txn)
            .await?;
        if collision.is_some() {
            return Err(CommerceError::DuplicateHandle {
                handle: translation.handle.clone(),
                locale: translation.locale.clone(),
            });
        }
    }
    Ok(())
}

async fn insert_translations(
    txn: &DatabaseTransaction,
    collection_id: Uuid,
    translations: &[CollectionTranslationExactLocaleRecord],
) -> CommerceResult<()> {
    for translation in translations {
        collection_translation::ActiveModel {
            id: Set(generate_id()),
            collection_id: Set(collection_id),
            locale: Set(translation.locale.clone()),
            title: Set(translation.title.clone()),
            handle: Set(translation.handle.clone()),
            description: Set(translation.description.clone()),
        }
        .insert(txn)
        .await?;
    }
    Ok(())
}

async fn load_translations(
    txn: &DatabaseTransaction,
    collection_id: Uuid,
) -> CommerceResult<Vec<collection_translation::Model>> {
    Ok(collection_translation::Entity::find()
        .filter(collection_translation::Column::CollectionId.eq(collection_id))
        .order_by_asc(collection_translation::Column::Locale)
        .all(txn)
        .await?)
}

fn translations_semantically_equal(
    current: &[collection_translation::Model],
    next: &[CollectionTranslationExactLocaleRecord],
) -> bool {
    current.len() == next.len()
        && current.iter().zip(next).all(|(current, next)| {
            current.locale == next.locale
                && current.title == next.title
                && current.handle == next.handle
                && current.description == next.description
        })
}

// Keep this byte-for-byte contract aligned with CollectionTranslationService's canonical
// resource revision. The retained PostgreSQL lifecycle evidence compares journal revisions to
// provider snapshots on every owner mutation, so drift is caught before this target can ship.
fn collection_copy_revision(
    collection: &collection::Model,
    translations: &[collection_translation::Model],
) -> String {
    let mut hasher = Sha256::new();
    digest_text(&mut hasher, COLLECTION_COPY_RESOURCE_REVISION_NAMESPACE);
    digest_text(&mut hasher, &collection.id.to_string());
    digest_text(&mut hasher, &collection.tenant_id.to_string());
    digest_text(&mut hasher, &collection.collection_type);
    digest_optional_json(&mut hasher, collection.conditions.as_ref());
    digest_json(&mut hasher, &collection.metadata);

    let mut exact = translations.iter().collect::<Vec<_>>();
    exact.sort_by(|left, right| left.locale.cmp(&right.locale));
    for translation in exact {
        digest_text(&mut hasher, &translation.collection_id.to_string());
        digest_text(&mut hasher, &translation.locale);
        digest_text(&mut hasher, &translation.title);
        digest_text(&mut hasher, &translation.handle);
        digest_optional_text(&mut hasher, translation.description.as_deref());
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn digest_json(hasher: &mut Sha256, value: &Value) {
    let bytes = serde_json::to_vec(value).expect("serde_json::Value serialization is infallible");
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn digest_optional_json(hasher: &mut Sha256, value: Option<&Value>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            digest_json(hasher, value);
        }
        None => hasher.update([0]),
    }
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
