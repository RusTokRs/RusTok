use rust_decimal::prelude::ToPrimitive;
use rustok_api::{PLATFORM_FALLBACK_LOCALE, locale_tags_match, normalize_locale_tag};
use rustok_commerce_foundation::dto::{
    ProductOptionTranslationInput, ProductOptionTranslationResponse,
    ProductOptionTranslationResponse as ProductOptionTranslationResponseDto, ProductResponse,
    ProductTranslationInput, ProductTranslationResponse,
};
use rustok_commerce_foundation::entities;
use rustok_commerce_foundation::error::{CommerceError, CommerceResult};
use rustok_core::field_schema::{CustomFieldsSchema, FieldDefinition, FieldType, ValidationRule};
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    QueryFilter, QueryOrder, Value as SqlValue, sea_query::Expr,
};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap, HashSet};
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

pub mod product_field_definitions_storage {
    rustok_core::define_field_definitions_entity!("product_field_definitions");
}

pub fn map_flex_cleanup_error(error: rustok_core::field_schema::FlexError) -> CommerceError {
    match error {
        rustok_core::field_schema::FlexError::Database(message) => {
            CommerceError::Database(sea_orm::DbErr::Custom(message))
        }
        other => CommerceError::Validation(other.to_string()),
    }
}

pub fn normalize_seller_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_owned())
}

pub fn slugify(text: &str) -> String {
    const MAX_LENGTH: usize = 255;
    const RESERVED_NAMES: &[&str] = &["admin", "api", "null", "undefined", "new", "edit", "delete"];

    let normalized: String = text.nfc().collect();

    let slug: String = normalized
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == ' ' || *c == '_')
        .map(|c| if c == ' ' || c == '_' { '-' } else { c })
        .collect();

    let slug = slug
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    let slug: String = slug.chars().take(MAX_LENGTH).collect();

    let slug = if RESERVED_NAMES.contains(&slug.as_str()) {
        format!("{}-1", slug)
    } else {
        slug
    };

    if slug.is_empty() {
        "untitled".to_string()
    } else {
        slug
    }
}

pub fn generate_variant_title(variant: &entities::product_variant::Model) -> String {
    generate_variant_title_from_inputs(
        variant.option1.as_deref(),
        variant.option2.as_deref(),
        variant.option3.as_deref(),
    )
}

pub fn generate_variant_title_from_inputs(
    option1: Option<&str>,
    option2: Option<&str>,
    option3: Option<&str>,
) -> String {
    let options: Vec<&str> = [option1, option2, option3].into_iter().flatten().collect();

    if options.is_empty() {
        "Default".to_string()
    } else {
        options.join(" / ")
    }
}

pub fn decimal_to_cents(amount: rust_decimal::Decimal) -> Option<i64> {
    (amount * rust_decimal::Decimal::from(100))
        .round_dp(0)
        .to_i64()
}

pub fn preferred_product_locale_from_translations(
    translations: &[ProductTranslationInput],
) -> String {
    translations
        .iter()
        .find_map(|translation| {
            let trimmed = translation.locale.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string())
}

pub fn preferred_product_locale_from_metadata(metadata: &Value) -> String {
    metadata
        .get("locale")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string())
}

pub fn collect_translation_locales(translations: &[ProductTranslationInput]) -> Vec<String> {
    let mut locales = Vec::new();
    for translation in translations {
        if !locales.iter().any(|locale| locale == &translation.locale) {
            locales.push(translation.locale.clone());
        }
    }
    locales
}

pub fn normalize_tag_names(tag_names: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for tag_name in tag_names {
        let trimmed = tag_name.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed.to_ascii_lowercase();
        if seen.insert(key) {
            normalized.push(trimmed.to_string());
        }
    }
    normalized
}

pub fn metadata_has_tags_field(metadata: &Value) -> bool {
    metadata
        .as_object()
        .map(|object| object.contains_key("tags"))
        .unwrap_or(false)
}

