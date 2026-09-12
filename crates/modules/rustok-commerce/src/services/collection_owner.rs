use std::collections::BTreeMap;

use chrono::Utc;
use rustok_api::TenantLocale;
use rustok_core::generate_id;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    CommerceError, CommerceResult,
    collection_translation_changes::{
        CollectionTranslationChangeLifecycle,
        record_collection_translation_lifecycle_change_in_tx,
    },
    entities::{collection, collection_translation},
};

use super::collection_translation::resource_revision;

const OWNER_SLUG: &str = "commerce";
const RESOURCE_KIND: &str = "collection_copy";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionOwnerTranslationInput {
    pub locale: String,
    pub title: String,
    pub handle: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateCollectionOwnerInput {
    pub collection_type: String,
    pub conditions: Option<serde_json::Value>,
    pub metadata: serde_json::Value,
    pub translations: Vec<CollectionOwnerTranslationInput>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateCollectionOwnerInput {
    pub collection_type: Option<String>,
    pub conditions: Option<Option<serde_json::Value>>,
    pub metadata: Option<serde_json::Value>,
    pub translations: Option<Vec<CollectionOwnerTranslationInput>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectionOwnerSnapshot {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub collection_type: String,
    pub conditions: Option<serde_json::Value>,
    pub metadata: serde_json::Value,
    pub translations: Vec<CollectionOwnerTranslationInput>,
}

#[derive(Clone)]
pub struct CollectionOwnerService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl CollectionOwnerService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    pub async fn create_collection(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        input: CreateCollectionOwnerInput,
    ) -> CommerceResult<CollectionOwnerSnapshot> {
        validate_tenant(tenant_id)?;
        let collection_type = normalize_collection_type(&input.collection_type)?;
        let translations = normalize_translations(input.translations)?;
        let txn = self.db.begin().await?;
        lock_tenant_collections(&txn, tenant_id).await?;
        ensure_handles_available(&txn, tenant_id, None, &translations).await?;

        let now = Utc::now();
        let model = collection::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(tenant_id),
            collection_type: Set(collection_type),
            conditions: Set(input.conditions),
            metadata: Set(input.metadata),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            deleted_at: Set(None),
        }
        .insert(&txn)
        .await?;
        insert_translations(&txn, model.id, &translations).await?;
        let persisted = load_translations(&txn, model.id).await?;
        let revision = resource_revision(&model, &persisted);
        let root_event_id = publish_copy_change(
            &self.event_bus,
            &txn,
            tenant_id,
            actor_user_id,
            model.id,
            &translations[0].locale,
            &revision,
            "create_collection",
        )
        .await?;
        record_collection_translation_lifecycle_change_in_tx(
            &txn,
            tenant_id,
            model.id,
            root_event_id,
            &revision,
            CollectionTranslationChangeLifecycle::Active,
        )
        .await?;
        txn.commit().await?;
        Ok(snapshot(model, persisted))
    }

    pub async fn update_collection(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        collection_id: Uuid,
        input: UpdateCollectionOwnerInput,
    ) -> CommerceResult<CollectionOwnerSnapshot> {
        validate_tenant(tenant_id)?;
        validate_collection_id(collection_id)?;
        let collection_type = input
            .collection_type
            .as_deref()
            .map(normalize_collection_type)
            .transpose()?;
        let normalized_translations = input
            .translations
            .map(normalize_translations)
            .transpose()?;

        let txn = self.db.begin().await?;
        let tenant_collections = lock_tenant_collections(&txn, tenant_id).await?;
        let current = tenant_collections
            .into_iter()
            .find(|collection| collection.id == collection_id && collection.deleted_at.is_none())
            .ok_or_else(|| CommerceError::Validation("collection_id was not found".to_string()))?;
        let existing_translations = load_translations(&txn, collection_id).await?;

        let copy_changed = normalized_translations
            .as_ref()
            .is_some_and(|next| !translations_semantically_equal(&existing_translations, next));
        if let Some(translations) = normalized_translations.as_ref()
            && copy_changed
        {
            ensure_handles_available(&txn, tenant_id, Some(collection_id), translations).await?;
        }

        let mut active: collection::ActiveModel = current.into();
        if let Some(collection_type) = collection_type {
            active.collection_type = Set(collection_type);
        }
        if let Some(conditions) = input.conditions {
            active.conditions = Set(conditions);
        }
        if let Some(metadata) = input.metadata {
            active.metadata = Set(metadata);
        }
        active.updated_at = Set(Utc::now().into());
        let model = active.update(&txn).await?;

        let persisted = match normalized_translations {
            Some(translations) if copy_changed => {
                collection_translation::Entity::delete_many()
                    .filter(collection_translation::Column::CollectionId.eq(collection_id))
                    .exec(&txn)
                    .await?;
                insert_translations(&txn, collection_id, &translations).await?;
                load_translations(&txn, collection_id).await?
            }
            _ => existing_translations,
        };

        if copy_changed {
            let revision = resource_revision(&model, &persisted);
            let changed_locale = persisted
                .first()
                .map(|translation| translation.locale.as_str())
                .expect("Collection owner requires at least one localized copy");
            let root_event_id = publish_copy_change(
                &self.event_bus,
                &txn,
                tenant_id,
                actor_user_id,
                collection_id,
                changed_locale,
                &revision,
                "update_collection",
            )
            .await?;
            record_collection_translation_lifecycle_change_in_tx(
                &txn,
                tenant_id,
                collection_id,
                root_event_id,
                &revision,
                CollectionTranslationChangeLifecycle::Active,
            )
            .await?;
        }
        txn.commit().await?;
        Ok(snapshot(model, persisted))
    }

    pub async fn delete_collection(
        &self,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        collection_id: Uuid,
    ) -> CommerceResult<()> {
        validate_tenant(tenant_id)?;
        validate_collection_id(collection_id)?;
        let txn = self.db.begin().await?;
        let tenant_collections = lock_tenant_collections(&txn, tenant_id).await?;
        let current = tenant_collections
            .into_iter()
            .find(|collection| collection.id == collection_id && collection.deleted_at.is_none())
            .ok_or_else(|| CommerceError::Validation("collection_id was not found".to_string()))?;
        let translations = load_translations(&txn, collection_id).await?;
        if translations.is_empty() {
            return Err(CommerceError::Validation(
                "Collection owner state must retain at least one localized copy".to_string(),
            ));
        }
        let revision = resource_revision(&current, &translations);
        let root_event_id = publish_copy_change(
            &self.event_bus,
            &txn,
            tenant_id,
            actor_user_id,
            collection_id,
            &translations[0].locale,
            &revision,
            "delete_collection",
        )
        .await?;
        record_collection_translation_lifecycle_change_in_tx(
            &txn,
            tenant_id,
            collection_id,
            root_event_id,
            &revision,
            CollectionTranslationChangeLifecycle::Deleted,
        )
        .await?;

        let mut active: collection::ActiveModel = current.into();
        let now = Utc::now();
        active.deleted_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());
        active.update(&txn).await?;
        txn.commit().await?;
        Ok(())
    }
}

