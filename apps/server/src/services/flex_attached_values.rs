use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    TransactionTrait,
};
use serde_json::Value;
use uuid::Uuid;

#[cfg(feature = "mod-product")]
use flex::PRODUCT_ENTITY_TYPE;
#[expect(unused_imports, reason = "Imports conditionally consumed depending on activated module feature flags")]
use flex::{
    AttachedEntityRef, FlexMappedErrorKind, ORDER_ENTITY_TYPE, TAXONOMY_CATEGORY_ENTITY_TYPE,
    TOPIC_ENTITY_TYPE, USER_ENTITY_TYPE, delete_attached_localized_values,
    delete_generic_attached_values, load_exact_locale_values, load_generic_attached_schema,
    load_generic_attached_shared_values, load_localized_values_by_locale,
    lock_attached_translation_schema_in_tx, map_flex_error, merge_donor_flex_metadata,
    persist_localized_values, persist_prepared_generic_attached_values,
    prepare_attached_values_create, prepare_attached_values_update,
    prepare_generic_attached_values_update, record_flex_attached_translation_deleted_in_tx,
    resolve_attached_payload, resolve_generic_attached_values, split_donor_metadata,
};
use rustok_core::field_schema::{CustomFieldsSchema, FlexError};
#[cfg(feature = "mod-product")]
use rustok_product_ports::{ProductAttachedValuesReadPort, ProductAttachedValuesWritePort};
#[cfg(feature = "mod-product")]
use std::sync::Arc;

use crate::error::{Error, Result as ServerResult};
use crate::services::order_field_service::OrderFieldService;
use crate::services::topic_field_service::TopicFieldService;
use crate::services::user_field_service::UserFieldService;

pub use flex::PreparedAttachedValuesWrite;

pub struct FlexAttachedValuesService;

impl FlexAttachedValuesService {
    pub async fn prepare_create(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        entity_type: &str,
        locale: &str,
        payload: Option<Value>,
    ) -> Result<PreparedAttachedValuesWrite, FlexError> {
        let schema = load_schema(db, tenant_id, entity_type).await?;
        prepare_attached_values_create(schema, payload, locale)
    }

    pub async fn prepare_update(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        locale: &str,
        existing_metadata: &Value,
        payload: Option<Value>,
    ) -> Result<PreparedAttachedValuesWrite, FlexError> {
        let schema = load_schema(db, tenant_id, entity_type).await?;
        prepare_attached_values_update(
            db,
            AttachedEntityRef {
                tenant_id,
                entity_type,
                entity_id,
            },
            schema,
            locale,
            existing_metadata,
            payload,
        )
        .await
    }