pub fn extract_metadata_tags(metadata: &Value) -> Vec<String> {
    metadata
        .get("tags")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

pub fn strip_metadata_tags(mut metadata: Value) -> Value {
    if let Some(object) = metadata.as_object_mut() {
        object.remove("tags");
    }
    metadata
}

pub fn normalize_metadata_tag_state(input_tags: &[String], metadata: &Value) -> Vec<String> {
    let normalized_input_tags = normalize_tag_names(input_tags);
    if !normalized_input_tags.is_empty() || !metadata_has_tags_field(metadata) {
        return normalized_input_tags;
    }

    normalize_tag_names(&extract_metadata_tags(metadata))
}

pub fn normalize_shipping_profile_slug(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

pub fn extract_shipping_profile_slug(metadata: &Value) -> Option<String> {
    metadata
        .get("shipping_profile")
        .and_then(|profile| profile.get("slug"))
        .and_then(Value::as_str)
        .and_then(normalize_shipping_profile_slug)
        .or_else(|| {
            metadata
                .get("shipping_profile_slug")
                .and_then(Value::as_str)
                .and_then(normalize_shipping_profile_slug)
        })
}

pub fn apply_shipping_profile_to_metadata(
    mut metadata: Value,
    shipping_profile_slug: Option<String>,
) -> Value {
    let Some(normalized_slug) =
        shipping_profile_slug.and_then(|value| normalize_shipping_profile_slug(&value))
    else {
        return metadata;
    };
    if !metadata.is_object() {
        metadata = Value::Object(Default::default());
    }

    if let Some(object) = metadata.as_object_mut() {
        object.remove("shipping_profile_slug");
        object.insert(
            "shipping_profile".to_string(),
            serde_json::json!({ "slug": normalized_slug }),
        );
    }

    metadata
}

pub fn normalize_create_product_metadata(
    input_tags: Vec<String>,
    shipping_profile_slug: Option<String>,
    metadata: Value,
) -> (Value, Option<Vec<String>>) {
    let normalized_tags = normalize_metadata_tag_state(&input_tags, &metadata);
    let metadata =
        apply_shipping_profile_to_metadata(strip_metadata_tags(metadata), shipping_profile_slug);

    (metadata, Some(normalized_tags))
}

pub fn normalize_update_product_metadata(
    input_tags: Option<Vec<String>>,
    shipping_profile_slug: Option<String>,
    metadata: Option<Value>,
    existing_metadata: Value,
) -> Option<(Value, Option<Vec<String>>)> {
    match (input_tags, shipping_profile_slug, metadata) {
        (Some(tags), profile_slug, metadata) => {
            let normalized_tags = normalize_tag_names(&tags);
            let metadata = metadata.unwrap_or(existing_metadata);
            Some((
                apply_shipping_profile_to_metadata(strip_metadata_tags(metadata), profile_slug),
                Some(normalized_tags),
            ))
        }
        (None, profile_slug, Some(metadata)) => {
            let normalized_tags = metadata_has_tags_field(&metadata)
                .then(|| normalize_tag_names(&extract_metadata_tags(&metadata)));
            Some((
                apply_shipping_profile_to_metadata(strip_metadata_tags(metadata), profile_slug),
                normalized_tags,
            ))
        }
        (None, Some(profile_slug), None) => Some((
            apply_shipping_profile_to_metadata(
                strip_metadata_tags(existing_metadata),
                Some(profile_slug),
            ),
            None,
        )),
        (None, None, None) => None,
    }
}

pub fn build_option_translations(
    translations: Vec<entities::product_option_translation::Model>,
    option_values: Vec<entities::product_option_value::Model>,
    option_value_translations_by_value: &HashMap<
        Uuid,
        Vec<entities::product_option_value_translation::Model>,
    >,
) -> Vec<ProductOptionTranslationResponse> {
    translations
        .into_iter()
        .map(|translation| {
            let values = option_values
                .iter()
                .map(|value| {
                    option_value_translations_by_value
                        .get(&value.id)
                        .and_then(|items| {
                            items
                                .iter()
                                .find(|item| locale_tags_match(&item.locale, &translation.locale))
                                .map(|item| item.value.clone())
                        })
                        .or_else(|| {
                            option_value_translations_by_value
                                .get(&value.id)
                                .and_then(|items| items.first())
                                .map(|item| item.value.clone())
                        })
                        .unwrap_or_default()
                })
                .collect();

            ProductOptionTranslationResponse {
                locale: translation.locale,
                name: translation.title,
                values,
            }
        })
        .collect()
}

pub fn expand_option_translations_for_product_locales(
    mut translations: Vec<ProductOptionTranslationInput>,
    product_locales: &[String],
) -> Vec<ProductOptionTranslationInput> {
    let Some(fallback) = translations.first().cloned() else {
        return translations;
    };

    for locale in product_locales {
        if translations
            .iter()
            .any(|translation| locale_tags_match(&translation.locale, locale))
        {
            continue;
        }

        translations.push(ProductOptionTranslationInput {
            locale: locale.clone(),
            name: fallback.name.clone(),
            values: fallback.values.clone(),
        });
    }

    translations
}

pub fn normalize_option_translations(
    translations: &[ProductOptionTranslationInput],
) -> CommerceResult<Vec<ProductOptionTranslationInput>> {
    if translations.is_empty() {
        return Err(CommerceError::Validation(
            "At least one option translation is required".into(),
        ));
    }

    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(translations.len());
    for translation in translations {
        let locale = normalize_locale_tag(&translation.locale).ok_or_else(|| {
            CommerceError::Validation("Invalid locale for option translation".into())
        })?;
        if !seen.insert(locale.clone()) {
            return Err(CommerceError::Validation(
                "Duplicate locale in option translations".into(),
            ));
        }
        let name = translation.name.trim();
        if name.is_empty() {
            return Err(CommerceError::Validation(
                "Option name cannot be empty".into(),
            ));
        }
        if translation.values.is_empty() {
            return Err(CommerceError::Validation(
                "Option values cannot be empty".into(),
            ));
        }
        normalized.push(ProductOptionTranslationInput {
            locale,
            name: name.to_string(),
            values: translation
                .values
                .iter()
                .map(|value| value.trim().to_string())
                .collect(),
        });
    }
    Ok(normalized)
}

pub fn ensure_option_values_consistent(
    translations: &[ProductOptionTranslationInput],
    base_values: &[String],
) -> CommerceResult<()> {
    for translation in translations {
        if translation.values.len() != base_values.len() {
            return Err(CommerceError::Validation(
                "Option value count must be consistent across translations".into(),
            ));
        }
    }
    Ok(())
}

pub fn resolve_option_display(
    translations: &[ProductOptionTranslationResponseDto],
    requested_locale: &str,
    fallback_locale: Option<&str>,
) -> (String, Vec<String>) {
    let requested = normalize_locale_tag(requested_locale);
    let fallback = fallback_locale.and_then(normalize_locale_tag);

    let resolved = requested
        .as_deref()
        .and_then(|locale| {
            translations.iter().find(|translation| {
                normalize_locale_tag(&translation.locale).as_deref() == Some(locale)
            })
        })
        .or_else(|| {
            fallback.as_deref().and_then(|locale| {
                translations.iter().find(|translation| {
                    normalize_locale_tag(&translation.locale).as_deref() == Some(locale)
                })
            })
        })
        .or_else(|| translations.first());

    resolved
        .map(|translation| (translation.name.clone(), translation.values.clone()))
        .unwrap_or_else(|| ("".to_string(), Vec::new()))
}

pub async fn load_product_custom_fields_schema<C>(
    db: &C,
    tenant_id: Uuid,
) -> CommerceResult<CustomFieldsSchema>
where
    C: ConnectionTrait,
{
    let rows = product_field_definitions_storage::Entity::find()
        .filter(product_field_definitions_storage::Column::TenantId.eq(tenant_id))
        .filter(product_field_definitions_storage::Column::IsActive.eq(true))
        .order_by_asc(product_field_definitions_storage::Column::Position)
        .all(db)
        .await
        .map_err(CommerceError::from)?;

    let definitions = rows
        .into_iter()
        .filter_map(product_field_definition_from_row)
        .collect();

    Ok(CustomFieldsSchema::new(definitions))
}

pub fn product_field_definition_from_row(
    row: product_field_definitions_storage::Model,
) -> Option<FieldDefinition> {
    let field_type: FieldType =
        serde_json::from_value(serde_json::Value::String(row.field_type.clone())).ok()?;
    let label = serde_json::from_value(row.label).unwrap_or_default();
    let description = row
        .description
        .and_then(|value| serde_json::from_value(value).ok());
    let validation: Option<ValidationRule> = row
        .validation
        .and_then(|value| serde_json::from_value(value).ok());

    Some(FieldDefinition {
        field_key: row.field_key,
        field_type,
        label,
        description,
        is_localized: row.is_localized,
        is_required: row.is_required,
        default_value: row.default_value,
        validation,
        position: row.position,
        is_active: row.is_active,
    })
}

pub fn split_product_metadata_payload(
    schema: &CustomFieldsSchema,
    metadata: &Value,
) -> (
    serde_json::Map<String, Value>,
    serde_json::Map<String, Value>,
) {
    let known_keys = schema
        .active_definitions()
        .into_iter()
        .map(|definition| definition.field_key.as_str())
        .collect::<HashSet<_>>();
    let mut reserved = serde_json::Map::new();
    let mut custom_fields = serde_json::Map::new();

    for (key, value) in metadata.as_object().cloned().unwrap_or_default() {
        if known_keys.contains(key.as_str()) {
            custom_fields.insert(key, value);
        } else {
            reserved.insert(key, value);
        }
    }

    (reserved, custom_fields)
}

pub fn merge_product_metadata_patch(
    mut existing: serde_json::Map<String, Value>,
    patch: serde_json::Map<String, Value>,
) -> serde_json::Map<String, Value> {
    for (key, value) in patch {
        existing.insert(key, value);
    }

    existing
}

pub fn merge_reserved_product_metadata(
    mut reserved: serde_json::Map<String, Value>,
    custom_fields: Option<Value>,
) -> Value {
    if let Some(custom_fields) = custom_fields.and_then(|value| value.as_object().cloned()) {
        for (key, value) in custom_fields {
            reserved.insert(key, value);
        }
    }

    Value::Object(reserved)
}

pub fn pick_product_translation<'a>(
    translations: &'a [entities::product_translation::Model],
    locale: &str,
    fallback_locale: &str,
) -> Option<&'a entities::product_translation::Model> {
    translations
        .iter()
        .find(|translation| locale_tags_match(&translation.locale, locale))
        .or_else(|| {
            (!locale_tags_match(fallback_locale, locale)).then(|| {
                translations
                    .iter()
                    .find(|translation| locale_tags_match(&translation.locale, fallback_locale))
            })?
        })
        .or_else(|| translations.first())
}

