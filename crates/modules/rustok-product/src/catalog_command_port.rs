use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use uuid::Uuid;

use crate::dto::{
    AddProductImageInput, CreateProductInput, CreateVariantInput, ProductImageResponse,
    ProductResponse, UpdateProductImageInput, UpdateProductInput, UpdateVariantInput,
    VariantResponse,
};
use crate::{CatalogService, CommerceError};

/// Transport-neutral owner boundary for Product catalog lifecycle commands.
///
/// Consumers must receive this port from host composition instead of constructing
/// `CatalogService` directly. The embedded adapter remains owner-local and can be
/// replaced by a remote provider without changing Commerce transports.
#[async_trait]
pub trait ProductCatalogCommandPort: Send + Sync {
    async fn create_product(
        &self,
        context: PortContext,
        input: CreateProductInput,
    ) -> Result<ProductResponse, PortError>;

    async fn update_product(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: UpdateProductInput,
    ) -> Result<ProductResponse, PortError>;

    async fn delete_product(&self, context: PortContext, product_id: Uuid)
    -> Result<(), PortError>;

    async fn publish_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<ProductResponse, PortError>;

    async fn unpublish_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<ProductResponse, PortError>;

    async fn create_variant(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: CreateVariantInput,
    ) -> Result<VariantResponse, PortError>;

    async fn update_variant(
        &self,
        context: PortContext,
        variant_id: Uuid,
        input: UpdateVariantInput,
    ) -> Result<VariantResponse, PortError>;

    async fn delete_variant(&self, context: PortContext, variant_id: Uuid)
    -> Result<(), PortError>;

    async fn add_product_image(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: AddProductImageInput,
    ) -> Result<ProductImageResponse, PortError>;

    async fn update_product_image(
        &self,
        context: PortContext,
        product_id: Uuid,
        image_id: Uuid,
        input: UpdateProductImageInput,
    ) -> Result<ProductImageResponse, PortError>;

    async fn delete_product_image(
        &self,
        context: PortContext,
        product_id: Uuid,
        image_id: Uuid,
    ) -> Result<(), PortError>;

    async fn reorder_product_images(
        &self,
        context: PortContext,
        product_id: Uuid,
        image_ids: Vec<Uuid>,
    ) -> Result<(), PortError>;
}

#[async_trait]
impl ProductCatalogCommandPort for CatalogService {
    async fn create_product(
        &self,
        context: PortContext,
        input: CreateProductInput,
    ) -> Result<ProductResponse, PortError> {
        let operation = "create_product";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        self.create_product(tenant_id, actor_id, input)
            .await
            .map_err(|error| product_command_error(&context, operation, error))
    }

    async fn update_product(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: UpdateProductInput,
    ) -> Result<ProductResponse, PortError> {
        let operation = "update_product";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        self.update_product(tenant_id, actor_id, product_id, input)
            .await
            .map_err(|error| product_command_error(&context, operation, error))
    }

    async fn delete_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<(), PortError> {
        let operation = "delete_product";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        self.delete_product(tenant_id, actor_id, product_id)
            .await
            .map_err(|error| product_command_error(&context, operation, error))
    }

    async fn publish_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<ProductResponse, PortError> {
        let operation = "publish_product";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        self.publish_product(tenant_id, actor_id, product_id)
            .await
            .map_err(|error| product_command_error(&context, operation, error))
    }

    async fn unpublish_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<ProductResponse, PortError> {
        let operation = "unpublish_product";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        self.unpublish_product(tenant_id, actor_id, product_id)
            .await
            .map_err(|error| product_command_error(&context, operation, error))
    }

    async fn create_variant(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: CreateVariantInput,
    ) -> Result<VariantResponse, PortError> {
        let operation = "create_variant";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        self.create_variant(tenant_id, actor_id, product_id, input)
            .await
            .map_err(|error| product_command_error(&context, operation, error))
    }

