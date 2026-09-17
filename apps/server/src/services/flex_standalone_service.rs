//! Canonical SeaORM-backed adapter implementation of `flex::FlexStandaloneService`.
//!
//! Owns transactional parent/localized persistence and Translation change evidence in the same
//! database transaction and serialization law.

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use flex::{
    FlexSchemaTranslationChangeLifecycle, FlexSchemaTranslationError,
    flex_schema_translation_deleted_revision, record_flex_schema_translation_change_in_tx,
    validate_create_entry_command, validate_create_schema_command,
    validate_optional_standalone_uuid, validate_standalone_uuid, validate_update_entry_command,
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
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::models::{
    flex_entries, flex_entry_localized_values, flex_schema_translations, flex_schemas, tenants,
};
use crate::services::flex_schema_translation_owner::resource_revision;

pub struct FlexStandaloneSeaOrmService {
    db: DatabaseConnection,
}

struct PreparedStandaloneEntryWrite {
    shared_data: JsonValue,
    localized_data: Option<JsonValue>,
    localized_keys: HashSet<String>,
}

impl FlexStandaloneSeaOrmService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn get_schema_or_not_found(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
    ) -> Result<flex_schemas::Model, FlexError> {
        flex_schemas::Entity::find_by_id(schema_id)
            .filter(flex_schemas::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?
            .ok_or(FlexError::NotFound(schema_id))
    }

    async fn tenant_default_locale(&self, tenant_id: Uuid) -> Result<String, FlexError> {
        tenant_default_locale_on(&self.db, tenant_id).await
    }

    pub fn select_schema_translation<'a>(
        translations: &'a [flex_schema_translations::Model],
        preferred_locale: &str,
    ) -> Option<&'a flex_schema_translations::Model> {
        select_schema_translation(translations, preferred_locale)
    }

    async fn load_schema_translation_map(
        &self,
        schema_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Vec<flex_schema_translations::Model>>, FlexError> {
        if schema_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = flex_schema_translations::Entity::find()
            .filter(flex_schema_translations::Column::SchemaId.is_in(schema_ids.iter().copied()))
            .all(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        let mut by_schema_id: HashMap<Uuid, Vec<flex_schema_translations::Model>> = HashMap::new();
        for row in rows {
            by_schema_id.entry(row.schema_id).or_default().push(row);
        }

        Ok(by_schema_id)
    }

    pub fn select_entry_localization<'a>(
        items: &'a [flex_entry_localized_values::Model],
        preferred_locale: &str,
    ) -> Option<&'a flex_entry_localized_values::Model> {
        let candidates = build_locale_candidates(
            [Some(preferred_locale), Some(PLATFORM_FALLBACK_LOCALE)],
            true,
        );

        for candidate in candidates {
            if let Some(row) = items
                .iter()
                .find(|item| locale_tags_match(&item.locale, &candidate))
            {
                return Some(row);
            }
        }

        items.first()
    }

    pub fn select_exact_entry_localization<'a>(
        items: &'a [flex_entry_localized_values::Model],
        locale: &str,
    ) -> Option<&'a flex_entry_localized_values::Model> {
        items
            .iter()
            .find(|item| locale_tags_match(&item.locale, locale))
    }

    async fn load_entry_localization_map<C: ConnectionTrait>(
        db: &C,
        tenant_id: Uuid,
        entry_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Vec<flex_entry_localized_values::Model>>, FlexError> {
        if entry_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = flex_entry_localized_values::Entity::find()
            .filter(flex_entry_localized_values::Column::TenantId.eq(tenant_id))
            .filter(flex_entry_localized_values::Column::EntryId.is_in(entry_ids.iter().copied()))
            .all(db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        let mut by_entry_id: HashMap<Uuid, Vec<flex_entry_localized_values::Model>> =
            HashMap::new();
        for row in rows {
            by_entry_id.entry(row.entry_id).or_default().push(row);
        }

        Ok(by_entry_id)
    }
}

#[async_trait]
impl flex::FlexStandaloneService for FlexStandaloneSeaOrmService {
    async fn list_schemas(&self, tenant_id: Uuid) -> Result<Vec<flex::FlexSchemaView>, FlexError> {
        validate_standalone_uuid(tenant_id, "tenant_id")?;
        let preferred_locale = self.tenant_default_locale(tenant_id).await?;
        let rows = flex_schemas::Entity::find()
            .filter(flex_schemas::Column::TenantId.eq(tenant_id))
            .order_by_asc(flex_schemas::Column::Slug)
            .all(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        let schema_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
        let translations = self.load_schema_translation_map(&schema_ids).await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let translation = translations
                    .get(&row.id)
                    .and_then(|items| Self::select_schema_translation(items, &preferred_locale));
                flex::standalone_schema_view_from_source(&row, translation)
            })
            .collect())
    }

    async fn find_schema(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
    ) -> Result<Option<flex::FlexSchemaView>, FlexError> {
        validate_standalone_uuid(tenant_id, "tenant_id")?;
        validate_standalone_uuid(schema_id, "schema_id")?;
        let preferred_locale = self.tenant_default_locale(tenant_id).await?;
        let row = flex_schemas::Entity::find_by_id(schema_id)
            .filter(flex_schemas::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let translations = self.load_schema_translation_map(&[row.id]).await?;
        let translation = translations
            .get(&row.id)
            .and_then(|items| Self::select_schema_translation(items, &preferred_locale));

        Ok(Some(flex::standalone_schema_view_from_source(
            &row,
            translation,
        )))
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
        validate_standalone_uuid(tenant_id, "tenant_id")?;
        validate_standalone_uuid(schema_id, "schema_id")?;
        let preferred_locale = self.tenant_default_locale(tenant_id).await?;
        let schema = self.get_schema_or_not_found(tenant_id, schema_id).await?;
        let localized_keys =
            flex::standalone_localized_field_keys(&schema.build_custom_fields_schema()?);

        let rows = flex_entries::Entity::find()
            .filter(flex_entries::Column::TenantId.eq(tenant_id))
            .filter(flex_entries::Column::SchemaId.eq(schema_id))
            .order_by_asc(flex_entries::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        let entry_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
        let localized =
            Self::load_entry_localization_map(&self.db, tenant_id, &entry_ids).await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let localized_data = localized
                    .get(&row.id)
                    .and_then(|items| Self::select_entry_localization(items, &preferred_locale))
                    .map(|item| &item.data);
                flex::standalone_entry_view_from_source(&row, localized_data, &localized_keys)
            })
            .collect())
    }

    async fn find_entry(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
        entry_id: Uuid,
    ) -> Result<Option<flex::FlexEntryView>, FlexError> {
        validate_standalone_uuid(tenant_id, "tenant_id")?;
        validate_standalone_uuid(schema_id, "schema_id")?;
        validate_standalone_uuid(entry_id, "entry_id")?;
        let preferred_locale = self.tenant_default_locale(tenant_id).await?;
        let schema = self.get_schema_or_not_found(tenant_id, schema_id).await?;
        let localized_keys =
            flex::standalone_localized_field_keys(&schema.build_custom_fields_schema()?);
        let row = flex_entries::Entity::find_by_id(entry_id)
            .filter(flex_entries::Column::TenantId.eq(tenant_id))
            .filter(flex_entries::Column::SchemaId.eq(schema_id))
            .one(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let localized =
            Self::load_entry_localization_map(&self.db, tenant_id, &[row.id]).await?;
        let localized_data = localized
            .get(&row.id)
            .and_then(|items| Self::select_entry_localization(items, &preferred_locale))
            .map(|item| &item.data);

        Ok(Some(flex::standalone_entry_view_from_source(
            &row,
            localized_data,
            &localized_keys,
        )))
    }

    async fn create_entry(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: flex::CreateFlexEntryCommand,
    ) -> Result<flex::FlexEntryView, FlexError> {
        validate_standalone_uuid(tenant_id, "tenant_id")?;
        validate_optional_standalone_uuid(actor_id, "actor_id")?;
        validate_create_entry_command(&input)?;

        let txn = self
            .db
            .begin()
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
        let locale = tenant_default_locale_on(&txn, tenant_id).await?;
        let schema = flex_schemas::Entity::find_by_id(input.schema_id)
            .filter(flex_schemas::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(&txn)
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?
            .ok_or(FlexError::NotFound(input.schema_id))?;
        let PreparedStandaloneEntryWrite {
            shared_data,
            localized_data,
            localized_keys,
        } = prepare_entry_write(&schema, input.data)?;

        let row = flex_entries::ActiveModel {
            id: Set(rustok_core::generate_id()),
            tenant_id: Set(tenant_id),
            schema_id: Set(input.schema_id),
            entity_type: Set(input.entity_type),
            entity_id: Set(input.entity_id),
            data: Set(shared_data),
            status: Set(input.status.unwrap_or_else(|| "draft".to_string())),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&txn)
        .await
        .map_err(|error| FlexError::Database(error.to_string()))?;
        let localized_data =
            upsert_entry_localization_on(&txn, row.id, tenant_id, &locale, localized_data).await?;
        txn.commit()
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;

        Ok(flex::standalone_entry_view_from_source(
            &row,
            localized_data.as_ref(),
            &localized_keys,
        ))
    }

    async fn update_entry(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        schema_id: Uuid,
        entry_id: Uuid,
        input: flex::UpdateFlexEntryCommand,
    ) -> Result<flex::FlexEntryView, FlexError> {
        validate_standalone_uuid(tenant_id, "tenant_id")?;
        validate_optional_standalone_uuid(actor_id, "actor_id")?;
        validate_standalone_uuid(schema_id, "schema_id")?;
        validate_standalone_uuid(entry_id, "entry_id")?;
        validate_update_entry_command(&input)?;

        let txn = self
            .db
            .begin()
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
        let locale = tenant_default_locale_on(&txn, tenant_id).await?;
        let schema = flex_schemas::Entity::find_by_id(schema_id)
            .filter(flex_schemas::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(&txn)
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?
            .ok_or(FlexError::NotFound(schema_id))?;
        let row = flex_entries::Entity::find_by_id(entry_id)
            .filter(flex_entries::Column::TenantId.eq(tenant_id))
            .filter(flex_entries::Column::SchemaId.eq(schema_id))
            .lock_exclusive()
            .one(&txn)
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?
            .ok_or(FlexError::NotFound(entry_id))?;

        let existing_row = row.clone();
        let mut model: flex_entries::ActiveModel = row.into();
        let localized_keys =
            flex::standalone_localized_field_keys(&schema.build_custom_fields_schema()?);
        let mut resolved_localized_data: Option<JsonValue> = None;

        if let Some(data) = input.data {
            let existing_localized =
                Self::load_entry_localization_map(&txn, tenant_id, &[entry_id]).await?;
            let existing_localized_data = existing_localized
                .get(&entry_id)
                .and_then(|items| Self::select_exact_entry_localization(items, &locale))
                .map(|item| &item.data);
            let merged_data = flex::merge_standalone_entry_patch(
                &existing_row.data,
                existing_localized_data,
                &localized_keys,
                data,
            );
            let PreparedStandaloneEntryWrite {
                shared_data,
                localized_data,
                ..
            } = prepare_entry_write(&schema, merged_data)?;
            model.data = Set(shared_data);
            resolved_localized_data =
                upsert_entry_localization_on(&txn, entry_id, tenant_id, &locale, localized_data)
                    .await?;
        }

        if let Some(status) = input.status {
            model.status = Set(status);
        }

        let updated = model
            .update(&txn)
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
        if resolved_localized_data.is_none() {
            let localized =
                Self::load_entry_localization_map(&txn, tenant_id, &[updated.id]).await?;
            resolved_localized_data = localized
                .get(&updated.id)
                .and_then(|items| Self::select_exact_entry_localization(items, &locale))
                .map(|item| item.data.clone());
        }
        txn.commit()
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;

        Ok(flex::standalone_entry_view_from_source(
            &updated,
            resolved_localized_data.as_ref(),
            &localized_keys,
        ))
    }

    async fn delete_entry(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        schema_id: Uuid,
        entry_id: Uuid,
    ) -> Result<(), FlexError> {
        validate_standalone_uuid(tenant_id, "tenant_id")?;
        validate_optional_standalone_uuid(actor_id, "actor_id")?;
        validate_standalone_uuid(schema_id, "schema_id")?;
        validate_standalone_uuid(entry_id, "entry_id")?;

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
        let row = flex_entries::Entity::find_by_id(entry_id)
            .filter(flex_entries::Column::TenantId.eq(tenant_id))
            .filter(flex_entries::Column::SchemaId.eq(schema_id))
            .lock_exclusive()
            .one(&txn)
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?
            .ok_or(FlexError::NotFound(entry_id))?;

        flex_entries::Entity::delete_by_id(row.id)
            .exec(&txn)
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
        txn.commit()
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
        Ok(())
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


async fn upsert_entry_localization_on<C>(
    db: &C,
    entry_id: Uuid,
    tenant_id: Uuid,
    locale: &str,
    data: Option<JsonValue>,
) -> Result<Option<JsonValue>, FlexError>
where
    C: ConnectionTrait,
{
    let locale =
        normalize_locale_tag(locale).ok_or_else(|| FlexError::InvalidLocale(locale.to_string()))?;
    let existing = flex_entry_localized_values::Entity::find()
        .filter(flex_entry_localized_values::Column::EntryId.eq(entry_id))
        .filter(flex_entry_localized_values::Column::TenantId.eq(tenant_id))
        .filter(flex_entry_localized_values::Column::Locale.eq(locale.as_str()))
        .one(db)
        .await
        .map_err(|error| FlexError::Database(error.to_string()))?;

    let Some(data) = data.filter(|value| !is_empty_object(value)) else {
        if let Some(row) = existing {
            let model: flex_entry_localized_values::ActiveModel = row.into();
            model
                .delete(db)
                .await
                .map_err(|error| FlexError::Database(error.to_string()))?;
        }
        return Ok(None);
    };

    match existing {
        Some(row) => {
            let mut model: flex_entry_localized_values::ActiveModel = row.into();
            model.data = Set(data.clone());
            model.tenant_id = Set(tenant_id);
            model
                .update(db)
                .await
                .map_err(|error| FlexError::Database(error.to_string()))?;
        }
        None => {
            flex_entry_localized_values::ActiveModel {
                entry_id: Set(entry_id),
                locale: Set(locale),
                tenant_id: Set(tenant_id),
                data: Set(data.clone()),
                created_at: sea_orm::ActiveValue::NotSet,
                updated_at: sea_orm::ActiveValue::NotSet,
            }
            .insert(db)
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
        }
    }
    Ok(Some(data))
}

fn prepare_entry_write(
    schema: &flex_schemas::Model,
    data: JsonValue,
) -> Result<PreparedStandaloneEntryWrite, FlexError> {
    let custom_fields_schema = schema.build_custom_fields_schema()?;
    let localized_keys = flex::standalone_localized_field_keys(&custom_fields_schema);
    let normalized = flex::normalize_and_validate_standalone_entry(&custom_fields_schema, data)?;
    let (shared, localized) = flex::split_standalone_entry_data(&normalized, &localized_keys);
    Ok(PreparedStandaloneEntryWrite {
        shared_data: JsonValue::Object(shared),
        localized_data: if localized.is_empty() {
            None
        } else {
            Some(JsonValue::Object(localized))
        },
        localized_keys,
    })
}

fn is_empty_object(value: &JsonValue) -> bool {
    value.as_object().is_none_or(|object| object.is_empty())
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

#[cfg(test)]
mod tests {
    use super::FlexStandaloneSeaOrmService;
    use crate::models::{
        flex_entries, flex_entry_localized_values, flex_schema_translations, flex_schemas, tenants,
    };
    use chrono::Utc;
    use flex::FlexStandaloneService;
    use rustok_core::field_schema::{FieldDefinition, FieldType};
    use rustok_migrations::SqliteTestMigrator as Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
    use serde_json::json;
    use std::collections::{HashMap, HashSet};
    use uuid::Uuid;

    fn translation(locale: &str, name: &str) -> flex_schema_translations::Model {
        let now = Utc::now().fixed_offset();
        flex_schema_translations::Model {
            schema_id: Uuid::new_v4(),
            locale: locale.to_string(),
            name: name.to_string(),
            description: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn select_schema_translation_prefers_exact_match_then_language_fallback() {
        let translations = vec![
            translation("pt", "Portuguese"),
            translation("en", "English"),
            translation("pt-BR", "Portuguese Brazil"),
        ];

        let selected =
            FlexStandaloneSeaOrmService::select_schema_translation(&translations, "pt-BR")
                .expect("translation must be selected");
        assert_eq!(selected.locale, "pt-BR");

        let selected =
            FlexStandaloneSeaOrmService::select_schema_translation(&translations, "pt-PT")
                .expect("translation must be selected");
        assert_eq!(selected.locale, "pt");
    }

    #[test]
    fn select_schema_translation_falls_back_to_en_then_first_available() {
        let translations = vec![translation("ru", "Russian"), translation("en", "English")];

        let selected =
            FlexStandaloneSeaOrmService::select_schema_translation(&translations, "de-DE")
                .expect("translation must be selected");
        assert_eq!(selected.locale, "en");

        let translations = vec![translation("ru", "Russian")];
        let selected =
            FlexStandaloneSeaOrmService::select_schema_translation(&translations, "de-DE")
                .expect("translation must be selected");
        assert_eq!(selected.locale, "ru");
    }

    #[test]
    fn entry_to_view_prefers_parallel_localized_row_and_keeps_shared_fields() {
        let now = Utc::now().fixed_offset();
        let row = flex_entries::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            schema_id: Uuid::new_v4(),
            entity_type: None,
            entity_id: None,
            data: json!({"slug": "landing", "title": "legacy"}),
            status: "draft".to_string(),
            created_at: now,
            updated_at: now,
        };
        let localized_keys = HashSet::from([String::from("title")]);
        let view = flex::standalone_entry_view_from_source(
            &row,
            Some(&json!({"title": "Привет"})),
            &localized_keys,
        );

        assert_eq!(view.data, json!({"slug": "landing", "title": "Привет"}));
    }

    #[test]
    fn entry_to_view_does_not_use_inline_localized_legacy_payload() {
        let now = Utc::now().fixed_offset();
        let row = flex_entries::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            schema_id: Uuid::new_v4(),
            entity_type: None,
            entity_id: None,
            data: json!({"slug": "landing", "title": "legacy"}),
            status: "draft".to_string(),
            created_at: now,
            updated_at: now,
        };
        let localized_keys = HashSet::from([String::from("title")]);
        let view = flex::standalone_entry_view_from_source(&row, None, &localized_keys);

        assert_eq!(view.data, json!({"slug": "landing"}));
    }

    #[test]
    fn merge_entry_patch_preserves_omitted_shared_and_localized_values() {
        let localized_keys = HashSet::from([String::from("title")]);
        let merged = flex::merge_standalone_entry_patch(
            &json!({"slug": "landing", "sort_order": 10}),
            Some(&json!({"title": "Привет"})),
            &localized_keys,
            json!({"sort_order": 20}),
        );

        assert_eq!(
            merged,
            json!({"slug": "landing", "sort_order": 20, "title": "Привет"})
        );
    }

    #[test]
    fn merge_entry_patch_allows_explicit_localized_override() {
        let localized_keys = HashSet::from([String::from("title")]);
        let merged = flex::merge_standalone_entry_patch(
            &json!({"slug": "landing"}),
            Some(&json!({"title": "Привет"})),
            &localized_keys,
            json!({"title": "Hello"}),
        );

        assert_eq!(merged, json!({"slug": "landing", "title": "Hello"}));
    }

    #[tokio::test]
    async fn create_entry_moves_localized_values_to_parallel_rows() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let builder = db.get_database_backend();
        let schema = sea_orm::Schema::new(builder);
        let mut stmt = schema.create_table_from_entity(flex_entry_localized_values::Entity);
        stmt.if_not_exists();
        db.execute_raw(builder.build(&stmt))
            .await
            .expect("create flex_entry_localized_values table for standalone flex tests");
        let service = FlexStandaloneSeaOrmService::new(db.clone());
        let tenant_id = Uuid::new_v4();
        let schema_id = Uuid::new_v4();

        tenants::ActiveModel {
            id: Set(tenant_id),
            name: Set("Flex Tenant".to_string()),
            slug: Set("flex-tenant".to_string()),
            domain: Set(None),
            settings: Set(json!({})),
            default_locale: Set("ru".to_string()),
            is_active: Set(true),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&db)
        .await
        .expect("tenant should insert");

        let fields_config = vec![
            FieldDefinition {
                field_key: "slug".to_string(),
                field_type: FieldType::Text,
                label: HashMap::from([("ru".to_string(), "Слаг".to_string())]),
                description: None,
                is_localized: false,
                is_required: true,
                default_value: None,
                validation: None,
                position: 0,
                is_active: true,
            },
            FieldDefinition {
                field_key: "title".to_string(),
                field_type: FieldType::Text,
                label: HashMap::from([("ru".to_string(), "Заголовок".to_string())]),
                description: None,
                is_localized: true,
                is_required: true,
                default_value: None,
                validation: None,
                position: 1,
                is_active: true,
            },
        ];

        flex_schemas::ActiveModel {
            id: Set(schema_id),
            tenant_id: Set(tenant_id),
            slug: Set("landing".to_string()),
            fields_config: Set(
                flex::serialize_standalone_fields_config(fields_config)
                    .expect("serialize schema fields"),
            ),
            settings: Set(json!({})),
            is_active: Set(true),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&db)
        .await
        .expect("schema should insert");

        let created = service
            .create_entry(
                tenant_id,
                None,
                flex::CreateFlexEntryCommand {
                    schema_id,
                    entity_type: None,
                    entity_id: None,
                    data: json!({
                        "slug": "landing",
                        "title": "Привет",
                    }),
                    status: Some("published".to_string()),
                },
            )
            .await
            .expect("entry create should succeed");

        assert_eq!(created.data, json!({"slug": "landing", "title": "Привет"}));

        let entry = flex_entries::Entity::find_by_id(created.id)
            .filter(flex_entries::Column::TenantId.eq(tenant_id))
            .one(&db)
            .await
            .expect("query entry")
            .expect("entry should exist");
        assert_eq!(entry.data, json!({"slug": "landing"}));

        let localized = flex_entry_localized_values::Entity::find()
            .filter(flex_entry_localized_values::Column::TenantId.eq(tenant_id))
            .filter(flex_entry_localized_values::Column::EntryId.eq(created.id))
            .all(&db)
            .await
            .expect("query localized");
        assert_eq!(localized.len(), 1);
        assert_eq!(localized[0].locale, "ru");
        assert_eq!(localized[0].data, json!({"title": "Привет"}));
    }

    #[tokio::test]
    async fn update_entry_preserves_omitted_localized_fields_when_patching_shared() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let builder = db.get_database_backend();
        let schema = sea_orm::Schema::new(builder);
        let mut stmt = schema.create_table_from_entity(flex_entry_localized_values::Entity);
        stmt.if_not_exists();
        db.execute_raw(builder.build(&stmt))
            .await
            .expect("create flex_entry_localized_values table for standalone flex tests");
        let service = FlexStandaloneSeaOrmService::new(db.clone());
        let tenant_id = Uuid::new_v4();
        let schema_id = Uuid::new_v4();

        tenants::ActiveModel {
            id: Set(tenant_id),
            name: Set("Flex Tenant".to_string()),
            slug: Set("flex-tenant".to_string()),
            domain: Set(None),
            settings: Set(json!({})),
            default_locale: Set("ru".to_string()),
            is_active: Set(true),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&db)
        .await
        .expect("tenant should insert");

        let fields_config = vec![
            FieldDefinition {
                field_key: "slug".to_string(),
                field_type: FieldType::Text,
                label: HashMap::from([("ru".to_string(), "Слаг".to_string())]),
                description: None,
                is_localized: false,
                is_required: true,
                default_value: None,
                validation: None,
                position: 0,
                is_active: true,
            },
            FieldDefinition {
                field_key: "title".to_string(),
                field_type: FieldType::Text,
                label: HashMap::from([("ru".to_string(), "Заголовок".to_string())]),
                description: None,
                is_localized: true,
                is_required: true,
                default_value: None,
                validation: None,
                position: 1,
                is_active: true,
            },
        ];

        flex_schemas::ActiveModel {
            id: Set(schema_id),
            tenant_id: Set(tenant_id),
            slug: Set("landing".to_string()),
            fields_config: Set(
                flex::serialize_standalone_fields_config(fields_config)
                    .expect("serialize schema fields"),
            ),
            settings: Set(json!({})),
            is_active: Set(true),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&db)
        .await
        .expect("schema should insert");

        let created = service
            .create_entry(
                tenant_id,
                None,
                flex::CreateFlexEntryCommand {
                    schema_id,
                    entity_type: None,
                    entity_id: None,
                    data: json!({
                        "slug": "landing",
                        "title": "Привет",
                    }),
                    status: Some("published".to_string()),
                },
            )
            .await
            .expect("entry create should succeed");

        let updated = service
            .update_entry(
                tenant_id,
                None,
                schema_id,
                created.id,
                flex::UpdateFlexEntryCommand {
                    data: Some(json!({"slug": "landing-updated"})),
                    status: None,
                },
            )
            .await
            .expect("entry update should succeed");

        assert_eq!(
            updated.data,
            json!({"slug": "landing-updated", "title": "Привет"})
        );

        let localized = flex_entry_localized_values::Entity::find()
            .filter(flex_entry_localized_values::Column::TenantId.eq(tenant_id))
            .filter(flex_entry_localized_values::Column::EntryId.eq(created.id))
            .all(&db)
            .await
            .expect("query localized");
        assert_eq!(localized.len(), 1);
        assert_eq!(localized[0].locale, "ru");
        assert_eq!(localized[0].data, json!({"title": "Привет"}));
    }

    #[tokio::test]
    async fn update_entry_does_not_seed_default_locale_from_another_locale() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let builder = db.get_database_backend();
        let schema = sea_orm::Schema::new(builder);
        let mut stmt = schema.create_table_from_entity(flex_entry_localized_values::Entity);
        stmt.if_not_exists();
        db.execute_raw(builder.build(&stmt))
            .await
            .expect("create flex_entry_localized_values table for standalone flex tests");
        let service = FlexStandaloneSeaOrmService::new(db.clone());
        let tenant_id = Uuid::new_v4();
        let schema_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();

        tenants::ActiveModel {
            id: Set(tenant_id),
            name: Set("Flex Tenant".to_string()),
            slug: Set("flex-tenant-exact-update".to_string()),
            domain: Set(None),
            settings: Set(json!({})),
            default_locale: Set("en".to_string()),
            is_active: Set(true),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&db)
        .await
        .expect("tenant should insert");

        let fields_config = vec![
            FieldDefinition {
                field_key: "slug".to_string(),
                field_type: FieldType::Text,
                label: HashMap::from([("en".to_string(), "Slug".to_string())]),
                description: None,
                is_localized: false,
                is_required: false,
                default_value: None,
                validation: None,
                position: 0,
                is_active: true,
            },
            FieldDefinition {
                field_key: "title".to_string(),
                field_type: FieldType::Text,
                label: HashMap::from([("en".to_string(), "Title".to_string())]),
                description: None,
                is_localized: true,
                is_required: false,
                default_value: None,
                validation: None,
                position: 1,
                is_active: true,
            },
            FieldDefinition {
                field_key: "tagline".to_string(),
                field_type: FieldType::Text,
                label: HashMap::from([("en".to_string(), "Tagline".to_string())]),
                description: None,
                is_localized: true,
                is_required: false,
                default_value: None,
                validation: None,
                position: 2,
                is_active: true,
            },
        ];

        flex_schemas::ActiveModel {
            id: Set(schema_id),
            tenant_id: Set(tenant_id),
            slug: Set("landing_form_exact_update".to_string()),
            fields_config: Set(
                flex::serialize_standalone_fields_config(fields_config)
                    .expect("serialize schema fields"),
            ),
            settings: Set(json!({})),
            is_active: Set(true),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&db)
        .await
        .expect("schema should insert");

        flex_entries::ActiveModel {
            id: Set(entry_id),
            tenant_id: Set(tenant_id),
            schema_id: Set(schema_id),
            entity_type: Set(None),
            entity_id: Set(None),
            data: Set(json!({"slug": "landing"})),
            status: Set("draft".to_string()),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&db)
        .await
        .expect("entry should insert");

        flex_entry_localized_values::ActiveModel {
            entry_id: Set(entry_id),
            locale: Set("ru".to_string()),
            tenant_id: Set(tenant_id),
            data: Set(json!({"title": "Russian title"})),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&db)
        .await
        .expect("foreign locale row should insert");

        let updated = service
            .update_entry(
                tenant_id,
                None,
                schema_id,
                entry_id,
                flex::UpdateFlexEntryCommand {
                    data: Some(json!({"tagline": "English tagline"})),
                    status: None,
                },
            )
            .await
            .expect("exact locale update should succeed");

        assert_eq!(
            updated.data,
            json!({"slug": "landing", "tagline": "English tagline"})
        );

        let english = flex_entry_localized_values::Entity::find()
            .filter(flex_entry_localized_values::Column::EntryId.eq(entry_id))
            .filter(flex_entry_localized_values::Column::Locale.eq("en"))
            .one(&db)
            .await
            .expect("English localization should load")
            .expect("English localization should exist");
        assert_eq!(english.data, json!({"tagline": "English tagline"}));

        let russian = flex_entry_localized_values::Entity::find()
            .filter(flex_entry_localized_values::Column::EntryId.eq(entry_id))
            .filter(flex_entry_localized_values::Column::Locale.eq("ru"))
            .one(&db)
            .await
            .expect("Russian localization should load")
            .expect("Russian localization should remain");
        assert_eq!(russian.data, json!({"title": "Russian title"}));
    }
}

