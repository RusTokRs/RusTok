use crate::dto::{
    ProductResponse, ProductTranslationInput, ProductTranslationResponse, VariantAxisValueResponse,
    VariantResponse,
};
use crate::entities;
use crate::error::{CommerceError, CommerceResult};
use crate::ports::{AsTaxonomyConnection, ProductTaxonomyPort};
use crate::services::catalog_schema_service::effective_forms::load_taxonomy_category_parent_map;
use rustok_api::{PLATFORM_FALLBACK_LOCALE, locale_tags_match};
use std::collections::{HashMap, HashSet, VecDeque};
use rustok_core::field_schema::{
    CustomFieldsSchema, FieldDefinition, attached_values as field_values,
};

use super::attached_localized_storage;
use crate::services::flex_schema_projection::ProductFlexSchemaSource;
/// A literal substring search across SQLite and PostgreSQL. `LIKE` interprets
/// percent, underscore and the escape character unless escaped explicitly.
pub fn product_title_substring_like_pattern(search: &str) -> String {
    let mut pattern = String::from("%");
    for character in search.chars() {
        if matches!(character, '%' | '_' | '!') {
            pattern.push('!');
        }
        pattern.push(character);
    }
    pattern.push('%');
    pattern
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

    let mut normalized = std::collections::BTreeSet::new();
    for value in values {
        if let Some(raw_slug) = value.as_str()
            && let Some(slug) = normalize_public_channel_slug(Some(raw_slug))
                .filter(|slug| slug.as_str() == raw_slug)
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
    let explicit_allowlist = metadata
        .as_object()
        .and_then(|object| object.get("channel_visibility"))
        .and_then(Value::as_object)
        .and_then(|object| object.get("allowed_channel_slugs"));
    let malformed_nonempty_allowlist = match explicit_allowlist {
        Some(Value::Array(values)) => !values.is_empty() && allowed_channel_slugs.is_empty(),
        Some(_) => true,
        None => false,
    };

    // A malformed non-empty allowlist must not collapse to the empty-list
    // "visible everywhere" default.
    if malformed_nonempty_allowlist {
        return false;
    }

    is_allowlist_visible_for_public_channel(&allowed_channel_slugs, public_channel_slug)
}
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    FromQueryResult, QueryFilter, QueryOrder, Statement, Value as SqlValue, sea_query::Expr,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

pub mod product_field_definitions_storage {
    rustok_core::define_field_definitions_entity!("product_field_definitions");
}

pub fn normalize_seller_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_owned())
}