    async fn update_variant(
        &self,
        context: PortContext,
        variant_id: Uuid,
        input: UpdateVariantInput,
    ) -> Result<VariantResponse, PortError> {
        let operation = "update_variant";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        self.update_variant(tenant_id, actor_id, variant_id, input)
            .await
            .map_err(|error| product_command_error(&context, operation, error))
    }

    async fn delete_variant(
        &self,
        context: PortContext,
        variant_id: Uuid,
    ) -> Result<(), PortError> {
        let operation = "delete_variant";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        self.delete_variant(tenant_id, actor_id, variant_id)
            .await
            .map_err(|error| product_command_error(&context, operation, error))
    }

    async fn add_product_image(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: AddProductImageInput,
    ) -> Result<ProductImageResponse, PortError> {
        let operation = "add_product_image";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        self.add_product_image(tenant_id, actor_id, product_id, input)
            .await
            .map_err(|error| product_command_error(&context, operation, error))
    }

    async fn update_product_image(
        &self,
        context: PortContext,
        product_id: Uuid,
        image_id: Uuid,
        input: UpdateProductImageInput,
    ) -> Result<ProductImageResponse, PortError> {
        let operation = "update_product_image";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        self.update_product_image(tenant_id, actor_id, product_id, image_id, input)
            .await
            .map_err(|error| product_command_error(&context, operation, error))
    }

    async fn delete_product_image(
        &self,
        context: PortContext,
        product_id: Uuid,
        image_id: Uuid,
    ) -> Result<(), PortError> {
        let operation = "delete_product_image";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        self.delete_product_image(tenant_id, actor_id, product_id, image_id)
            .await
            .map_err(|error| product_command_error(&context, operation, error))
    }

    async fn reorder_product_images(
        &self,
        context: PortContext,
        product_id: Uuid,
        image_ids: Vec<Uuid>,
    ) -> Result<(), PortError> {
        let operation = "reorder_product_images";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        self.reorder_product_images(tenant_id, actor_id, product_id, image_ids)
            .await
            .map_err(|error| product_command_error(&context, operation, error))
    }
}

fn command_scope(
    context: &PortContext,
    operation: &'static str,
) -> Result<(Uuid, Uuid), PortError> {
    context
        .require_policy(PortCallPolicy::write())
        .map_err(|error| command_context_error(context, operation, error))?;

    let tenant_id = Uuid::parse_str(context.tenant_id.as_str()).map_err(|_| {
        tracing::warn!(
            correlation_id = %context.correlation_id,
            operation,
            code = "product.tenant_id_invalid",
            "product command tenant context is invalid"
        );
        PortError::validation(
            "product.tenant_id_invalid",
            "product request context is invalid",
        )
    })?;
    let actor_id = Uuid::parse_str(context.actor.id.as_str()).map_err(|_| {
        tracing::warn!(
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation,
            code = "product.actor_id_invalid",
            "product command actor context is invalid"
        );
        PortError::validation(
            "product.actor_id_invalid",
            "product request context is invalid",
        )
    })?;

    Ok((tenant_id, actor_id))
}

fn command_context_error(
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
        code = "product.context_invalid",
        "product command call context was rejected"
    );
    error
}

