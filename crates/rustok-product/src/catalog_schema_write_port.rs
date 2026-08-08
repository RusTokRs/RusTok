use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use uuid::Uuid;

use crate::services::{
    BindCategoryAttributeInput, BindSchemaAttributeInput, CatalogCategoryRecord,
    CreateCatalogCategoryInput, CreateCategoryAttributeGroupInput,
    CreateProductAttributeInput, CreateProductAttributeOptionInput,
    CreateProductAttributeSchemaGroupInput, CreateProductAttributeSchemaInput,
    ProductAttributeGroupRecord, ProductAttributeOptionRecord, ProductAttributeRecord,
    ProductAttributeSchemaRecord, ProductAttributeValuePatch, ProductAttributeValueRecord,
    ProductCatalogSchemaService, SetCategorySchemaModeInput,
};
use crate::CommerceError;

/// Transport-neutral owner boundary for Product catalog schema writes.
///
/// Every call requires `PortCallPolicy::write()`, including a non-empty caller-owned
/// idempotency identity and deadline. Consumers must receive this capability from host
/// composition rather than constructing `ProductCatalogSchemaService` directly.
///
/// The port preserves the caller idempotency identity across embedded and remote
/// providers. The embedded adapter currently delegates to the existing Product owner
/// transaction implementation; durable replay evidence remains a separate verification
/// task and must not be inferred from this source contract alone.
#[async_trait]
pub trait ProductCatalogSchemaWritePort: Send + Sync {
    async fn create_attribute(
        &self,
        context: PortContext,
        input: CreateProductAttributeInput,
    ) -> Result<ProductAttributeRecord, PortError>;

    async fn create_attribute_option(
        &self,
        context: PortContext,
        input: CreateProductAttributeOptionInput,
    ) -> Result<ProductAttributeOptionRecord, PortError>;

    async fn create_category(
        &self,
        context: PortContext,
        input: CreateCatalogCategoryInput,
    ) -> Result<CatalogCategoryRecord, PortError>;

    async fn create_schema(
        &self,
        context: PortContext,
        input: CreateProductAttributeSchemaInput,
    ) -> Result<ProductAttributeSchemaRecord, PortError>;

    async fn create_schema_group(
        &self,
        context: PortContext,
        input: CreateProductAttributeSchemaGroupInput,
    ) -> Result<ProductAttributeGroupRecord, PortError>;

    async fn create_category_group(
        &self,
        context: PortContext,
        input: CreateCategoryAttributeGroupInput,
    ) -> Result<ProductAttributeGroupRecord, PortError>;

    async fn set_category_schema_mode(
        &self,
        context: PortContext,
        input: SetCategorySchemaModeInput,
    ) -> Result<(), PortError>;

    async fn bind_schema_attribute(
        &self,
        context: PortContext,
        input: BindSchemaAttributeInput,
    ) -> Result<(), PortError>;

    async fn bind_category_attribute(
        &self,
        context: PortContext,
        input: BindCategoryAttributeInput,
    ) -> Result<(), PortError>;

    async fn save_product_attribute_values(
        &self,
        context: PortContext,
        product_id: Uuid,
        locale: String,
        patches: Vec<ProductAttributeValuePatch>,
    ) -> Result<Vec<ProductAttributeValueRecord>, PortError>;

    async fn clear_detached_product_attribute_values(
        &self,
        context: PortContext,
        product_id: Uuid,
        locale: String,
        attribute_ids: Vec<Uuid>,
    ) -> Result<Vec<ProductAttributeValueRecord>, PortError>;
}

#[async_trait]
impl ProductCatalogSchemaWritePort for ProductCatalogSchemaService {
    async fn create_attribute(
        &self,
        context: PortContext,
        input: CreateProductAttributeInput,
    ) -> Result<ProductAttributeRecord, PortError> {
        let operation = "create_attribute";
        let (tenant_id, actor_id) = schema_write_scope(&context, operation)?;
        self.create_attribute(tenant_id, actor_id, input)
            .await
            .map_err(|error| schema_write_error(&context, operation, error))
    }

