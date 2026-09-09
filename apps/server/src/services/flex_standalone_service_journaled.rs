//! Journaled canonical host adapter for standalone Flex schema ownership.
//!
//! The historical adapter remains a private delegate for read paths and standalone-entry
//! CRUD. Schema create/update/delete are owned here so parent/translation persistence and
//! Translation change evidence share one transaction and one parent serialization law.

use async_trait::async_trait;
use flex::{
    FlexSchemaTranslationChangeLifecycle, FlexSchemaTranslationError,
    flex_schema_translation_deleted_revision, record_flex_schema_translation_change_in_tx,
    validate_create_schema_command, validate_optional_standalone_uuid, validate_standalone_uuid,
    validate_update_schema_command,
};
use rustok_api::{
    PLATFORM_FALLBACK_LOCALE, build_locale_candidates, locale_tags_match, normalize_locale_tag,
};
use rustok_core::field_schema::FlexError;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection,
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};
use uuid::Uuid;

use crate::models::{flex_schema_translations, flex_schemas, tenants};
use crate::services::flex_schema_translation_owner::resource_revision;
use crate::services::flex_standalone_service_legacy;

pub struct FlexStandaloneSeaOrmService {
    db: DatabaseConnection,
    legacy: flex_standalone_service_legacy::FlexStandaloneSeaOrmService,
}

impl FlexStandaloneSeaOrmService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            legacy: flex_standalone_service_legacy::FlexStandaloneSeaOrmService::new(db.clone()),
            db,
        }
    }
}

#[async_trait]
impl flex::FlexStandaloneService for FlexStandaloneSeaOrmService {
    async fn list_schemas(&self, tenant_id: Uuid) -> Result<Vec<flex::FlexSchemaView>, FlexError> {
        flex::FlexStandaloneService::list_schemas(&self.legacy, tenant_id).await
    }

    async fn find_schema(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
    ) -> Result<Option<flex::FlexSchemaView>, FlexError> {
        flex::FlexStandaloneService::find_schema(&self.legacy, tenant_id, schema_id).await
    }

    async fn create_schema(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: flex::CreateFlexSchemaCommand,
    ) -> Result<flex::FlexSchemaView, FlexError> {
        validate_standalone_uuid(tenant_id, "tenant_id")?;
        validate_optional_standalone_uuid(actor_id, "actor_id")?;
        validate_create_schema_command(&input)?;

        let txn = self
            .db
            .begin()
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
        let locale = tenant_default_locale_on(&txn, tenant_id).await?;
        let schema_id = rustok_core::generate_id();
        let root_event_id = rustok_core::generate_id();
        let row = flex_schemas::ActiveModel {
            id: Set(schema_id),
            tenant_id: Set(tenant_id),
            slug: Set(input.slug),
            fields_config: Set(flex::serialize_standalone_fields_config(
                input.fields_config,
            )?),
            settings: Set(input.settings.unwrap_or_else(|| serde_json::json!({}))),
            is_active: Set(input.is_active.unwrap_or(true)),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&txn)
        .await
        .map_err(|error| FlexError::Database(error.to_string()))?;
        let translation = upsert_schema_translation_on(
            &txn,
            row.id,
            &locale,
            &row.slug,
            Some(input.name),
            input.description,
        )
        .await?;
        let translations = load_schema_translations_on(&txn, row.id).await?;
        let revision = resource_revision(&row, &translations);
        record_flex_schema_translation_change_in_tx(
            &txn,
            root_event_id,
            tenant_id,
            row.id,
            &revision,
            FlexSchemaTranslationChangeLifecycle::from_is_active(row.is_active),
        )
        .await
        .map_err(journal_error)?;
        txn.commit()
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;

        Ok(flex::standalone_schema_view_from_source(
            &row,
            Some(&translation),
        ))
    }

