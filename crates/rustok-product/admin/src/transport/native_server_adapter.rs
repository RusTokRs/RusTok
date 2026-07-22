#![allow(dead_code)]

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{
    BindCategoryAttributeDraft, BindSchemaAttributeDraft, CatalogCategoryDraft,
    CatalogCategoryList, CategoryAttributeGroupDraft, ProductAttributeDraft, ProductAttributeList,
    ProductAttributeOptionDraft, ProductAttributeSchemaDraft, ProductAttributeSchemaGroupDraft,
    ProductAttributeSchemaList, ProductAttributeValueItem, ProductAttributeValuePatchDraft,
    ProductCatalogSearchOptions, ProductEffectiveForm, SetCategorySchemaModeDraft,
};
#[cfg(feature = "ssr")]
use crate::model::{
    CatalogCategorySummary, ProductAttributeOptionSummary, ProductAttributeSchemaSummary,
    ProductAttributeSummary, ProductCatalogSearchOption, ProductEffectiveFormAttribute,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

pub(super) async fn fetch_product_attributes(
    tenant_id: String,
    locale: String,
) -> Result<ProductAttributeList, ApiError> {
    product_admin_attributes_native(tenant_id, locale)
        .await
        .map_err(Into::into)
}

pub(super) async fn fetch_catalog_categories(
    tenant_id: String,
    locale: String,
) -> Result<CatalogCategoryList, ApiError> {
    product_admin_categories_native(tenant_id, locale)
        .await
        .map_err(Into::into)
}

pub(super) async fn fetch_catalog_search_options(
    locale: String,
) -> Result<ProductCatalogSearchOptions, ApiError> {
    product_admin_catalog_search_options_native(locale)
        .await
        .map_err(Into::into)
}

pub(super) async fn fetch_attribute_schemas(
    tenant_id: String,
    locale: String,
) -> Result<ProductAttributeSchemaList, ApiError> {
    product_admin_attribute_schemas_native(tenant_id, locale)
        .await
        .map_err(Into::into)
}

pub(super) async fn fetch_effective_product_form(
    tenant_id: String,
    product_id: Option<String>,
    category_id: Option<String>,
    locale: String,
) -> Result<Option<ProductEffectiveForm>, ApiError> {
    product_admin_effective_form_native(tenant_id, product_id, category_id, locale)
        .await
        .map_err(Into::into)
}

pub(super) async fn fetch_product_attribute_values(
    tenant_id: String,
    product_id: String,
    locale: String,
) -> Result<Vec<ProductAttributeValueItem>, ApiError> {
    product_admin_attribute_values_native(tenant_id, product_id, locale)
        .await
        .map_err(Into::into)
}

pub(super) async fn create_product_attribute(
    tenant_id: String,
    locale: String,
    draft: ProductAttributeDraft,
) -> Result<bool, ApiError> {
    product_admin_create_attribute_native(tenant_id, locale, draft)
        .await
        .map_err(Into::into)
}

pub(super) async fn create_product_attribute_option(
    tenant_id: String,
    locale: String,
    draft: ProductAttributeOptionDraft,
) -> Result<bool, ApiError> {
    product_admin_create_attribute_option_native(tenant_id, locale, draft)
        .await
        .map_err(Into::into)
}

pub(super) async fn create_catalog_category(
    tenant_id: String,
    locale: String,
    draft: CatalogCategoryDraft,
) -> Result<bool, ApiError> {
    product_admin_create_category_native(tenant_id, locale, draft)
        .await
        .map_err(Into::into)
}

pub(super) async fn create_attribute_schema(
    tenant_id: String,
    locale: String,
    draft: ProductAttributeSchemaDraft,
) -> Result<bool, ApiError> {
    product_admin_create_schema_native(tenant_id, locale, draft)
        .await
        .map_err(Into::into)
}

pub(super) async fn set_category_schema_mode(
    tenant_id: String,
    draft: SetCategorySchemaModeDraft,
) -> Result<bool, ApiError> {
    product_admin_set_category_schema_mode_native(tenant_id, draft)
        .await
        .map_err(Into::into)
}

pub(super) async fn bind_schema_attribute(
    tenant_id: String,
    draft: BindSchemaAttributeDraft,
) -> Result<bool, ApiError> {
    product_admin_bind_schema_attribute_native(tenant_id, draft)
        .await
        .map_err(Into::into)
}

pub(super) async fn create_product_attribute_schema_group(
    tenant_id: String,
    locale: String,
    draft: ProductAttributeSchemaGroupDraft,
) -> Result<bool, ApiError> {
    product_admin_create_schema_group_native(tenant_id, locale, draft)
        .await
        .map_err(|err| ApiError::ServerFn(err.to_string()))
}

pub(super) async fn create_category_attribute_group(
    tenant_id: String,
    locale: String,
    draft: CategoryAttributeGroupDraft,
) -> Result<bool, ApiError> {
    product_admin_create_category_group_native(tenant_id, locale, draft)
        .await
        .map_err(|err| ApiError::ServerFn(err.to_string()))
}

pub(super) async fn bind_category_attribute(
    tenant_id: String,
    draft: BindCategoryAttributeDraft,
) -> Result<bool, ApiError> {
    product_admin_bind_category_attribute_native(tenant_id, draft)
        .await
        .map_err(Into::into)
}

pub(super) async fn save_product_attribute_values(
    tenant_id: String,
    product_id: String,
    locale: String,
    patches: Vec<ProductAttributeValuePatchDraft>,
) -> Result<Vec<ProductAttributeValueItem>, ApiError> {
    product_admin_save_attribute_values_native(tenant_id, product_id, locale, patches)
        .await
        .map_err(Into::into)
}

pub(super) async fn clear_detached_product_attribute_values(
    tenant_id: String,
    product_id: String,
    locale: String,
    attribute_ids: Vec<String>,
) -> Result<Vec<ProductAttributeValueItem>, ApiError> {
    product_admin_clear_detached_attribute_values_native(
        tenant_id,
        product_id,
        locale,
        attribute_ids,
    )
    .await
    .map_err(Into::into)
}

#[cfg(feature = "ssr")]
fn ensure_permission(
    permissions: &[rustok_api::Permission],
    required: &[rustok_api::Permission],
    message: &str,
) -> Result<(), ServerFnError> {
    if rustok_api::has_any_effective_permission(permissions, required) {
        Ok(())
    } else {
        Err(ServerFnError::new(format!("Permission denied: {message}")))
    }
}

#[cfg(feature = "ssr")]
fn parse_uuid(value: &str, field_name: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim())
        .map_err(|_| ServerFnError::new(format!("Invalid {field_name}")))
}