    pub async fn resolve_merged_payload(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        shared_metadata: &Value,
        preferred_locale: &str,
        tenant_default_locale: &str,
    ) -> Result<Option<Value>, FlexError> {
        let schema = load_schema(db, tenant_id, entity_type).await?;
        resolve_attached_payload(
            db,
            AttachedEntityRef {
                tenant_id,
                entity_type,
                entity_id,
            },
            schema,
            shared_metadata,
            preferred_locale,
            tenant_default_locale,
        )
        .await
    }
    /// Canonical exact-locale mutation for a registered generic donor.
    ///
    /// Lock order is Flex schema generation -> donor owner. The write is prepared from
    /// current shared/exact values inside that transaction, persisted only on semantic
    /// change, and then advances the donor aggregate revision in the same commit. This is
    /// the mutation law shared with attached Translation CAS.
    #[tracing::instrument(skip(db, payload), fields(tenant_id = %tenant_id, entity_type = %entity_type, entity_id = %entity_id, locale = %locale))]
    pub async fn update_registered_generic_values(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        locale: &str,
        payload: Option<Value>,
    ) -> ServerResult<()> {
        match entity_type {
            USER_ENTITY_TYPE => {
                let txn = db.begin().await?;
                let schema = load_schema(db, tenant_id, entity_type)
                    .await
                    .map_err(map_flex_host_error)?;
                let user = crate::models::_entities::users::Entity::find_by_id(entity_id)
                    .filter(crate::models::_entities::users::Column::TenantId.eq(tenant_id))
                    .one(&txn)
                    .await?
                    .ok_or(Error::NotFound)?;
                let entity = attached_ref(tenant_id, entity_type, entity_id);
                let prepared = prepare_attached_values_update(
                    &txn,
                    entity,
                    schema.clone(),
                    locale,
                    &user.metadata,
                    payload,
                )
                .await
                .map_err(map_flex_host_error)?;

                let new_metadata = match prepared.metadata.as_ref() {
                    Some(flex_meta) => {
                        merge_donor_flex_metadata(&schema, &user.metadata, flex_meta)
                    }
                    None => user.metadata.clone(),
                };

                let mut active: crate::models::_entities::users::ActiveModel = user.into();
                active.metadata = sea_orm::ActiveValue::Set(new_metadata);
                active.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
                active.update(&txn).await?;

                if let (Some(loc), Some(values)) = (
                    prepared.locale.as_deref(),
                    prepared.localized_values.as_ref(),
                ) {
                    persist_localized_values(&txn, tenant_id, entity_type, entity_id, loc, values)
                        .await
                        .map_err(map_flex_host_error)?;
                }
                txn.commit().await?;
                Ok(())
            }
            #[cfg(feature = "mod-order")]
            ORDER_ENTITY_TYPE => {
                let txn = db.begin().await?;
                let schema = load_schema(db, tenant_id, entity_type)
                    .await
                    .map_err(map_flex_host_error)?;
                let order = rustok_order::entities::order::Entity::find_by_id(entity_id)
                    .filter(rustok_order::entities::order::Column::TenantId.eq(tenant_id))
                    .one(&txn)
                    .await?
                    .ok_or(Error::NotFound)?;
                let entity = attached_ref(tenant_id, entity_type, entity_id);
                let prepared = prepare_attached_values_update(
                    &txn,
                    entity,
                    schema.clone(),
                    locale,
                    &order.metadata,
                    payload,
                )
                .await
                .map_err(map_flex_host_error)?;

                let new_metadata = match prepared.metadata.as_ref() {
                    Some(flex_meta) => {
                        merge_donor_flex_metadata(&schema, &order.metadata, flex_meta)
                    }
                    None => order.metadata.clone(),
                };

                let mut active: rustok_order::entities::order::ActiveModel = order.into();
                active.metadata = sea_orm::ActiveValue::Set(new_metadata);
                active.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
                active.update(&txn).await?;

                if let (Some(loc), Some(values)) = (
                    prepared.locale.as_deref(),
                    prepared.localized_values.as_ref(),
                ) {
                    persist_localized_values(&txn, tenant_id, entity_type, entity_id, loc, values)
                        .await
                        .map_err(map_flex_host_error)?;
                }
                txn.commit().await?;
                Ok(())
            }
            #[cfg(feature = "mod-forum")]
            TOPIC_ENTITY_TYPE => {
                let txn = db.begin().await?;
                let schema = load_schema(db, tenant_id, entity_type)
                    .await
                    .map_err(map_flex_host_error)?;
                let topic = rustok_forum::entities::forum_topic::Entity::find_by_id(entity_id)
                    .filter(rustok_forum::entities::forum_topic::Column::TenantId.eq(tenant_id))
                    .one(&txn)
                    .await?
                    .ok_or(Error::NotFound)?;
                let entity = attached_ref(tenant_id, entity_type, entity_id);
                let prepared = prepare_attached_values_update(
                    &txn,
                    entity,
                    schema.clone(),
                    locale,
                    &topic.metadata,
                    payload,
                )
                .await
                .map_err(map_flex_host_error)?;

                let new_metadata = match prepared.metadata.as_ref() {
                    Some(flex_meta) => {
                        merge_donor_flex_metadata(&schema, &topic.metadata, flex_meta)
                    }
                    None => topic.metadata.clone(),
                };

                let mut active: rustok_forum::entities::forum_topic::ActiveModel = topic.into();
                active.metadata = sea_orm::ActiveValue::Set(new_metadata);
                active.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
                active.update(&txn).await?;

                if let (Some(loc), Some(values)) = (
                    prepared.locale.as_deref(),
                    prepared.localized_values.as_ref(),
                ) {
                    persist_localized_values(&txn, tenant_id, entity_type, entity_id, loc, values)
                        .await
                        .map_err(map_flex_host_error)?;
                }
                txn.commit().await?;
                Ok(())
            }
            #[cfg(feature = "mod-taxonomy")]
            TAXONOMY_CATEGORY_ENTITY_TYPE => {
                let txn = db.begin().await?;
                let schema = lock_attached_translation_schema_in_tx(&txn, tenant_id, entity_type)
                    .await
                    .map_err(map_flex_host_error)?;
                let owner =
                    rustok_taxonomy::lock_category_owner_revision_in_tx(&txn, tenant_id, entity_id)
                        .await
                        .map_err(map_taxonomy_owner_error)?;
                let entity = attached_ref(tenant_id, entity_type, entity_id);
                let before_shared = load_generic_attached_shared_values(&txn, entity.clone())
                    .await
                    .map_err(map_flex_host_error)?;
                let prepared = prepare_generic_attached_values_update(
                    &txn,
                    entity.clone(),
                    schema.schema,
                    locale,
                    payload,
                )
                .await
                .map_err(map_flex_host_error)?;
                let before_localized = match prepared.locale.as_deref() {
                    Some(locale) if prepared.localized_values.is_some() => {
                        load_exact_locale_values(&txn, tenant_id, entity_type, entity_id, locale)
                            .await
                            .map_err(map_flex_host_error)?
                    }
                    _ => None,
                };
                let changed =
                    prepared_write_changed(&before_shared, before_localized.as_ref(), &prepared);
                if changed {
                    persist_prepared_generic_attached_values(&txn, entity, &prepared)
                        .await
                        .map_err(map_flex_host_error)?;
                    rustok_taxonomy::advance_category_owner_revision_in_tx(
                        &txn,
                        tenant_id,
                        entity_id,
                        owner.revision,
                    )
                    .await
                    .map_err(map_taxonomy_owner_error)?;
                }
                txn.commit().await?;
                Ok(())
            }
            other => Err(Error::BadRequest(format!(
                "generic Flex owner adapter is not registered for {other}"
            ))),
        }
    }

