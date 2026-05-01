use std::collections::{BTreeMap, HashMap, HashSet};

use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
};
use serde_json::{Map, Value};
use uuid::Uuid;

use rustok_core::field_schema::{CustomFieldsSchema, FieldDefinition, FlexError};
use rustok_core::{
    build_locale_candidates, locale_tags_match, normalize_locale_tag, PLATFORM_FALLBACK_LOCALE,
};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "flex_attached_localized_values")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub field_key: String,
    pub locale: String,
    pub value: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedAttachedValuesWrite {
    pub metadata: Option<Value>,
    pub localized_values: Option<Value>,
    pub locale: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AttachedEntityRef<'a> {
    pub tenant_id: Uuid,
    pub entity_type: &'a str,
    pub entity_id: Uuid,
}

pub fn prepare_attached_values_create(
    schema: CustomFieldsSchema,
    payload: Option<Value>,
    locale: &str,
) -> Result<PreparedAttachedValuesWrite, FlexError> {
    prepare_write(
        schema,
        payload,
        Value::Object(Map::new()),
        Value::Object(Map::new()),
        locale,
    )
}

pub async fn prepare_attached_values_update<C>(
    db: &C,
    entity: AttachedEntityRef<'_>,
    schema: CustomFieldsSchema,
    locale: &str,
    existing_metadata: &Value,
    payload: Option<Value>,
) -> Result<PreparedAttachedValuesWrite, FlexError>
where
    C: ConnectionTrait,
{
    let exact_locale = canonical_locale(locale);
    let localized_by_locale =
        load_localized_values_by_locale(db, entity.tenant_id, entity.entity_type, entity.entity_id).await?;
    let existing_localized =
        load_exact_locale_values(
            db,
            entity.tenant_id,
            entity.entity_type,
            entity.entity_id,
            exact_locale.as_str(),
        )
            .await?
            .or_else(|| first_available_localized_values(localized_by_locale))
            .unwrap_or_else(|| Value::Object(Map::new()));

    prepare_write(
        schema,
        payload,
        existing_metadata.clone(),
        existing_localized,
        exact_locale.as_str(),
    )
}