#[cfg(feature = "ssr")]
fn parse_optional_uuid(
    value: Option<String>,
    field_name: &str,
) -> Result<Option<uuid::Uuid>, ServerFnError> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| parse_uuid(value, field_name))
        .transpose()
}

#[cfg(feature = "ssr")]
fn empty_json() -> serde_json::Value {
    serde_json::Value::Object(Default::default())
}

#[cfg(feature = "ssr")]
fn parse_attribute_value_type(
    value: &str,
) -> Result<rustok_product::services::AttributeValueType, ServerFnError> {
    rustok_product::services::AttributeValueType::from_storage(value.trim())
        .map_err(|error| ServerFnError::new(format!("{error:?}")))
}

#[cfg(feature = "ssr")]
fn parse_category_kind(
    value: &str,
) -> Result<rustok_product::services::CatalogCategoryKind, ServerFnError> {
    rustok_product::services::CatalogCategoryKind::from_storage(value.trim())
        .map_err(|error| ServerFnError::new(format!("{error:?}")))
}

#[cfg(feature = "ssr")]
fn parse_schema_mode(
    value: &str,
) -> Result<rustok_product::services::CategorySchemaMode, ServerFnError> {
    rustok_product::services::CategorySchemaMode::from_storage(value.trim())
        .map_err(|error| ServerFnError::new(format!("{error:?}")))
}

#[cfg(feature = "ssr")]
fn parse_binding_kind(
    value: &str,
) -> Result<rustok_product::services::CategoryAttributeBindingKind, ServerFnError> {
    rustok_product::services::CategoryAttributeBindingKind::from_storage(value.trim())
        .map_err(|error| ServerFnError::new(format!("{error:?}")))
}

#[cfg(feature = "ssr")]
fn map_attribute_record(
    value: rustok_product::services::ProductAttributeListRecord,
) -> ProductAttributeSummary {
    ProductAttributeSummary {
        id: value.id.to_string(),
        code: value.code,
        value_type: value.value_type.as_str().to_string(),
        is_localized: value.is_localized,
        is_filterable: value.is_filterable,
        is_searchable: value.is_searchable,
        is_sortable: value.is_sortable,
        show_on_storefront: value.show_on_storefront,
        label: value.label,
    }
}

#[cfg(feature = "ssr")]
fn map_category_record(
    value: rustok_product::services::CatalogCategoryListRecord,
) -> CatalogCategorySummary {
    CatalogCategorySummary {
        id: value.id.to_string(),
        code: value.code,
        slug: value.slug,
        path: value.path,
        kind: value.kind.as_str().to_string(),
        name: value.name,
        parent_id: value.parent_id.map(|id| id.to_string()),
    }
}

fn first_non_empty(values: impl IntoIterator<Item = String>) -> String {
    values
        .into_iter()
        .find(|value| !value.trim().is_empty())
        .unwrap_or_default()
}

