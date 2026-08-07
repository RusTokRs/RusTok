use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use uuid::Uuid;

use crate::services::{
    CatalogCategoryListRecord, ProductAttributeListRecord, ProductAttributeSchemaListRecord,
    ProductCatalogSchemaService,
};

const LIST_ATTRIBUTES_OPERATION: &str = "list_catalog_attributes";
const LIST_CATEGORIES_OPERATION: &str = "list_catalog_categories";
const LIST_SCHEMAS_OPERATION: &str = "list_attribute_schemas";

/// Optional Product-owned read boundary for catalog schema directory projections.
#[async_trait]
pub trait ProductCatalogSchemaReadPort: Send + Sync {
    async fn list_attributes(
        &self,
        context: PortContext,
    ) -> Result<Vec<ProductAttributeListRecord>, PortError>;

    async fn list_categories(
        &self,
        context: PortContext,
    ) -> Result<Vec<CatalogCategoryListRecord>, PortError>;

    async fn list_schemas(
        &self,
        context: PortContext,
    ) -> Result<Vec<ProductAttributeSchemaListRecord>, PortError>;
}

#[async_trait]
impl ProductCatalogSchemaReadPort for ProductCatalogSchemaService {
    async fn list_attributes(
        &self,
        context: PortContext,
    ) -> Result<Vec<ProductAttributeListRecord>, PortError> {
        let owner_operation = LIST_ATTRIBUTES_OPERATION;
        require_schema_read_context(&context, owner_operation)?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        ProductCatalogSchemaService::list_attributes(self, tenant_id, context.locale.as_str())
            .await
            .map_err(|error| schema_error_to_port_error(&context, owner_operation, error))
    }

    async fn list_categories(
        &self,
        context: PortContext,
    ) -> Result<Vec<CatalogCategoryListRecord>, PortError> {
        let owner_operation = LIST_CATEGORIES_OPERATION;
        require_schema_read_context(&context, owner_operation)?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        ProductCatalogSchemaService::list_categories(self, tenant_id, context.locale.as_str())
            .await
            .map_err(|error| schema_error_to_port_error(&context, owner_operation, error))
    }

    async fn list_schemas(
        &self,
        context: PortContext,
    ) -> Result<Vec<ProductAttributeSchemaListRecord>, PortError> {
        let owner_operation = LIST_SCHEMAS_OPERATION;
        require_schema_read_context(&context, owner_operation)?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        ProductCatalogSchemaService::list_schemas(self, tenant_id, context.locale.as_str())
            .await
            .map_err(|error| schema_error_to_port_error(&context, owner_operation, error))
    }
}

fn require_schema_read_context(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<(), PortError> {
    context
        .require_policy(PortCallPolicy::read())
        .map_err(|error| {
            tracing::warn!(
                internal_code = %error.code,
                internal_message = %error.message,
                kind = ?error.kind,
                retryable = error.retryable,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "product.schema_context_invalid",
                "Product catalog schema read context was rejected"
            );
            let PortError {
                kind,
                code,
                retryable,
                ..
            } = error;
            match kind {
                PortErrorKind::Timeout => {
                    PortError::timeout(code, "product schema request context is invalid")
                }
                PortErrorKind::Validation => {
                    PortError::validation(code, "product schema request context is invalid")
                }
                kind => PortError::new(
                    kind,
                    "product.schema_context_invalid",
                    "product schema request context is invalid",
                    retryable,
                ),
            }
        })
}

fn parse_tenant_id(context: &PortContext, owner_operation: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|error| {
        tracing::warn!(
            error = ?error,
            internal_tenant_id = %context.tenant_id,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation = owner_operation,
            code = "product.tenant_id_invalid",
            "Product catalog schema tenant context is invalid"
        );
        PortError::validation(
            "product.tenant_id_invalid",
            "product schema request context is invalid",
        )
    })
}

fn schema_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: crate::CommerceError,
) -> PortError {
    use crate::CommerceError;

    let code = match &error {
        CommerceError::Database(_) => "product.database_unavailable",
        CommerceError::Validation(_) => "product.validation",
        CommerceError::ProductNotFound(_) => "product.product_not_found",
        _ => "product.invariant_violation",
    };
    tracing::error!(
        error = ?error,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation = owner_operation,
        code,
        "Product catalog schema owner read failed"
    );

    match error {
        CommerceError::Database(_) => PortError::unavailable(
            "product.database_unavailable",
            "product storage is temporarily unavailable",
        ),
        CommerceError::Validation(_) => {
            PortError::validation("product.validation", "product request is invalid")
        }
        CommerceError::ProductNotFound(_) => {
            PortError::not_found("product.product_not_found", "product was not found")
        }
        _ => PortError::invariant_violation(
            "product.invariant_violation",
            "product operation could not be completed safely",
        ),
    }
}