fn product_command_error(
    context: &PortContext,
    operation: &'static str,
    error: CommerceError,
) -> PortError {
    let error_kind = product_error_kind(&error);
    tracing::error!(
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation,
        error_kind,
        code = product_error_code(&error),
        "product catalog owner command failed"
    );

    match error {
        CommerceError::Database(_) => PortError::unavailable(
            "product.database_unavailable",
            "product storage is temporarily unavailable",
        ),
        CommerceError::ProductNotFound(_) => {
            PortError::not_found("product.product_not_found", "product was not found")
        }
        CommerceError::VariantNotFound(_) => {
            PortError::not_found("product.variant_not_found", "product variant was not found")
        }
        CommerceError::ImageNotFound(_) => {
            PortError::not_found("product.image_not_found", "product image was not found")
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
            PortError::validation("product.validation", "product request is invalid")
        }
        CommerceError::NoVariants => PortError::validation(
            "product.no_variants",
            "product requires at least one variant",
        ),
        CommerceError::CannotDeleteOnlyVariant => PortError::conflict(
            "product.cannot_delete_only_variant",
            "cannot delete the only variant of a product",
        ),
        CommerceError::CannotDeletePublished => PortError::conflict(
            "product.lifecycle_conflict",
            "product operation conflicts with the current state",
        ),
        CommerceError::Core(_) => PortError::invariant_violation(
            "product.invariant_violation",
            "product operation could not be completed safely",
        ),
    }
}

fn product_error_kind(error: &CommerceError) -> &'static str {
    match error {
        CommerceError::Database(_) => "database",
        CommerceError::ProductNotFound(_) => "not_found",
        CommerceError::VariantNotFound(_) => "variant_not_found",
        CommerceError::ImageNotFound(_) => "image_not_found",
        CommerceError::DuplicateHandle { .. } => "duplicate_handle",
        CommerceError::DuplicateSku(_) => "duplicate_sku",
        CommerceError::Validation(_) => "validation",
        CommerceError::NoVariants => "no_variants",
        CommerceError::CannotDeleteOnlyVariant => "cannot_delete_only_variant",
        CommerceError::CannotDeletePublished => "lifecycle_conflict",
        CommerceError::Core(_) => "core",
    }
}

fn product_error_code(error: &CommerceError) -> &'static str {
    match error {
        CommerceError::Database(_) => "product.database_unavailable",
        CommerceError::ProductNotFound(_) => "product.product_not_found",
        CommerceError::VariantNotFound(_) => "product.variant_not_found",
        CommerceError::ImageNotFound(_) => "product.image_not_found",
        CommerceError::DuplicateHandle { .. } => "product.duplicate_handle",
        CommerceError::DuplicateSku(_) => "product.duplicate_sku",
        CommerceError::Validation(_) => "product.validation",
        CommerceError::NoVariants => "product.no_variants",
        CommerceError::CannotDeleteOnlyVariant => "product.cannot_delete_only_variant",
        CommerceError::CannotDeletePublished => "product.lifecycle_conflict",
        CommerceError::Core(_) => "product.invariant_violation",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::{PortActor, PortErrorKind};

    fn test_context() -> PortContext {
        PortContext::new(
            Uuid::nil().to_string(),
            PortActor::service("product-command-test"),
            "en",
            "corr-cmd-1",
        )
    }

    #[test]
    fn variant_command_errors_map_to_typed_port_errors() {
        let context = test_context();
        let variant_not_found = product_command_error(
            &context,
            "update_variant",
            CommerceError::VariantNotFound(Uuid::nil()),
        );
        assert_eq!(variant_not_found.kind, PortErrorKind::NotFound);
        assert_eq!(variant_not_found.code, "product.variant_not_found");
        assert_eq!(variant_not_found.message, "product variant was not found");

        let image_not_found = product_command_error(
            &context,
            "delete_product_image",
            CommerceError::ImageNotFound(Uuid::nil()),
        );
        assert_eq!(image_not_found.kind, PortErrorKind::NotFound);
        assert_eq!(image_not_found.code, "product.image_not_found");
        assert_eq!(image_not_found.message, "product image was not found");

        let cannot_delete = product_command_error(
            &context,
            "delete_variant",
            CommerceError::CannotDeleteOnlyVariant,
        );
        assert_eq!(cannot_delete.kind, PortErrorKind::Conflict);
        assert_eq!(cannot_delete.code, "product.cannot_delete_only_variant");
        assert_eq!(
            cannot_delete.message,
            "cannot delete the only variant of a product"
        );
    }
}