/// Canonical Product SKU representation shared by every Product write path.
/// Empty/whitespace-only values mean that the variant has no SKU.
pub fn normalize_variant_sku(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

pub fn slugify(text: &str) -> String {
    const MAX_LENGTH: usize = 255;
    const RESERVED_NAMES: &[&str] = &["admin", "api", "null", "undefined", "new", "edit", "delete"];

    let normalized: String = text.nfc().collect();

    let slug: String = normalized
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == ' ' || *c == '_')
        .map(|c| if c == ' ' || c == '_' { '-' } else { c })
        .collect();

    let slug = slug
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    let slug: String = slug.chars().take(MAX_LENGTH).collect();
    let slug = slug.trim_matches('-').to_owned();

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

/// Resolves a Product handle. An omitted or blank form value means "generate from the
/// localized title"; a nonblank authored value is preserved after trimming.
pub fn product_translation_handle(handle: Option<&str>, title: &str) -> String {
    handle
        .map(str::trim)
        .filter(|handle| !handle.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| slugify(title))
}

pub fn generate_variant_title_from_axis_values(values: &[&str]) -> String {
    if values.is_empty() {
        "Default".to_string()
    } else {
        values.join(" / ")
    }
}

/// Display title of one variant: authored Translation copy first, then the readable
/// projection of its axis assignments.
///
/// The machine `combination_identity` (pairs of UUIDs) is never shown to a person:
/// an authored title wins, otherwise the localized option labels of the sparse
/// tuple are joined (`"Red / M"`), and a variant without assignments is `Default`.
pub fn variant_display_title(
    translations: &[entities::variant_translation::Model],
    axis_values: &[VariantAxisValueResponse],
    locale: &str,
    fallback_locale: &str,
) -> String {
    let authored = translations
        .iter()
        .find(|translation| {
            locale_tags_match(&translation.locale, locale)
                && translation
                    .title
                    .as_deref()
                    .is_some_and(|title| !title.trim().is_empty())
        })
        .or_else(|| {
            translations.iter().find(|translation| {
                locale_tags_match(&translation.locale, fallback_locale)
                    && translation
                        .title
                        .as_deref()
                        .is_some_and(|title| !title.trim().is_empty())
            })
        })
        .and_then(|translation| translation.title.clone());
    if let Some(title) = authored {
        return title;
    }
    let labels = axis_values
        .iter()
        .map(|value| {
            value
                .label
                .clone()
                .filter(|label| !label.trim().is_empty())
                .unwrap_or_else(|| value.option_id.to_string())
        })
        .collect::<Vec<_>>();
    let references = labels.iter().map(String::as_str).collect::<Vec<_>>();
    generate_variant_title_from_axis_values(references.as_slice())
}

/// Bound for one category filter's subtree resolution: lists and facets refuse a
/// filter whose live subtree exceeds this size instead of silently truncating it.
pub const MAX_CATEGORY_FILTER_SUBTREE: usize = 1024;
const MAX_CATEGORY_FILTER_OWNER_ROWS: usize = 10_000;

#[derive(Debug, sea_orm::FromQueryResult)]
struct CategoryFilterSubtreeRow {
    id: Uuid,
    kind: String,
    is_active: bool,
}

/// Resolves a category filter from Product lifecycle/kind state and Taxonomy-owned ancestry.
///
/// Product keeps lifecycle and category kind; Taxonomy alone supplies parent links. Deleted
/// Product rows are included in the owner read so an active child of a deleted parent is
/// not silently promoted to a root, but traversal only follows active structural nodes.
pub async fn load_category_filter_subtree<C>(
    conn: &C,
    taxonomy_port: &dyn ProductTaxonomyPort,
    tenant_id: Uuid,
    category_id: Uuid,
) -> CommerceResult<Vec<Uuid>>
where
    C: ConnectionTrait + AsTaxonomyConnection,
{
    let backend = conn.get_database_backend();
    let rows = CategoryFilterSubtreeRow::find_by_statement(Statement::from_sql_and_values(
        backend,
        format!(
            "SELECT id, kind, (deleted_at IS NULL) AS is_active FROM catalog_categories WHERE tenant_id = {tenant} ORDER BY id ASC LIMIT {limit}",
            tenant = sql_placeholder(backend, 1),
            limit = MAX_CATEGORY_FILTER_OWNER_ROWS + 1,
        ),
        vec![tenant_id.into()],
    ))
    .all(conn)
    .await?;
    if rows.len() > MAX_CATEGORY_FILTER_OWNER_ROWS {
        return Err(CommerceError::Validation(format!(
            "category owner projection exceeds its bound of {MAX_CATEGORY_FILTER_OWNER_ROWS} rows"
        )));
    }
    if rows.is_empty()
        || !rows.iter().any(|row| {
            row.id == category_id && row.is_active && row.kind == "structural"
        })
    {
        return Ok(Vec::new());
    }

    let category_ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
    let parent_map = load_taxonomy_category_parent_map(
        conn,
        taxonomy_port,
        tenant_id,
        &category_ids,
        PLATFORM_FALLBACK_LOCALE,
        Some(PLATFORM_FALLBACK_LOCALE),
    )
    .await?;

    category_filter_subtree_from_taxonomy(category_id, rows, &parent_map)
}

/// Applies the storefront/admin subtree policy after the strict Taxonomy owner read.
/// Deleted or non-structural nodes are branch boundaries, matching the former recursive walk.
fn category_filter_subtree_from_taxonomy(
    root_id: Uuid,
    categories: Vec<CategoryFilterSubtreeRow>,
    parent_map: &HashMap<Uuid, Option<Uuid>>,
) -> CommerceResult<Vec<Uuid>> {
    let mut kinds = HashMap::with_capacity(categories.len());
    let mut active = HashMap::with_capacity(categories.len());
    for category in categories {
        if kinds.insert(category.id, category.kind).is_some() {
            return Err(crate::services::catalog_schema_service::taxonomy_projection_error(
                "Product category filter contains duplicate category identities",
            ));
        }
        active.insert(category.id, category.is_active);
    }
    if kinds.len() != parent_map.len()
        || kinds.keys().any(|category_id| !parent_map.contains_key(category_id))
    {
        return Err(crate::services::catalog_schema_service::taxonomy_projection_error(
            "Product category filter and Taxonomy hierarchy projections do not match",
        ));
    }
    if !active.get(&root_id).copied().unwrap_or(false)
        || kinds.get(&root_id).map(String::as_str) != Some("structural")
    {
        return Ok(Vec::new());
    }

    let mut children_by_parent = HashMap::<Uuid, Vec<Uuid>>::new();
    for (category_id, kind) in &kinds {
        if kind != "structural" || !active.get(category_id).copied().unwrap_or(false) {
            continue;
        }
        let parent_id = *parent_map.get(category_id).ok_or_else(|| {
            crate::services::catalog_schema_service::taxonomy_projection_error(
                "Product category is missing its Taxonomy parent projection",
            )
        })?;
        if let Some(parent_id) = parent_id {
            if active.get(&parent_id).copied().unwrap_or(false)
                && kinds.get(&parent_id).map(String::as_str) == Some("structural")
            {
                children_by_parent
                    .entry(parent_id)
                    .or_default()
                    .push(*category_id);
            }
        }
    }
    for children in children_by_parent.values_mut() {
        children.sort_unstable();
    }

    let mut pending = VecDeque::new();
    pending.push_back(root_id);
    let mut visited = HashSet::new();
    let mut subtree = Vec::new();
    while let Some(category_id) = pending.pop_front() {
        if !visited.insert(category_id) {
            return Err(crate::services::catalog_schema_service::taxonomy_projection_error(
                "Product category filter encountered a cyclic Taxonomy subtree",
            ));
        }
        subtree.push(category_id);
        if subtree.len() > MAX_CATEGORY_FILTER_SUBTREE {
            return Err(CommerceError::Validation(format!(
                "category filter subtree exceeds its bound of {MAX_CATEGORY_FILTER_SUBTREE} live categories"
            )));
        }
        if let Some(children) = children_by_parent.get(&category_id) {
            pending.extend(children.iter().copied());
        }
    }
    Ok(subtree)
}

pub fn format_product_media_url(media_id: Uuid) -> String {
    format!("/api/v1/media/{media_id}")
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

fn reject_reserved_tag_metadata(metadata: &Value) -> CommerceResult<()> {
    if metadata
        .as_object()
        .is_some_and(|object| object.contains_key("tags"))
    {
        return Err(CommerceError::Validation(
            "product metadata key `tags` is reserved; use the typed tags field".to_owned(),
        ));
    }
    Ok(())
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
    if let Some(slug_raw) = shipping_profile_slug {
        if let Some(normalized_slug) = normalize_shipping_profile_slug(&slug_raw) {
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
        } else if let Some(object) = metadata.as_object_mut() {
            object.remove("shipping_profile_slug");
            object.remove("shipping_profile");
        }
    }

    metadata
}

/// Removes the legacy metadata shadow of the typed shipping-profile binding.
///
/// Used when an operator clears `shipping_profile_slug`: the typed column and the compatibility
/// shadow must not disagree, otherwise the projection resurrects a removed profile.
pub fn clear_shipping_profile_metadata_shadow(mut metadata: Value) -> Value {
    if let Some(object) = metadata.as_object_mut() {
        object.remove("shipping_profile_slug");
        object.remove("shipping_profile");
    }

    metadata
}

pub fn normalize_create_product_metadata(
    input_tags: Vec<String>,
    shipping_profile_slug: Option<String>,
    metadata: Value,
) -> (Value, Option<Vec<String>>) {
    let normalized_tags = normalize_tag_names(&input_tags);
    let metadata = apply_shipping_profile_to_metadata(metadata, shipping_profile_slug);

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
                apply_shipping_profile_to_metadata(metadata, profile_slug),
                Some(normalized_tags),
            ))
        }
        (None, profile_slug, Some(metadata)) => Some((
            apply_shipping_profile_to_metadata(metadata, profile_slug),
            None,
        )),
        (None, Some(profile_slug), None) => Some((
            apply_shipping_profile_to_metadata(existing_metadata, Some(profile_slug)),
            None,
        )),
        (None, None, None) => None,
    }
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
        .map(product_field_definition_from_row)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(CustomFieldsSchema::new(definitions))
}