    pub async fn resolve_registered_generic_values(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        preferred_locale: &str,
        tenant_default_locale: &str,
    ) -> ServerResult<Option<Value>> {
        let schema = load_schema(db, tenant_id, entity_type)
            .await
            .map_err(map_flex_host_error)?;
        match entity_type {
            USER_ENTITY_TYPE => {
                let user = crate::models::_entities::users::Entity::find_by_id(entity_id)
                    .filter(crate::models::_entities::users::Column::TenantId.eq(tenant_id))
                    .one(db)
                    .await?
                    .ok_or(Error::NotFound)?;
                resolve_attached_payload(
                    db,
                    attached_ref(tenant_id, entity_type, entity_id),
                    schema,
                    &user.metadata,
                    preferred_locale,
                    tenant_default_locale,
                )
                .await
                .map_err(map_flex_host_error)
            }
            #[cfg(feature = "mod-order")]
            ORDER_ENTITY_TYPE => {
                let order = rustok_order::entities::order::Entity::find_by_id(entity_id)
                    .filter(rustok_order::entities::order::Column::TenantId.eq(tenant_id))
                    .one(db)
                    .await?
                    .ok_or(Error::NotFound)?;
                resolve_attached_payload(
                    db,
                    attached_ref(tenant_id, entity_type, entity_id),
                    schema,
                    &order.metadata,
                    preferred_locale,
                    tenant_default_locale,
                )
                .await
                .map_err(map_flex_host_error)
            }
            #[cfg(feature = "mod-forum")]
            TOPIC_ENTITY_TYPE => {
                let topic = rustok_forum::entities::forum_topic::Entity::find_by_id(entity_id)
                    .filter(rustok_forum::entities::forum_topic::Column::TenantId.eq(tenant_id))
                    .one(db)
                    .await?
                    .ok_or(Error::NotFound)?;
                resolve_attached_payload(
                    db,
                    attached_ref(tenant_id, entity_type, entity_id),
                    schema,
                    &topic.metadata,
                    preferred_locale,
                    tenant_default_locale,
                )
                .await
                .map_err(map_flex_host_error)
            }
            #[cfg(feature = "mod-taxonomy")]
            TAXONOMY_CATEGORY_ENTITY_TYPE => {
                ensure_registered_owner_exists(db, tenant_id, entity_type, entity_id).await?;
                resolve_generic_attached_values(
                    db,
                    attached_ref(tenant_id, entity_type, entity_id),
                    schema,
                    preferred_locale,
                    tenant_default_locale,
                )
                .await
                .map_err(map_flex_host_error)
            }
            other => Err(Error::BadRequest(format!(
                "generic Flex owner adapter is not registered for {other}"
            ))),
        }
    }

