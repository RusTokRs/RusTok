use async_graphql::{Context, ErrorExtensions, Object, Result};
use rustok_api::Permission;
use rustok_api::graphql::require_module_enabled;
use uuid::Uuid;

use rustok_product::{CatalogService, ProductCatalogSchemaService};

use super::super::{
    PRODUCT_MODULE_SLUG as MODULE_SLUG, map_product_service_error, product_mutation_actor,
    require_commerce_permission, types::*,
};
use super::helpers::*;

#[derive(Default)]
pub struct CommerceCatalogMutation;

fn invalid_catalog_input(error: impl std::fmt::Debug) -> async_graphql::Error {
    tracing::warn!(
        error = ?error,
        operation = "product_catalog_mutation",
        "invalid product catalog mutation input"
    );
    async_graphql::Error::new("Invalid product catalog input")
        .extend_with(|_, extensions| extensions.set("code", "INVALID_PRODUCT_CATALOG_INPUT"))
}

#[Object]
impl CommerceCatalogMutation {
    async fn create_product(
        &self,
        ctx: &Context<'_>,
        input: CreateProductInput,
    ) -> Result<GqlProduct> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_CREATE],
            "Permission denied: products:create required",
        )?;
        let (tenant_id, user_id) = product_mutation_actor(ctx)?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let catalog = CatalogService::new(db.clone(), event_bus.clone());
        validate_product_shipping_profile_input(
            db,
            tenant_id,
            input.shipping_profile_slug.as_deref(),
        )
        .await?;
        let domain_input = convert_create_product_input(input)?;
        let product = catalog
            .create_product(tenant_id, user_id, domain_input)
            .await
            .map_err(|error| map_product_service_error(error, "product_catalog_mutation"))?;

        Ok(product.into())
    }

    async fn update_product(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: UpdateProductInput,
    ) -> Result<GqlProduct> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_UPDATE],
            "Permission denied: products:update required",
        )?;
        let (tenant_id, user_id) = product_mutation_actor(ctx)?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let catalog = CatalogService::new(db.clone(), event_bus.clone());
        validate_product_shipping_profile_input(
            db,
            tenant_id,
            input.shipping_profile_slug.as_deref(),
        )
        .await?;
        let domain_input = crate::dto::UpdateProductInput {
            translations: input.translations.map(|translations| {
                translations
                    .into_iter()
                    .map(|translation| crate::dto::ProductTranslationInput {
                        locale: translation.locale,
                        title: translation.title,
                        handle: translation.handle,
                        description: translation.description,
                        meta_title: translation.meta_title,
                        meta_description: translation.meta_description,
                    })
                    .collect()
            }),
            seller_id: input.seller_id,
            vendor: input.vendor,
            product_type: input.product_type,
            shipping_profile_slug: input.shipping_profile_slug,
            primary_category_id: input.primary_category_id,
            tags: input.tags,
            metadata: None,
            status: input.status.map(Into::into),
        };

        let product = catalog
            .update_product(tenant_id, user_id, id, domain_input)
            .await
            .map_err(|error| map_product_service_error(error, "product_catalog_mutation"))?;

        Ok(product.into())
    }

    async fn publish_product(&self, ctx: &Context<'_>, id: Uuid) -> Result<GqlProduct> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_UPDATE],
            "Permission denied: products:update required",
        )?;
        let (tenant_id, user_id) = product_mutation_actor(ctx)?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let catalog = CatalogService::new(db.clone(), event_bus.clone());
        let product = catalog
            .publish_product(tenant_id, user_id, id)
            .await
            .map_err(|error| map_product_service_error(error, "product_catalog_mutation"))?;

        Ok(product.into())
    }

    async fn delete_product(&self, ctx: &Context<'_>, id: Uuid) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_DELETE],
            "Permission denied: products:delete required",
        )?;
        let (tenant_id, user_id) = product_mutation_actor(ctx)?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let catalog = CatalogService::new(db.clone(), event_bus.clone());
        catalog
            .delete_product(tenant_id, user_id, id)
            .await
            .map_err(|error| map_product_service_error(error, "product_catalog_mutation"))?;

        Ok(true)
    }

    async fn create_product_attribute(
        &self,
        ctx: &Context<'_>,
        locale: String,
        input: CreateProductAttributeInput,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_MANAGE],
            "Permission denied: products:manage required",
        )?;
        let (tenant_id, user_id) = product_mutation_actor(ctx)?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let service = ProductCatalogSchemaService::new(db.clone(), event_bus.clone());
        service
            .create_attribute(
                tenant_id,
                user_id,
                rustok_product::services::CreateProductAttributeInput {
                    code: input.code,
                    value_type: parse_attribute_value_type(&input.value_type)?,
                    scope: "product".to_string(),
                    is_localized: input.is_localized,
                    is_filterable: input.is_filterable,
                    is_searchable: input.is_searchable,
                    is_sortable: input.is_sortable,
                    is_comparable: false,
                    show_on_storefront: input.show_on_storefront,
                    show_in_admin_grid: true,
                    search_weight: 0,
                    filter_display: None,
                    facet_mode: None,
                    position: 0,
                    validation: serde_json::Value::Object(Default::default()),
                    default_value: None,
                    metadata: serde_json::Value::Object(Default::default()),
                    translations: vec![rustok_product::services::AttributeTranslationInput {
                        locale,
                        label: input.label,
                        help_text: input.help_text,
                        facet_label: None,
                        seo_label: None,
                    }],
                },
            )
            .await
            .map_err(|error| map_product_service_error(error, "product_catalog_mutation"))?;

        Ok(true)
    }

    async fn create_product_attribute_option(
        &self,
        ctx: &Context<'_>,
        locale: String,
        input: CreateProductAttributeOptionInput,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_MANAGE],
            "Permission denied: products:manage required",
        )?;
        let (tenant_id, user_id) = product_mutation_actor(ctx)?;
        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        ProductCatalogSchemaService::new(db.clone(), event_bus.clone())
            .create_attribute_option(
                tenant_id,
                user_id,
                rustok_product::services::CreateProductAttributeOptionInput {
                    attribute_id: input.attribute_id,
                    code: input.code,
                    position: input.position,
                    metadata: serde_json::Value::Object(Default::default()),
                    translations: vec![rustok_product::services::AttributeOptionTranslationInput {
                        locale,
                        label: input.label,
                    }],
                },
            )
            .await
            .map_err(|error| map_product_service_error(error, "product_catalog_mutation"))?;
        Ok(true)
    }

    async fn create_catalog_category(
        &self,
        ctx: &Context<'_>,
        locale: String,
        input: CreateCatalogCategoryInput,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_MANAGE],
            "Permission denied: products:manage required",
        )?;
        let (tenant_id, user_id) = product_mutation_actor(ctx)?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let service = ProductCatalogSchemaService::new(db.clone(), event_bus.clone());
        service
            .create_category(
                tenant_id,
                user_id,
                rustok_product::services::CreateCatalogCategoryInput {
                    parent_id: input.parent_id,
                    code: input.code,
                    slug: input.slug,
                    kind: parse_catalog_category_kind(&input.kind)?,
                    position: 0,
                    rule_config: serde_json::Value::Object(Default::default()),
                    metadata: serde_json::Value::Object(Default::default()),
                    translations: vec![rustok_product::services::CategoryTranslationInput {
                        locale,
                        name: input.name,
                        description: input.description,
                        meta_title: None,
                        meta_description: None,
                    }],
                },
            )
            .await
            .map_err(|error| map_product_service_error(error, "product_catalog_mutation"))?;

        Ok(true)
    }

    async fn create_product_attribute_schema(
        &self,
        ctx: &Context<'_>,
        locale: String,
        input: CreateProductAttributeSchemaInput,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_MANAGE],
            "Permission denied: products:manage required",
        )?;
        let (tenant_id, user_id) = product_mutation_actor(ctx)?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let service = ProductCatalogSchemaService::new(db.clone(), event_bus.clone());
        service
            .create_schema(
                tenant_id,
                user_id,
                rustok_product::services::CreateProductAttributeSchemaInput {
                    code: input.code,
                    metadata: serde_json::Value::Object(Default::default()),
                    translations: vec![rustok_product::services::SchemaTranslationInput {
                        locale,
                        name: input.name,
                        description: input.description,
                    }],
                },
            )
            .await
            .map_err(|error| map_product_service_error(error, "product_catalog_mutation"))?;

        Ok(true)
    }

    async fn create_product_attribute_schema_group(
        &self,
        ctx: &Context<'_>,
        locale: String,
        input: CreateProductAttributeSchemaGroupInput,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_MANAGE],
            "Permission denied: products:manage required",
        )?;
        let (tenant_id, user_id) = product_mutation_actor(ctx)?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        ProductCatalogSchemaService::new(db.clone(), event_bus.clone())
            .create_schema_group(
                tenant_id,
                user_id,
                rustok_product::services::CreateProductAttributeSchemaGroupInput {
                    schema_id: input.schema_id,
                    code: input.code,
                    position: input.position,
                    metadata: serde_json::Value::Object(Default::default()),
                    translations: vec![rustok_product::services::AttributeGroupTranslationInput {
                        locale,
                        label: input.label,
                    }],
                },
            )
            .await
            .map_err(|error| map_product_service_error(error, "product_catalog_mutation"))?;
        Ok(true)
    }

    async fn create_catalog_category_attribute_group(
        &self,
        ctx: &Context<'_>,
        locale: String,
        input: CreateCategoryAttributeGroupInput,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_MANAGE],
            "Permission denied: products:manage required",
        )?;
        let (tenant_id, user_id) = product_mutation_actor(ctx)?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        ProductCatalogSchemaService::new(db.clone(), event_bus.clone())
            .create_category_group(
                tenant_id,
                user_id,
                rustok_product::services::CreateCategoryAttributeGroupInput {
                    category_id: input.category_id,
                    code: input.code,
                    position: input.position,
                    metadata: serde_json::Value::Object(Default::default()),
                    translations: vec![rustok_product::services::AttributeGroupTranslationInput {
                        locale,
                        label: input.label,
                    }],
                },
            )
            .await
            .map_err(|error| map_product_service_error(error, "product_catalog_mutation"))?;
        Ok(true)
    }

    async fn set_catalog_category_schema_mode(
        &self,
        ctx: &Context<'_>,
        input: SetCategorySchemaModeInput,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_MANAGE],
            "Permission denied: products:manage required",
        )?;
        let (tenant_id, user_id) = product_mutation_actor(ctx)?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let service = ProductCatalogSchemaService::new(db.clone(), event_bus.clone());
        service
            .set_category_schema_mode(
                tenant_id,
                user_id,
                rustok_product::services::SetCategorySchemaModeInput {
                    category_id: input.category_id,
                    mode: parse_category_schema_mode(&input.mode)?,
                    schema_id: input.schema_id,
                    clone_from_category_id: input.clone_from_category_id,
                },
            )
            .await
            .map_err(|error| map_product_service_error(error, "product_catalog_mutation"))?;

        Ok(true)
    }

    async fn bind_product_attribute_schema_attribute(
        &self,
        ctx: &Context<'_>,
        input: BindSchemaAttributeInput,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_MANAGE],
            "Permission denied: products:manage required",
        )?;
        let (tenant_id, user_id) = product_mutation_actor(ctx)?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let service = ProductCatalogSchemaService::new(db.clone(), event_bus.clone());
        service
            .bind_schema_attribute(
                tenant_id,
                user_id,
                rustok_product::services::BindSchemaAttributeInput {
                    schema_id: input.schema_id,
                    attribute_id: input.attribute_id,
                    group_code: input.group_code,
                    is_required: input.is_required,
                    is_disabled: input.is_disabled,
                    position: input.position,
                    visibility_overrides: serde_json::Value::Object(Default::default()),
                    validation_overrides: serde_json::Value::Object(Default::default()),
                    metadata: serde_json::Value::Object(Default::default()),
                },
            )
            .await
            .map_err(|error| map_product_service_error(error, "product_catalog_mutation"))?;

        Ok(true)
    }

    async fn bind_catalog_category_attribute(
        &self,
        ctx: &Context<'_>,
        input: BindCategoryAttributeInput,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_MANAGE],
            "Permission denied: products:manage required",
        )?;
        let (tenant_id, user_id) = product_mutation_actor(ctx)?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let service = ProductCatalogSchemaService::new(db.clone(), event_bus.clone());
        service
            .bind_category_attribute(
                tenant_id,
                user_id,
                rustok_product::services::BindCategoryAttributeInput {
                    category_id: input.category_id,
                    attribute_id: input.attribute_id,
                    group_code: input.group_code,
                    binding_kind: parse_category_attribute_binding_kind(&input.binding_kind)?,
                    is_required: input.is_required,
                    is_disabled: input.is_disabled,
                    position: input.position,
                    visibility_overrides: serde_json::Value::Object(Default::default()),
                    validation_overrides: serde_json::Value::Object(Default::default()),
                    metadata: serde_json::Value::Object(Default::default()),
                },
            )
            .await
            .map_err(|error| map_product_service_error(error, "product_catalog_mutation"))?;

        Ok(true)
    }

    async fn save_product_attribute_values(
        &self,
        ctx: &Context<'_>,
        product_id: Uuid,
        locale: String,
        patches: Vec<ProductAttributeValuePatchInput>,
    ) -> Result<Vec<GqlProductAttributeValue>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_MANAGE],
            "Permission denied: products:manage required",
        )?;
        let (tenant_id, user_id) = product_mutation_actor(ctx)?;

        let patches = patches
            .into_iter()
            .map(parse_product_attribute_value_patch)
            .collect::<Result<Vec<_>>>()?;
        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        ProductCatalogSchemaService::new(db.clone(), event_bus.clone())
            .save_product_attribute_values(tenant_id, user_id, product_id, locale.trim(), patches)
            .await
            .map_err(|error| map_product_service_error(error, "product_catalog_mutation"))
            .map(|items| items.into_iter().map(Into::into).collect())
    }

    async fn clear_detached_product_attribute_values(
        &self,
        ctx: &Context<'_>,
        product_id: Uuid,
        locale: String,
        attribute_ids: Vec<Uuid>,
    ) -> Result<Vec<GqlProductAttributeValue>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_MANAGE],
            "Permission denied: products:manage required",
        )?;
        let (tenant_id, user_id) = product_mutation_actor(ctx)?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        ProductCatalogSchemaService::new(db.clone(), event_bus.clone())
            .clear_detached_product_attribute_values(
                tenant_id,
                user_id,
                product_id,
                locale.trim(),
                attribute_ids,
            )
            .await
            .map_err(|error| map_product_service_error(error, "product_catalog_mutation"))
            .map(|items| items.into_iter().map(Into::into).collect())
    }
}