#[cfg(feature = "ssr")]
fn map_schema_record(
    value: rustok_product::services::ProductAttributeSchemaListRecord,
) -> ProductAttributeSchemaSummary {
    ProductAttributeSchemaSummary {
        id: value.id.to_string(),
        code: value.code,
        name: value.name,
    }
}

#[cfg(feature = "ssr")]
fn effective_attribute_source_name(
    source: rustok_product::services::EffectiveAttributeSource,
) -> &'static str {
    match source {
        rustok_product::services::EffectiveAttributeSource::Schema => "schema",
        rustok_product::services::EffectiveAttributeSource::Inherited => "inherited",
        rustok_product::services::EffectiveAttributeSource::CloneSnapshot => "clone_snapshot",
        rustok_product::services::EffectiveAttributeSource::CategoryLocal => "category_local",
    }
}

#[cfg(feature = "ssr")]
fn map_attribute_value(
    record: rustok_product::services::ProductAttributeValueRecord,
) -> ProductAttributeValueItem {
    use rustok_product::services::ProductAttributeValue;

    let mut item = ProductAttributeValueItem {
        attribute_id: record.attribute_id.to_string(),
        kind: "unset".to_string(),
        text: None,
        integer: None,
        decimal: None,
        boolean: None,
        date: None,
        datetime: None,
        option_id: None,
        option_ids: None,
        json: None,
        detached: record.detached,
    };
    match record.value {
        None => {}
        Some(ProductAttributeValue::Text(value)) => {
            item.kind = "text".to_string();
            item.text = Some(value);
        }
        Some(ProductAttributeValue::Integer(value)) => {
            item.kind = "integer".to_string();
            item.integer = Some(value);
        }
        Some(ProductAttributeValue::Decimal(value)) => {
            item.kind = "decimal".to_string();
            item.decimal = Some(value.to_string());
        }
        Some(ProductAttributeValue::Boolean(value)) => {
            item.kind = "boolean".to_string();
            item.boolean = Some(value);
        }
        Some(ProductAttributeValue::Date(value)) => {
            item.kind = "date".to_string();
            item.date = Some(value.to_string());
        }
        Some(ProductAttributeValue::Datetime(value)) => {
            item.kind = "datetime".to_string();
            item.datetime = Some(value.to_rfc3339());
        }
        Some(ProductAttributeValue::Select(value)) => {
            item.kind = "select".to_string();
            item.option_id = Some(value.to_string());
        }
        Some(ProductAttributeValue::Multiselect(value)) => {
            item.kind = "multiselect".to_string();
            item.option_ids = Some(value.into_iter().map(|id| id.to_string()).collect());
        }
        Some(ProductAttributeValue::Json(value)) => {
            item.kind = "json".to_string();
            item.json = Some(value);
        }
    }
    item
}