    #[tracing::instrument(skip(db), fields(tenant_id = %tenant_id, entity_type = %entity_type, entity_id = %entity_id))]
    pub async fn delete_registered_generic_values(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
    ) -> ServerResult<()> {
        match entity_type {
            USER_ENTITY_TYPE => {
                let txn = db.begin().await?;
                let user = crate::models::_entities::users::Entity::find_by_id(entity_id)
                    .filter(crate::models::_entities::users::Column::TenantId.eq(tenant_id))
                    .one(&txn)
                    .await?
                    .ok_or(Error::NotFound)?;
                let schema = load_schema(db, tenant_id, entity_type)
                    .await
                    .map_err(map_flex_host_error)?;
                let (reserved, _) = split_donor_metadata(&schema, &user.metadata);
                let mut active: crate::models::_entities::users::ActiveModel = user.into();
                active.metadata = sea_orm::ActiveValue::Set(serde_json::Value::Object(reserved));
                active.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
                active.update(&txn).await?;

                delete_attached_localized_values(&txn, tenant_id, entity_type, entity_id)
                    .await
                    .map_err(map_flex_host_error)?;
                txn.commit().await?;
                Ok(())
            }
            #[cfg(feature = "mod-order")]
            ORDER_ENTITY_TYPE => {
                let txn = db.begin().await?;
                let order = rustok_order::entities::order::Entity::find_by_id(entity_id)
                    .filter(rustok_order::entities::order::Column::TenantId.eq(tenant_id))
                    .one(&txn)
                    .await?
                    .ok_or(Error::NotFound)?;
                let schema = load_schema(db, tenant_id, entity_type)
                    .await
                    .map_err(map_flex_host_error)?;
                let (reserved, _) = split_donor_metadata(&schema, &order.metadata);
                let mut active: rustok_order::entities::order::ActiveModel = order.into();
                active.metadata = sea_orm::ActiveValue::Set(serde_json::Value::Object(reserved));
                active.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
                active.update(&txn).await?;

                delete_attached_localized_values(&txn, tenant_id, entity_type, entity_id)
                    .await
                    .map_err(map_flex_host_error)?;
                txn.commit().await?;
                Ok(())
            }
            #[cfg(feature = "mod-forum")]
            TOPIC_ENTITY_TYPE => {
                let txn = db.begin().await?;
                let topic = rustok_forum::entities::forum_topic::Entity::find_by_id(entity_id)
                    .filter(rustok_forum::entities::forum_topic::Column::TenantId.eq(tenant_id))
                    .one(&txn)
                    .await?
                    .ok_or(Error::NotFound)?;
                let schema = load_schema(db, tenant_id, entity_type)
                    .await
                    .map_err(map_flex_host_error)?;
                let (reserved, _) = split_donor_metadata(&schema, &topic.metadata);
                let mut active: rustok_forum::entities::forum_topic::ActiveModel = topic.into();
                active.metadata = sea_orm::ActiveValue::Set(serde_json::Value::Object(reserved));
                active.updated_at = sea_orm::ActiveValue::Set(chrono::Utc::now().into());
                active.update(&txn).await?;

                delete_attached_localized_values(&txn, tenant_id, entity_type, entity_id)
                    .await
                    .map_err(map_flex_host_error)?;
                txn.commit().await?;
                Ok(())
            }
            #[cfg(feature = "mod-taxonomy")]
            TAXONOMY_CATEGORY_ENTITY_TYPE => {
                let txn = db.begin().await?;
                let owner =
                    rustok_taxonomy::lock_category_owner_revision_in_tx(&txn, tenant_id, entity_id)
                        .await
                        .map_err(map_taxonomy_owner_error)?;
                let entity = attached_ref(tenant_id, entity_type, entity_id);
                let before_shared = load_generic_attached_shared_values(&txn, entity.clone())
                    .await
                    .map_err(map_flex_host_error)?;
                let before_localized =
                    load_localized_values_by_locale(&txn, tenant_id, entity_type, entity_id)
                        .await
                        .map_err(map_flex_host_error)?;
                let changed = !normalized_object(Some(&before_shared))
                    .as_object()
                    .is_some_and(|map| map.is_empty())
                    || !before_localized.is_empty();

                // Always clean capability rows, including any legacy-invalid locale rows
                // hidden by normalized read helpers. Only owner-visible semantic state bumps
                // the Category revision.
                delete_generic_attached_values(&txn, entity)
                    .await
                    .map_err(map_flex_host_error)?;
                if changed {
                    rustok_taxonomy::advance_category_owner_revision_in_tx(
                        &txn,
                        tenant_id,
                        entity_id,
                        owner.revision,
                    )
                    .await
                    .map_err(map_taxonomy_owner_error)?;
                }
                txn.commit().await?;
                Ok(())
            }
            other => Err(Error::BadRequest(format!(
                "generic Flex owner adapter is not registered for {other}"
            ))),
        }
    }