    async fn create_attribute_option(
        &self,
        context: PortContext,
        input: CreateProductAttributeOptionInput,
    ) -> Result<ProductAttributeOptionRecord, PortError> {
        let operation = "create_attribute_option";
        let (tenant_id, actor_id) = schema_write_scope(&context, operation)?;
        self.create_attribute_option(tenant_id, actor_id, input)
            .await
            .map_err(|error| schema_write_error(&context, operation, error))
    }

    async fn create_category(
        &self,
        context: PortContext,
        input: CreateCatalogCategoryInput,
    ) -> Result<CatalogCategoryRecord, PortError> {
        let operation = "create_category";
        let (tenant_id, actor_id) = schema_write_scope(&context, operation)?;
        self.create_category(tenant_id, actor_id, input)
            .await
            .map_err(|error| schema_write_error(&context, operation, error))
    }

    async fn create_schema(
        &self,
        context: PortContext,
        input: CreateProductAttributeSchemaInput,
    ) -> Result<ProductAttributeSchemaRecord, PortError> {
        let operation = "create_schema";
        let (tenant_id, actor_id) = schema_write_scope(&context, operation)?;
        self.create_schema(tenant_id, actor_id, input)
            .await
            .map_err(|error| schema_write_error(&context, operation, error))
    }

    async fn create_schema_group(
        &self,
        context: PortContext,
        input: CreateProductAttributeSchemaGroupInput,
    ) -> Result<ProductAttributeGroupRecord, PortError> {
        let operation = "create_schema_group";
        let (tenant_id, actor_id) = schema_write_scope(&context, operation)?;
        self.create_schema_group(tenant_id, actor_id, input)
            .await
            .map_err(|error| schema_write_error(&context, operation, error))
    }

    async fn create_category_group(
        &self,
        context: PortContext,
        input: CreateCategoryAttributeGroupInput,
    ) -> Result<ProductAttributeGroupRecord, PortError> {
        let operation = "create_category_group";
        let (tenant_id, actor_id) = schema_write_scope(&context, operation)?;
        self.create_category_group(tenant_id, actor_id, input)
            .await
            .map_err(|error| schema_write_error(&context, operation, error))
    }

    async fn set_category_schema_mode(
        &self,
        context: PortContext,
        input: SetCategorySchemaModeInput,
    ) -> Result<(), PortError> {
        let operation = "set_category_schema_mode";
        let (tenant_id, actor_id) = schema_write_scope(&context, operation)?;
        self.set_category_schema_mode(tenant_id, actor_id, input)
            .await
            .map_err(|error| schema_write_error(&context, operation, error))
    }

    async fn bind_schema_attribute(
        &self,
        context: PortContext,
        input: BindSchemaAttributeInput,
    ) -> Result<(), PortError> {
        let operation = "bind_schema_attribute";
        let (tenant_id, actor_id) = schema_write_scope(&context, operation)?;
        self.bind_schema_attribute(tenant_id, actor_id, input)
            .await
            .map_err(|error| schema_write_error(&context, operation, error))
    }

    async fn bind_category_attribute(
        &self,
        context: PortContext,
        input: BindCategoryAttributeInput,
    ) -> Result<(), PortError> {
        let operation = "bind_category_attribute";
        let (tenant_id, actor_id) = schema_write_scope(&context, operation)?;
        self.bind_category_attribute(tenant_id, actor_id, input)
            .await
            .map_err(|error| schema_write_error(&context, operation, error))
    }

    async fn save_product_attribute_values(
        &self,
        context: PortContext,
        product_id: Uuid,
        locale: String,
        patches: Vec<ProductAttributeValuePatch>,
    ) -> Result<Vec<ProductAttributeValueRecord>, PortError> {
        let operation = "save_product_attribute_values";
        let (tenant_id, actor_id) = schema_write_scope(&context, operation)?;
        self.save_product_attribute_values(tenant_id, actor_id, product_id, &locale, patches)
            .await
            .map_err(|error| schema_write_error(&context, operation, error))
    }

    async fn clear_detached_product_attribute_values(
        &self,
        context: PortContext,
        product_id: Uuid,
        locale: String,
        attribute_ids: Vec<Uuid>,
    ) -> Result<Vec<ProductAttributeValueRecord>, PortError> {
        let operation = "clear_detached_product_attribute_values";
        let (tenant_id, actor_id) = schema_write_scope(&context, operation)?;
        self.clear_detached_product_attribute_values(
            tenant_id,
            actor_id,
            product_id,
            &locale,
            attribute_ids,
        )
        .await
        .map_err(|error| schema_write_error(&context, operation, error))
    }
}