pub fn pick_response_translation<'a>(
    translations: &'a [ProductTranslationResponse],
    locale: &str,
    fallback_locale: &str,
) -> Option<&'a ProductTranslationResponse> {
    translations
        .iter()
        .find(|translation| locale_tags_match(&translation.locale, locale))
        .or_else(|| {
            (!locale_tags_match(fallback_locale, locale)).then(|| {
                translations
                    .iter()
                    .find(|translation| locale_tags_match(&translation.locale, fallback_locale))
            })?
        })
        .or_else(|| translations.first())
}

pub fn localize_product_response(
    mut product: ProductResponse,
    locale: &str,
    fallback_locale: &str,
) -> ProductResponse {
    let selected_translation =
        pick_response_translation(product.translations.as_slice(), locale, fallback_locale)
            .cloned()
            .into_iter()
            .collect::<Vec<_>>();

    if !selected_translation.is_empty() {
        product.translations = selected_translation;
    }

    product
}

pub async fn prepare_product_custom_fields_for_create<C>(
    conn: &C,
    tenant_id: Uuid,
    locale: &str,
    payload: Value,
) -> CommerceResult<flex::PreparedAttachedValuesWrite>
where
    C: ConnectionTrait,
{
    let schema = load_product_custom_fields_schema(conn, tenant_id).await?;
    let (reserved_payload, flex_payload) = split_product_metadata_payload(&schema, &payload);
    flex::prepare_attached_values_create(schema, Some(Value::Object(flex_payload)), locale)
        .map(|mut prepared| {
            prepared.metadata = Some(merge_reserved_product_metadata(
                reserved_payload,
                prepared.metadata,
            ));
            prepared
        })
        .map_err(|error| CommerceError::Validation(error.to_string()))
}