fn parse_product_attribute_value_patch(
    input: ProductAttributeValuePatchInput,
) -> Result<rustok_product::services::ProductAttributeValuePatch> {
    use rustok_product::services::ProductAttributeValuePatchValue as Value;

    let payload_count = [
        input.text.is_some(),
        input.integer.is_some(),
        input.decimal.is_some(),
        input.boolean.is_some(),
        input.date.is_some(),
        input.datetime.is_some(),
        input.option_id.is_some(),
        input.option_ids.is_some(),
        input.json.is_some(),
    ]
    .into_iter()
    .filter(|present| *present)
    .count();
    let expected_payload_count = usize::from(input.kind != ProductAttributeValueInputKind::Clear);
    if payload_count != expected_payload_count {
        return Err(async_graphql::Error::new(
            "attribute value patch must contain exactly the payload selected by kind",
        ));
    }

    let missing = || async_graphql::Error::new("attribute value payload does not match kind");
    let value = match input.kind {
        ProductAttributeValueInputKind::Clear => Value::Clear,
        ProductAttributeValueInputKind::Text => Value::Text(input.text.ok_or_else(missing)?),
        ProductAttributeValueInputKind::Integer => {
            Value::Integer(input.integer.ok_or_else(missing)?)
        }
        ProductAttributeValueInputKind::Decimal => Value::Decimal(
            input
                .decimal
                .ok_or_else(missing)?
                .parse()
                .map_err(|_| async_graphql::Error::new("invalid decimal value"))?,
        ),
        ProductAttributeValueInputKind::Boolean => {
            Value::Boolean(input.boolean.ok_or_else(missing)?)
        }
        ProductAttributeValueInputKind::Date => Value::Date(
            input
                .date
                .ok_or_else(missing)?
                .parse()
                .map_err(|_| async_graphql::Error::new("invalid ISO date value"))?,
        ),
        ProductAttributeValueInputKind::Datetime => Value::Datetime(
            chrono::DateTime::parse_from_rfc3339(&input.datetime.ok_or_else(missing)?)
                .map_err(|_| async_graphql::Error::new("invalid RFC3339 datetime value"))?
                .with_timezone(&chrono::Utc),
        ),
        ProductAttributeValueInputKind::Select => {
            Value::Select(input.option_id.ok_or_else(missing)?)
        }
        ProductAttributeValueInputKind::Multiselect => {
            Value::Multiselect(input.option_ids.ok_or_else(missing)?)
        }
        ProductAttributeValueInputKind::Json => Value::Json(input.json.ok_or_else(missing)?.0),
    };
    Ok(rustok_product::services::ProductAttributeValuePatch {
        attribute_id: input.attribute_id,
        value,
    })
}

fn parse_attribute_value_type(value: &str) -> Result<rustok_product::services::AttributeValueType> {
    rustok_product::services::AttributeValueType::from_storage(value.trim())
        .map_err(invalid_catalog_input)
}

fn parse_catalog_category_kind(
    value: &str,
) -> Result<rustok_product::services::CatalogCategoryKind> {
    rustok_product::services::CatalogCategoryKind::from_storage(value.trim())
        .map_err(invalid_catalog_input)
}

fn parse_category_schema_mode(value: &str) -> Result<rustok_product::services::CategorySchemaMode> {
    rustok_product::services::CategorySchemaMode::from_storage(value.trim())
        .map_err(invalid_catalog_input)
}

fn parse_category_attribute_binding_kind(
    value: &str,
) -> Result<rustok_product::services::CategoryAttributeBindingKind> {
    rustok_product::services::CategoryAttributeBindingKind::from_storage(value.trim())
        .map_err(invalid_catalog_input)
}