pub async fn resolve_attached_payload<C>(
    db: &C,
    entity: AttachedEntityRef<'_>,
    schema: CustomFieldsSchema,
    shared_metadata: &Value,
    preferred_locale: &str,
    tenant_default_locale: &str,
) -> Result<Option<Value>, FlexError>
where
    C: ConnectionTrait,
{
    if schema.active_definitions().is_empty() {
        return normalize_owner_payload(shared_metadata);
    }

    let (_, localized_keys) = split_definitions(&schema);
    let (shared_values, _) = split_existing_metadata(shared_metadata, &localized_keys);
    let localized_by_locale =
        load_localized_values_by_locale(db, entity.tenant_id, entity.entity_type, entity.entity_id).await?;

    let candidates = build_locale_candidates(
        [
            Some(preferred_locale),
            Some(tenant_default_locale),
            Some(PLATFORM_FALLBACK_LOCALE),
        ],
        true,
    );

    let resolved_localized = candidates
        .iter()
        .find_map(|candidate| {
            localized_by_locale
                .iter()
                .find(|(locale, _)| locale_tags_match(locale, candidate))
                .map(|(_, values)| values.clone())
        })
        .or_else(|| localized_by_locale.values().next().cloned());

    let mut merged = shared_values;
    if let Some(localized) = resolved_localized {
        for (key, value) in localized {
            merged.insert(key, value);
        }
    }

    if merged.is_empty() {
        Ok(None)
    } else {
        Ok(Some(Value::Object(merged)))
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
    let locale = canonical_locale(locale);
    let desired = object_map(Some(values));
    let existing = Entity::find()
        .filter(Column::TenantId.eq(tenant_id))
        .filter(Column::EntityType.eq(entity_type))
        .filter(Column::EntityId.eq(entity_id))
        .filter(Column::Locale.eq(locale.as_str()))
        .all(db)
        .await
        .map_err(|error| FlexError::Database(error.to_string()))?;

    let mut existing_by_key = HashMap::new();
    for row in existing {
        existing_by_key.insert(row.field_key.clone(), row);
    }

    for (field_key, row) in &existing_by_key {
        if !desired.contains_key(field_key) {
            let model: ActiveModel = row.clone().into();
            model
                .delete(db)
                .await
                .map_err(|error| FlexError::Database(error.to_string()))?;
        }
    }

    for (field_key, value) in desired {
        if let Some(row) = existing_by_key.remove(&field_key) {
            let mut model: ActiveModel = row.into();
            model.value = Set(value);
            model
                .update(db)
                .await
                .map_err(|error| FlexError::Database(error.to_string()))?;
        } else {
            ActiveModel {
                id: Set(rustok_core::generate_id()),
                tenant_id: Set(tenant_id),
                entity_type: Set(entity_type.to_string()),
                entity_id: Set(entity_id),
                field_key: Set(field_key),
                locale: Set(locale.clone()),
                value: Set(value),
                created_at: sea_orm::ActiveValue::NotSet,
                updated_at: sea_orm::ActiveValue::NotSet,
            }
            .insert(db)
            .await
            .map_err(|error| FlexError::Database(error.to_string()))?;
        }
    }

    Ok(())
}

pub async fn delete_attached_localized_values<C>(
    db: &C,
    tenant_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
) -> Result<u64, FlexError>
where
    C: ConnectionTrait,
{
    match Entity::delete_many()
        .filter(Column::TenantId.eq(tenant_id))
        .filter(Column::EntityType.eq(entity_type))
        .filter(Column::EntityId.eq(entity_id))
        .exec(db)
        .await
    {
        Ok(result) => Ok(result.rows_affected),
        Err(error) if is_missing_attached_storage_error(&error.to_string()) => Ok(0),
        Err(error) => Err(FlexError::Database(error.to_string())),
    }
}

pub async fn load_exact_locale_values<C>(
    db: &C,
    tenant_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
    locale: &str,
) -> Result<Option<Value>, FlexError>
where
    C: ConnectionTrait,
{
    let rows = Entity::find()
        .filter(Column::TenantId.eq(tenant_id))
        .filter(Column::EntityType.eq(entity_type))
        .filter(Column::EntityId.eq(entity_id))
        .filter(Column::Locale.eq(locale))
        .all(db)
        .await
        .map_err(|error| FlexError::Database(error.to_string()))?;

    if rows.is_empty() {
        return Ok(None);
    }

    let mut values = Map::new();
    for row in rows {
        values.insert(row.field_key, row.value);
    }
    Ok(Some(Value::Object(values)))
}

pub async fn load_localized_values_by_locale<C>(
    db: &C,
    tenant_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
) -> Result<BTreeMap<String, Map<String, Value>>, FlexError>
where
    C: ConnectionTrait,
{
    let rows = Entity::find()
        .filter(Column::TenantId.eq(tenant_id))
        .filter(Column::EntityType.eq(entity_type))
        .filter(Column::EntityId.eq(entity_id))
        .all(db)
        .await
        .map_err(|error| FlexError::Database(error.to_string()))?;

    let mut localized_by_locale: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
    for row in rows {
        let locale = canonical_locale(&row.locale);
        localized_by_locale
            .entry(locale)
            .or_default()
            .insert(row.field_key, row.value);
    }

    Ok(localized_by_locale)
}

fn first_available_localized_values(
    localized_by_locale: BTreeMap<String, Map<String, Value>>,
) -> Option<Value> {
    localized_by_locale.into_values().next().map(Value::Object)
}

fn prepare_write(
    schema: CustomFieldsSchema,
    payload: Option<Value>,
    existing_metadata: Value,
    existing_localized: Value,
    locale: &str,
) -> Result<PreparedAttachedValuesWrite, FlexError> {
    if schema.active_definitions().is_empty() {
        return Ok(PreparedAttachedValuesWrite {
            metadata: payload,
            localized_values: None,
            locale: None,
        });
    }

    let (definitions, localized_keys) = split_definitions(&schema);
    let (shared_definitions, localized_definitions): (Vec<_>, Vec<_>) = definitions
        .into_iter()
        .partition(|definition| !definition.is_localized);

    let payload_map = object_map(payload.as_ref());
    let (shared_patch, localized_patch) = split_patch(&payload_map, &localized_keys);
    let (mut shared_values, _) = split_existing_metadata(&existing_metadata, &localized_keys);
    let mut localized_values = object_map(Some(&existing_localized));

    merge_patch(&mut shared_values, shared_patch);
    merge_patch(&mut localized_values, localized_patch);

    let metadata = if shared_definitions.is_empty() {
        None
    } else {
        let shared_schema = CustomFieldsSchema::new(shared_definitions);
        let mut shared_value = Value::Object(shared_values);
        shared_schema.apply_defaults(&mut shared_value);
        shared_schema.strip_unknown(&mut shared_value);
        validate_schema(&shared_schema, &shared_value)?;
        Some(shared_value)
    };

    let localized_values = if localized_definitions.is_empty() {
        None
    } else {
        let localized_schema = CustomFieldsSchema::new(localized_definitions);
        let mut localized_value = Value::Object(localized_values);
        localized_schema.apply_defaults(&mut localized_value);
        localized_schema.strip_unknown(&mut localized_value);
        validate_schema(&localized_schema, &localized_value)?;
        Some(localized_value)
    };

    Ok(PreparedAttachedValuesWrite {
        metadata,
        localized_values,
        locale: Some(canonical_locale(locale)),
    })
}

fn split_definitions(schema: &CustomFieldsSchema) -> (Vec<FieldDefinition>, HashSet<String>) {
    let definitions: Vec<FieldDefinition> =
        schema.active_definitions().into_iter().cloned().collect();
    let localized_keys = definitions
        .iter()
        .filter(|definition| definition.is_localized)
        .map(|definition| definition.field_key.clone())
        .collect();
    (definitions, localized_keys)
}

fn split_existing_metadata(
    metadata: &Value,
    localized_keys: &HashSet<String>,
) -> (Map<String, Value>, Map<String, Value>) {
    let mut shared = Map::new();
    let mut localized = Map::new();

    for (key, value) in object_map(Some(metadata)) {
        if localized_keys.contains(&key) {
            localized.insert(key, value);
        } else {
            shared.insert(key, value);
        }
    }

    (shared, localized)
}

fn split_patch(
    payload: &Map<String, Value>,
    localized_keys: &HashSet<String>,
) -> (Map<String, Value>, Map<String, Value>) {
    let mut shared = Map::new();
    let mut localized = Map::new();

    for (key, value) in payload {
        if localized_keys.contains(key) {
            localized.insert(key.clone(), value.clone());
        } else {
            shared.insert(key.clone(), value.clone());
        }
    }

    (shared, localized)
}

fn merge_patch(target: &mut Map<String, Value>, patch: Map<String, Value>) {
    for (key, value) in patch {
        target.insert(key, value);
    }
}

fn object_map(value: Option<&Value>) -> Map<String, Value> {
    value
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

fn validate_schema(schema: &CustomFieldsSchema, payload: &Value) -> Result<(), FlexError> {
    let errors = schema.validate(payload);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(FlexError::ValidationFailed(errors))
    }
}

fn canonical_locale(locale: &str) -> String {
    normalize_locale_tag(locale).unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string())
}