pub async fn prepare_product_custom_fields_for_update<C>(
    conn: &C,
    tenant_id: Uuid,
    product_id: Uuid,
    locale: &str,
    existing_metadata: &Value,
    payload: Value,
) -> CommerceResult<flex::PreparedAttachedValuesWrite>
where
    C: ConnectionTrait,
{
    let schema = load_product_custom_fields_schema(conn, tenant_id).await?;
    let (reserved_patch, flex_payload) = split_product_metadata_payload(&schema, &payload);
    let (existing_reserved_metadata, existing_flex_metadata) =
        split_product_metadata_payload(&schema, existing_metadata);
    let reserved_payload = merge_product_metadata_patch(existing_reserved_metadata, reserved_patch);
    flex::prepare_attached_values_update(
        conn,
        flex::AttachedEntityRef {
            tenant_id,
            entity_type: "product",
            entity_id: product_id,
        },
        schema,
        locale,
        &Value::Object(existing_flex_metadata),
        Some(Value::Object(flex_payload)),
    )
    .await
    .map(|mut prepared| {
        prepared.metadata = Some(merge_reserved_product_metadata(
            reserved_payload,
            prepared.metadata,
        ));
        prepared
    })
    .map_err(|error| CommerceError::Validation(error.to_string()))
}

pub async fn resolve_product_metadata<C>(
    conn: &C,
    tenant_id: Uuid,
    product_id: Uuid,
    metadata: &Value,
    locale: &str,
    fallback_locale: &str,
) -> CommerceResult<Value>
where
    C: ConnectionTrait,
{
    let shared_metadata = strip_metadata_tags(metadata.clone());
    let schema = load_product_custom_fields_schema(conn, tenant_id).await?;
    flex::resolve_attached_payload(
        conn,
        flex::AttachedEntityRef {
            tenant_id,
            entity_type: "product",
            entity_id: product_id,
        },
        schema,
        &shared_metadata,
        locale,
        fallback_locale,
    )
    .await
    .map(|payload| payload.unwrap_or_else(|| serde_json::json!({})))
    .map_err(|error| CommerceError::Validation(error.to_string()))
}