    pub async fn persist_localized_values<C>(
        db: &C,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        locale: &str,
        values: &Value,
    ) -> Result<(), FlexError>
    where
        C: ConnectionTrait,
    {
        persist_localized_values(db, tenant_id, entity_type, entity_id, locale, values).await
    }

    #[tracing::instrument(skip(db), fields(tenant_id = %tenant_id, entity_type = %entity_type, entity_id = %entity_id))]
    pub async fn delete_localized_values<C>(
        db: &C,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<u64, FlexError>
    where
        C: ConnectionTrait,
    {
        delete_attached_localized_values(db, tenant_id, entity_type, entity_id).await
    }
}

#[derive(Clone)]
pub struct FlexAttachedValuesGraphqlAdapter {
    db: DatabaseConnection,
    #[cfg(feature = "mod-product")]
    product_read: Option<Arc<dyn ProductAttachedValuesReadPort>>,
    #[cfg(feature = "mod-product")]
    product_write: Option<(
        rustok_product::ProductCatalogCommandProfile,
        Arc<dyn ProductAttachedValuesWritePort>,
    )>,
}

impl FlexAttachedValuesGraphqlAdapter {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            #[cfg(feature = "mod-product")]
            product_read: None,
            #[cfg(feature = "mod-product")]
            product_write: None,
        }
    }

    #[cfg(feature = "mod-product")]
    pub fn with_product_ports(
        mut self,
        read: Option<Arc<dyn ProductAttachedValuesReadPort>>,
        write: Option<(
            rustok_product::ProductCatalogCommandProfile,
            Arc<dyn ProductAttachedValuesWritePort>,
        )>,
    ) -> Self {
        self.product_read = read;
        self.product_write = write;
        self
    }
}

#[async_trait::async_trait]
impl flex::graphql::AttachedValuesGraphqlPort for FlexAttachedValuesGraphqlAdapter {
    async fn resolve_values(
        &self,
        context: rustok_api::PortContext,
        entity_type: &str,
        entity_id: Uuid,
        tenant_default_locale: &str,
    ) -> Result<Option<Value>, FlexError> {
        #[cfg(feature = "mod-product")]
        if entity_type == PRODUCT_ENTITY_TYPE {
            return self
                .product_read
                .as_ref()
                .ok_or_else(product_values_unavailable)?
                .read_attached_values(context, entity_id, tenant_default_locale)
                .await
                .map_err(|error| map_product_attached_error(error, entity_id));
        }
        let tenant_id = attached_values_tenant(&context)?;
        FlexAttachedValuesService::resolve_registered_generic_values(
            &self.db,
            tenant_id,
            entity_type,
            entity_id,
            &context.locale,
            tenant_default_locale,
        )
        .await
        .map_err(|error| map_host_error_to_flex(error, entity_id))
    }

    async fn update_values(
        &self,
        context: rustok_api::PortContext,
        entity_type: &str,
        entity_id: Uuid,
        payload: Option<Value>,
    ) -> Result<Option<Value>, flex::graphql::AttachedValuesCommandError> {
        #[cfg(feature = "mod-product")]
        if entity_type == PRODUCT_ENTITY_TYPE {
            let (profile, port) = self
                .product_write
                .as_ref()
                .ok_or_else(product_values_unavailable)?;
            let context =
                product_write_context(context, *profile, entity_id, "update_attached_values")?;
            return port
                .update_attached_values(context, entity_id, payload)
                .await
                .map_err(Into::into);
        }
        let tenant_id = attached_values_tenant(&context)?;
        let locale = &context.locale;
        FlexAttachedValuesService::update_registered_generic_values(
            &self.db,
            tenant_id,
            entity_type,
            entity_id,
            locale,
            payload,
        )
        .await
        .map_err(|error| map_host_error_to_flex(error, entity_id))?;
        FlexAttachedValuesService::resolve_registered_generic_values(
            &self.db,
            tenant_id,
            entity_type,
            entity_id,
            locale,
            locale,
        )
        .await
        .map_err(|error| map_host_error_to_flex(error, entity_id).into())
    }

