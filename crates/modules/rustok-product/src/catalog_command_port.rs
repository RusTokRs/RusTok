use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use rustok_outbox::idempotency;
use serde::de::DeserializeOwned;
use std::future::Future;
use uuid::Uuid;

use crate::dto::{
    AddProductImageInput, CreateProductInput, CreateVariantInput, ProductImageResponse,
    ProductResponse, SetVariantAxesInput, UpdateProductImageInput, UpdateProductInput,
    UpdateVariantInput, VariantAxisConfigResponse, VariantResponse,
};
use crate::ports::{product_owner_error_facts, product_port_context_facts};
use crate::services::{fail_product_operation_receipt, with_product_operation_receipt};
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

    async fn set_variant_axes(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: SetVariantAxesInput,
    ) -> Result<Vec<VariantAxisConfigResponse>, PortError>;

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

    async fn archive_product(
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
        let request = serde_json::json!({
            "actor": &context.actor,
            "locale": &context.locale,
            "input": &input,
        });
        execute_catalog_command(
            self,
            &context,
            tenant_id,
            operation,
            &request,
            self.create_product(tenant_id, actor_id, input),
        )
        .await
    }

    async fn set_variant_axes(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: SetVariantAxesInput,
    ) -> Result<Vec<VariantAxisConfigResponse>, PortError> {
        let operation = "set_variant_axes";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        let request = serde_json::json!({
            "actor": &context.actor,
            "locale": &context.locale,
            "product_id": &product_id,
            "input": &input,
        });
        execute_catalog_command(
            self,
            &context,
            tenant_id,
            operation,
            &request,
            self.set_variant_axes(tenant_id, actor_id, product_id, input),
        )
        .await
    }

    async fn update_product(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: UpdateProductInput,
    ) -> Result<ProductResponse, PortError> {
        let operation = "update_product";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        if input.missing_expected_revision() {
            let context_facts = product_port_context_facts(&context);
            tracing::warn!(
                operation,
                tenant_id_length = context_facts.tenant_id_length,
                correlation_id_length = context_facts.correlation_id_length,
                product_id_non_nil = !product_id.is_nil(),
                code = "product.revision_required",
                "Product document update without a predecessor revision was refused"
            );
            return Err(PortError::validation(
                "product.revision_required",
                "product document update requires the revision of the read document",
            ));
        }
        let request = serde_json::json!({
            "actor": &context.actor,
            "locale": &context.locale,
            "product_id": &product_id,
            "input": &input,
        });
        execute_catalog_command(
            self,
            &context,
            tenant_id,
            operation,
            &request,
            self.update_product(tenant_id, actor_id, product_id, input),
        )
        .await
    }

    async fn delete_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<(), PortError> {
        let operation = "delete_product";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        let request = serde_json::json!({
            "actor": &context.actor,
            "locale": &context.locale,
            "product_id": &product_id,
        });
        execute_catalog_command(
            self,
            &context,
            tenant_id,
            operation,
            &request,
            self.delete_product(tenant_id, actor_id, product_id),
        )
        .await
    }

    async fn publish_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<ProductResponse, PortError> {
        let operation = "publish_product";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        let request = serde_json::json!({
            "actor": &context.actor,
            "locale": &context.locale,
            "product_id": &product_id,
        });
        execute_catalog_command(
            self,
            &context,
            tenant_id,
            operation,
            &request,
            self.publish_product(tenant_id, actor_id, product_id),
        )
        .await
    }

    async fn unpublish_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<ProductResponse, PortError> {
        let operation = "unpublish_product";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        let request = serde_json::json!({
            "actor": &context.actor,
            "locale": &context.locale,
            "product_id": &product_id,
        });
        execute_catalog_command(
            self,
            &context,
            tenant_id,
            operation,
            &request,
            self.unpublish_product(tenant_id, actor_id, product_id),
        )
        .await
    }

    async fn archive_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<ProductResponse, PortError> {
        let operation = "archive_product";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        let request = serde_json::json!({
            "actor": &context.actor,
            "locale": &context.locale,
            "product_id": &product_id,
        });
        execute_catalog_command(
            self,
            &context,
            tenant_id,
            operation,
            &request,
            self.archive_product(tenant_id, actor_id, product_id),
        )
        .await
    }

    async fn create_variant(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: CreateVariantInput,
    ) -> Result<VariantResponse, PortError> {
        let operation = "create_variant";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        let request = serde_json::json!({
            "actor": &context.actor,
            "locale": &context.locale,
            "product_id": &product_id,
            "input": &input,
        });
        execute_catalog_command(
            self,
            &context,
            tenant_id,
            operation,
            &request,
            self.create_variant(tenant_id, actor_id, product_id, input),
        )
        .await
    }

    async fn update_variant(
        &self,
        context: PortContext,
        variant_id: Uuid,
        input: UpdateVariantInput,
    ) -> Result<VariantResponse, PortError> {
        let operation = "update_variant";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        let request = serde_json::json!({
            "actor": &context.actor,
            "locale": &context.locale,
            "variant_id": &variant_id,
            "input": &input,
        });
        execute_catalog_command(
            self,
            &context,
            tenant_id,
            operation,
            &request,
            self.update_variant(tenant_id, actor_id, variant_id, input),
        )
        .await
    }

    async fn delete_variant(
        &self,
        context: PortContext,
        variant_id: Uuid,
    ) -> Result<(), PortError> {
        let operation = "delete_variant";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        let request = serde_json::json!({
            "actor": &context.actor,
            "locale": &context.locale,
            "variant_id": &variant_id,
        });
        execute_catalog_command(
            self,
            &context,
            tenant_id,
            operation,
            &request,
            self.delete_variant(tenant_id, actor_id, variant_id),
        )
        .await
    }

    async fn add_product_image(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: AddProductImageInput,
    ) -> Result<ProductImageResponse, PortError> {
        let operation = "add_product_image";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        let request = serde_json::json!({
            "actor": &context.actor,
            "locale": &context.locale,
            "product_id": &product_id,
            "input": &input,
        });
        execute_catalog_command(
            self,
            &context,
            tenant_id,
            operation,
            &request,
            self.add_product_image(tenant_id, actor_id, product_id, input),
        )
        .await
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
        let request = serde_json::json!({
            "actor": &context.actor,
            "locale": &context.locale,
            "product_id": &product_id,
            "image_id": &image_id,
            "input": &input,
        });
        execute_catalog_command(
            self,
            &context,
            tenant_id,
            operation,
            &request,
            self.update_product_image(tenant_id, actor_id, product_id, image_id, input),
        )
        .await
    }

    async fn delete_product_image(
        &self,
        context: PortContext,
        product_id: Uuid,
        image_id: Uuid,
    ) -> Result<(), PortError> {
        let operation = "delete_product_image";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        let request = serde_json::json!({
            "actor": &context.actor,
            "locale": &context.locale,
            "product_id": &product_id,
            "image_id": &image_id,
        });
        execute_catalog_command(
            self,
            &context,
            tenant_id,
            operation,
            &request,
            self.delete_product_image(tenant_id, actor_id, product_id, image_id),
        )
        .await
    }

    async fn reorder_product_images(
        &self,
        context: PortContext,
        product_id: Uuid,
        image_ids: Vec<Uuid>,
    ) -> Result<(), PortError> {
        let operation = "reorder_product_images";
        let (tenant_id, actor_id) = command_scope(&context, operation)?;
        let request = serde_json::json!({
            "actor": &context.actor,
            "locale": &context.locale,
            "product_id": &product_id,
            "image_ids": &image_ids,
        });
        execute_catalog_command(
            self,
            &context,
            tenant_id,
            operation,
            &request,
            self.reorder_product_images(tenant_id, actor_id, product_id, image_ids),
        )
        .await
    }
}