fn schema_write_scope(
    context: &PortContext,
    operation: &'static str,
) -> Result<(Uuid, Uuid), PortError> {
    context
        .require_policy(PortCallPolicy::write())
        .map_err(|error| schema_context_error(context, operation, error))?;

    let tenant_id = Uuid::parse_str(context.tenant_id.as_str()).map_err(|_| {
        tracing::warn!(
            correlation_id = %context.correlation_id,
            operation,
            code = "product.schema_tenant_id_invalid",
            "product schema write tenant context is invalid"
        );
        PortError::validation(
            "product.schema_tenant_id_invalid",
            "product request context is invalid",
        )
    })?;
    let actor_id = Uuid::parse_str(context.actor.id.as_str()).map_err(|_| {
        tracing::warn!(
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation,
            code = "product.schema_actor_id_invalid",
            "product schema write actor context is invalid"
        );
        PortError::validation(
            "product.schema_actor_id_invalid",
            "product request context is invalid",
        )
    })?;

    Ok((tenant_id, actor_id))
}

fn schema_context_error(
    context: &PortContext,
    operation: &'static str,
    error: PortError,
) -> PortError {
    tracing::warn!(
        internal_code = %error.code,
        retryable = error.retryable,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation,
        code = "product.schema_context_invalid",
        "product schema write call context was rejected"
    );
    error
}

fn schema_write_error(
    context: &PortContext,
    operation: &'static str,
    error: CommerceError,
) -> PortError {
    let error_kind = schema_error_kind(&error);
    tracing::error!(
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation,
        error_kind,
        code = schema_error_code(&error),
        "product catalog owner schema write failed"
    );

    match error {
        CommerceError::Database(_) => PortError::unavailable(
            "product.schema_database_unavailable",
            "product schema storage is temporarily unavailable",
        ),
        CommerceError::ProductNotFound(_) => {
            PortError::not_found("product.product_not_found", "product was not found")
        }
        CommerceError::DuplicateHandle { .. } => PortError::conflict(
            "product.duplicate_handle",
            "product handle conflicts with an existing product",
        ),
        CommerceError::DuplicateSku(_) => PortError::conflict(
            "product.duplicate_sku",
            "product SKU conflicts with an existing variant",
        ),
        CommerceError::Validation(_) => {
            PortError::validation("product.schema_validation", "product schema request is invalid")
        }
        CommerceError::NoVariants => PortError::validation(
            "product.no_variants",
            "product requires at least one variant",
        ),
        CommerceError::CannotDeletePublished => PortError::conflict(
            "product.lifecycle_conflict",
            "product operation conflicts with the current state",
        ),
        CommerceError::Core(_) => PortError::invariant_violation(
            "product.schema_invariant_violation",
            "product schema operation could not be completed safely",
        ),
    }
}

fn schema_error_kind(error: &CommerceError) -> &'static str {
    match error {
        CommerceError::Database(_) => "database",
        CommerceError::ProductNotFound(_) => "not_found",
        CommerceError::DuplicateHandle { .. } => "duplicate_handle",
        CommerceError::DuplicateSku(_) => "duplicate_sku",
        CommerceError::Validation(_) => "validation",
        CommerceError::NoVariants => "no_variants",
        CommerceError::CannotDeletePublished => "lifecycle_conflict",
        CommerceError::Core(_) => "core",
    }
}

fn schema_error_code(error: &CommerceError) -> &'static str {
    match error {
        CommerceError::Database(_) => "product.schema_database_unavailable",
        CommerceError::ProductNotFound(_) => "product.product_not_found",
        CommerceError::DuplicateHandle { .. } => "product.duplicate_handle",
        CommerceError::DuplicateSku(_) => "product.duplicate_sku",
        CommerceError::Validation(_) => "product.schema_validation",
        CommerceError::NoVariants => "product.no_variants",
        CommerceError::CannotDeletePublished => "product.lifecycle_conflict",
        CommerceError::Core(_) => "product.schema_invariant_violation",
    }
}