rustok_api::impl_field_definition_source!(product_field_definitions_storage::Model);

pub fn product_field_definition_from_row(
    row: product_field_definitions_storage::Model,
) -> Result<FieldDefinition, CommerceError> {
    rustok_core::field_schema::field_definition_from_source(&row)
        .map_err(|err| CommerceError::Validation(err.to_string()))
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
        // A missing requested/default locale must not depend on unspecified SQL
        // row order (or the order supplied by an external projection).
        .or_else(|| translations.iter().min_by(|a, b| a.locale.cmp(&b.locale)))
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
        // A missing requested/default locale must not depend on unspecified SQL
        // row order (or the order supplied by an external projection).
        .or_else(|| translations.iter().min_by(|a, b| a.locale.cmp(&b.locale)))
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

pub(super) async fn prepare_product_custom_fields_and_schema_for_create(
    conn: &impl ConnectionTrait,
    source: ProductFlexSchemaSource,
    tenant_id: Uuid,
    locale: &str,
    payload: Value,
) -> CommerceResult<(
    field_values::PreparedAttachedValuesWrite,
    CustomFieldsSchema,
)> {
    reject_reserved_tag_metadata(&payload)?;
    let schema = source.load(conn, tenant_id).await?;
    let prepared =
        field_values::prepare_donor_attached_values_create(schema.clone(), &payload, locale)?;
    Ok((prepared, schema))
}

pub(super) async fn prepare_product_custom_fields_and_schema_for_update(
    conn: &impl ConnectionTrait,
    source: ProductFlexSchemaSource,
    tenant_id: Uuid,
    product_id: Uuid,
    locale: &str,
    existing_metadata: &Value,
    payload: Value,
) -> CommerceResult<(
    field_values::PreparedAttachedValuesWrite,
    CustomFieldsSchema,
)> {
    reject_reserved_tag_metadata(&payload)?;
    let schema = source.load(conn, tenant_id).await?;
    let existing_localized =
        attached_localized_storage::load_exact(conn, tenant_id, product_id, locale).await?;
    let prepared = field_values::prepare_donor_attached_values_update_from_values(
        schema.clone(),
        locale,
        existing_metadata,
        existing_localized,
        Some(&payload),
    )?;
    Ok((prepared, schema))
}

pub(super) async fn resolve_product_attached_payload<C>(
    conn: &C,
    tenant_id: Uuid,
    product_id: Uuid,
    metadata: &Value,
    locale: &str,
    fallback_locale: &str,
    schema: &CustomFieldsSchema,
) -> CommerceResult<Option<Value>>
where
    C: ConnectionTrait,
{
    let localized = if schema.active_definitions().is_empty() {
        Default::default()
    } else {
        attached_localized_storage::load_by_locale(conn, tenant_id, product_id).await?
    };
    field_values::resolve_attached_payload_from_values(
        schema,
        metadata,
        &localized,
        locale,
        fallback_locale,
    )
    .map_err(CommerceError::from)
}

pub async fn resolve_product_metadata_with_schema<C>(
    conn: &C,
    tenant_id: Uuid,
    product_id: Uuid,
    metadata: &Value,
    locale: &str,
    fallback_locale: &str,
    schema: &CustomFieldsSchema,
) -> CommerceResult<Value>
where
    C: ConnectionTrait,
{
    resolve_product_attached_payload(
        conn,
        tenant_id,
        product_id,
        metadata,
        locale,
        fallback_locale,
        schema,
    )
    .await
    .map(|payload| payload.unwrap_or_else(|| serde_json::json!({})))
}

/// Positional placeholder for one bound value in a hand-written SQL fragment.
///
/// SQLite uses anonymous `?` markers while PostgreSQL numbers them; every `custom_condition`-style
/// fragment in the catalog service resolves its `{pN}` markers through this helper, and sea-query
/// re-numbers the emitted placeholders as the surrounding statement is written.
pub(crate) fn sql_placeholder(backend: DbBackend, index: usize) -> String {
    match backend {
        DbBackend::Sqlite => "?".to_string(),
        _ => format!("${index}"),
    }
}

pub fn product_channel_visibility_condition(
    backend: DbBackend,
    public_channel_slug: Option<&str>,
) -> Condition {
    if backend == DbBackend::Sqlite {
        return match normalize_public_channel_slug(public_channel_slug) {
            None => Condition::all().add(Expr::cust(
                "COALESCE(json_type(metadata, '$.channel_visibility.allowed_channel_slugs'), 'array') = 'array'
                 AND COALESCE(json_extract(metadata, '$.channel_visibility.allowed_channel_slugs'), '[]') = '[]'",
            )),
            Some(slug) => Condition::all().add(Expr::cust_with_values(
                "COALESCE(json_type(metadata, '$.channel_visibility.allowed_channel_slugs'), 'array') = 'array'
                 AND (
                     COALESCE(json_extract(metadata, '$.channel_visibility.allowed_channel_slugs'), '[]') = '[]'
                     OR EXISTS (
                         SELECT 1
                         FROM json_each(COALESCE(json_extract(metadata, '$.channel_visibility.allowed_channel_slugs'), '[]'))
                         WHERE value = ?
                     )
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

pub(super) fn resolve_variant_inventory_quantity(
    variant_id: Uuid,
    legacy_quantity: i32,
    inventory_quantities: &HashMap<Uuid, i32>,
) -> i32 {
    inventory_quantities
        .get(&variant_id)
        .copied()
        .unwrap_or(legacy_quantity)
}

pub(super) fn inventory_policy_allows_backorder(policy: &str) -> bool {
    policy.trim().eq_ignore_ascii_case("continue")
}

pub async fn apply_public_channel_inventory_to_product(
    inventory_port: Option<&std::sync::Arc<dyn crate::ports::ProductInventoryPort>>,
    db: &DatabaseConnection,
    tenant_id: Uuid,
    product: &mut ProductResponse,
    public_channel_slug: Option<&str>,
) -> CommerceResult<()> {
    let variant_ids = product
        .variants
        .iter()
        .map(|variant| variant.id)
        .collect::<Vec<_>>();
    let available_by_variant = if let Some(port) = inventory_port {
        port.load_available_inventory_by_variant_for_public_channel(
            db.into(),
            tenant_id,
            public_channel_slug,
            &variant_ids,
        )
        .await?
    } else {
        std::collections::HashMap::new()
    };

    project_public_channel_inventory(&mut product.variants, &available_by_variant);

    Ok(())
}

/// The public channel projection has no unscoped stock fallback. A missing
/// Inventory identity or provider cannot prove that stock is available here.
fn project_public_channel_inventory(
    variants: &mut [VariantResponse],
    available_by_variant: &HashMap<Uuid, i32>,
) {
    for variant in variants {
        let available_inventory = available_by_variant.get(&variant.id).copied().unwrap_or(0);
        variant.inventory_quantity = available_inventory;
        variant.in_stock =
            available_inventory > 0 || inventory_policy_allows_backorder(&variant.inventory_policy);
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

    if !locale_tags_match(fallback_locale, locale)
        && let Some(product_id) = find_published_product_id_for_locale(
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
        .filter(entities::product_translation::Column::TenantId.eq(tenant_id))
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
        .filter(entities::product_translation::Column::TenantId.eq(tenant_id))
        .filter(entities::product_translation::Column::Handle.eq(handle))
        .all(db)
        .await?;

    find_first_published_product(db, tenant_id, translations, public_channel_slug).await
}

pub async fn find_first_published_product(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    mut translations: Vec<entities::product_translation::Model>,
    public_channel_slug: Option<&str>,
) -> CommerceResult<Option<Uuid>> {
    // Handles are unique per tenant and locale, not across locales. Preserve
    // the caller's requested/fallback tiers and make each tier deterministic.
    translations.sort_unstable_by(|a, b| {
        a.locale
            .cmp(&b.locale)
            .then_with(|| a.product_id.cmp(&b.product_id))
            .then_with(|| a.id.cmp(&b.id))
    });
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
    tenant_id: Uuid,
    product_id: Uuid,
    translations: Option<&[ProductTranslationInput]>,
) -> CommerceResult<String>
where
    C: ConnectionTrait,
{
    if let Some(translations) = translations
        && !translations.is_empty()
    {
        return Ok(preferred_product_locale_from_translations(translations));
    }

    let existing = entities::product_translation::Entity::find()
        .filter(entities::product_translation::Column::TenantId.eq(tenant_id))
        .filter(entities::product_translation::Column::ProductId.eq(product_id))
        .all(conn)
        .await?;

    // Tags-only patches have no submitted translation order.
    if let Some(translation) = existing.iter().min_by(|a, b| a.locale.cmp(&b.locale)) {
        return Ok(translation.locale.clone());
    }

    Ok(PLATFORM_FALLBACK_LOCALE.to_string())
}

#[cfg(test)]
mod product_metadata_tests {
    use super::{
        normalize_variant_sku, reject_reserved_tag_metadata, resolve_product_metadata_with_schema,
    };
    use rustok_core::field_schema::{CustomFieldsSchema, FieldDefinition, FieldType};
    use serde_json::json;
    use std::collections::HashMap;
    use uuid::Uuid;

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
    fn metadata_tags_are_a_rejected_reserved_key() {
        let error = reject_reserved_tag_metadata(&json!({"tags": ["sale"]}))
            .expect_err("metadata tags must be rejected");
        assert!(error.to_string().contains("typed tags field"));
        reject_reserved_tag_metadata(&json!({"source": "erp"}))
            .expect("unrelated metadata should remain valid");
    }

    #[test]
    fn variant_sku_is_trimmed_and_blank_values_become_unset() {
        assert_eq!(
            normalize_variant_sku(Some("  SKU-123 \n")),
            Some("SKU-123".to_owned())
        );
        assert_eq!(normalize_variant_sku(Some(" \t ")), None);
        assert_eq!(normalize_variant_sku(None), None);
    }

    #[test]
    fn shipping_profile_clears_from_metadata_when_empty_string_provided() {
        let metadata = json!({
            "source": "erp",
            "shipping_profile": { "slug": "express" },
            "shipping_profile_slug": "express"
        });
        let cleared = super::apply_shipping_profile_to_metadata(metadata, Some("   ".to_string()));
        assert_eq!(cleared.get("source"), Some(&json!("erp")));
        assert!(cleared.get("shipping_profile").is_none());
        assert!(cleared.get("shipping_profile_slug").is_none());
    }

    #[tokio::test]
    async fn resolve_product_metadata_with_schema_preserves_reserved_and_resolves_flex() {
        use sea_orm::ConnectionTrait;
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("sqlite in-memory db");

        db.execute_raw(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            r#"
            CREATE TABLE product_attached_localized_values (
                id TEXT PRIMARY KEY NOT NULL,
                tenant_id TEXT NOT NULL,
                product_id TEXT NOT NULL,
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
        .expect("create product_attached_localized_values");

        let tenant_id = Uuid::new_v4();
        let product_id = Uuid::new_v4();
        let schema = CustomFieldsSchema::new(vec![definition("fit"), definition("material")]);

        let metadata = json!({
            "source": "erp",
            "fit": "oversized",
            "material": "cotton",
        });

        let resolved = resolve_product_metadata_with_schema(
            &db, tenant_id, product_id, &metadata, "en", "en", &schema,
        )
        .await
        .expect("resolve with schema");

        assert_eq!(resolved.get("source"), Some(&json!("erp")));
        assert_eq!(resolved.get("fit"), Some(&json!("oversized")));
        assert_eq!(resolved.get("material"), Some(&json!("cotton")));
    }
}

#[cfg(test)]
mod variant_title_tests {
    use super::*;
    use rustok_api::PLATFORM_FALLBACK_LOCALE;

    fn translation(locale: &str, title: Option<&str>) -> entities::variant_translation::Model {
        entities::variant_translation::Model {
            id: Uuid::new_v4(),
            variant_id: Uuid::new_v4(),
            locale: locale.to_owned(),
            title: title.map(str::to_owned),
        }
    }

    fn axis_value(
        attribute_id: Uuid,
        option_id: Uuid,
        code: Option<&str>,
        label: Option<&str>,
    ) -> VariantAxisValueResponse {
        VariantAxisValueResponse {
            attribute_id,
            option_id,
            code: code.map(str::to_owned),
            label: label.map(str::to_owned),
        }
    }

    #[test]
    fn axis_value_labels_join_into_a_display_title() {
        assert_eq!(
            generate_variant_title_from_axis_values(&["Red", "M"]),
            "Red / M"
        );
        assert_eq!(
            generate_variant_title_from_axis_values(&["Relaxed"]),
            "Relaxed"
        );
        assert_eq!(
            generate_variant_title_from_axis_values(&[]),
            "Default",
            "a variant without assignments is titled Default"
        );
    }

    #[test]
    fn display_title_prefers_the_authored_title_of_the_requested_locale() {
        let translations = vec![
            translation("en", Some("Authored EN")),
            translation("ru", Some("Оригинальное название")),
        ];
        let values = vec![axis_value(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some("color"),
            Some("Red"),
        )];

        assert_eq!(
            variant_display_title(&translations, &values, "ru", PLATFORM_FALLBACK_LOCALE),
            "Оригинальное название"
        );
        assert_eq!(
            variant_display_title(&translations, &values, "en", PLATFORM_FALLBACK_LOCALE),
            "Authored EN"
        );
    }

    #[test]
    fn display_title_falls_back_to_fallback_locale_title_then_axis_labels() {
        let translations = vec![translation("en", Some("Authored EN"))];
        let values = vec![
            axis_value(Uuid::new_v4(), Uuid::new_v4(), Some("color"), Some("Red")),
            axis_value(Uuid::new_v4(), Uuid::new_v4(), Some("size"), Some("M")),
        ];

        assert_eq!(
            variant_display_title(&translations, &values, "de", "en"),
            "Authored EN",
            "the fallback locale's authored title wins before axis labels"
        );
        assert_eq!(
            variant_display_title(&translations, &values, "de", "fr"),
            "Red / M",
            "without any authored title the tuple labels become the title"
        );
    }

    #[test]
    fn display_title_treats_blank_authored_titles_as_absent() {
        let translations = vec![translation("de", Some("   ")), translation("fr", Some(""))];
        let values = vec![axis_value(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Some("color"),
            Some("Red"),
        )];

        assert_eq!(
            variant_display_title(&translations, &values, "de", "fr"),
            "Red"
        );
    }

    #[test]
    fn display_title_renders_the_option_id_when_no_label_exists() {
        let option_id = Uuid::new_v4();
        let values = vec![axis_value(Uuid::new_v4(), option_id, Some("color"), None)];

        assert_eq!(
            variant_display_title(&[], &values, "de", "en"),
            option_id.to_string()
        );
    }

    #[test]
    fn display_title_defaults_without_translations_or_assignments() {
        assert_eq!(variant_display_title(&[], &[], "de", "en"), "Default");
    }
}

#[cfg(test)]
mod category_filter_subtree_tests {
    use super::*;

    fn node(id: Uuid, kind: &str, is_active: bool) -> CategoryFilterSubtreeRow {
        CategoryFilterSubtreeRow {
            id,
            kind: kind.to_string(),
            is_active,
        }
    }

    #[test]
    fn subtree_walks_only_live_structural_descendants() {
        let root = Uuid::new_v4();
        let child = Uuid::new_v4();
        let grandchild = Uuid::new_v4();
        let deleted_child = Uuid::new_v4();
        let child_of_deleted = Uuid::new_v4();
        let categories = vec![
            node(root, "structural", true),
            node(child, "structural", true),
            node(grandchild, "structural", true),
            node(deleted_child, "structural", false),
            node(child_of_deleted, "structural", true),
        ];
        let parents = HashMap::from([
            (root, None),
            (child, Some(root)),
            (grandchild, Some(child)),
            (deleted_child, Some(root)),
            (child_of_deleted, Some(deleted_child)),
        ]);

        let mut subtree =
            category_filter_subtree_from_taxonomy(root, categories, &parents)
                .expect("resolve Taxonomy-backed subtree");
        subtree.sort_unstable();
        let mut expected = vec![root, child, grandchild];
        expected.sort_unstable();
        assert_eq!(subtree, expected);
    }

    #[test]
    fn subtree_stops_at_non_structural_categories_and_their_descendants() {
        let root = Uuid::new_v4();
        let virtual_child = Uuid::new_v4();
        let structural_grandchild = Uuid::new_v4();
        let categories = vec![
            node(root, "structural", true),
            node(virtual_child, "virtual", true),
            node(structural_grandchild, "structural", true),
        ];
        let parents = HashMap::from([
            (root, None),
            (virtual_child, Some(root)),
            (structural_grandchild, Some(virtual_child)),
        ]);

        assert_eq!(
            category_filter_subtree_from_taxonomy(root, categories, &parents)
                .expect("resolve structural subtree"),
            vec![root]
        );
        assert!(
            category_filter_subtree_from_taxonomy(
                virtual_child,
                vec![node(virtual_child, "virtual", true)],
                &HashMap::from([(virtual_child, Some(root))]),
            )
            .expect("non-structural root resolves to empty")
            .is_empty()
        );
    }

    #[test]
    fn subtree_returns_empty_for_unknown_or_deleted_roots() {
        let root = Uuid::new_v4();
        let categories = vec![node(root, "structural", false)];
        let parents = HashMap::from([(root, None)]);
        assert!(
            category_filter_subtree_from_taxonomy(root, categories, &parents)
                .expect("deleted root resolves to empty")
                .is_empty()
        );
        assert!(
            category_filter_subtree_from_taxonomy(
                Uuid::new_v4(),
                vec![node(root, "structural", true)],
                &parents,
            )
            .expect("unknown root resolves to empty")
            .is_empty()
        );
    }

    #[test]
    fn subtree_refuses_live_trees_that_exceed_the_bound() {
        let root = Uuid::new_v4();
        let mut categories = vec![node(root, "structural", true)];
        let mut parents = HashMap::from([(root, None)]);
        let mut parent = root;
        for _ in 0..MAX_CATEGORY_FILTER_SUBTREE {
            let child = Uuid::new_v4();
            categories.push(node(child, "structural", true));
            parents.insert(child, Some(parent));
            parent = child;
        }

        let error = category_filter_subtree_from_taxonomy(root, categories, &parents)
            .expect_err("oversized subtree must be refused");
        assert!(matches!(error, CommerceError::Validation(_)));
    }
}

#[cfg(test)]
mod public_channel_inventory_tests {
    use super::*;

    fn variant(policy: &str) -> VariantResponse {
        VariantResponse {
            id: Uuid::new_v4(),
            revision: "1".to_owned(),
            product_id: Uuid::new_v4(),
            sku: None,
            barcode: None,
            shipping_profile_slug: None,
            title: "Variant".to_owned(),
            translations: Vec::new(),
            combination_identity: None,
            axis_values: Vec::new(),
            prices: Vec::new(),
            inventory_quantity: 50,
            inventory_policy: policy.to_owned(),
            in_stock: true,
            weight: None,
            weight_unit: None,
            position: 0,
        }
    }

    #[test]
    fn channel_projection_never_reuses_unscoped_stock() {
        let mut variants = [
            variant("deny"),
            variant("continue"),
            variant("deny"),
            variant("deny"),
            variant("deny"),
        ];
        let channel_stock = HashMap::from([
            (variants[2].id, 0),
            (variants[3].id, 3),
            (variants[4].id, -2),
        ]);
        project_public_channel_inventory(&mut variants, &channel_stock);
        assert_eq!(
            variants.map(|variant| (variant.inventory_quantity, variant.in_stock)),
            [(0, false), (0, true), (0, false), (3, true), (-2, false)]
        );
    }

    #[test]
    fn absent_channel_provider_does_not_advertise_catalog_stock() {
        let mut variants = [variant("deny"), variant("continue")];
        project_public_channel_inventory(&mut variants, &HashMap::new());
        assert_eq!(
            variants.map(|variant| (variant.inventory_quantity, variant.in_stock)),
            [(0, false), (0, true)]
        );
    }
}

#[cfg(test)]
mod product_handle_and_visibility_tests {
    use super::{
        is_metadata_visible_for_public_channel, product_title_substring_like_pattern,
        product_translation_handle, slugify,
    };
    use serde_json::json;

    #[test]
    fn product_title_substring_search_treats_like_metacharacters_as_literals() {
        assert_eq!(
            product_title_substring_like_pattern("100%_ready!"),
            "%100!%!_ready!!%"
        );
        assert_eq!(product_title_substring_like_pattern("a b"), "%a b%");
        assert_eq!(product_title_substring_like_pattern(""), "%%");
    }

    #[test]
    fn missing_locale_uses_deterministic_translation_fallback() {
        use super::pick_response_translation;
        use crate::dto::ProductTranslationResponse;

        let translation = |locale: &str| ProductTranslationResponse {
            locale: locale.to_owned(),
            title: locale.to_owned(),
            handle: locale.to_owned(),
            description: None,
            meta_title: None,
            meta_description: None,
        };
        let reversed = vec![translation("zh"), translation("de"), translation("fr")];
        let forward = vec![translation("fr"), translation("de"), translation("zh")];
        for items in [&reversed, &forward] {
            assert_eq!(
                pick_response_translation(items, "it", "en")
                    .map(|translation| translation.locale.as_str()),
                Some("de"),
            );
        }
        assert_eq!(
            pick_response_translation(&reversed, "fr", "en")
                .map(|translation| translation.locale.as_str()),
            Some("fr"),
        );
    }

    #[test]
    fn slugify_preserves_unicode_letters_and_numbers() {
        assert_eq!(slugify("Красный чай 2026"), "красный-чай-2026");
        assert_eq!(slugify("東京 2026"), "東京-2026");
        assert_eq!(slugify("Café Déjà Vu"), "café-déjà-vu");
    }

    #[test]
    fn blank_product_handles_fall_back_to_the_localized_title() {
        assert_eq!(
            product_translation_handle(None, "Красный чай"),
            "красный-чай"
        );
        assert_eq!(product_translation_handle(Some(""), "Chair"), "chair");
        assert_eq!(product_translation_handle(Some("  \t "), "Chair"), "chair");
        assert_eq!(
            product_translation_handle(Some("custom-handle"), "Chair"),
            "custom-handle"
        );
    }

    #[test]
    fn malformed_nonempty_channel_allowlists_do_not_become_public() {
        let invalid_values = json!({
            "channel_visibility": { "allowed_channel_slugs": ["", 42, null] }
        });
        let noncanonical_values = json!({
            "channel_visibility": { "allowed_channel_slugs": ["WEB"] }
        });
        let wrong_shape = json!({
            "channel_visibility": { "allowed_channel_slugs": "web" }
        });
        let null_values = json!({
            "channel_visibility": { "allowed_channel_slugs": null }
        });
        let valid_values = json!({
            "channel_visibility": { "allowed_channel_slugs": ["web"] }
        });

        for metadata in [
            &invalid_values,
            &noncanonical_values,
            &wrong_shape,
            &null_values,
        ] {
            assert!(!is_metadata_visible_for_public_channel(
                metadata,
                Some("web")
            ));
            assert!(!is_metadata_visible_for_public_channel(metadata, None));
        }
        assert!(is_metadata_visible_for_public_channel(
            &valid_values,
            Some("web")
        ));
        assert!(!is_metadata_visible_for_public_channel(
            &valid_values,
            Some("mobile")
        ));
        assert!(is_metadata_visible_for_public_channel(
            &json!({ "channel_visibility": { "allowed_channel_slugs": [] } }),
            None
        ));
        assert!(is_metadata_visible_for_public_channel(
            &json!({}),
            Some("web")
        ));
    }
}