    async fn delete_values(
        &self,
        context: rustok_api::PortContext,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<(), flex::graphql::AttachedValuesCommandError> {
        #[cfg(feature = "mod-product")]
        if entity_type == PRODUCT_ENTITY_TYPE {
            let (profile, port) = self
                .product_write
                .as_ref()
                .ok_or_else(product_values_unavailable)?;
            let context =
                product_write_context(context, *profile, entity_id, "delete_attached_values")?;
            return port
                .delete_attached_values(context, entity_id)
                .await
                .map_err(Into::into);
        }
        FlexAttachedValuesService::delete_registered_generic_values(
            &self.db,
            attached_values_tenant(&context)?,
            entity_type,
            entity_id,
        )
        .await
        .map_err(|error| map_host_error_to_flex(error, entity_id).into())
    }
}

fn attached_values_tenant(context: &rustok_api::PortContext) -> Result<Uuid, FlexError> {
    Uuid::parse_str(&context.tenant_id)
        .map_err(|_| FlexError::Database("Invalid attached values tenant context".into()))
}

#[cfg(feature = "mod-product")]
fn product_write_context(
    mut context: rustok_api::PortContext,
    profile: rustok_product::ProductCatalogCommandProfile,
    product_id: Uuid,
    operation: &str,
) -> Result<rustok_api::PortContext, flex::graphql::AttachedValuesCommandError> {
    use rustok_api::PortError;
    let tenant_id = attached_values_tenant(&context)?;
    let actor_id = Uuid::parse_str(&context.actor.id)
        .ok()
        .filter(|id| !id.is_nil())
        .ok_or_else(|| {
            PortError::validation("product.actor_id_invalid", "Product actor is invalid")
        })?;
    let key = match context.idempotency_key.take() {
        Some(key) if (1..=191).contains(&key.trim().len()) => key,
        Some(_) => {
            return Err(PortError::validation(
                "product.idempotency_invalid",
                "idempotencyKey must contain 1 to 191 bytes",
            )
            .into());
        }
        None if profile == rustok_product::ProductCatalogCommandProfile::EmbeddedNative => {
            Uuid::new_v4().to_string()
        }
        None => {
            return Err(PortError::validation(
                "port.idempotency_key_required",
                "idempotencyKey is required for remote Product writes",
            )
            .into());
        }
    };
    let namespace = match context.actor.kind {
        rustok_api::PortActorKind::User => "flex-product-user",
        rustok_api::PortActorKind::Service => "flex-product-service",
        rustok_api::PortActorKind::System => {
            return Err(PortError::forbidden(
                "product.actor_invalid",
                "Product attached values require an authenticated actor",
            )
            .into());
        }
    };
    Ok(
        context.with_idempotency_key(rustok_outbox::idempotency::scoped_caller_idempotency_key(
            namespace,
            tenant_id,
            actor_id,
            operation,
            Some(product_id),
            &key,
        )),
    )
}

#[cfg(feature = "mod-product")]
fn product_values_unavailable() -> FlexError {
    FlexError::Database(
        "Product attached values are unavailable in the selected deployment".to_owned(),
    )
}

#[cfg(feature = "mod-product")]
fn map_product_attached_error(error: rustok_api::PortError, product_id: Uuid) -> FlexError {
    match error.kind {
        rustok_api::PortErrorKind::NotFound => FlexError::NotFound(product_id),
        rustok_api::PortErrorKind::Validation => FlexError::UnknownEntityType(error.message),
        _ => FlexError::Database(error.message),
    }
}

#[cfg(all(test, feature = "mod-product"))]
mod product_attached_values_tests {
    use flex::graphql::{AttachedValuesCommandError, AttachedValuesGraphqlPort};

    use super::*;

