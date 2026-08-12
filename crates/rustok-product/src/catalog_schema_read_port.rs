use std::collections::HashMap;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use uuid::Uuid;

use crate::services::{
    AttributeValueType, CatalogCategoryListRecord, EffectiveAttributeSource,
    ProductAttributeFilter, ProductAttributeListRecord, ProductAttributeOptionListRecord,
    ProductAttributeSchemaListRecord, ProductAttributeValueRecord, ProductCatalogSchemaService,
    ProductResolvedAttributeFilter,
};

const LIST_ATTRIBUTES_OPERATION: &str = "list_catalog_attributes";
const LIST_CATEGORIES_OPERATION: &str = "list_catalog_categories";
const LIST_SCHEMAS_OPERATION: &str = "list_attribute_schemas";
const READ_EFFECTIVE_FORM_OPERATION: &str = "read_effective_product_form";
const READ_PRODUCT_ATTRIBUTE_VALUES_OPERATION: &str = "read_product_attribute_values";
const RESOLVE_STOREFRONT_ATTRIBUTE_FILTERS_OPERATION: &str = "resolve_storefront_attribute_filters";

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProductEffectiveFormSubject {
    Product { product_id: Uuid },
    Category { category_id: Uuid },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ProductEffectiveFormRequest {
    pub subject: ProductEffectiveFormSubject,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ProductEffectiveFormProjection {
    pub category_id: Uuid,
    pub attributes: Vec<ProductEffectiveFormAttributeProjection>,
    pub detached_attribute_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ProductEffectiveFormAttributeProjection {
    pub attribute_id: Uuid,
    pub code: String,
    pub label: String,
    pub value_type: AttributeValueType,
    pub is_localized: bool,
    pub options: Vec<ProductAttributeOptionListRecord>,
    pub group_code: Option<String>,
    pub group_label: Option<String>,
    pub is_required: bool,
    pub is_disabled: bool,
    pub position: i32,
    pub source: EffectiveAttributeSource,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ProductAttributeValuesRequest {
    pub product_id: Uuid,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ProductStorefrontAttributeFilterResolutionRequest {
    pub fallback_locale: String,
    pub filters: Vec<ProductAttributeFilter>,
}

/// Optional Product-owned read boundary for catalog schema directory, effective-form,
/// product attribute-value projections, and Storefront attribute-filter resolution.
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

    /// Optional effective-form projection. Existing schema-directory adapters remain
    /// source-compatible until they explicitly support this aggregate projection.
    async fn read_effective_form(
        &self,
        _context: PortContext,
        _request: ProductEffectiveFormRequest,
    ) -> Result<Option<ProductEffectiveFormProjection>, PortError> {
        Err(PortError::unavailable(
            "product.effective_form_unavailable",
            "product effective form is unavailable",
        ))
    }

    /// Optional product attribute-value projection. Existing schema-directory adapters
    /// remain source-compatible until they explicitly support this owner read.
    async fn read_product_attribute_values(
        &self,
        _context: PortContext,
        _request: ProductAttributeValuesRequest,
    ) -> Result<Vec<ProductAttributeValueRecord>, PortError> {
        Err(PortError::unavailable(
            "product.attribute_values_unavailable",
            "product attribute values are unavailable",
        ))
    }

    /// Optional Product-owned Storefront filter resolver. Consumers receive canonical Product term
    /// expressions and never resolve attribute/option storage identities themselves.
    async fn resolve_storefront_attribute_filters(
        &self,
        _context: PortContext,
        _request: ProductStorefrontAttributeFilterResolutionRequest,
    ) -> Result<Vec<ProductResolvedAttributeFilter>, PortError> {
        Err(PortError::unavailable(
            "product.storefront_attribute_filter_resolution_unavailable",
            "product storefront attribute filter resolution is unavailable",
        ))
    }
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

    async fn read_effective_form(
        &self,
        context: PortContext,
        request: ProductEffectiveFormRequest,
    ) -> Result<Option<ProductEffectiveFormProjection>, PortError> {
        let owner_operation = READ_EFFECTIVE_FORM_OPERATION;
        require_schema_read_context(&context, owner_operation)?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        let form = match request.subject {
            ProductEffectiveFormSubject::Product { product_id } => {
                ProductCatalogSchemaService::load_effective_form_for_product(
                    self, tenant_id, product_id,
                )
                .await
                .map_err(|error| schema_error_to_port_error(&context, owner_operation, error))?
            }
            ProductEffectiveFormSubject::Category { category_id } => Some(
                ProductCatalogSchemaService::load_effective_form_for_category(
                    self,
                    tenant_id,
                    category_id,
                    &[],
                )
                .await
                .map_err(|error| schema_error_to_port_error(&context, owner_operation, error))?,
            ),
        };
        let Some(form) = form else {
            return Ok(None);
        };

        let group_labels = ProductCatalogSchemaService::load_effective_form_group_labels(
            self,
            tenant_id,
            form.category_id,
            context.locale.as_str(),
        )
        .await
        .map_err(|error| schema_error_to_port_error(&context, owner_operation, error))?;
        let definitions =
            ProductCatalogSchemaService::list_attributes(self, tenant_id, context.locale.as_str())
                .await
                .map_err(|error| schema_error_to_port_error(&context, owner_operation, error))?
                .into_iter()
                .map(|attribute| (attribute.id, attribute))
                .collect::<HashMap<_, _>>();
        let effective_attribute_ids = form
            .attributes
            .iter()
            .map(|binding| binding.attribute_id)
            .collect::<Vec<_>>();
        let mut options_by_attribute = ProductCatalogSchemaService::list_attribute_options(
            self,
            tenant_id,
            &effective_attribute_ids,
            context.locale.as_str(),
        )
        .await
        .map_err(|error| schema_error_to_port_error(&context, owner_operation, error))?
        .into_iter()
        .fold(
            HashMap::<Uuid, Vec<ProductAttributeOptionListRecord>>::new(),
            |mut map, option| {
                map.entry(option.attribute_id).or_default().push(option);
                map
            },
        );

        let mut attributes = Vec::with_capacity(form.attributes.len());
        for binding in form.attributes {
            let Some(definition) = definitions.get(&binding.attribute_id) else {
                tracing::error!(
                    internal_attribute_id = %binding.attribute_id,
                    correlation_id = %context.correlation_id,
                    tenant_id = %context.tenant_id,
                    operation = owner_operation,
                    code = "product.attribute_definition_missing",
                    "effective Product form references a missing attribute definition"
                );
                return Err(PortError::invariant_violation(
                    "product.attribute_definition_missing",
                    "product operation could not be completed safely",
                ));
            };
            let group_label = binding
                .group_code
                .as_ref()
                .and_then(|code| group_labels.get(code).cloned());
            attributes.push(ProductEffectiveFormAttributeProjection {
                attribute_id: binding.attribute_id,
                code: definition.code.clone(),
                label: definition.label.clone(),
                value_type: definition.value_type,
                is_localized: definition.is_localized,
                options: options_by_attribute
                    .remove(&binding.attribute_id)
                    .unwrap_or_default(),
                group_code: binding.group_code,
                group_label,
                is_required: binding.is_required,
                is_disabled: binding.is_disabled,
                position: binding.position,
                source: binding.source,
            });
        }

        Ok(Some(ProductEffectiveFormProjection {
            category_id: form.category_id,
            attributes,
            detached_attribute_ids: form.detached_attribute_ids,
        }))
    }

    async fn read_product_attribute_values(
        &self,
        context: PortContext,
        request: ProductAttributeValuesRequest,
    ) -> Result<Vec<ProductAttributeValueRecord>, PortError> {
        let owner_operation = READ_PRODUCT_ATTRIBUTE_VALUES_OPERATION;
        require_schema_read_context(&context, owner_operation)?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        ProductCatalogSchemaService::load_product_attribute_values(
            self,
            tenant_id,
            request.product_id,
            context.locale.as_str(),
        )
        .await
        .map_err(|error| schema_error_to_port_error(&context, owner_operation, error))
    }

    async fn resolve_storefront_attribute_filters(
        &self,
        context: PortContext,
        request: ProductStorefrontAttributeFilterResolutionRequest,
    ) -> Result<Vec<ProductResolvedAttributeFilter>, PortError> {
        let owner_operation = RESOLVE_STOREFRONT_ATTRIBUTE_FILTERS_OPERATION;
        require_schema_read_context(&context, owner_operation)?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        ProductCatalogSchemaService::resolve_storefront_attribute_filter_terms(
            self,
            tenant_id,
            context.locale.as_str(),
            request.fallback_locale.as_str(),
            request.filters.as_slice(),
        )
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

fn parse_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
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