pub fn normalize_public_channel_slug(channel_slug: Option<&str>) -> Option<String> {
    channel_slug
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(|slug| slug.to_ascii_lowercase())
}

pub fn extract_allowed_channel_slugs(metadata: &Value) -> Vec<String> {
    let Some(values) = metadata
        .as_object()
        .and_then(|object| object.get("channel_visibility"))
        .and_then(|value| value.as_object())
        .and_then(|object| object.get("allowed_channel_slugs"))
        .and_then(|value| value.as_array())
    else {
        return Vec::new();
    };

    let mut normalized = BTreeSet::new();
    for value in values {
        if let Some(slug) = value
            .as_str()
            .and_then(|value| normalize_public_channel_slug(Some(value)))
        {
            normalized.insert(slug);
        }
    }

    normalized.into_iter().collect()
}

pub fn is_allowlist_visible_for_public_channel(
    allowed_channel_slugs: &[String],
    public_channel_slug: Option<&str>,
) -> bool {
    if allowed_channel_slugs.is_empty() {
        return true;
    }

    let Some(public_channel_slug) = normalize_public_channel_slug(public_channel_slug) else {
        return false;
    };

    allowed_channel_slugs
        .iter()
        .any(|slug| slug == &public_channel_slug)
}

pub fn is_metadata_visible_for_public_channel(
    metadata: &Value,
    public_channel_slug: Option<&str>,
) -> bool {
    let allowed_channel_slugs = extract_allowed_channel_slugs(metadata);
    is_allowlist_visible_for_public_channel(&allowed_channel_slugs, public_channel_slug)
}