    #[test]
    fn remote_product_keys_are_required_and_scoped_to_authenticated_callers() {
        use rustok_api::{PortActor, PortContext};
        use rustok_product::ProductCatalogCommandProfile::{EmbeddedNative, External};
        let tenant = Uuid::new_v4();
        let product = Uuid::new_v4();
        let actor = Uuid::new_v4().to_string();
        let context =
            PortContext::new(tenant.to_string(), PortActor::user(&actor), "en", "request");
        let scope = |context, profile| {
            product_write_context(context, profile, product, "update_attached_values")
        };
        assert!(
            matches!(scope(context.clone(), External), Err(AttachedValuesCommandError::Owner(error)) if error.code == "port.idempotency_key_required")
        );
        assert!(
            scope(context.clone(), EmbeddedNative)
                .unwrap()
                .idempotency_key
                .is_some()
        );
        let scoped = scope(context.clone().with_idempotency_key("client-key"), External).unwrap();
        let mut retry = context.with_idempotency_key(" client-key ");
        retry.correlation_id = "second-request".into();
        assert_eq!(
            scope(retry.clone(), External).unwrap().idempotency_key,
            scoped.idempotency_key
        );
        retry.actor = PortActor::service(actor);
        assert_ne!(
            scope(retry, External).unwrap().idempotency_key,
            scoped.idempotency_key
        );
    }

    #[tokio::test]
    async fn missing_product_ports_fail_before_touching_local_storage() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let adapter = FlexAttachedValuesGraphqlAdapter::new(db);
        let tenant = Uuid::new_v4();
        let id = Uuid::new_v4();
        let context = rustok_api::PortContext::new(
            tenant.to_string(),
            rustok_api::PortActor::system(),
            "en",
            "test-attached-values",
        )
        .with_deadline(std::time::Duration::from_secs(5));

        let read_error = adapter
            .resolve_values(context.clone(), PRODUCT_ENTITY_TYPE, id, "en")
            .await
            .unwrap_err();
        assert!(matches!(read_error, FlexError::Database(_)));
        for error in [
            adapter
                .update_values(context.clone(), PRODUCT_ENTITY_TYPE, id, None)
                .await
                .unwrap_err(),
            adapter
                .delete_values(context, PRODUCT_ENTITY_TYPE, id)
                .await
                .unwrap_err(),
        ] {
            assert!(matches!(
                error,
                AttachedValuesCommandError::Flex(FlexError::Database(message))
                    if message == "Product attached values are unavailable in the selected deployment"
            ));
        }
    }
}

/// Host implementation of the Taxonomy-owned Category delete cleanup boundary.
///
/// Taxonomy controls the owner transaction and calls this port before deleting the Category row;
/// the host delegates only capability-owned attached rows to Flex.
#[cfg(feature = "mod-taxonomy")]
pub struct FlexTaxonomyCategoryDeleteCleanup;

#[cfg(feature = "mod-taxonomy")]
#[async_trait::async_trait]
impl rustok_taxonomy::TaxonomyCategoryDeleteCleanupPort for FlexTaxonomyCategoryDeleteCleanup {
    async fn cleanup_in_tx(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> rustok_taxonomy::TaxonomyResult<()> {
        delete_generic_attached_values(
            txn,
            attached_ref(tenant_id, TAXONOMY_CATEGORY_ENTITY_TYPE, category_id),
        )
        .await
        .map_err(|error| {
            rustok_taxonomy::TaxonomyError::Database(sea_orm::DbErr::Custom(error.to_string()))
        })?;
        record_flex_attached_translation_deleted_in_tx(
            txn,
            tenant_id,
            TAXONOMY_CATEGORY_ENTITY_TYPE,
            category_id,
        )
        .await
        .map_err(|error| {
            rustok_taxonomy::TaxonomyError::Database(sea_orm::DbErr::Custom(error.to_string()))
        })?;
        Ok(())
    }
}

fn attached_ref<'a>(
    tenant_id: Uuid,
    entity_type: &'a str,
    entity_id: Uuid,
) -> AttachedEntityRef<'a> {
    AttachedEntityRef {
        tenant_id,
        entity_type,
        entity_id,
    }
}

#[cfg(feature = "mod-taxonomy")]
fn prepared_write_changed(
    before_shared: &Value,
    before_localized: Option<&Value>,
    prepared: &PreparedAttachedValuesWrite,
) -> bool {
    let desired_shared = normalized_object(prepared.metadata.as_ref());
    if normalized_object(Some(before_shared)) != desired_shared {
        return true;
    }
    match prepared.localized_values.as_ref() {
        Some(desired) => normalized_object(before_localized) != normalized_object(Some(desired)),
        None => false,
    }
}