#[cfg(feature = "ssr")]
fn parse_attribute_value_patch(
    draft: ProductAttributeValuePatchDraft,
) -> Result<rustok_product::services::ProductAttributeValuePatch, ServerFnError> {
    use rustok_product::services::ProductAttributeValuePatchValue as Value;

    let payload_count = [
        draft.text.is_some(),
        draft.integer.is_some(),
        draft.decimal.is_some(),
        draft.boolean.is_some(),
        draft.date.is_some(),
        draft.datetime.is_some(),
        draft.option_id.is_some(),
        draft.option_ids.is_some(),
        draft.json.is_some(),
    ]
    .into_iter()
    .filter(|present| *present)
    .count();
    let kind = draft.kind.trim();
    let expected_payload_count = usize::from(kind != "clear");
    if payload_count != expected_payload_count {
        return Err(ServerFnError::new(
            "attribute value patch must contain exactly the payload selected by kind",
        ));
    }
    let missing = || ServerFnError::new("attribute value payload does not match kind");
    let value = match kind {
        "clear" => Value::Clear,
        "text" => Value::Text(draft.text.ok_or_else(missing)?),
        "integer" => Value::Integer(draft.integer.ok_or_else(missing)?),
        "decimal" => Value::Decimal(
            draft
                .decimal
                .ok_or_else(missing)?
                .parse()
                .map_err(|_| ServerFnError::new("invalid decimal value"))?,
        ),
        "boolean" => Value::Boolean(draft.boolean.ok_or_else(missing)?),
        "date" => Value::Date(
            draft
                .date
                .ok_or_else(missing)?
                .parse()
                .map_err(|_| ServerFnError::new("invalid ISO date value"))?,
        ),
        "datetime" => Value::Datetime(
            chrono::DateTime::parse_from_rfc3339(&draft.datetime.ok_or_else(missing)?)
                .map_err(|_| ServerFnError::new("invalid RFC3339 datetime value"))?
                .with_timezone(&chrono::Utc),
        ),
        "select" => Value::Select(parse_uuid(
            &draft.option_id.ok_or_else(missing)?,
            "option_id",
        )?),
        "multiselect" => Value::Multiselect(
            draft
                .option_ids
                .ok_or_else(missing)?
                .into_iter()
                .map(|id| parse_uuid(&id, "option_id"))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        "json" => Value::Json(draft.json.ok_or_else(missing)?),
        _ => return Err(ServerFnError::new("unsupported attribute value kind")),
    };
    Ok(rustok_product::services::ProductAttributeValuePatch {
        attribute_id: parse_uuid(&draft.attribute_id, "attribute_id")?,
        value,
    })
}

#[cfg(feature = "ssr")]
fn service_from_context(
    runtime_ctx: &rustok_api::HostRuntimeContext,
) -> Result<rustok_product::ProductCatalogSchemaService, ServerFnError> {
    let event_bus = runtime_ctx
        .shared_get::<rustok_outbox::TransactionalEventBus>()
        .ok_or_else(|| {
            ServerFnError::new(
                "product/admin native transport requires TransactionalEventBus in host runtime context",
            )
        })?;
    Ok(rustok_product::ProductCatalogSchemaService::new(
        runtime_ctx.db_clone(),
        event_bus,
    ))
}

#[cfg(feature = "ssr")]
async fn native_context() -> Result<
    (
        rustok_product::ProductCatalogSchemaService,
        rustok_api::AuthContext,
        rustok_api::TenantContext,
    ),
    ServerFnError,
> {
    let runtime_ctx = leptos::prelude::expect_context::<rustok_api::HostRuntimeContext>();
    let service = service_from_context(&runtime_ctx)?;
    let auth = leptos_axum::extract::<rustok_api::AuthContext>()
        .await
        .map_err(ServerFnError::new)?;
    let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
        .await
        .map_err(ServerFnError::new)?;
    Ok((service, auth, tenant))
}

#[server(prefix = "/api/fn", endpoint = "product/admin/attributes")]
async fn product_admin_attributes_native(
    tenant_id: String,
    locale: String,
) -> Result<ProductAttributeList, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = native_context().await?;
        ensure_permission(
            &auth.permissions,
            &[rustok_api::Permission::PRODUCTS_READ],
            "products:read required",
        )?;
        let tenant_id = parse_uuid(&tenant_id, "tenant_id")?;
        if tenant_id != tenant.id {
            return Err(ServerFnError::new(
                "tenant_id does not match current tenant",
            ));
        }
        let items = service
            .list_attributes(tenant.id, locale.trim())
            .await
            .map_err(ServerFnError::new)?
            .into_iter()
            .map(map_attribute_record)
            .collect::<Vec<_>>();
        Ok(ProductAttributeList {
            total: items.len() as u64,
            items,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "product/admin/attributes requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "product/admin/categories")]
async fn product_admin_categories_native(
    tenant_id: String,
    locale: String,
) -> Result<CatalogCategoryList, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = native_context().await?;
        ensure_permission(
            &auth.permissions,
            &[rustok_api::Permission::PRODUCTS_READ],
            "products:read required",
        )?;
        let tenant_id = parse_uuid(&tenant_id, "tenant_id")?;
        if tenant_id != tenant.id {
            return Err(ServerFnError::new(
                "tenant_id does not match current tenant",
            ));
        }
        let items = service
            .list_categories(tenant.id, locale.trim())
            .await
            .map_err(ServerFnError::new)?
            .into_iter()
            .map(map_category_record)
            .collect::<Vec<_>>();
        Ok(CatalogCategoryList {
            total: items.len() as u64,
            items,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "product/admin/categories requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "product/admin/catalog-search-options")]
async fn product_admin_catalog_search_options_native(
    locale: String,
) -> Result<ProductCatalogSearchOptions, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = native_context().await?;
        ensure_permission(
            &auth.permissions,
            &[rustok_api::Permission::PRODUCTS_READ],
            "products:read required",
        )?;
        let categories = service
            .list_categories(tenant.id, locale.trim())
            .await
            .map_err(ServerFnError::new)?
            .into_iter()
            .map(map_category_record)
            .map(|category| ProductCatalogSearchOption {
                value: category.id,
                label: first_non_empty([category.path, category.name, category.code]),
            })
            .collect();
        let attributes = service
            .list_attributes(tenant.id, locale.trim())
            .await
            .map_err(ServerFnError::new)?
            .into_iter()
            .map(map_attribute_record)
            .filter(|attribute| attribute.is_filterable || attribute.is_sortable)
            .map(|attribute| {
                let label = first_non_empty([attribute.label, attribute.code.clone()]);
                ProductCatalogSearchOption {
                    value: attribute.code.clone(),
                    label: format!("{label} ({})", attribute.code),
                }
            })
            .collect();

        Ok(ProductCatalogSearchOptions {
            category_options: categories,
            attribute_options: attributes,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = locale;
        Err(ServerFnError::new(
            "product/admin/catalog-search-options requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "product/admin/attribute-schemas")]
async fn product_admin_attribute_schemas_native(
    tenant_id: String,
    locale: String,
) -> Result<ProductAttributeSchemaList, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = native_context().await?;
        ensure_permission(
            &auth.permissions,
            &[rustok_api::Permission::PRODUCTS_READ],
            "products:read required",
        )?;
        let tenant_id = parse_uuid(&tenant_id, "tenant_id")?;
        if tenant_id != tenant.id {
            return Err(ServerFnError::new(
                "tenant_id does not match current tenant",
            ));
        }
        let items = service
            .list_schemas(tenant.id, locale.trim())
            .await
            .map_err(ServerFnError::new)?
            .into_iter()
            .map(map_schema_record)
            .collect::<Vec<_>>();
        Ok(ProductAttributeSchemaList {
            total: items.len() as u64,
            items,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "product/admin/attribute-schemas requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "product/admin/effective-form")]
async fn product_admin_effective_form_native(
    tenant_id: String,
    product_id: Option<String>,
    category_id: Option<String>,
    locale: String,
) -> Result<Option<ProductEffectiveForm>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = native_context().await?;
        ensure_permission(
            &auth.permissions,
            &[rustok_api::Permission::PRODUCTS_READ],
            "products:read required",
        )?;
        let tenant_id = parse_uuid(&tenant_id, "tenant_id")?;
        if tenant_id != tenant.id {
            return Err(ServerFnError::new(
                "tenant_id does not match current tenant",
            ));
        }
        let form = match (product_id, category_id) {
            (Some(product_id), _) => service
                .load_effective_form_for_product(tenant.id, parse_uuid(&product_id, "product_id")?)
                .await
                .map_err(ServerFnError::new)?,
            (None, Some(category_id)) => Some(
                service
                    .load_effective_form_for_category(
                        tenant.id,
                        parse_uuid(&category_id, "category_id")?,
                        &[],
                    )
                    .await
                    .map_err(ServerFnError::new)?,
            ),
            (None, None) => {
                return Err(ServerFnError::new(
                    "Either product_id or category_id is required",
                ));
            }
        };
        let Some(form) = form else {
            return Ok(None);
        };
        let group_labels = service
            .load_effective_form_group_labels(tenant.id, form.category_id, locale.trim())
            .await
            .map_err(ServerFnError::new)?;
        let definitions = service
            .list_attributes(tenant.id, locale.trim())
            .await
            .map_err(ServerFnError::new)?
            .into_iter()
            .map(|attribute| (attribute.id, attribute))
            .collect::<std::collections::HashMap<_, _>>();
        let effective_attribute_ids = form
            .attributes
            .iter()
            .map(|binding| binding.attribute_id)
            .collect::<Vec<_>>();
        let mut options_by_attribute = service
            .list_attribute_options(tenant.id, &effective_attribute_ids, locale.trim())
            .await
            .map_err(ServerFnError::new)?
            .into_iter()
            .fold(
                std::collections::HashMap::<String, Vec<ProductAttributeOptionSummary>>::new(),
                |mut map, option| {
                    map.entry(option.attribute_id.to_string())
                        .or_default()
                        .push(ProductAttributeOptionSummary {
                            id: option.id.to_string(),
                            code: option.code,
                            label: option.label,
                            position: option.position,
                        });
                    map
                },
            );
        let attributes = form
            .attributes
            .into_iter()
            .map(|binding| {
                let definition = definitions.get(&binding.attribute_id).ok_or_else(|| {
                    ServerFnError::new(format!(
                        "attribute definition {} is missing",
                        binding.attribute_id
                    ))
                })?;
                Ok(ProductEffectiveFormAttribute {
                    attribute_id: binding.attribute_id.to_string(),
                    code: definition.code.clone(),
                    label: definition.label.clone(),
                    value_type: definition.value_type.as_str().to_string(),
                    is_localized: definition.is_localized,
                    options: options_by_attribute
                        .remove(&binding.attribute_id.to_string())
                        .unwrap_or_default(),
                    group_label: binding
                        .group_code
                        .as_ref()
                        .and_then(|code| group_labels.get(code).cloned()),
                    group_code: binding.group_code,
                    is_required: binding.is_required,
                    is_disabled: binding.is_disabled,
                    position: binding.position,
                    source: effective_attribute_source_name(binding.source).to_string(),
                })
            })
            .collect::<Result<Vec<_>, ServerFnError>>()?;
        Ok(Some(ProductEffectiveForm {
            category_id: form.category_id.to_string(),
            attributes,
            detached_attribute_ids: form
                .detached_attribute_ids
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
        }))
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "product/admin/effective-form requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "product/admin/attribute-values")]
async fn product_admin_attribute_values_native(
    tenant_id: String,
    product_id: String,
    locale: String,
) -> Result<Vec<ProductAttributeValueItem>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = native_context().await?;
        ensure_permission(
            &auth.permissions,
            &[rustok_api::Permission::PRODUCTS_READ],
            "products:read required",
        )?;
        let tenant_id = parse_uuid(&tenant_id, "tenant_id")?;
        if tenant_id != tenant.id {
            return Err(ServerFnError::new(
                "tenant_id does not match current tenant",
            ));
        }
        service
            .load_product_attribute_values(
                tenant.id,
                parse_uuid(&product_id, "product_id")?,
                locale.trim(),
            )
            .await
            .map_err(ServerFnError::new)
            .map(|items| items.into_iter().map(map_attribute_value).collect())
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "product/admin/attribute-values requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "product/admin/save-attribute-values")]
async fn product_admin_save_attribute_values_native(
    tenant_id: String,
    product_id: String,
    locale: String,
    patches: Vec<ProductAttributeValuePatchDraft>,
) -> Result<Vec<ProductAttributeValueItem>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = native_context().await?;
        ensure_permission(
            &auth.permissions,
            &[rustok_api::Permission::PRODUCTS_MANAGE],
            "products:manage required",
        )?;
        let tenant_id = parse_uuid(&tenant_id, "tenant_id")?;
        if tenant_id != tenant.id {
            return Err(ServerFnError::new(
                "tenant_id does not match current tenant",
            ));
        }
        let patches = patches
            .into_iter()
            .map(parse_attribute_value_patch)
            .collect::<Result<Vec<_>, _>>()?;
        service
            .save_product_attribute_values(
                tenant.id,
                auth.user_id,
                parse_uuid(&product_id, "product_id")?,
                locale.trim(),
                patches,
            )
            .await
            .map_err(ServerFnError::new)
            .map(|items| items.into_iter().map(map_attribute_value).collect())
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "product/admin/save-attribute-values requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "product/admin/clear-detached-attribute-values"
)]
async fn product_admin_clear_detached_attribute_values_native(
    tenant_id: String,
    product_id: String,
    locale: String,
    attribute_ids: Vec<String>,
) -> Result<Vec<ProductAttributeValueItem>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = native_context().await?;
        ensure_permission(
            &auth.permissions,
            &[rustok_api::Permission::PRODUCTS_MANAGE],
            "products:manage required",
        )?;
        let tenant_id = parse_uuid(&tenant_id, "tenant_id")?;
        if tenant_id != tenant.id {
            return Err(ServerFnError::new(
                "tenant_id does not match current tenant",
            ));
        }
        let attribute_ids = attribute_ids
            .into_iter()
            .map(|attribute_id| parse_uuid(&attribute_id, "attribute_id"))
            .collect::<Result<Vec<_>, _>>()?;
        service
            .clear_detached_product_attribute_values(
                tenant.id,
                auth.user_id,
                parse_uuid(&product_id, "product_id")?,
                locale.trim(),
                attribute_ids,
            )
            .await
            .map_err(ServerFnError::new)
            .map(|items| items.into_iter().map(map_attribute_value).collect())
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "product/admin/clear-detached-attribute-values requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "product/admin/create-attribute")]
async fn product_admin_create_attribute_native(
    tenant_id: String,
    locale: String,
    draft: ProductAttributeDraft,
) -> Result<bool, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = native_context().await?;
        ensure_permission(
            &auth.permissions,
            &[rustok_api::Permission::PRODUCTS_MANAGE],
            "products:manage required",
        )?;
        let tenant_id = parse_uuid(&tenant_id, "tenant_id")?;
        if tenant_id != tenant.id {
            return Err(ServerFnError::new(
                "tenant_id does not match current tenant",
            ));
        }
        service
            .create_attribute(
                tenant.id,
                auth.user_id,
                rustok_product::services::CreateProductAttributeInput {
                    code: draft.code,
                    value_type: parse_attribute_value_type(&draft.value_type)?,
                    scope: "product".to_string(),
                    is_localized: draft.is_localized,
                    is_filterable: draft.is_filterable,
                    is_searchable: draft.is_searchable,
                    is_sortable: draft.is_sortable,
                    is_comparable: false,
                    show_on_storefront: draft.show_on_storefront,
                    show_in_admin_grid: true,
                    search_weight: 0,
                    filter_display: None,
                    facet_mode: None,
                    position: 0,
                    validation: empty_json(),
                    default_value: None,
                    metadata: empty_json(),
                    translations: vec![rustok_product::services::AttributeTranslationInput {
                        locale,
                        label: draft.label,
                        help_text: draft.help_text,
                        facet_label: None,
                        seo_label: None,
                    }],
                },
            )
            .await
            .map_err(ServerFnError::new)?;
        Ok(true)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "product/admin/create-attribute requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "product/admin/create-attribute-option")]
async fn product_admin_create_attribute_option_native(
    tenant_id: String,
    locale: String,
    draft: ProductAttributeOptionDraft,
) -> Result<bool, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = native_context().await?;
        ensure_permission(
            &auth.permissions,
            &[rustok_api::Permission::PRODUCTS_MANAGE],
            "products:manage required",
        )?;
        let tenant_id = parse_uuid(&tenant_id, "tenant_id")?;
        if tenant_id != tenant.id {
            return Err(ServerFnError::new(
                "tenant_id does not match current tenant",
            ));
        }
        service
            .create_attribute_option(
                tenant.id,
                auth.user_id,
                rustok_product::services::CreateProductAttributeOptionInput {
                    attribute_id: parse_uuid(&draft.attribute_id, "attribute_id")?,
                    code: draft.code,
                    position: draft.position,
                    metadata: empty_json(),
                    translations: vec![rustok_product::services::AttributeOptionTranslationInput {
                        locale,
                        label: draft.label,
                    }],
                },
            )
            .await
            .map_err(ServerFnError::new)?;
        Ok(true)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "product/admin/create-attribute-option requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "product/admin/create-category")]
async fn product_admin_create_category_native(
    tenant_id: String,
    locale: String,
    draft: CatalogCategoryDraft,
) -> Result<bool, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = native_context().await?;
        ensure_permission(
            &auth.permissions,
            &[rustok_api::Permission::PRODUCTS_MANAGE],
            "products:manage required",
        )?;
        let tenant_id = parse_uuid(&tenant_id, "tenant_id")?;
        if tenant_id != tenant.id {
            return Err(ServerFnError::new(
                "tenant_id does not match current tenant",
            ));
        }
        service
            .create_category(
                tenant.id,
                auth.user_id,
                rustok_product::services::CreateCatalogCategoryInput {
                    parent_id: parse_optional_uuid(draft.parent_id, "parent_id")?,
                    code: draft.code,
                    slug: draft.slug,
                    kind: parse_category_kind(&draft.kind)?,
                    position: 0,
                    rule_config: empty_json(),
                    metadata: empty_json(),
                    translations: vec![rustok_product::services::CategoryTranslationInput {
                        locale,
                        name: draft.name,
                        description: draft.description,
                        meta_title: None,
                        meta_description: None,
                    }],
                },
            )
            .await
            .map_err(ServerFnError::new)?;
        Ok(true)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "product/admin/create-category requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "product/admin/create-attribute-schema")]
async fn product_admin_create_schema_native(
    tenant_id: String,
    locale: String,
    draft: ProductAttributeSchemaDraft,
) -> Result<bool, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = native_context().await?;
        ensure_permission(
            &auth.permissions,
            &[rustok_api::Permission::PRODUCTS_MANAGE],
            "products:manage required",
        )?;
        let tenant_id = parse_uuid(&tenant_id, "tenant_id")?;
        if tenant_id != tenant.id {
            return Err(ServerFnError::new(
                "tenant_id does not match current tenant",
            ));
        }
        service
            .create_schema(
                tenant.id,
                auth.user_id,
                rustok_product::services::CreateProductAttributeSchemaInput {
                    code: draft.code,
                    metadata: empty_json(),
                    translations: vec![rustok_product::services::SchemaTranslationInput {
                        locale,
                        name: draft.name,
                        description: draft.description,
                    }],
                },
            )
            .await
            .map_err(ServerFnError::new)?;
        Ok(true)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "product/admin/create-attribute-schema requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "product/admin/set-category-schema-mode"
)]
async fn product_admin_set_category_schema_mode_native(
    tenant_id: String,
    draft: SetCategorySchemaModeDraft,
) -> Result<bool, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = native_context().await?;
        ensure_permission(
            &auth.permissions,
            &[rustok_api::Permission::PRODUCTS_MANAGE],
            "products:manage required",
        )?;
        let tenant_id = parse_uuid(&tenant_id, "tenant_id")?;
        if tenant_id != tenant.id {
            return Err(ServerFnError::new(
                "tenant_id does not match current tenant",
            ));
        }
        service
            .set_category_schema_mode(
                tenant.id,
                auth.user_id,
                rustok_product::services::SetCategorySchemaModeInput {
                    category_id: parse_uuid(&draft.category_id, "category_id")?,
                    mode: parse_schema_mode(&draft.mode)?,
                    schema_id: parse_optional_uuid(draft.schema_id, "schema_id")?,
                    clone_from_category_id: parse_optional_uuid(
                        draft.clone_from_category_id,
                        "clone_from_category_id",
                    )?,
                },
            )
            .await
            .map_err(ServerFnError::new)?;
        Ok(true)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "product/admin/set-category-schema-mode requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "product/admin/bind-schema-attribute")]
async fn product_admin_bind_schema_attribute_native(
    tenant_id: String,
    draft: BindSchemaAttributeDraft,
) -> Result<bool, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = native_context().await?;
        ensure_permission(
            &auth.permissions,
            &[rustok_api::Permission::PRODUCTS_MANAGE],
            "products:manage required",
        )?;
        let tenant_id = parse_uuid(&tenant_id, "tenant_id")?;
        if tenant_id != tenant.id {
            return Err(ServerFnError::new(
                "tenant_id does not match current tenant",
            ));
        }
        service
            .bind_schema_attribute(
                tenant.id,
                auth.user_id,
                rustok_product::services::BindSchemaAttributeInput {
                    schema_id: parse_uuid(&draft.schema_id, "schema_id")?,
                    attribute_id: parse_uuid(&draft.attribute_id, "attribute_id")?,
                    group_code: draft.group_code,
                    is_required: draft.is_required,
                    is_disabled: draft.is_disabled,
                    position: draft.position,
                    visibility_overrides: empty_json(),
                    validation_overrides: empty_json(),
                    metadata: empty_json(),
                },
            )
            .await
            .map_err(ServerFnError::new)?;
        Ok(true)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "product/admin/bind-schema-attribute requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "product/admin/create-schema-group")]
async fn product_admin_create_schema_group_native(
    tenant_id: String,
    locale: String,
    draft: ProductAttributeSchemaGroupDraft,
) -> Result<bool, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = native_context().await?;
        ensure_permission(
            &auth.permissions,
            &[rustok_api::Permission::PRODUCTS_MANAGE],
            "products:manage required",
        )?;
        let tenant_id = parse_uuid(&tenant_id, "tenant_id")?;
        if tenant_id != tenant.id {
            return Err(ServerFnError::new(
                "tenant_id does not match current tenant",
            ));
        }
        service
            .create_schema_group(
                tenant.id,
                auth.user_id,
                rustok_product::services::CreateProductAttributeSchemaGroupInput {
                    schema_id: parse_uuid(&draft.schema_id, "schema_id")?,
                    code: draft.code,
                    position: draft.position,
                    metadata: empty_json(),
                    translations: vec![rustok_product::services::AttributeGroupTranslationInput {
                        locale,
                        label: draft.label,
                    }],
                },
            )
            .await
            .map_err(ServerFnError::new)?;
        Ok(true)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "product/admin/create-schema-group requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "product/admin/create-category-group")]
async fn product_admin_create_category_group_native(
    tenant_id: String,
    locale: String,
    draft: CategoryAttributeGroupDraft,
) -> Result<bool, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = native_context().await?;
        ensure_permission(
            &auth.permissions,
            &[rustok_api::Permission::PRODUCTS_MANAGE],
            "products:manage required",
        )?;
        let tenant_id = parse_uuid(&tenant_id, "tenant_id")?;
        if tenant_id != tenant.id {
            return Err(ServerFnError::new(
                "tenant_id does not match current tenant",
            ));
        }
        service
            .create_category_group(
                tenant.id,
                auth.user_id,
                rustok_product::services::CreateCategoryAttributeGroupInput {
                    category_id: parse_uuid(&draft.category_id, "category_id")?,
                    code: draft.code,
                    position: draft.position,
                    metadata: empty_json(),
                    translations: vec![rustok_product::services::AttributeGroupTranslationInput {
                        locale,
                        label: draft.label,
                    }],
                },
            )
            .await
            .map_err(ServerFnError::new)?;
        Ok(true)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "product/admin/create-category-group requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "product/admin/bind-category-attribute")]
async fn product_admin_bind_category_attribute_native(
    tenant_id: String,
    draft: BindCategoryAttributeDraft,
) -> Result<bool, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (service, auth, tenant) = native_context().await?;
        ensure_permission(
            &auth.permissions,
            &[rustok_api::Permission::PRODUCTS_MANAGE],
            "products:manage required",
        )?;
        let tenant_id = parse_uuid(&tenant_id, "tenant_id")?;
        if tenant_id != tenant.id {
            return Err(ServerFnError::new(
                "tenant_id does not match current tenant",
            ));
        }
        service
            .bind_category_attribute(
                tenant.id,
                auth.user_id,
                rustok_product::services::BindCategoryAttributeInput {
                    category_id: parse_uuid(&draft.category_id, "category_id")?,
                    attribute_id: parse_uuid(&draft.attribute_id, "attribute_id")?,
                    group_code: draft.group_code,
                    binding_kind: parse_binding_kind(&draft.binding_kind)?,
                    is_required: draft.is_required,
                    is_disabled: draft.is_disabled,
                    position: draft.position,
                    visibility_overrides: empty_json(),
                    validation_overrides: empty_json(),
                    metadata: empty_json(),
                },
            )
            .await
            .map_err(ServerFnError::new)?;
        Ok(true)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "product/admin/bind-category-attribute requires the `ssr` feature",
        ))
    }
}