fn validate_tenant(tenant_id: Uuid) -> CommerceResult<()> {
    if tenant_id.is_nil() {
        return Err(CommerceError::Validation("tenant_id must not be nil".to_string()));
    }
    Ok(())
}

fn validate_collection_id(collection_id: Uuid) -> CommerceResult<()> {
    if collection_id.is_nil() {
        return Err(CommerceError::Validation(
            "collection_id must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn normalize_collection_type(value: &str) -> CommerceResult<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() || value.chars().count() > 32 {
        return Err(CommerceError::Validation(
            "collection_type must contain between 1 and 32 characters".to_string(),
        ));
    }
    Ok(value)
}

fn normalize_translations(
    translations: Vec<CollectionOwnerTranslationInput>,
) -> CommerceResult<Vec<CollectionOwnerTranslationInput>> {
    if translations.is_empty() {
        return Err(CommerceError::Validation(
            "at least one Collection translation is required".to_string(),
        ));
    }
    let mut normalized = BTreeMap::new();
    for translation in translations {
        let locale = TenantLocale::new(&translation.locale)
            .map(TenantLocale::into_inner)
            .map_err(|error| CommerceError::Validation(error.to_string()))?;
        let title = translation.title.trim();
        let handle = translation.handle.trim();
        if title.is_empty() || title.chars().count() > 255 {
            return Err(CommerceError::Validation(
                "Collection title must contain between 1 and 255 characters".to_string(),
            ));
        }
        if handle.is_empty() || handle.chars().count() > 255 {
            return Err(CommerceError::Validation(
                "Collection handle must contain between 1 and 255 characters".to_string(),
            ));
        }
        let description = translation
            .description
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if normalized
            .insert(
                locale.clone(),
                CollectionOwnerTranslationInput {
                    locale,
                    title: title.to_string(),
                    handle: handle.to_string(),
                    description,
                },
            )
            .is_some()
        {
            return Err(CommerceError::Validation(
                "Collection translations must contain unique locales".to_string(),
            ));
        }
    }
    Ok(normalized.into_values().collect())
}

async fn lock_tenant_collections<C>(
    db: &C,
    tenant_id: Uuid,
) -> CommerceResult<Vec<collection::Model>>
where
    C: sea_orm::ConnectionTrait,
{
    Ok(collection::Entity::find()
        .filter(collection::Column::TenantId.eq(tenant_id))
        .order_by_asc(collection::Column::Id)
        .lock_exclusive()
        .all(db)
        .await?)
}

async fn ensure_handles_available<C>(
    db: &C,
    tenant_id: Uuid,
    current_collection_id: Option<Uuid>,
    translations: &[CollectionOwnerTranslationInput],
) -> CommerceResult<()>
where
    C: sea_orm::ConnectionTrait,
{
    let active_ids = collection::Entity::find()
        .select_only()
        .column(collection::Column::Id)
        .filter(collection::Column::TenantId.eq(tenant_id))
        .filter(collection::Column::DeletedAt.is_null())
        .into_tuple::<Uuid>()
        .all(db)
        .await?;
    for translation in translations {
        let mut query = collection_translation::Entity::find()
            .filter(collection_translation::Column::CollectionId.is_in(active_ids.clone()))
            .filter(collection_translation::Column::Locale.eq(&translation.locale))
            .filter(collection_translation::Column::Handle.eq(&translation.handle));
        if let Some(current_collection_id) = current_collection_id {
            query = query.filter(
                collection_translation::Column::CollectionId.ne(current_collection_id),
            );
        }
        if query.one(db).await?.is_some() {
            return Err(CommerceError::DuplicateHandle {
                handle: translation.handle.clone(),
                locale: translation.locale.clone(),
            });
        }
    }
    Ok(())
}

async fn insert_translations<C>(
    db: &C,
    collection_id: Uuid,
    translations: &[CollectionOwnerTranslationInput],
) -> CommerceResult<()>
where
    C: sea_orm::ConnectionTrait,
{
    for translation in translations {
        collection_translation::ActiveModel {
            id: Set(generate_id()),
            collection_id: Set(collection_id),
            locale: Set(translation.locale.clone()),
            title: Set(translation.title.clone()),
            handle: Set(translation.handle.clone()),
            description: Set(translation.description.clone()),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

async fn load_translations<C>(
    db: &C,
    collection_id: Uuid,
) -> CommerceResult<Vec<collection_translation::Model>>
where
    C: sea_orm::ConnectionTrait,
{
    Ok(collection_translation::Entity::find()
        .filter(collection_translation::Column::CollectionId.eq(collection_id))
        .order_by_asc(collection_translation::Column::Locale)
        .all(db)
        .await?)
}

fn translations_semantically_equal(
    current: &[collection_translation::Model],
    next: &[CollectionOwnerTranslationInput],
) -> bool {
    current.len() == next.len()
        && current.iter().zip(next).all(|(current, next)| {
            current.locale == next.locale
                && current.title == next.title
                && current.handle == next.handle
                && current.description == next.description
        })
}

async fn publish_copy_change(
    event_bus: &TransactionalEventBus,
    txn: &sea_orm::DatabaseTransaction,
    tenant_id: Uuid,
    actor_user_id: Option<Uuid>,
    collection_id: Uuid,
    changed_locale: &str,
    resource_revision: &str,
    operation: &str,
) -> CommerceResult<Uuid> {
    event_bus
        .publish_in_tx_with_envelope_id(
            txn,
            tenant_id,
            actor_user_id,
            DomainEvent::TranslationTargetChanged {
                owner_slug: OWNER_SLUG.to_string(),
                resource_kind: RESOURCE_KIND.to_string(),
                resource_id: collection_id.to_string(),
                changed_locale: changed_locale.to_string(),
                resource_revision: resource_revision.to_string(),
                target_revision: resource_revision.to_string(),
                operation: operation.to_string(),
                correlation_id: generate_id().to_string(),
            },
        )
        .await
        .map_err(CommerceError::Core)
}

fn snapshot(
    model: collection::Model,
    translations: Vec<collection_translation::Model>,
) -> CollectionOwnerSnapshot {
    CollectionOwnerSnapshot {
        id: model.id,
        tenant_id: model.tenant_id,
        collection_type: model.collection_type,
        conditions: model.conditions,
        metadata: model.metadata,
        translations: translations
            .into_iter()
            .map(|translation| CollectionOwnerTranslationInput {
                locale: translation.locale,
                title: translation.title,
                handle: translation.handle,
                description: translation.description,
            })
            .collect(),
    }
}
