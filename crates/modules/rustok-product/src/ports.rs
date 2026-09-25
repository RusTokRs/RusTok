use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dto::ProductResponse;
use crate::entities::product_variant;
use crate::{
    AdminProductList, AdminProductListQuery, StorefrontProductList, StorefrontProductListQuery,
    StorefrontProductSortBy, StorefrontProductSortDirection,
};

const MAX_PUBLISHED_PRODUCTS_PER_PAGE: u64 = 48;
const MAX_ADMIN_PRODUCTS_PER_PAGE: u64 = 100;
const READ_PRODUCT_PROJECTION_OPERATION: &str = "read_product_projection";
const READ_VARIANT_PRODUCT_PROJECTION_OPERATION: &str = "read_variant_product_projection";
const READ_STOREFRONT_PRODUCT_PROJECTION_OPERATION: &str = "read_storefront_product_projection";
const LIST_PUBLISHED_PRODUCTS_OPERATION: &str = "list_published_products";
const LIST_FILTERED_PUBLISHED_PRODUCTS_OPERATION: &str = "list_filtered_published_products";
const LIST_LEGACY_STOREFRONT_PRODUCTS_OPERATION: &str = "list_legacy_storefront_products";
const LIST_ADMIN_PRODUCTS_OPERATION: &str = "list_admin_products";
const LIST_LEGACY_ADMIN_PRODUCTS_OPERATION: &str = "list_legacy_admin_products";

/// Transport-neutral owner boundary for product catalog read projections.
#[async_trait]
pub trait ProductCatalogReadPort: Send + Sync {
    async fn read_product_projection(
        &self,
        context: PortContext,
        request: ProductProjectionRequest,
    ) -> Result<ProductResponse, PortError>;

    /// Resolve the owner product projection for a variant-first consumer input.
    ///
    /// Checkout consumers may receive a cart line with a variant id before a
    /// product id is materialized. The product owner resolves that association
    /// so consumers do not query product entities directly.
    async fn read_variant_product_projection(
        &self,
        context: PortContext,
        request: VariantProductProjectionRequest,
    ) -> Result<ProductResponse, PortError>;

    async fn list_published_products(
        &self,
        context: PortContext,
        request: PublishedProductsRequest,
    ) -> Result<StorefrontProductList, PortError>;

    /// Optional filtered storefront-list capability for consumers that need the full owner
    /// search/category/sort/attribute-filter contract. Existing adapters remain compatible.
    async fn list_filtered_published_products(
        &self,
        _context: PortContext,
        _request: FilteredPublishedProductsRequest,
    ) -> Result<StorefrontProductList, PortError> {
        Err(PortError::unavailable(
            "product.filtered_published_list_unavailable",
            "filtered product listing is unavailable",
        ))
    }

    /// Optional published storefront detail projection for legacy consumers that need the
    /// owner to apply lifecycle/channel visibility, locale narrowing, and public inventory.
    /// Resolve a published storefront product by variant id, keeping variant-to-product
    /// association and storefront visibility inside the Product owner.
    async fn read_storefront_variant_product_projection(
        &self,
        _context: PortContext,
        _request: StorefrontVariantProductProjectionRequest,
    ) -> Result<Option<ProductResponse>, PortError> {
        Err(PortError::unavailable(
            "product.storefront_variant_detail_unavailable",
            "storefront variant product detail is unavailable",
        ))
    }

    async fn read_storefront_product_projection(
        &self,
        _context: PortContext,
        _request: StorefrontProductProjectionRequest,
    ) -> Result<Option<ProductResponse>, PortError> {
        Err(PortError::unavailable(
            "product.storefront_detail_unavailable",
            "storefront product detail is unavailable",
        ))
    }

    /// Optional compatibility projection for the mounted legacy storefront GraphQL list.
    /// Existing adapters remain source-compatible and fail closed until they explicitly
    /// implement vendor/product-type/raw-search plus shipping-profile projection semantics.
    async fn list_legacy_storefront_products(
        &self,
        _context: PortContext,
        _request: LegacyStorefrontProductsRequest,
    ) -> Result<LegacyStorefrontProductList, PortError> {
        Err(PortError::unavailable(
            "product.legacy_storefront_list_unavailable",
            "legacy storefront product listing is unavailable",
        ))
    }

    /// Optional admin-list capability. Existing remote/test adapters remain source-compatible
    /// until they explicitly support this projection; mounted consumers fail closed otherwise.
    async fn list_admin_products(
        &self,
        _context: PortContext,
        _request: AdminProductsRequest,
    ) -> Result<AdminProductList, PortError> {
        Err(PortError::unavailable(
            "product.admin_list_unavailable",
            "product admin listing is unavailable",
        ))
    }