/// One admission/finalization path for every catalog command. Concrete methods
/// record the actual public result and complete this lease inside their write
/// transaction; this adapter never persists success after the owner commit.
async fn execute_catalog_command<T, F>(
    service: &CatalogService,
    context: &PortContext,
    tenant_id: Uuid,
    operation: &'static str,
    request: &serde_json::Value,
    future: F,
) -> Result<T, PortError>
where
    T: DeserializeOwned,
    F: Future<Output = Result<T, CommerceError>>,
{
    let admission = idempotency::admit(
        service.database(),
        idempotency::OwnerOperationScope::Tenant(tenant_id),
        "product",
        context.idempotency_key.as_deref().unwrap_or_default(),
        operation,
        request,
    )
    .await
    .map_err(|error| catalog_receipt_error(context, operation, error))?;
    let lease = match admission {
        idempotency::Admission::Run(lease) => lease,
        idempotency::Admission::Replay(value) => {
            return serde_json::from_value(value).map_err(|_| {
                let context_facts = product_port_context_facts(context);
                tracing::error!(
                    operation,
                    correlation_id_length = context_facts.correlation_id_length,
                    "Product catalog receipt replay could not be decoded"
                );
                PortError::invariant_violation(
                    "product.receipt_corrupt",
                    "product operation could not be replayed safely",
                )
            });
        }
        idempotency::Admission::ReplayError(error) => return Err(error),
    };
    let result = crate::media_reference::with_media_context(
        context.clone(),
        with_product_operation_receipt(lease, future),
    )
    .await;
    match result {
        Ok(value) => Ok(value),
        Err(error) => {
            let error = product_command_error(context, operation, error);
            // Await rollback before making a deterministic failure terminal.
            // Unknown storage/commit outcomes retain their reclaimable lease.
            fail_product_operation_receipt(service.database(), lease, &error)
                .await
                .map_err(|error| catalog_receipt_error(context, operation, error))?;
            Err(error)
        }
    }
}