fn normalize_owner_payload(shared_metadata: &Value) -> Result<Option<Value>, FlexError> {
    let payload = object_map(Some(shared_metadata));
    if payload.is_empty() {
        Ok(None)
    } else {
        Ok(Some(Value::Object(payload)))
    }
}

fn is_missing_attached_storage_error(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("no such table: flex_attached_localized_values")
        || normalized.contains("relation \"flex_attached_localized_values\" does not exist")
        || normalized.contains("relation 'flex_attached_localized_values' does not exist")
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use sea_orm::{
        ActiveModelTrait, ConnectionTrait, Database, DatabaseBackend, EntityTrait, Set, Statement,
    };
    use serde_json::{json, Map};
    use uuid::Uuid;

    use rustok_core::field_schema::{CustomFieldsSchema, FieldDefinition, FieldType};

    use super::{
        delete_attached_localized_values, first_available_localized_values,
        prepare_attached_values_create, split_existing_metadata, ActiveModel, Entity,
    };

    fn definition(field_key: &str, is_localized: bool) -> FieldDefinition {
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

    #[test]
    fn split_existing_metadata_separates_legacy_localized_keys() {
        let schema =
            CustomFieldsSchema::new(vec![definition("nickname", false), definition("bio", true)]);
        let (_, localized_keys) = super::split_definitions(&schema);
        let (shared, localized) = split_existing_metadata(
            &json!({"nickname": "neo", "bio": "ru copy"}),
            &localized_keys,
        );

        assert_eq!(shared.get("nickname"), Some(&json!("neo")));
        assert_eq!(localized.get("bio"), Some(&json!("ru copy")));
    }

    #[test]
    fn prepare_write_moves_localized_fields_out_of_owner_metadata() {
        let schema =
            CustomFieldsSchema::new(vec![definition("nickname", false), definition("bio", true)]);
        let prepared = prepare_attached_values_create(
            schema,
            Some(json!({"nickname": "neo", "bio": "Привет"})),
            "ru",
        )
        .expect("write should prepare");

        assert_eq!(prepared.metadata, Some(json!({"nickname": "neo"})));
        assert_eq!(prepared.localized_values, Some(json!({"bio": "Привет"})));
        assert_eq!(prepared.locale.as_deref(), Some("ru"));
    }

    #[test]
    fn first_available_localized_values_uses_deterministic_locale_order() {
        let mut localized_by_locale = BTreeMap::new();
        localized_by_locale.insert(
            "ru".to_string(),
            Map::from_iter([("bio".into(), json!("Привет"))]),
        );
        localized_by_locale.insert(
            "en".to_string(),
            Map::from_iter([("bio".into(), json!("Hello"))]),
        );

        let resolved = first_available_localized_values(localized_by_locale);

        assert_eq!(resolved, Some(json!({"bio": "Hello"})));
    }

    #[tokio::test]
    async fn delete_attached_localized_values_is_noop_when_storage_is_absent() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite in-memory db");

        let deleted =
            delete_attached_localized_values(&db, Uuid::new_v4(), "product", Uuid::new_v4())
                .await
                .expect("missing storage should be tolerated");

        assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn delete_attached_localized_values_removes_only_requested_owner_rows() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite in-memory db");

        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            r#"
            CREATE TABLE flex_attached_localized_values (
                id TEXT PRIMARY KEY NOT NULL,
                tenant_id TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                field_key TEXT NOT NULL,
                locale TEXT NOT NULL,
                value TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#
            .to_string(),
        ))
        .await
        .expect("table should be created");

        let tenant_id = Uuid::new_v4();
        let entity_id = Uuid::new_v4();
        let other_entity_id = Uuid::new_v4();

        for (owner_id, field_key) in [(entity_id, "bio"), (other_entity_id, "tagline")] {
            ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                entity_type: Set("product".to_string()),
                entity_id: Set(owner_id),
                field_key: Set(field_key.to_string()),
                locale: Set("en".to_string()),
                value: Set(json!(field_key)),
                created_at: sea_orm::ActiveValue::NotSet,
                updated_at: sea_orm::ActiveValue::NotSet,
            }
            .insert(&db)
            .await
            .expect("row should insert");
        }

        let deleted = delete_attached_localized_values(&db, tenant_id, "product", entity_id)
            .await
            .expect("owner rows should delete");

        assert_eq!(deleted, 1);

        let remaining = Entity::find().all(&db).await.expect("rows should load");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].entity_id, other_entity_id);
    }
}
