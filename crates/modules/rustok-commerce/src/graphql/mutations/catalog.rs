use async_graphql::{Context, ErrorExtensions, Object, Result};
use rustok_api::graphql::require_module_enabled;
use rustok_api::{
    AuthContext, Permission, PortActor, PortContext, PortError, PortErrorKind, RequestContext,
    TenantContext,
};
use sha2::{Digest, Sha256};
use std::time::Duration;
use uuid::Uuid;

use rustok_product::{ProductCatalogCommandRuntime, ProductCatalogSchemaWritePort};

use super::super::{
    PRODUCT_MODULE_SLUG as MODULE_SLUG, product_mutation_actor, require_commerce_permission,
    types::*,
};
use super::helpers::*;

const PRODUCT_COMMAND_DEADLINE: Duration = Duration::from_secs(2);
const MAX_PRODUCT_GRAPHQL_IDEMPOTENCY_KEY_LENGTH: usize = 191;

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

fn invalid_product_idempotency_key(message: impl Into<String>) -> async_graphql::Error {
    async_graphql::Error::new(message.into())
        .extend_with(|_, extensions| extensions.set("code", "BAD_USER_INPUT"))
}

fn product_command_runtime(ctx: &Context<'_>) -> Result<ProductCatalogCommandRuntime> {
    let db = ctx.data::<sea_orm::DatabaseConnection>()?;
    let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
    Ok(
        crate::graphql_runtime::product_catalog_command_runtime_for_current_graphql_scope(
            db.clone(),
            event_bus.clone(),
        ),
    )
}

fn product_command_context(
    ctx: &Context<'_>,
    actor: (Uuid, Uuid),
    product_id: Option<Uuid>,
    idempotency_key: String,
    operation: &'static str,
) -> Result<PortContext> {
    let (tenant_id, user_id) = actor;
    let caller_key = idempotency_key.trim();
    if caller_key.is_empty() {
        return Err(invalid_product_idempotency_key(
            "Product mutation idempotency key must not be empty",
        ));
    }
    if caller_key.len() > MAX_PRODUCT_GRAPHQL_IDEMPOTENCY_KEY_LENGTH {
        return Err(invalid_product_idempotency_key(format!(
            "Product mutation idempotency key must contain at most {MAX_PRODUCT_GRAPHQL_IDEMPOTENCY_KEY_LENGTH} bytes"
        )));
    }

    let tenant = ctx.data::<TenantContext>()?;
    let auth = ctx.data::<AuthContext>()?;
    let request_context = ctx.data_opt::<RequestContext>();
    let locale = request_context
        .map(|request| request.locale.clone())
        .unwrap_or_else(|| tenant.default_locale.clone());

    let mut digest = Sha256::new();
    digest.update(tenant_id.as_bytes());
    digest.update(user_id.as_bytes());
    digest.update(operation.as_bytes());
    if let Some(product_id) = product_id {
        digest.update(product_id.as_bytes());
    }
    digest.update(caller_key.as_bytes());
    let scoped_key = format!(
        "commerce-graphql-product:{operation}:{}",
        hex::encode(digest.finalize())
    );

    let mut context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(user_id.to_string()),
        locale,
        scoped_key.clone(),
    )
    .with_idempotency_key(scoped_key)
    .with_deadline(PRODUCT_COMMAND_DEADLINE);
    for permission in &auth.permissions {
        context = context.with_claim(permission.to_string());
    }
    if let Some(channel) = request_context
        .and_then(|request| request.channel_slug.as_deref())
        .map(str::trim)
        .filter(|channel| !channel.is_empty())
    {
        context = context.with_channel(channel);
    }
    Ok(context)
}

fn product_schema_write_context(
    ctx: &Context<'_>,
    actor: (Uuid, Uuid),
    product_id: Option<Uuid>,
    idempotency_key: String,
    operation: &'static str,
) -> Result<PortContext> {
    product_command_context(ctx, actor, product_id, idempotency_key, operation)
}