#[cfg(feature = "mod-taxonomy")]
fn normalized_object(value: Option<&Value>) -> Value {
    Value::Object(
        value
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default(),
    )
}

async fn ensure_registered_owner_exists<C>(
    db: &C,
    tenant_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
) -> ServerResult<()>
where
    C: ConnectionTrait,
{
    match entity_type {
        USER_ENTITY_TYPE => {
            let exists = crate::models::_entities::users::Entity::find_by_id(entity_id)
                .filter(crate::models::_entities::users::Column::TenantId.eq(tenant_id))
                .one(db)
                .await?
                .is_some();
            if exists { Ok(()) } else { Err(Error::NotFound) }
        }
        #[cfg(feature = "mod-order")]
        ORDER_ENTITY_TYPE => {
            let exists = rustok_order::entities::order::Entity::find_by_id(entity_id)
                .filter(rustok_order::entities::order::Column::TenantId.eq(tenant_id))
                .one(db)
                .await?
                .is_some();
            if exists { Ok(()) } else { Err(Error::NotFound) }
        }
        #[cfg(feature = "mod-forum")]
        TOPIC_ENTITY_TYPE => {
            let exists = rustok_forum::entities::forum_topic::Entity::find_by_id(entity_id)
                .filter(rustok_forum::entities::forum_topic::Column::TenantId.eq(tenant_id))
                .one(db)
                .await?
                .is_some();
            if exists { Ok(()) } else { Err(Error::NotFound) }
        }
        #[cfg(feature = "mod-taxonomy")]
        TAXONOMY_CATEGORY_ENTITY_TYPE => {
            let exists = rustok_taxonomy::taxonomy_term_identity_exists(
                db,
                tenant_id,
                rustok_taxonomy::TaxonomyTermKind::Category,
                entity_id,
            )
            .await
            .map_err(|error| {
                Error::Message(format!(
                    "Taxonomy Flex owner identity lookup failed: {error}"
                ))
            })?;
            if exists { Ok(()) } else { Err(Error::NotFound) }
        }
        other => Err(Error::BadRequest(format!(
            "generic Flex owner adapter is not registered for {other}"
        ))),
    }
}

#[cfg(feature = "mod-taxonomy")]
fn map_taxonomy_owner_error(error: rustok_taxonomy::TaxonomyError) -> Error {
    match error {
        rustok_taxonomy::TaxonomyError::TermNotFound(_) => Error::NotFound,
        rustok_taxonomy::TaxonomyError::Validation(message) => Error::BadRequest(message),
        rustok_taxonomy::TaxonomyError::Database(error) => Error::Database(error),
        error => Error::Message(format!("Taxonomy Flex owner mutation failed: {error}")),
    }
}

fn map_flex_host_error(error: FlexError) -> Error {
    let mapped = map_flex_error(error);
    match mapped.kind {
        FlexMappedErrorKind::Internal => Error::Message(mapped.message),
        FlexMappedErrorKind::NotFound => Error::NotFound,
        FlexMappedErrorKind::BadUserInput => Error::BadRequest(mapped.message),
    }
}

fn map_host_error_to_flex(error: Error, entity_id: Uuid) -> FlexError {
    match error {
        Error::NotFound => FlexError::NotFound(entity_id),
        Error::BadRequest(message) | Error::Validation(message) => {
            FlexError::UnknownEntityType(message)
        }
        error => FlexError::Database(error.to_string()),
    }
}

async fn load_schema(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    entity_type: &str,
) -> Result<CustomFieldsSchema, FlexError> {
    match entity_type {
        USER_ENTITY_TYPE => UserFieldService::get_schema(db, tenant_id).await,
        ORDER_ENTITY_TYPE => OrderFieldService::get_schema(db, tenant_id).await,
        TOPIC_ENTITY_TYPE => TopicFieldService::get_schema(db, tenant_id).await,
        #[cfg(feature = "mod-taxonomy")]
        TAXONOMY_CATEGORY_ENTITY_TYPE => {
            load_generic_attached_schema(db, tenant_id, entity_type).await
        }
        other => Err(FlexError::UnknownEntityType(other.to_string())),
    }
}