pub fn product_channel_visibility_condition(
    backend: DbBackend,
    public_channel_slug: Option<&str>,
) -> Condition {
    if backend == DbBackend::Sqlite {
        return match normalize_public_channel_slug(public_channel_slug) {
            None => Condition::all().add(Expr::cust(
                "COALESCE(json_extract(metadata, '$.channel_visibility.allowed_channel_slugs'), '[]') = '[]'",
            )),
            Some(slug) => Condition::all().add(Expr::cust_with_values(
                "COALESCE(json_extract(metadata, '$.channel_visibility.allowed_channel_slugs'), '[]') = '[]'
                 OR EXISTS (
                     SELECT 1
                     FROM json_each(COALESCE(json_extract(metadata, '$.channel_visibility.allowed_channel_slugs'), '[]'))
                     WHERE value = ?
                 )",
                vec![SqlValue::from(slug)],
            )),
        };
    }

    match normalize_public_channel_slug(public_channel_slug) {
        None => Condition::all().add(Expr::cust(
            "COALESCE(metadata #> '{channel_visibility,allowed_channel_slugs}', '[]'::jsonb) = '[]'::jsonb",
        )),
        Some(slug) => {
            Condition::all().add(Expr::cust_with_values(
                "COALESCE(metadata #> '{channel_visibility,allowed_channel_slugs}', '[]'::jsonb) = '[]'::jsonb
                 OR metadata @> jsonb_build_object(
                     'channel_visibility',
                     jsonb_build_object('allowed_channel_slugs', jsonb_build_array($1::text))
                 )",
                vec![SqlValue::from(slug)],
            ))
        }
    }
}

pub async fn find_published_product_id_by_handle(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    handle: &str,
    locale: &str,
    fallback_locale: &str,
    public_channel_slug: Option<&str>,
) -> CommerceResult<Option<Uuid>> {
    if let Some(product_id) =
        find_published_product_id_for_locale(db, tenant_id, handle, locale, public_channel_slug)
            .await?
    {
        return Ok(Some(product_id));
    }

    if !locale_tags_match(fallback_locale, locale) {
        if let Some(product_id) = find_published_product_id_for_locale(
            db,
            tenant_id,
            handle,
            fallback_locale,
            public_channel_slug,
        )
        .await?
        {
            return Ok(Some(product_id));
        }
    }

    find_published_product_id_any_locale(db, tenant_id, handle, public_channel_slug).await
}

pub async fn find_published_product_id_for_locale(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    handle: &str,
    locale: &str,
    public_channel_slug: Option<&str>,
) -> CommerceResult<Option<Uuid>> {
    let translations = entities::product_translation::Entity::find()
        .filter(entities::product_translation::Column::Handle.eq(handle))
        .all(db)
        .await?
        .into_iter()
        .filter(|translation| locale_tags_match(&translation.locale, locale))
        .collect();

    find_first_published_product(db, tenant_id, translations, public_channel_slug).await
}

pub async fn find_published_product_id_any_locale(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    handle: &str,
    public_channel_slug: Option<&str>,
) -> CommerceResult<Option<Uuid>> {
    let translations = entities::product_translation::Entity::find()
        .filter(entities::product_translation::Column::Handle.eq(handle))
        .all(db)
        .await?;

    find_first_published_product(db, tenant_id, translations, public_channel_slug).await
}