fn catalog_receipt_error(
    context: &PortContext,
    operation: &'static str,
    error: PortError,
) -> PortError {
    let context_facts = product_port_context_facts(context);
    tracing::error!(
        operation,
        correlation_id_length = context_facts.correlation_id_length,
        tenant_id_length = context_facts.tenant_id_length,
        owner_code_length = error.code.chars().count(),
        owner_message_length = error.message.chars().count(),
        owner_kind = ?error.kind,
        retryable = error.retryable,
        "Product catalog receipt operation failed"
    );
    match error.kind {
        PortErrorKind::Validation => PortError::validation(
            "product.idempotency_invalid",
            "product idempotency request is invalid",
        ),
        PortErrorKind::Conflict => PortError::conflict(
            "product.idempotency_conflict",
            "product idempotency key conflicts with an earlier request",
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => PortError::unavailable(
            "product.receipt_unavailable",
            "product receipt storage is temporarily unavailable",
        ),
        PortErrorKind::Forbidden => PortError::forbidden(
            "product.receipt_forbidden",
            "product receipt access is denied",
        ),
        PortErrorKind::NotFound | PortErrorKind::InvariantViolation => {
            PortError::invariant_violation(
                "product.receipt_invariant",
                "product operation could not be completed safely",
            )
        }
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
        let context_facts = product_port_context_facts(context);
        tracing::warn!(
            correlation_id_length = context_facts.correlation_id_length,
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
        let context_facts = product_port_context_facts(context);
        tracing::warn!(
            correlation_id_length = context_facts.correlation_id_length,
            tenant_id_length = context_facts.tenant_id_length,
            operation,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
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
    let context_facts = product_port_context_facts(context);
    tracing::warn!(
        owner_code_length = error.code.chars().count(),
        owner_message_length = error.message.chars().count(),
        owner_kind = ?error.kind,
        retryable = error.retryable,
        correlation_id_length = context_facts.correlation_id_length,
        tenant_id_length = context_facts.tenant_id_length,
        operation,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = context_facts.idempotency_key_length,
        deadline_ms = context_facts.deadline_ms,
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
    let error_facts = product_owner_error_facts(&error);
    let context_facts = product_port_context_facts(context);
    tracing::error!(
        correlation_id_length = context_facts.correlation_id_length,
        tenant_id_length = context_facts.tenant_id_length,
        operation,
        error_kind,
        error_variant = error_facts.error_variant,
        text_field_count = error_facts.text_field_count,
        text_total_length = error_facts.text_total_length,
        uuid_field_count = error_facts.uuid_field_count,
        uuid_non_nil_count = error_facts.uuid_non_nil_count,
        opaque_payload_present = error_facts.opaque_payload_present,
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
        CommerceError::Validation(ref message)
            if crate::services::catalog::revision_conflict_of(message).is_some() =>
        {
            PortError::conflict(
                "product.revision_conflict",
                "product was modified by another editor",
            )
        }
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
        CommerceError::Core(error) => crate::error::core_port_error(&error),
        CommerceError::Conflict(_) => PortError::conflict(
            "product.conflict",
            "product request conflicts with the current state",
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
        CommerceError::Validation(message) => {
            if crate::services::catalog::revision_conflict_of(message).is_some() {
                "revision_conflict"
            } else {
                "validation"
            }
        }
        CommerceError::NoVariants => "no_variants",
        CommerceError::CannotDeleteOnlyVariant => "cannot_delete_only_variant",
        CommerceError::CannotDeletePublished => "lifecycle_conflict",
        CommerceError::Core(_) => "core",
        CommerceError::Conflict(_) => "conflict",
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
        CommerceError::Validation(message) => {
            if crate::services::catalog::revision_conflict_of(message).is_some() {
                "product.revision_conflict"
            } else {
                "product.validation"
            }
        }
        CommerceError::NoVariants => "product.no_variants",
        CommerceError::CannotDeleteOnlyVariant => "product.cannot_delete_only_variant",
        CommerceError::CannotDeletePublished => "product.lifecycle_conflict",
        CommerceError::Core(error) => crate::error::core_error_code(error),
        CommerceError::Conflict(_) => "product.conflict",
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
    fn receipt_mapping_ignores_forged_payload_and_keeps_existing_classes() {
        let context = test_context();
        for (kind, expected_kind, code) in [
            (
                PortErrorKind::Validation,
                PortErrorKind::Validation,
                "product.idempotency_invalid",
            ),
            (
                PortErrorKind::Conflict,
                PortErrorKind::Conflict,
                "product.idempotency_conflict",
            ),
            (
                PortErrorKind::Forbidden,
                PortErrorKind::Forbidden,
                "product.receipt_forbidden",
            ),
            (
                PortErrorKind::Unavailable,
                PortErrorKind::Unavailable,
                "product.receipt_unavailable",
            ),
            (
                PortErrorKind::Timeout,
                PortErrorKind::Unavailable,
                "product.receipt_unavailable",
            ),
            (
                PortErrorKind::NotFound,
                PortErrorKind::InvariantViolation,
                "product.receipt_invariant",
            ),
            (
                PortErrorKind::InvariantViolation,
                PortErrorKind::InvariantViolation,
                "product.receipt_invariant",
            ),
        ] {
            for retryable in [false, true] {
                let mapped = catalog_receipt_error(
                    &context,
                    "update_product",
                    PortError {
                        kind: kind.clone(),
                        code: "private-owner-code".into(),
                        message: "private-owner-message".into(),
                        retryable,
                    },
                );
                assert_eq!(mapped.kind, expected_kind);
                assert_eq!(mapped.code, code);
                assert!(!mapped.message.contains("private-"));
                assert_eq!(
                    mapped.retryable,
                    expected_kind == PortErrorKind::Unavailable
                );
            }
        }
    }

    #[test]
    fn command_scope_keeps_write_admission_and_uuid_checks() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let context = PortContext::new(
            tenant_id.to_string(),
            PortActor::user(actor_id.to_string()),
            "en",
            "private-correlation",
        )
        .with_idempotency_key("private-key")
        .with_deadline(std::time::Duration::from_secs(2));
        assert_eq!(
            command_scope(&context, "update_product").unwrap(),
            (tenant_id, actor_id)
        );
        let mut invalid = context.clone();
        invalid.idempotency_key = None;
        let error = command_scope(&invalid, "update_product").unwrap_err();
        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "port.idempotency_key_required");
        invalid = context.clone();
        invalid.deadline_ms = None;
        let error = command_scope(&invalid, "update_product").unwrap_err();
        assert_eq!(error.kind, PortErrorKind::Timeout);
        assert_eq!(error.code, "port.deadline_required");
        for tenant_invalid in [false, true] {
            invalid = context.clone();
            if tenant_invalid {
                invalid.tenant_id = "private-tenant".into();
            } else {
                invalid.actor.id = "private-actor".into();
            }
            let error = command_scope(&invalid, "update_product").unwrap_err();
            assert_eq!(error.kind, PortErrorKind::Validation);
            assert_eq!(
                error.code,
                if tenant_invalid {
                    "product.tenant_id_invalid"
                } else {
                    "product.actor_id_invalid"
                }
            );
            assert_eq!(error.message, "product request context is invalid");
        }
    }

    #[test]
    fn core_and_taxonomy_conflicts_have_truthful_receipt_classes() {
        let context = test_context();
        let conflict = product_command_error(
            &context,
            "update_product",
            CommerceError::Conflict("private-marker".into()),
        );
        assert_eq!(conflict.kind, PortErrorKind::Conflict);
        assert!(crate::services::is_terminal_product_failure(&conflict));
        let forbidden = product_command_error(
            &context,
            "update_product",
            CommerceError::Core(rustok_core::error::Error::Forbidden(
                "private-marker".into(),
            )),
        );
        assert_eq!(forbidden.kind, PortErrorKind::Forbidden);
        assert!(!crate::services::is_terminal_product_failure(&forbidden));
        let outage = product_command_error(
            &context,
            "update_product",
            CommerceError::Core(rustok_core::error::Error::External("private-marker".into())),
        );
        assert!(outage.retryable);
        assert!(!crate::services::is_terminal_product_failure(&outage));
        for error in [conflict, forbidden, outage] {
            assert!(!error.message.contains("private-marker"));
        }
    }

    #[test]
    fn flex_failure_classes_reach_the_command_port_without_public_details() {
        use rustok_core::field_schema::FlexError;
        let context = test_context();
        let storage = product_command_error(
            &context,
            "update_product",
            FlexError::Database("private storage detail".into()).into(),
        );
        assert_eq!(storage.kind, PortErrorKind::Unavailable);
        assert!(storage.retryable);
        let registration = product_command_error(
            &context,
            "update_product",
            FlexError::UnknownEntityType("private entity detail".into()).into(),
        );
        assert_eq!(registration.kind, PortErrorKind::InvariantViolation);
        assert!(!crate::services::is_terminal_product_failure(&registration));
        let payload = product_command_error(
            &context,
            "update_product",
            FlexError::InvalidLocale("private locale detail".into()).into(),
        );
        assert_eq!(payload.kind, PortErrorKind::Validation);
        assert!(crate::services::is_terminal_product_failure(&payload));
        for error in [storage, registration, payload] {
            assert!(!error.message.contains("private"));
        }
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