fn product_command_port_error(
    context: &PortContext,
    error: PortError,
    operation: &'static str,
) -> async_graphql::Error {
    let (message, code) = match (&error.kind, error.code.as_str()) {
        (PortErrorKind::Unavailable | PortErrorKind::Timeout, _) => (
            "Product data is temporarily unavailable",
            "PRODUCT_TEMPORARILY_UNAVAILABLE",
        ),
        (PortErrorKind::NotFound, _) => ("Product was not found", "PRODUCT_NOT_FOUND"),
        (PortErrorKind::Conflict, "product.duplicate_handle") => (
            "Product handle conflicts with an existing product",
            "DUPLICATE_HANDLE",
        ),
        (PortErrorKind::Conflict, "product.duplicate_sku") => (
            "Product SKU conflicts with an existing product",
            "DUPLICATE_SKU",
        ),
        (PortErrorKind::Conflict, "product.lifecycle_conflict") => (
            "Published products must be archived before removal",
            "CANNOT_DELETE_PUBLISHED",
        ),
        (PortErrorKind::Validation, "product.no_variants") => {
            ("Product requires at least one variant", "NO_VARIANTS")
        }
        (PortErrorKind::Validation, _) => ("Product request is invalid", "PRODUCT_VALIDATION"),
        (PortErrorKind::Forbidden, _) => ("Product access is denied", "PRODUCT_ACCESS_DENIED"),
        (PortErrorKind::Conflict | PortErrorKind::InvariantViolation, _) => (
            "Product operation could not be completed safely",
            "PRODUCT_OPERATION_FAILED",
        ),
    };

    tracing::error!(
        operation,
        internal_code = %error.code,
        retryable = error.retryable,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        public_code = code,
        "Product GraphQL owner command failed"
    );

    async_graphql::Error::new(message).extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set("retryable", error.retryable);
        extensions.set("correlation_id", context.correlation_id.clone());
    })
}

fn product_schema_write_port(
    ctx: &Context<'_>,
    context: &PortContext,
) -> Result<std::sync::Arc<dyn ProductCatalogSchemaWritePort>> {
    let runtime = product_command_runtime(ctx)?;
    runtime.schema_write_port().ok_or_else(|| {
        tracing::error!(
            profile = runtime.profile().as_str(),
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            code = "product.schema_write_port_unavailable",
            "Product GraphQL schema-write capability is unavailable"
        );
        async_graphql::Error::new("Product data is temporarily unavailable").extend_with(
            |_, extensions| {
                extensions.set("code", "PRODUCT_TEMPORARILY_UNAVAILABLE");
                extensions.set("retryable", true);
                extensions.set("correlation_id", context.correlation_id.clone());
            },
        )
    })
}

#[Object]
impl CommerceCatalogMutation {
    async fn create_product(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
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
        validate_product_shipping_profile_input(
            db,
            tenant_id,
            input.shipping_profile_slug.as_deref(),
        )
        .await?;
        let domain_input = convert_create_product_input(input)?;
        let port_context = product_command_context(
            ctx,
            (tenant_id, user_id),
            None,
            idempotency_key,
            "create_product",
        )?;
        let product = product_command_runtime(ctx)?
            .command_port()
            .create_product(port_context.clone(), domain_input)
            .await
            .map_err(|error| product_command_port_error(&port_context, error, "create_product"))?;

        Ok(product.into())
    }

    async fn update_product(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
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

        let port_context = product_command_context(
            ctx,
            (tenant_id, user_id),
            Some(id),
            idempotency_key,
            "update_product",
        )?;
        let product = product_command_runtime(ctx)?
            .command_port()
            .update_product(port_context.clone(), id, domain_input)
            .await
            .map_err(|error| product_command_port_error(&port_context, error, "update_product"))?;

        Ok(product.into())
    }