pub async fn find_first_published_product(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    translations: Vec<entities::product_translation::Model>,
    public_channel_slug: Option<&str>,
) -> CommerceResult<Option<Uuid>> {
    for translation in translations {
        let product = entities::product::Entity::find_by_id(translation.product_id)
            .filter(entities::product::Column::TenantId.eq(tenant_id))
            .filter(entities::product::Column::Status.eq(entities::product::ProductStatus::Active))
            .filter(entities::product::Column::PublishedAt.is_not_null())
            .one(db)
            .await?;

        if product.as_ref().is_some_and(|product| {
            is_metadata_visible_for_public_channel(&product.metadata, public_channel_slug)
        }) {
            return Ok(Some(translation.product_id));
        }
    }

    Ok(None)
}

pub async fn resolve_tag_locale_for_update<C>(
    conn: &C,
    product_id: Uuid,
    translations: Option<&[ProductTranslationInput]>,
) -> CommerceResult<String>
where
    C: ConnectionTrait,
{
    if let Some(translations) = translations {
        if !translations.is_empty() {
            return Ok(preferred_product_locale_from_translations(translations));
        }
    }

    let existing = entities::product_translation::Entity::find()
        .filter(entities::product_translation::Column::ProductId.eq(product_id))
        .all(conn)
        .await?;

    if let Some(translation) = existing.first() {
        return Ok(translation.locale.clone());
    }

    Ok(PLATFORM_FALLBACK_LOCALE.to_string())
}

#[cfg(test)]
mod product_metadata_tests {
    use super::{
        merge_product_metadata_patch, merge_reserved_product_metadata,
        split_product_metadata_payload,
    };
    use rustok_core::field_schema::{CustomFieldsSchema, FieldDefinition, FieldType};
    use serde_json::json;
    use std::collections::HashMap;

    fn definition(field_key: &str) -> FieldDefinition {
        FieldDefinition {
            field_key: field_key.to_string(),
            field_type: FieldType::Text,
            label: HashMap::from([("en".to_string(), field_key.to_string())]),
            description: None,
            is_localized: false,
            is_required: false,
            default_value: None,
            validation: None,
            position: 0,
            is_active: true,
        }
    }

    #[test]
    fn split_product_metadata_payload_routes_only_known_flex_keys() {
        let schema = CustomFieldsSchema::new(vec![definition("fit"), definition("material")]);

        let (reserved, flex) = split_product_metadata_payload(
            &schema,
            &json!({
                "fit": "regular",
                "material": "linen",
                "source": "erp",
                "shipping_profile": { "slug": "standard" }
            }),
        );

        assert_eq!(reserved.get("source"), Some(&json!("erp")));
        assert_eq!(
            reserved.get("shipping_profile"),
            Some(&json!({ "slug": "standard" }))
        );
        assert_eq!(flex.get("fit"), Some(&json!("regular")));
        assert_eq!(flex.get("material"), Some(&json!("linen")));
    }

    #[test]
    fn merge_product_metadata_patch_preserves_reserved_existing_keys() {
        let existing = json!({
            "source": "erp",
            "shipping_profile": { "slug": "standard" }
        })
        .as_object()
        .cloned()
        .expect("existing object");
        let patch = json!({ "source": "manual" })
            .as_object()
            .cloned()
            .expect("patch object");

        let merged = merge_product_metadata_patch(existing, patch);

        assert_eq!(merged.get("source"), Some(&json!("manual")));
        assert_eq!(
            merged.get("shipping_profile"),
            Some(&json!({ "slug": "standard" }))
        );
    }

    #[test]
    fn merge_reserved_product_metadata_keeps_reserved_and_writes_shared_flex_values() {
        let reserved = json!({ "source": "erp" })
            .as_object()
            .cloned()
            .expect("reserved object");

        let merged = merge_reserved_product_metadata(
            reserved,
            Some(json!({ "fit": "regular", "material": "linen" })),
        );

        assert_eq!(
            merged,
            json!({
                "source": "erp",
                "fit": "regular",
                "material": "linen"
            })
        );
    }
}