    async fn update_schema(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        schema_id: Uuid,
        input: flex::UpdateFlexSchemaCommand,
    ) -> Result<flex::FlexSchemaView, FlexError> {
        validate_standalone_uuid(tenant_id, "tenant_id")?;
        validate_optional_standalone_uuid(actor_id, "actor_id")?;
        validate_standalone_uuid(schema_id, "schema_id")?;
        validate_update_schema_command(&input)?;

        let txn = self
            .db
            .begin()
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
        let locale = tenant_default_locale_on(&txn, tenant_id).await?;
        let row = flex_schemas::Entity::find_by_id(schema_id)
            .filter(flex_schemas::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(&txn)
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?
            .ok_or(FlexError::NotFound(schema_id))?;
        let translations_before = load_schema_translations_on(&txn, schema_id).await?;
        let before_revision = resource_revision(&row, &translations_before);
        let mut model: flex_schemas::ActiveModel = row.into();

        if let Some(fields_config) = input.fields_config {
            model.fields_config = Set(flex::serialize_standalone_fields_config(fields_config)?);
        }
        if let Some(settings) = input.settings {
            model.settings = Set(settings);
        }
        if let Some(is_active) = input.is_active {
            model.is_active = Set(is_active);
        }
        let updated = model
            .update(&txn)
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;

        if input.name.is_some() || input.description.is_some() {
            upsert_schema_translation_on(
                &txn,
                updated.id,
                &locale,
                &updated.slug,
                input.name,
                input.description,
            )
            .await?;
        }
        let translations_after = load_schema_translations_on(&txn, updated.id).await?;
        let after_revision = resource_revision(&updated, &translations_after);
        if before_revision != after_revision {
            record_flex_schema_translation_change_in_tx(
                &txn,
                rustok_core::generate_id(),
                tenant_id,
                updated.id,
                &after_revision,
                FlexSchemaTranslationChangeLifecycle::from_is_active(updated.is_active),
            )
            .await
            .map_err(journal_error)?;
        }

        let translation = select_schema_translation(&translations_after, &locale).cloned();
        txn.commit()
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
        Ok(flex::standalone_schema_view_from_source(
            &updated,
            translation.as_ref(),
        ))
    }

    async fn delete_schema(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        schema_id: Uuid,
    ) -> Result<(), FlexError> {
        validate_standalone_uuid(tenant_id, "tenant_id")?;
        validate_optional_standalone_uuid(actor_id, "actor_id")?;
        validate_standalone_uuid(schema_id, "schema_id")?;

        let txn = self
            .db
            .begin()
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
        flex_schemas::Entity::find_by_id(schema_id)
            .filter(flex_schemas::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(&txn)
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?
            .ok_or(FlexError::NotFound(schema_id))?;

        let root_event_id = rustok_core::generate_id();
        flex_schemas::Entity::delete_by_id(schema_id)
            .exec(&txn)
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
        let revision = flex_schema_translation_deleted_revision(root_event_id, schema_id);
        record_flex_schema_translation_change_in_tx(
            &txn,
            root_event_id,
            tenant_id,
            schema_id,
            &revision,
            FlexSchemaTranslationChangeLifecycle::Deleted,
        )
        .await
        .map_err(journal_error)?;
        txn.commit()
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
        Ok(())
    }

    async fn list_entries(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
    ) -> Result<Vec<flex::FlexEntryView>, FlexError> {
        flex::FlexStandaloneService::list_entries(&self.legacy, tenant_id, schema_id).await
    }

    async fn find_entry(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
        entry_id: Uuid,
    ) -> Result<Option<flex::FlexEntryView>, FlexError> {
        flex::FlexStandaloneService::find_entry(&self.legacy, tenant_id, schema_id, entry_id).await
    }

    async fn create_entry(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: flex::CreateFlexEntryCommand,
    ) -> Result<flex::FlexEntryView, FlexError> {
        flex::FlexStandaloneService::create_entry(&self.legacy, tenant_id, actor_id, input).await
    }

    async fn update_entry(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        schema_id: Uuid,
        entry_id: Uuid,
        input: flex::UpdateFlexEntryCommand,
    ) -> Result<flex::FlexEntryView, FlexError> {
        flex::FlexStandaloneService::update_entry(
            &self.legacy,
            tenant_id,
            actor_id,
            schema_id,
            entry_id,
            input,
        )
        .await
    }

    async fn delete_entry(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        schema_id: Uuid,
        entry_id: Uuid,
    ) -> Result<(), FlexError> {
        flex::FlexStandaloneService::delete_entry(
            &self.legacy,
            tenant_id,
            actor_id,
            schema_id,
            entry_id,
        )
        .await
    }
}

async fn tenant_default_locale_on<C>(db: &C, tenant_id: Uuid) -> Result<String, FlexError>
where
    C: ConnectionTrait,
{
    let tenant = <tenants::Entity as EntityTrait>::find_by_id(tenant_id)
        .one(db)
        .await
        .map_err(|error| FlexError::Database(error.to_string()))?
        .ok_or_else(|| {
            FlexError::Database(format!(
                "tenant {tenant_id} is missing while resolving Flex locale"
            ))
        })?;
    normalize_locale_tag(&tenant.default_locale)
        .ok_or(FlexError::InvalidLocale(tenant.default_locale))
}

async fn load_schema_translations_on<C>(
    db: &C,
    schema_id: Uuid,
) -> Result<Vec<flex_schema_translations::Model>, FlexError>
where
    C: ConnectionTrait,
{
    flex_schema_translations::Entity::find()
        .filter(flex_schema_translations::Column::SchemaId.eq(schema_id))
        .order_by_asc(flex_schema_translations::Column::Locale)
        .all(db)
        .await
        .map_err(|error| FlexError::Database(error.to_string()))
}

async fn upsert_schema_translation_on<C>(
    db: &C,
    schema_id: Uuid,
    locale: &str,
    slug_fallback: &str,
    name: Option<String>,
    description: Option<String>,
) -> Result<flex_schema_translations::Model, FlexError>
where
    C: ConnectionTrait,
{
    let existing = flex_schema_translations::Entity::find()
        .filter(flex_schema_translations::Column::SchemaId.eq(schema_id))
        .filter(flex_schema_translations::Column::Locale.eq(locale))
        .one(db)
        .await
        .map_err(|error| FlexError::Database(error.to_string()))?;
    match existing {
        Some(row) => {
            let mut model: flex_schema_translations::ActiveModel = row.into();
            if let Some(name) = name {
                model.name = Set(name);
            }
            if let Some(description) = description {
                model.description = Set(Some(description));
            }
            model
                .update(db)
                .await
                .map_err(|error| FlexError::Database(error.to_string()))
        }
        None => flex_schema_translations::ActiveModel {
            schema_id: Set(schema_id),
            locale: Set(locale.to_string()),
            name: Set(name.unwrap_or_else(|| slug_fallback.to_string())),
            description: Set(description),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(db)
        .await
        .map_err(|error| FlexError::Database(error.to_string())),
    }
}

fn select_schema_translation<'a>(
    translations: &'a [flex_schema_translations::Model],
    preferred_locale: &str,
) -> Option<&'a flex_schema_translations::Model> {
    let candidates = build_locale_candidates(
        [Some(preferred_locale), Some(PLATFORM_FALLBACK_LOCALE)],
        true,
    );
    for candidate in candidates {
        if let Some(row) = translations
            .iter()
            .find(|translation| locale_tags_match(&translation.locale, &candidate))
        {
            return Some(row);
        }
    }
    translations.first()
}

fn journal_error(error: FlexSchemaTranslationError) -> FlexError {
    FlexError::Database(format!(
        "Flex schema translation change journal failed: {error}"
    ))
}