    async fn publish_product(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        id: Uuid,
    ) -> Result<GqlProduct> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_UPDATE],
            "Permission denied: products:update required",
        )?;
        let (tenant_id, user_id) = product_mutation_actor(ctx)?;

        let port_context = product_command_context(
            ctx,
            (tenant_id, user_id),
            Some(id),
            idempotency_key,
            "publish_product",
        )?;
        let product = product_command_runtime(ctx)?
            .command_port()
            .publish_product(port_context.clone(), id)
            .await
            .map_err(|error| product_command_port_error(&port_context, error, "publish_product"))?;

        Ok(product.into())
    }

    async fn delete_product(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        id: Uuid,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_DELETE],
            "Permission denied: products:delete required",
        )?;
        let (tenant_id, user_id) = product_mutation_actor(ctx)?;

        let port_context = product_command_context(
            ctx,
            (tenant_id, user_id),
            Some(id),
            idempotency_key,
            "delete_product",
        )?;
        product_command_runtime(ctx)?
            .command_port()
            .delete_product(port_context.clone(), id)
            .await
            .map_err(|error| product_command_port_error(&port_context, error, "delete_product"))?;

        Ok(true)
    }

    async fn create_product_attribute(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
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
        let domain_input = rustok_product::services::CreateProductAttributeInput {
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
        };
        let port_context = product_schema_write_context(
            ctx,
            (tenant_id, user_id),
            None,
            idempotency_key,
            "create_attribute",
        )?;
        product_schema_write_port(ctx, &port_context)?
            .create_attribute(port_context.clone(), domain_input)
            .await
            .map_err(|error| {
                product_command_port_error(&port_context, error, "create_attribute")
            })?;
        Ok(true)
    }

    async fn create_product_attribute_option(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
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
        let domain_input = rustok_product::services::CreateProductAttributeOptionInput {
            attribute_id: input.attribute_id,
            code: input.code,
            position: input.position,
            metadata: serde_json::Value::Object(Default::default()),
            translations: vec![rustok_product::services::AttributeOptionTranslationInput {
                locale,
                label: input.label,
            }],
        };
        let port_context = product_schema_write_context(
            ctx,
            (tenant_id, user_id),
            None,
            idempotency_key,
            "create_attribute_option",
        )?;
        product_schema_write_port(ctx, &port_context)?
            .create_attribute_option(port_context.clone(), domain_input)
            .await
            .map_err(|error| {
                product_command_port_error(&port_context, error, "create_attribute_option")
            })?;
        Ok(true)
    }

    async fn create_catalog_category(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
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
        let domain_input = rustok_product::services::CreateCatalogCategoryInput {
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
        };
        let port_context = product_schema_write_context(
            ctx,
            (tenant_id, user_id),
            None,
            idempotency_key,
            "create_category",
        )?;
        product_schema_write_port(ctx, &port_context)?
            .create_category(port_context.clone(), domain_input)
            .await
            .map_err(|error| product_command_port_error(&port_context, error, "create_category"))?;
        Ok(true)
    }

    async fn create_product_attribute_schema(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
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
        let domain_input = rustok_product::services::CreateProductAttributeSchemaInput {
            code: input.code,
            metadata: serde_json::Value::Object(Default::default()),
            translations: vec![rustok_product::services::SchemaTranslationInput {
                locale,
                name: input.name,
                description: input.description,
            }],
        };
        let port_context = product_schema_write_context(
            ctx,
            (tenant_id, user_id),
            None,
            idempotency_key,
            "create_schema",
        )?;
        product_schema_write_port(ctx, &port_context)?
            .create_schema(port_context.clone(), domain_input)
            .await
            .map_err(|error| product_command_port_error(&port_context, error, "create_schema"))?;
        Ok(true)
    }

    async fn create_product_attribute_schema_group(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
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
        let domain_input = rustok_product::services::CreateProductAttributeSchemaGroupInput {
            schema_id: input.schema_id,
            code: input.code,
            position: input.position,
            metadata: serde_json::Value::Object(Default::default()),
            translations: vec![rustok_product::services::AttributeGroupTranslationInput {
                locale,
                label: input.label,
            }],
        };
        let port_context = product_schema_write_context(
            ctx,
            (tenant_id, user_id),
            None,
            idempotency_key,
            "create_schema_group",
        )?;
        product_schema_write_port(ctx, &port_context)?
            .create_schema_group(port_context.clone(), domain_input)
            .await
            .map_err(|error| {
                product_command_port_error(&port_context, error, "create_schema_group")
            })?;
        Ok(true)
    }

    async fn create_catalog_category_attribute_group(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
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
        let domain_input = rustok_product::services::CreateCategoryAttributeGroupInput {
            category_id: input.category_id,
            code: input.code,
            position: input.position,
            metadata: serde_json::Value::Object(Default::default()),
            translations: vec![rustok_product::services::AttributeGroupTranslationInput {
                locale,
                label: input.label,
            }],
        };
        let port_context = product_schema_write_context(
            ctx,
            (tenant_id, user_id),
            None,
            idempotency_key,
            "create_category_group",
        )?;
        product_schema_write_port(ctx, &port_context)?
            .create_category_group(port_context.clone(), domain_input)
            .await
            .map_err(|error| {
                product_command_port_error(&port_context, error, "create_category_group")
            })?;
        Ok(true)
    }

    async fn set_catalog_category_schema_mode(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        input: SetCategorySchemaModeInput,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_MANAGE],
            "Permission denied: products:manage required",
        )?;
        let (tenant_id, user_id) = product_mutation_actor(ctx)?;
        let domain_input = rustok_product::services::SetCategorySchemaModeInput {
            category_id: input.category_id,
            mode: parse_category_schema_mode(&input.mode)?,
            schema_id: input.schema_id,
            clone_from_category_id: input.clone_from_category_id,
        };
        let port_context = product_schema_write_context(
            ctx,
            (tenant_id, user_id),
            None,
            idempotency_key,
            "set_category_schema_mode",
        )?;
        product_schema_write_port(ctx, &port_context)?
            .set_category_schema_mode(port_context.clone(), domain_input)
            .await
            .map_err(|error| {
                product_command_port_error(&port_context, error, "set_category_schema_mode")
            })?;
        Ok(true)
    }

    async fn bind_product_attribute_schema_attribute(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        input: BindSchemaAttributeInput,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_MANAGE],
            "Permission denied: products:manage required",
        )?;
        let (tenant_id, user_id) = product_mutation_actor(ctx)?;
        let domain_input = rustok_product::services::BindSchemaAttributeInput {
            schema_id: input.schema_id,
            attribute_id: input.attribute_id,
            group_code: input.group_code,
            is_required: input.is_required,
            is_disabled: input.is_disabled,
            position: input.position,
            visibility_overrides: serde_json::Value::Object(Default::default()),
            validation_overrides: serde_json::Value::Object(Default::default()),
            metadata: serde_json::Value::Object(Default::default()),
        };
        let port_context = product_schema_write_context(
            ctx,
            (tenant_id, user_id),
            None,
            idempotency_key,
            "bind_schema_attribute",
        )?;
        product_schema_write_port(ctx, &port_context)?
            .bind_schema_attribute(port_context.clone(), domain_input)
            .await
            .map_err(|error| {
                product_command_port_error(&port_context, error, "bind_schema_attribute")
            })?;
        Ok(true)
    }

    async fn bind_catalog_category_attribute(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        input: BindCategoryAttributeInput,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_MANAGE],
            "Permission denied: products:manage required",
        )?;
        let (tenant_id, user_id) = product_mutation_actor(ctx)?;
        let domain_input = rustok_product::services::BindCategoryAttributeInput {
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
        };
        let port_context = product_schema_write_context(
            ctx,
            (tenant_id, user_id),
            None,
            idempotency_key,
            "bind_category_attribute",
        )?;
        product_schema_write_port(ctx, &port_context)?
            .bind_category_attribute(port_context.clone(), domain_input)
            .await
            .map_err(|error| {
                product_command_port_error(&port_context, error, "bind_category_attribute")
            })?;
        Ok(true)
    }

    async fn save_product_attribute_values(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
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
        let port_context = product_schema_write_context(
            ctx,
            (tenant_id, user_id),
            Some(product_id),
            idempotency_key,
            "save_product_attribute_values",
        )?;
        product_schema_write_port(ctx, &port_context)?
            .save_product_attribute_values(
                port_context.clone(),
                product_id,
                locale.trim().to_string(),
                patches,
            )
            .await
            .map_err(|error| {
                product_command_port_error(&port_context, error, "save_product_attribute_values")
            })
            .map(|items| items.into_iter().map(Into::into).collect())
    }

    async fn clear_detached_product_attribute_values(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
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
        let port_context = product_schema_write_context(
            ctx,
            (tenant_id, user_id),
            Some(product_id),
            idempotency_key,
            "clear_detached_product_attribute_values",
        )?;
        product_schema_write_port(ctx, &port_context)?
            .clear_detached_product_attribute_values(
                port_context.clone(),
                product_id,
                locale.trim().to_string(),
                attribute_ids,
            )
            .await
            .map_err(|error| {
                product_command_port_error(
                    &port_context,
                    error,
                    "clear_detached_product_attribute_values",
                )
            })
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