    /// Optional compatibility projection for the mounted legacy admin GraphQL list.
    /// Existing adapters remain source-compatible and fail closed until they explicitly
    /// implement the exact legacy list semantics.
    async fn list_legacy_admin_products(
        &self,
        _context: PortContext,
        _request: LegacyAdminProductsRequest,
    ) -> Result<AdminProductList, PortError> {
        Err(PortError::unavailable(
            "product.legacy_admin_list_unavailable",
            "legacy product admin listing is unavailable",
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductProjectionRequest {
    pub product_id: Uuid,
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VariantProductProjectionRequest {
    pub variant_id: Uuid,
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublishedProductsRequest {
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub public_channel_slug: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilteredPublishedProductsRequest {
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub public_channel_slug: Option<String>,
    pub query: StorefrontProductListQuery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorefrontProductProjectionSubject {
    ProductId { product_id: Uuid },
    Handle { handle: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorefrontVariantProductProjectionRequest {
    pub variant_id: Uuid,
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub public_channel_slug: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorefrontProductProjectionRequest {
    pub subject: StorefrontProductProjectionSubject,
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub public_channel_slug: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyStorefrontProductsRequest {
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub public_channel_slug: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub search: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyStorefrontProductList {
    pub items: Vec<LegacyStorefrontProductListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyStorefrontProductListItem {
    pub id: Uuid,
    pub status: crate::entities::product::ProductStatus,
    pub title: String,
    pub handle: String,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub shipping_profile_slug: String,
    pub tags: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminProductsRequest {
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub query: AdminProductListQuery,
    /// Compatibility-only exact filters for mounted legacy REST. Owner-native callers leave
    /// these unset and retain the typed query semantics above.
    pub raw_status: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    /// Preserve the mounted legacy REST projection: empty missing titles and normalized
    /// shipping-profile metadata fallback. Owner-native callers leave this disabled.
    pub empty_missing_title: bool,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyAdminProductsRequest {
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub search: Option<String>,
    pub status: Option<crate::entities::product::ProductStatus>,
    pub vendor: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

#[async_trait]
impl ProductCatalogReadPort for crate::CatalogService {
    async fn read_product_projection(
        &self,
        context: PortContext,
        request: ProductProjectionRequest,
    ) -> Result<ProductResponse, PortError> {
        let owner_operation = READ_PRODUCT_PROJECTION_OPERATION;
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let locale = request.locale.as_deref().unwrap_or(context.locale.as_str());
        self.get_product_with_locale_fallback(
            tenant_id,
            request.product_id,
            locale,
            request.fallback_locale.as_deref(),
        )
        .await
        .map_err(|error| product_error_to_port_error(&context, owner_operation, error))
    }

    async fn read_variant_product_projection(
        &self,
        context: PortContext,
        request: VariantProductProjectionRequest,
    ) -> Result<ProductResponse, PortError> {
        let owner_operation = READ_VARIANT_PRODUCT_PROJECTION_OPERATION;
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let variant = product_variant::Entity::find_by_id(request.variant_id)
            .filter(product_variant::Column::TenantId.eq(tenant_id))
            .one(self.database())
            .await
            .map_err(|error| product_storage_error(&context, owner_operation, error))?
            .ok_or_else(|| {
                product_variant_not_found(&context, owner_operation, request.variant_id)
            })?;
        let locale = request.locale.as_deref().unwrap_or(context.locale.as_str());

        self.get_product_with_locale_fallback(
            tenant_id,
            variant.product_id,
            locale,
            request.fallback_locale.as_deref(),
        )
        .await
        .map_err(|error| product_error_to_port_error(&context, owner_operation, error))
    }

    async fn list_published_products(
        &self,
        context: PortContext,
        request: PublishedProductsRequest,
    ) -> Result<StorefrontProductList, PortError> {
        let owner_operation = LIST_PUBLISHED_PRODUCTS_OPERATION;
        context.require_policy(PortCallPolicy::read())?;
        validate_published_products_request(&context, owner_operation, &request)?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let locale = request.locale.as_deref().unwrap_or(context.locale.as_str());
        self.list_published_products_with_locale_fallback(
            tenant_id,
            locale,
            request.fallback_locale.as_deref(),
            request.public_channel_slug.as_deref(),
            request.page,
            request.per_page,
        )
        .await
        .map_err(|error| product_error_to_port_error(&context, owner_operation, error))
    }

    async fn list_filtered_published_products(
        &self,
        context: PortContext,
        request: FilteredPublishedProductsRequest,
    ) -> Result<StorefrontProductList, PortError> {
        let owner_operation = LIST_FILTERED_PUBLISHED_PRODUCTS_OPERATION;
        context
            .require_policy(PortCallPolicy::read())
            .map_err(|error| product_context_error(&context, owner_operation, error))?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let locale = request.locale.as_deref().unwrap_or(context.locale.as_str());
        crate::CatalogService::list_published_products_with_query(
            self,
            tenant_id,
            locale,
            request.fallback_locale.as_deref(),
            request.public_channel_slug.as_deref(),
            request.query,
        )
        .await
        .map_err(|error| product_error_to_port_error(&context, owner_operation, error))
    }

    async fn read_storefront_product_projection(
        &self,
        context: PortContext,
        request: StorefrontProductProjectionRequest,
    ) -> Result<Option<ProductResponse>, PortError> {
        let owner_operation = READ_STOREFRONT_PRODUCT_PROJECTION_OPERATION;
        context
            .require_policy(PortCallPolicy::read())
            .map_err(|error| product_context_error(&context, owner_operation, error))?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let StorefrontProductProjectionRequest {
            subject,
            locale,
            fallback_locale,
            public_channel_slug,
        } = request;
        let locale = locale.as_deref().unwrap_or(context.locale.as_str());
        let result = match subject {
            StorefrontProductProjectionSubject::ProductId { product_id } => {
                self.get_published_product_by_id_with_locale_fallback(
                    tenant_id,
                    product_id,
                    locale,
                    fallback_locale.as_deref(),
                    public_channel_slug.as_deref(),
                )
                .await
            }
            StorefrontProductProjectionSubject::Handle { handle } => {
                self.get_published_product_by_handle_with_locale_fallback(
                    tenant_id,
                    handle.as_str(),
                    locale,
                    fallback_locale.as_deref(),
                    public_channel_slug.as_deref(),
                )
                .await
            }
        };
        result.map_err(|error| product_error_to_port_error(&context, owner_operation, error))
    }

    async fn read_storefront_variant_product_projection(
        &self,
        context: PortContext,
        request: StorefrontVariantProductProjectionRequest,
    ) -> Result<Option<ProductResponse>, PortError> {
        let owner_operation = "read_storefront_variant_product_projection";
        context
            .require_policy(PortCallPolicy::read())
            .map_err(|error| product_context_error(&context, owner_operation, error))?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let variant = product_variant::Entity::find_by_id(request.variant_id)
            .filter(product_variant::Column::TenantId.eq(tenant_id))
            .one(self.database())
            .await
            .map_err(|error| product_storage_error(&context, owner_operation, error))?;

        let Some(variant) = variant else {
            return Ok(None);
        };

        let locale = request.locale.as_deref().unwrap_or(context.locale.as_str());
        let product = self
            .get_published_product_by_id_with_locale_fallback(
                tenant_id,
                variant.product_id,
                locale,
                request.fallback_locale.as_deref(),
                request.public_channel_slug.as_deref(),
            )
            .await
            .map_err(|error| product_error_to_port_error(&context, owner_operation, error))?;

        Ok(product.and_then(|product| {
            product
                .variants
                .iter()
                .any(|item| item.id == request.variant_id)
                .then_some(product)
        }))
    }

    async fn list_legacy_storefront_products(
        &self,
        context: PortContext,
        request: LegacyStorefrontProductsRequest,
    ) -> Result<LegacyStorefrontProductList, PortError> {
        let owner_operation = LIST_LEGACY_STOREFRONT_PRODUCTS_OPERATION;
        context
            .require_policy(PortCallPolicy::read())
            .map_err(|error| product_context_error(&context, owner_operation, error))?;
        validate_legacy_storefront_products_request(&context, owner_operation, &request)?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let LegacyStorefrontProductsRequest {
            locale,
            fallback_locale,
            public_channel_slug,
            vendor,
            product_type,
            search,
            page,
            per_page,
        } = request;
        let locale = locale.as_deref().unwrap_or(context.locale.as_str());
        self.list_legacy_storefront_products_with_locale_fallback(
            tenant_id,
            locale,
            fallback_locale.as_deref(),
            public_channel_slug.as_deref(),
            vendor.as_deref(),
            product_type.as_deref(),
            search.as_deref(),
            page,
            per_page,
        )
        .await
        .map_err(|error| product_error_to_port_error(&context, owner_operation, error))
    }

    async fn list_admin_products(
        &self,
        context: PortContext,
        request: AdminProductsRequest,
    ) -> Result<AdminProductList, PortError> {
        let owner_operation = LIST_ADMIN_PRODUCTS_OPERATION;
        context
            .require_policy(PortCallPolicy::read())
            .map_err(|error| product_context_error(&context, owner_operation, error))?;
        validate_admin_products_request(&context, owner_operation, &request)?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let AdminProductsRequest {
            locale,
            fallback_locale,
            query,
            raw_status,
            vendor,
            product_type,
            empty_missing_title,
            page,
            per_page,
        } = request;
        let locale = locale.as_deref().unwrap_or(context.locale.as_str());
        let page = page.max(1);
        self.list_admin_products_with_compatibility_query(
            tenant_id,
            locale,
            fallback_locale.as_deref(),
            query,
            page,
            per_page,
            raw_status.as_deref(),
            vendor.as_deref(),
            product_type.as_deref(),
            empty_missing_title,
            empty_missing_title,
        )
        .await
        .map_err(|error| product_error_to_port_error(&context, owner_operation, error))
    }

    async fn list_legacy_admin_products(
        &self,
        context: PortContext,
        request: LegacyAdminProductsRequest,
    ) -> Result<AdminProductList, PortError> {
        let owner_operation = LIST_LEGACY_ADMIN_PRODUCTS_OPERATION;
        context
            .require_policy(PortCallPolicy::read())
            .map_err(|error| product_context_error(&context, owner_operation, error))?;
        validate_legacy_admin_products_request(&context, owner_operation, &request)?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let LegacyAdminProductsRequest {
            locale,
            fallback_locale,
            search,
            status,
            vendor,
            page,
            per_page,
        } = request;
        let locale = locale.as_deref().unwrap_or(context.locale.as_str());
        let page = page.max(1);
        self.list_admin_products_with_compatibility_query(
            tenant_id,
            locale,
            fallback_locale.as_deref(),
            AdminProductListQuery {
                search,
                status,
                category_id: None,
                sort_by: StorefrontProductSortBy::CreatedAt,
                sort_direction: StorefrontProductSortDirection::Desc,
                attribute_filters: Vec::new(),
            },
            page,
            per_page,
            None,
            vendor.as_deref(),
            None,
            false,
            true,
        )
        .await
        .map_err(|error| product_error_to_port_error(&context, owner_operation, error))
    }
}

fn validate_published_products_request(
    context: &PortContext,
    owner_operation: &'static str,
    request: &PublishedProductsRequest,
) -> Result<(), PortError> {
    if request.page == 0 {
        tracing::warn!(
            page = request.page,
            per_page = request.per_page,
            correlation_id = %context.correlation_id,
            tenant_id_length = context.tenant_id.chars().count(),
            operation = owner_operation,
            code = "product.page_invalid",
            "published product page validation failed"
        );
        return Err(PortError::validation(
            "product.page_invalid",
            "published products page is invalid",
        ));
    }
    if !(1..=MAX_PUBLISHED_PRODUCTS_PER_PAGE).contains(&request.per_page) {
        tracing::warn!(
            page = request.page,
            per_page = request.per_page,
            max_per_page = MAX_PUBLISHED_PRODUCTS_PER_PAGE,
            correlation_id = %context.correlation_id,
            tenant_id_length = context.tenant_id.chars().count(),
            operation = owner_operation,
            code = "product.per_page_invalid",
            "published product page-size validation failed"
        );
        return Err(PortError::validation(
            "product.per_page_invalid",
            "published products page size is invalid",
        ));
    }
    Ok(())
}

fn validate_legacy_storefront_products_request(
    context: &PortContext,
    owner_operation: &'static str,
    request: &LegacyStorefrontProductsRequest,
) -> Result<(), PortError> {
    if request.page == 0 {
        tracing::warn!(
            page = request.page,
            per_page = request.per_page,
            correlation_id = %context.correlation_id,
            tenant_id_length = context.tenant_id.chars().count(),
            operation = owner_operation,
            code = "product.page_invalid",
            "legacy storefront product page validation failed"
        );
        return Err(PortError::validation(
            "product.page_invalid",
            "published products page is invalid",
        ));
    }
    if !(1..=MAX_PUBLISHED_PRODUCTS_PER_PAGE).contains(&request.per_page) {
        tracing::warn!(
            page = request.page,
            per_page = request.per_page,
            max_per_page = MAX_PUBLISHED_PRODUCTS_PER_PAGE,
            correlation_id = %context.correlation_id,
            tenant_id_length = context.tenant_id.chars().count(),
            operation = owner_operation,
            code = "product.per_page_invalid",
            "legacy storefront product page-size validation failed"
        );
        return Err(PortError::validation(
            "product.per_page_invalid",
            "published products page size is invalid",
        ));
    }
    Ok(())
}

fn validate_admin_products_request(
    context: &PortContext,
    owner_operation: &'static str,
    request: &AdminProductsRequest,
) -> Result<(), PortError> {
    if !(1..=MAX_ADMIN_PRODUCTS_PER_PAGE).contains(&request.per_page) {
        tracing::warn!(
            page = request.page,
            per_page = request.per_page,
            max_per_page = MAX_ADMIN_PRODUCTS_PER_PAGE,
            correlation_id = %context.correlation_id,
            tenant_id_length = context.tenant_id.chars().count(),
            operation = owner_operation,
            code = "product.per_page_invalid",
            "admin product page-size validation failed"
        );
        return Err(PortError::validation(
            "product.per_page_invalid",
            "admin products page size is invalid",
        ));
    }
    Ok(())
}

fn validate_legacy_admin_products_request(
    context: &PortContext,
    owner_operation: &'static str,
    request: &LegacyAdminProductsRequest,
) -> Result<(), PortError> {
    if !(1..=MAX_ADMIN_PRODUCTS_PER_PAGE).contains(&request.per_page) {
        tracing::warn!(
            page = request.page,
            per_page = request.per_page,
            max_per_page = MAX_ADMIN_PRODUCTS_PER_PAGE,
            correlation_id = %context.correlation_id,
            tenant_id_length = context.tenant_id.chars().count(),
            operation = owner_operation,
            code = "product.per_page_invalid",
            "legacy admin product page-size validation failed"
        );
        return Err(PortError::validation(
            "product.per_page_invalid",
            "admin products page size is invalid",
        ));
    }
    Ok(())
}

struct ProductPortContextFacts {
    correlation_id_length: usize,
    tenant_id_length: usize,
    actor_kind: &'static str,
    actor_id_length: usize,
    claim_count: usize,
    role_count: usize,
    channel_present: bool,
    channel_length: Option<usize>,
    locale_length: usize,
    causation_id_present: bool,
    causation_id_length: Option<usize>,
    traceparent_present: bool,
    traceparent_length: Option<usize>,
    idempotency_key_present: bool,
    idempotency_key_length: Option<usize>,
    deadline_ms: Option<u64>,
}

struct ProductOwnerErrorFacts {
    error_variant: &'static str,
    text_field_count: usize,
    text_total_length: usize,
    uuid_field_count: usize,
    uuid_non_nil_count: usize,
    opaque_payload_present: bool,
}

fn product_port_context_facts(context: &PortContext) -> ProductPortContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    ProductPortContextFacts {
        correlation_id_length: context.correlation_id.chars().count(),
        tenant_id_length: context.tenant_id.chars().count(),
        actor_kind,
        actor_id_length: context.actor.id.chars().count(),
        claim_count: context.claims.len(),
        role_count: context.roles.len(),
        channel_present: context.channel.is_some(),
        channel_length: context.channel.as_ref().map(|value| value.chars().count()),
        locale_length: context.locale.chars().count(),
        causation_id_present: context.causation_id.is_some(),
        causation_id_length: context
            .causation_id
            .as_ref()
            .map(|value| value.chars().count()),
        traceparent_present: context.traceparent.is_some(),
        traceparent_length: context
            .traceparent
            .as_ref()
            .map(|value| value.chars().count()),
        idempotency_key_present: context.idempotency_key.is_some(),
        idempotency_key_length: context
            .idempotency_key
            .as_ref()
            .map(|value| value.chars().count()),
        deadline_ms: context.deadline_ms,
    }
}

impl ProductOwnerErrorFacts {
    fn empty(error_variant: &'static str) -> Self {
        Self {
            error_variant,
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: false,
        }
    }

    fn text(error_variant: &'static str, values: &[&str]) -> Self {
        Self {
            text_field_count: values.len(),
            text_total_length: values.iter().map(|value| value.chars().count()).sum(),
            ..Self::empty(error_variant)
        }
    }

    fn uuids(error_variant: &'static str, values: &[Uuid]) -> Self {
        Self {
            uuid_field_count: values.len(),
            uuid_non_nil_count: values.iter().filter(|value| !value.is_nil()).count(),
            ..Self::empty(error_variant)
        }
    }

    fn opaque(error_variant: &'static str) -> Self {
        Self {
            opaque_payload_present: true,
            ..Self::empty(error_variant)
        }
    }
}

fn product_owner_error_facts(error: &crate::error::CommerceError) -> ProductOwnerErrorFacts {
    use crate::error::CommerceError;

    match error {
        CommerceError::Database(_) => ProductOwnerErrorFacts::opaque("database"),
        CommerceError::ProductNotFound(value) => {
            ProductOwnerErrorFacts::uuids("product_not_found", &[*value])
        }
        CommerceError::DuplicateHandle { handle, locale } => {
            ProductOwnerErrorFacts::text("duplicate_handle", &[handle.as_str(), locale.as_str()])
        }
        CommerceError::DuplicateSku(value) => {
            ProductOwnerErrorFacts::text("duplicate_sku", &[value.as_str()])
        }
        CommerceError::Validation(value) => {
            ProductOwnerErrorFacts::text("validation", &[value.as_str()])
        }
        CommerceError::NoVariants => ProductOwnerErrorFacts::empty("no_variants"),
        CommerceError::VariantNotFound(value) => {
            ProductOwnerErrorFacts::uuids("variant_not_found", &[*value])
        }
        CommerceError::ImageNotFound(value) => {
            ProductOwnerErrorFacts::uuids("image_not_found", &[*value])
        }
        CommerceError::CannotDeleteOnlyVariant => {
            ProductOwnerErrorFacts::empty("cannot_delete_only_variant")
        }
        CommerceError::CannotDeletePublished => {
            ProductOwnerErrorFacts::empty("cannot_delete_published")
        }
        CommerceError::Core(_) => ProductOwnerErrorFacts::opaque("core"),
    }
}

fn product_port_error_kind(kind: &PortErrorKind) -> &'static str {
    match kind {
        PortErrorKind::Validation => "validation",
        PortErrorKind::NotFound => "not_found",
        PortErrorKind::Conflict => "conflict",
        PortErrorKind::Forbidden => "forbidden",
        PortErrorKind::Unavailable => "unavailable",
        PortErrorKind::Timeout => "timeout",
        PortErrorKind::InvariantViolation => "invariant_violation",
    }
}

fn log_product_port_failure(
    context: &PortContext,
    owner_operation: &'static str,
    code: &'static str,
    error_facts: &ProductOwnerErrorFacts,
    technical_failure: bool,
) {
    let context_facts = product_port_context_facts(context);
    if technical_failure {
        tracing::error!(
            owner = "rustok_product",
            correlation_id = %context.correlation_id,
            correlation_id_length = context_facts.correlation_id_length,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            operation = owner_operation,
            code,
            error_variant = error_facts.error_variant,
            text_field_count = error_facts.text_field_count,
            text_total_length = error_facts.text_total_length,
            uuid_field_count = error_facts.uuid_field_count,
            uuid_non_nil_count = error_facts.uuid_non_nil_count,
            opaque_payload_present = error_facts.opaque_payload_present,
            boundary = "product_catalog_read_port",
            "product catalog owner operation failed with bounded diagnostics"
        );
    } else {
        tracing::warn!(
            owner = "rustok_product",
            correlation_id = %context.correlation_id,
            correlation_id_length = context_facts.correlation_id_length,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            operation = owner_operation,
            code,
            error_variant = error_facts.error_variant,
            text_field_count = error_facts.text_field_count,
            text_total_length = error_facts.text_total_length,
            uuid_field_count = error_facts.uuid_field_count,
            uuid_non_nil_count = error_facts.uuid_non_nil_count,
            opaque_payload_present = error_facts.opaque_payload_present,
            boundary = "product_catalog_read_port",
            "product catalog owner operation was rejected with bounded diagnostics"
        );
    }
}

fn log_product_context_rejection(
    context: &PortContext,
    operation: &'static str,
    code: &'static str,
    parse_target: &'static str,
) {
    let context_facts = product_port_context_facts(context);
    tracing::warn!(
        owner = "rustok_product",
        correlation_id = %context.correlation_id,
        correlation_id_length = context_facts.correlation_id_length,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        claim_count = context_facts.claim_count,
        role_count = context_facts.role_count,
        channel_present = context_facts.channel_present,
        channel_length = ?context_facts.channel_length,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        causation_id_length = ?context_facts.causation_id_length,
        traceparent_present = context_facts.traceparent_present,
        traceparent_length = ?context_facts.traceparent_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = ?context_facts.idempotency_key_length,
        deadline_ms = ?context_facts.deadline_ms,
        operation,
        code,
        parse_target,
        parse_failed = true,
        boundary = "product_catalog_read_port",
        "product catalog port context was rejected with bounded diagnostics"
    );
}

fn parse_port_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        log_product_context_rejection(
            context,
            owner_operation,
            "product.tenant_id_invalid",
            "tenant_id",
        );
        PortError::validation(
            "product.tenant_id_invalid",
            "product request context is invalid",
        )
    })
}

fn product_context_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: PortError,
) -> PortError {
    let error_kind = product_port_error_kind(&error.kind);
    let error_code_length = error.code.chars().count();
    let error_message_length = error.message.chars().count();
    let context_facts = product_port_context_facts(context);
    tracing::warn!(
        owner = "rustok_product",
        correlation_id = %context.correlation_id,
        correlation_id_length = context_facts.correlation_id_length,
        tenant_id_length = context_facts.tenant_id_length,
        operation = owner_operation,
        error_kind,
        error_code_length,
        error_message_present = !error.message.trim().is_empty(),
        error_message_length,
        retryable = error.retryable,
        boundary = "product_catalog_read_port",
        "product catalog call context was rejected"
    );

    let PortError {
        kind,
        code,
        retryable,
        ..
    } = error;
    match kind {
        PortErrorKind::Timeout => PortError::timeout(code, "product request context is invalid"),
        PortErrorKind::Validation => {
            PortError::validation(code, "product request context is invalid")
        }
        kind => PortError::new(
            kind,
            "product.context_invalid",
            "product request context is invalid",
            retryable,
        ),
    }
}

fn product_storage_error(
    context: &PortContext,
    owner_operation: &'static str,
    _error: sea_orm::DbErr,
) -> PortError {
    let context_facts = product_port_context_facts(context);
    tracing::error!(
        owner = "rustok_product",
        correlation_id = %context.correlation_id,
        correlation_id_length = context_facts.correlation_id_length,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        operation = owner_operation,
        error_variant = "database",
        boundary = "product_catalog_read_port",
        "product catalog storage failed with bounded diagnostics"
    );
    PortError::unavailable(
        "product.database_unavailable",
        "product storage is temporarily unavailable",
    )
}

fn product_variant_not_found(
    context: &PortContext,
    owner_operation: &'static str,
    variant_id: Uuid,
) -> PortError {
    let context_facts = product_port_context_facts(context);
    tracing::warn!(
        owner = "rustok_product",
        correlation_id = %context.correlation_id,
        correlation_id_length = context_facts.correlation_id_length,
        tenant_id_length = context_facts.tenant_id_length,
        operation = owner_operation,
        variant_id_non_nil = !variant_id.is_nil(),
        code = "product.variant_not_found",
        boundary = "product_catalog_read_port",
        "product variant projection was not found"
    );
    PortError::not_found("product.variant_not_found", "product variant was not found")
}

fn product_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: crate::error::CommerceError,
) -> PortError {
    use crate::error::CommerceError;

    let code = product_error_code(&error);
    let error_facts = product_owner_error_facts(&error);
    let technical_failure = matches!(
        &error,
        CommerceError::Database(_) | CommerceError::Core(_)
    );
    log_product_port_failure(
        context,
        owner_operation,
        code,
        &error_facts,
        technical_failure,
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
        CommerceError::CannotDeleteOnlyVariant => PortError::conflict(
            "product.cannot_delete_only_variant",
            "cannot delete the only variant of a product",
        ),
        CommerceError::DuplicateHandle { .. } => PortError::conflict(
            "product.duplicate_handle",
            "product handle conflicts with an existing product",
        ),
        CommerceError::DuplicateSku(_) => PortError::conflict(
            "product.duplicate_sku",
            "product SKU conflicts with an existing product",
        ),
        CommerceError::Validation(_) => {
            PortError::validation("product.validation", "product request is invalid")
        }
        CommerceError::NoVariants => PortError::conflict(
            "product.no_variants",
            "product must have at least one variant",
        ),
        CommerceError::CannotDeletePublished => PortError::conflict(
            "product.cannot_delete_published",
            "cannot delete a published product",
        ),
        CommerceError::Core(_) => PortError::invariant_violation(
            "product.invariant_violation",
            "product operation could not be completed safely",
        ),
    }
}

fn product_error_code(error: &crate::error::CommerceError) -> &'static str {
    use crate::error::CommerceError;

    match error {
        CommerceError::Database(_) => "product.database_unavailable",
        CommerceError::ProductNotFound(_) => "product.product_not_found",
        CommerceError::VariantNotFound(_) => "product.variant_not_found",
        CommerceError::ImageNotFound(_) => "product.image_not_found",
        CommerceError::CannotDeleteOnlyVariant => "product.cannot_delete_only_variant",
        CommerceError::DuplicateHandle { .. } => "product.duplicate_handle",
        CommerceError::Validation(_) => "product.validation",
        _ => "product.invariant_violation",
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::error::CommerceError;
    use rustok_api::{PortActor, PortErrorKind};

    use super::*;

    fn base_context() -> PortContext {
        PortContext::new(
            Uuid::nil().to_string(),
            PortActor::service("product-contract-test"),
            "ru",
            "corr-product-a",
        )
    }

    fn published_request() -> PublishedProductsRequest {
        PublishedProductsRequest {
            locale: None,
            fallback_locale: Some("en".to_string()),
            public_channel_slug: Some("web".to_string()),
            page: 1,
            per_page: 24,
        }
    }

    #[test]
    fn product_read_ports_require_deadline_policy() {
        let error = base_context()
            .require_policy(PortCallPolicy::read())
            .expect_err("product read ports require deadline semantics");

        assert_eq!(error.kind, PortErrorKind::Timeout);
        assert_eq!(error.code, "port.deadline_required");
        assert!(error.retryable);

        assert!(
            base_context()
                .with_deadline(Duration::from_secs(3))
                .require_policy(PortCallPolicy::read())
                .is_ok()
        );
    }

    #[test]
    fn product_port_tenant_scope_requires_uuid_context() {
        let context = PortContext::new(
            "tenant-slug",
            PortActor::service("product-contract-test"),
            "ru",
            "corr-product-b",
        );
        let error = parse_port_tenant_id(&context, READ_PRODUCT_PROJECTION_OPERATION)
            .expect_err("product port tenant_id must be a UUID");

        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "product.tenant_id_invalid");
        assert_eq!(error.message, "product request context is invalid");
        assert!(!error.retryable);

        assert_eq!(
            parse_port_tenant_id(&base_context(), READ_PRODUCT_PROJECTION_OPERATION)
                .expect("nil UUID is a valid UUID"),
            Uuid::nil()
        );
    }

    #[test]
    fn published_products_request_enforces_bounded_pagination() {
        let context = base_context();
        let mut request = published_request();
        request.page = 0;

        let error = validate_published_products_request(
            &context,
            LIST_PUBLISHED_PRODUCTS_OPERATION,
            &request,
        )
        .expect_err("page zero must be rejected before storage access");

        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "product.page_invalid");
        assert_eq!(error.message, "published products page is invalid");

        request.page = 1;
        request.per_page = MAX_PUBLISHED_PRODUCTS_PER_PAGE + 1;

        let error = validate_published_products_request(
            &context,
            LIST_PUBLISHED_PRODUCTS_OPERATION,
            &request,
        )
        .expect_err("oversized page size must be rejected before storage access");

        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "product.per_page_invalid");
        assert_eq!(error.message, "published products page size is invalid");

        request.per_page = MAX_PUBLISHED_PRODUCTS_PER_PAGE;
        assert!(
            validate_published_products_request(
                &context,
                LIST_PUBLISHED_PRODUCTS_OPERATION,
                &request,
            )
            .is_ok()
        );
    }

    #[test]
    fn commerce_errors_map_to_typed_product_port_errors() {
        let context = base_context();
        let not_found = product_error_to_port_error(
            &context,
            READ_PRODUCT_PROJECTION_OPERATION,
            CommerceError::ProductNotFound(Uuid::nil()),
        );
        assert_eq!(not_found.kind, PortErrorKind::NotFound);
        assert_eq!(not_found.code, "product.product_not_found");
        assert_eq!(not_found.message, "product was not found");
        assert!(!not_found.retryable);

        let validation = product_error_to_port_error(
            &context,
            READ_PRODUCT_PROJECTION_OPERATION,
            CommerceError::Validation("bad".to_string()),
        );
        assert_eq!(validation.kind, PortErrorKind::Validation);
        assert_eq!(validation.code, "product.validation");
        assert_eq!(validation.message, "product request is invalid");
        assert!(!validation.retryable);

        let duplicate = product_error_to_port_error(
            &context,
            READ_PRODUCT_PROJECTION_OPERATION,
            CommerceError::DuplicateHandle {
                handle: "sku-a".to_string(),
                locale: "ru".to_string(),
            },
        );
        assert_eq!(duplicate.kind, PortErrorKind::Conflict);
        assert_eq!(duplicate.code, "product.duplicate_handle");
        assert_eq!(
            duplicate.message,
            "product handle conflicts with an existing product"
        );
        assert!(!duplicate.retryable);

        let variant_not_found = product_error_to_port_error(
            &context,
            READ_PRODUCT_PROJECTION_OPERATION,
            CommerceError::VariantNotFound(Uuid::nil()),
        );
        assert_eq!(variant_not_found.kind, PortErrorKind::NotFound);
        assert_eq!(variant_not_found.code, "product.variant_not_found");
        assert_eq!(variant_not_found.message, "product variant was not found");
        assert!(!variant_not_found.retryable);

        let cannot_delete_only = product_error_to_port_error(
            &context,
            READ_PRODUCT_PROJECTION_OPERATION,
            CommerceError::CannotDeleteOnlyVariant,
        );
        assert_eq!(cannot_delete_only.kind, PortErrorKind::Conflict);
        assert_eq!(
            cannot_delete_only.code,
            "product.cannot_delete_only_variant"
        );
        assert_eq!(
            cannot_delete_only.message,
            "cannot delete the only variant of a product"
        );
        assert!(!cannot_delete_only.retryable);

        let image_not_found = product_error_to_port_error(
            &context,
            READ_PRODUCT_PROJECTION_OPERATION,
            CommerceError::ImageNotFound(Uuid::nil()),
        );
        assert_eq!(image_not_found.kind, PortErrorKind::NotFound);
        assert_eq!(image_not_found.code, "product.image_not_found");
        assert_eq!(image_not_found.message, "product image was not found");
        assert!(!image_not_found.retryable);
    }
}
