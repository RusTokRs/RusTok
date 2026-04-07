//! SeaORM-backed adapter implementation of `flex::FlexStandaloneService`.

use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder,
};
use serde_json::{Map, Value as JsonValue};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use rustok_core::{
    build_locale_candidates,
    field_schema::{CustomFieldsSchema, FlexError},
    locale_tags_match, normalize_locale_tag, PLATFORM_FALLBACK_LOCALE,
};

use crate::models::{
    flex_entries, flex_entry_localized_values, flex_schema_translations, flex_schemas, tenants,
};
use crate::services::flex_standalone_validation_service::FlexStandaloneValidationService;

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

    fn schema_to_view(
        model: flex_schemas::Model,
        translation: Option<&flex_schema_translations::Model>,
    ) -> flex::FlexSchemaView {
        let slug_fallback = model.slug.clone();
        let fields_config = model.parse_field_definitions().unwrap_or_default();

        flex::FlexSchemaView {
            id: model.id,
            slug: model.slug,
            name: translation
                .map(|row| row.name.clone())
                .unwrap_or(slug_fallback),
            description: translation.and_then(|row| row.description.clone()),
            fields_config,
            settings: model.settings,
            is_active: model.is_active,
            created_at: model.created_at.to_rfc3339(),
            updated_at: model.updated_at.to_rfc3339(),
        }
    }

    fn entry_to_view(
        model: flex_entries::Model,
        localized_data: Option<&JsonValue>,
        localized_keys: &HashSet<String>,
    ) -> flex::FlexEntryView {
        let (mut shared_data, legacy_localized) =
            Self::split_entry_data(&model.data, localized_keys);
        let resolved_localized = localized_data
            .and_then(|value| value.as_object().cloned())
            .or_else(|| {
                if legacy_localized.is_empty() {
                    None
                } else {
                    Some(legacy_localized)
                }
            });

        if let Some(localized) = resolved_localized {
            for (key, value) in localized {
                shared_data.insert(key, value);
            }
        }

        flex::FlexEntryView {
            id: model.id,
            schema_id: model.schema_id,
            entity_type: model.entity_type,
            entity_id: model.entity_id,
            data: JsonValue::Object(shared_data),
            status: model.status,
            created_at: model.created_at.to_rfc3339(),
            updated_at: model.updated_at.to_rfc3339(),
        }
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
        let tenant = tenants::Entity::find_by_id(&self.db, tenant_id)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        Ok(tenant
            .and_then(|row| normalize_locale_tag(&row.default_locale))
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string()))
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

    async fn upsert_schema_translation(
        &self,
        schema_id: Uuid,
        locale: &str,
        slug_fallback: &str,
        name: Option<String>,
        description: Option<String>,
    ) -> Result<flex_schema_translations::Model, FlexError> {
        let existing = flex_schema_translations::Entity::find()
            .filter(flex_schema_translations::Column::SchemaId.eq(schema_id))
            .filter(flex_schema_translations::Column::Locale.eq(locale))
            .one(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

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
                    .update(&self.db)
                    .await
                    .map_err(|e| FlexError::Database(e.to_string()))
            }
            None => flex_schema_translations::ActiveModel {
                schema_id: Set(schema_id),
                locale: Set(locale.to_string()),
                name: Set(name.unwrap_or_else(|| slug_fallback.to_string())),
                description: Set(description),
                created_at: sea_orm::ActiveValue::NotSet,
                updated_at: sea_orm::ActiveValue::NotSet,
            }
            .insert(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string())),
        }
    }

    fn select_entry_localization<'a>(
        rows: &'a [flex_entry_localized_values::Model],
        preferred_locale: &str,
    ) -> Option<&'a flex_entry_localized_values::Model> {
        let candidates = build_locale_candidates(
            [Some(preferred_locale), Some(PLATFORM_FALLBACK_LOCALE)],
            true,
        );

        for candidate in candidates {
            if let Some(row) = rows
                .iter()
                .find(|localized| locale_tags_match(&localized.locale, &candidate))
            {
                return Some(row);
            }
        }

        rows.first()
    }

    async fn load_entry_localization_map(
        &self,
        entry_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Vec<flex_entry_localized_values::Model>>, FlexError> {
        if entry_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = flex_entry_localized_values::Entity::find()
            .filter(flex_entry_localized_values::Column::EntryId.is_in(entry_ids.iter().copied()))
            .all(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        let mut by_entry_id: HashMap<Uuid, Vec<flex_entry_localized_values::Model>> =
            HashMap::new();
        for row in rows {
            by_entry_id.entry(row.entry_id).or_default().push(row);
        }

        Ok(by_entry_id)
    }

    async fn upsert_entry_localization(
        &self,
        entry_id: Uuid,
        tenant_id: Uuid,
        locale: &str,
        data: Option<JsonValue>,
    ) -> Result<Option<JsonValue>, FlexError> {
        let locale =
            normalize_locale_tag(locale).unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());

        let existing = flex_entry_localized_values::Entity::find()
            .filter(flex_entry_localized_values::Column::EntryId.eq(entry_id))
            .filter(flex_entry_localized_values::Column::Locale.eq(locale.as_str()))
            .one(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        let Some(data) = data.filter(|value| !Self::is_empty_object(value)) else {
            if let Some(row) = existing {
                let model: flex_entry_localized_values::ActiveModel = row.into();
                model
                    .delete(&self.db)
                    .await
                    .map_err(|e| FlexError::Database(e.to_string()))?;
            }
            return Ok(None);
        };

        match existing {
            Some(row) => {
                let mut model: flex_entry_localized_values::ActiveModel = row.into();
                model.data = Set(data.clone());
                model.tenant_id = Set(tenant_id);
                model
                    .update(&self.db)
                    .await
                    .map_err(|e| FlexError::Database(e.to_string()))?;
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
                .insert(&self.db)
                .await
                .map_err(|e| FlexError::Database(e.to_string()))?;
            }
        }

        Ok(Some(data))
    }

    fn localized_field_keys(schema: &CustomFieldsSchema) -> HashSet<String> {
        schema
            .active_definitions()
            .into_iter()
            .filter(|definition| definition.is_localized)
            .map(|definition| definition.field_key.clone())
            .collect()
    }

    fn split_entry_data(
        data: &JsonValue,
        localized_keys: &HashSet<String>,
    ) -> (Map<String, JsonValue>, Map<String, JsonValue>) {
        let mut shared = Map::new();
        let mut localized = Map::new();

        for (key, value) in Self::object_map(Some(data)) {
            if localized_keys.contains(&key) {
                localized.insert(key, value);
            } else {
                shared.insert(key, value);
            }
        }

        (shared, localized)
    }

    fn object_map(value: Option<&JsonValue>) -> Map<String, JsonValue> {
        value
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default()
    }

    fn is_empty_object(value: &JsonValue) -> bool {
        value.as_object().is_none_or(|object| object.is_empty())
    }

    fn prepare_entry_write(
        &self,
        schema: &flex_schemas::Model,
        data: JsonValue,
    ) -> Result<PreparedStandaloneEntryWrite, FlexError> {
        let custom_fields_schema = FlexStandaloneValidationService::build_schema(schema)?;
        let localized_keys = Self::localized_field_keys(&custom_fields_schema);
        let normalized = FlexStandaloneValidationService::normalize_and_validate_entry(
            &custom_fields_schema,
            data,
        )?;
        let (shared, localized) = Self::split_entry_data(&normalized, &localized_keys);

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
}

#[async_trait]
impl flex::FlexStandaloneService for FlexStandaloneSeaOrmService {
    async fn list_schemas(&self, tenant_id: Uuid) -> Result<Vec<flex::FlexSchemaView>, FlexError> {
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
                Self::schema_to_view(row, translation)
            })
            .collect())
    }

    async fn find_schema(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
    ) -> Result<Option<flex::FlexSchemaView>, FlexError> {
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

        Ok(Some(Self::schema_to_view(row, translation)))
    }

    async fn create_schema(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        input: flex::CreateFlexSchemaCommand,
    ) -> Result<flex::FlexSchemaView, FlexError> {
        let locale = self.tenant_default_locale(tenant_id).await?;
        let row = flex_schemas::ActiveModel {
            id: Set(rustok_core::generate_id()),
            tenant_id: Set(tenant_id),
            slug: Set(input.slug),
            fields_config: Set(serde_json::to_value(input.fields_config).unwrap_or_default()),
            settings: Set(input.settings.unwrap_or_else(|| serde_json::json!({}))),
            is_active: Set(input.is_active.unwrap_or(true)),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&self.db)
        .await
        .map_err(|e| FlexError::Database(e.to_string()))?;

        let translation = self
            .upsert_schema_translation(
                row.id,
                &locale,
                &row.slug,
                Some(input.name),
                input.description,
            )
            .await?;

        Ok(Self::schema_to_view(row, Some(&translation)))
    }

    async fn update_schema(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        schema_id: Uuid,
        input: flex::UpdateFlexSchemaCommand,
    ) -> Result<flex::FlexSchemaView, FlexError> {
        let locale = self.tenant_default_locale(tenant_id).await?;
        let row = self.get_schema_or_not_found(tenant_id, schema_id).await?;
        let mut model: flex_schemas::ActiveModel = row.into();

        if let Some(fields_config) = input.fields_config {
            model.fields_config = Set(serde_json::to_value(fields_config).unwrap_or_default());
        }
        if let Some(settings) = input.settings {
            model.settings = Set(settings);
        }
        if let Some(is_active) = input.is_active {
            model.is_active = Set(is_active);
        }

        let updated = model
            .update(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        let translation = if input.name.is_some() || input.description.is_some() {
            Some(
                self.upsert_schema_translation(
                    updated.id,
                    &locale,
                    &updated.slug,
                    input.name,
                    input.description,
                )
                .await?,
            )
        } else {
            let mut translations = self.load_schema_translation_map(&[updated.id]).await?;
            translations
                .remove(&updated.id)
                .and_then(|items| Self::select_schema_translation(&items, &locale).cloned())
        };

        Ok(Self::schema_to_view(updated, translation.as_ref()))
    }

    async fn delete_schema(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        schema_id: Uuid,
    ) -> Result<(), FlexError> {
        let row = self.get_schema_or_not_found(tenant_id, schema_id).await?;

        flex_schemas::Entity::delete_by_id(row.id)
            .exec(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        Ok(())
    }

    async fn list_entries(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
    ) -> Result<Vec<flex::FlexEntryView>, FlexError> {
        let preferred_locale = self.tenant_default_locale(tenant_id).await?;
        let schema = self.get_schema_or_not_found(tenant_id, schema_id).await?;
        let localized_keys = Self::localized_field_keys(&schema.build_custom_fields_schema()?);

        let rows = flex_entries::Entity::find()
            .filter(flex_entries::Column::TenantId.eq(tenant_id))
            .filter(flex_entries::Column::SchemaId.eq(schema_id))
            .order_by_asc(flex_entries::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        let entry_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
        let localized = self.load_entry_localization_map(&entry_ids).await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let localized_data = localized
                    .get(&row.id)
                    .and_then(|items| Self::select_entry_localization(items, &preferred_locale))
                    .map(|item| &item.data);
                Self::entry_to_view(row, localized_data, &localized_keys)
            })
            .collect())
    }

    async fn find_entry(
        &self,
        tenant_id: Uuid,
        schema_id: Uuid,
        entry_id: Uuid,
    ) -> Result<Option<flex::FlexEntryView>, FlexError> {
        let preferred_locale = self.tenant_default_locale(tenant_id).await?;
        let schema = self.get_schema_or_not_found(tenant_id, schema_id).await?;
        let localized_keys = Self::localized_field_keys(&schema.build_custom_fields_schema()?);
        let row = flex_entries::Entity::find_by_id(entry_id)
            .filter(flex_entries::Column::TenantId.eq(tenant_id))
            .filter(flex_entries::Column::SchemaId.eq(schema_id))
            .one(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let localized = self.load_entry_localization_map(&[row.id]).await?;
        let localized_data = localized
            .get(&row.id)
            .and_then(|items| Self::select_entry_localization(items, &preferred_locale))
            .map(|item| &item.data);

        Ok(Some(Self::entry_to_view(
            row,
            localized_data,
            &localized_keys,
        )))
    }

    async fn create_entry(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        input: flex::CreateFlexEntryCommand,
    ) -> Result<flex::FlexEntryView, FlexError> {
        let locale = self.tenant_default_locale(tenant_id).await?;
        let schema = self
            .get_schema_or_not_found(tenant_id, input.schema_id)
            .await?;
        let prepared = self.prepare_entry_write(&schema, input.data)?;

        let row = flex_entries::ActiveModel {
            id: Set(rustok_core::generate_id()),
            tenant_id: Set(tenant_id),
            schema_id: Set(input.schema_id),
            entity_type: Set(input.entity_type),
            entity_id: Set(input.entity_id),
            data: Set(prepared.shared_data),
            status: Set(input.status.unwrap_or_else(|| "draft".to_string())),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&self.db)
        .await
        .map_err(|e| FlexError::Database(e.to_string()))?;

        let localized_data = self
            .upsert_entry_localization(row.id, tenant_id, &locale, prepared.localized_data)
            .await?;

        Ok(Self::entry_to_view(
            row,
            localized_data.as_ref(),
            &prepared.localized_keys,
        ))
    }

    async fn update_entry(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        schema_id: Uuid,
        entry_id: Uuid,
        input: flex::UpdateFlexEntryCommand,
    ) -> Result<flex::FlexEntryView, FlexError> {
        let locale = self.tenant_default_locale(tenant_id).await?;
        let schema = self.get_schema_or_not_found(tenant_id, schema_id).await?;
        let row = flex_entries::Entity::find_by_id(entry_id)
            .filter(flex_entries::Column::TenantId.eq(tenant_id))
            .filter(flex_entries::Column::SchemaId.eq(schema_id))
            .one(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?
            .ok_or(FlexError::NotFound(entry_id))?;

        let mut model: flex_entries::ActiveModel = row.into();
        let localized_keys = Self::localized_field_keys(&schema.build_custom_fields_schema()?);
        let mut resolved_localized_data: Option<JsonValue> = None;

        if let Some(data) = input.data {
            let prepared = self.prepare_entry_write(&schema, data)?;
            model.data = Set(prepared.shared_data);
            resolved_localized_data = self
                .upsert_entry_localization(entry_id, tenant_id, &locale, prepared.localized_data)
                .await?;
        }

        if let Some(status) = input.status {
            model.status = Set(status);
        }

        let updated = model
            .update(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        if resolved_localized_data.is_none() {
            let localized = self.load_entry_localization_map(&[updated.id]).await?;
            resolved_localized_data = localized
                .get(&updated.id)
                .and_then(|items| Self::select_entry_localization(items, &locale))
                .map(|item| item.data.clone());
        }

        Ok(Self::entry_to_view(
            updated,
            resolved_localized_data.as_ref(),
            &localized_keys,
        ))
    }

    async fn delete_entry(
        &self,
        tenant_id: Uuid,
        _actor_id: Option<Uuid>,
        schema_id: Uuid,
        entry_id: Uuid,
    ) -> Result<(), FlexError> {
        let row = flex_entries::Entity::find_by_id(entry_id)
            .filter(flex_entries::Column::TenantId.eq(tenant_id))
            .filter(flex_entries::Column::SchemaId.eq(schema_id))
            .one(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?
            .ok_or(FlexError::NotFound(entry_id))?;

        flex_entries::Entity::delete_by_id(row.id)
            .exec(&self.db)
            .await
            .map_err(|e| FlexError::Database(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::FlexStandaloneSeaOrmService;
    use crate::models::{
        flex_entries, flex_entry_localized_values, flex_schema_translations, flex_schemas, tenants,
    };
    use chrono::Utc;
    use flex::FlexStandaloneService;
    use migration::Migrator;
    use rustok_core::field_schema::{FieldDefinition, FieldType};
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
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
        let view = FlexStandaloneSeaOrmService::entry_to_view(
            row,
            Some(&json!({"title": "Привет"})),
            &localized_keys,
        );

        assert_eq!(view.data, json!({"slug": "landing", "title": "Привет"}));
    }

    #[tokio::test]
    async fn create_entry_moves_localized_values_to_parallel_rows() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
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

        flex_schemas::ActiveModel {
            id: Set(schema_id),
            tenant_id: Set(tenant_id),
            slug: Set("landing_form".to_string()),
            fields_config: Set(json!([
                field_definition("slug", false),
                field_definition("title", true)
            ])),
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
                    data: json!({"slug": "landing", "title": "Привет"}),
                    status: None,
                },
            )
            .await
            .expect("entry should create");

        assert_eq!(created.data, json!({"slug": "landing", "title": "Привет"}));

        let stored_entry = flex_entries::Entity::find_by_id(created.id)
            .one(&db)
            .await
            .expect("entry load should succeed")
            .expect("entry row should exist");
        assert_eq!(stored_entry.data, json!({"slug": "landing"}));

        let localized = flex_entry_localized_values::Entity::find()
            .filter(flex_entry_localized_values::Column::EntryId.eq(created.id))
            .one(&db)
            .await
            .expect("localized row load should succeed")
            .expect("localized row should exist");
        assert_eq!(localized.locale, "ru");
        assert_eq!(localized.data, json!({"title": "Привет"}));

        let found = service
            .find_entry(tenant_id, schema_id, created.id)
            .await
            .expect("find entry should succeed")
            .expect("entry should resolve");
        assert_eq!(found.data, json!({"slug": "landing", "title": "Привет"}));
    }

    fn field_definition(field_key: &str, is_localized: bool) -> FieldDefinition {
        FieldDefinition {
            field_key: field_key.to_string(),
            field_type: FieldType::Text,
            label: HashMap::from([("en".to_string(), field_key.to_string())]),
            description: None,
            is_localized,
            is_required: false,
            default_value: None,
            validation: None,
            position: 0,
            is_active: true,
        }
    }
}
