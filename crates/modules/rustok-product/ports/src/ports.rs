use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use uuid::Uuid;

use crate::dto::{
    AddProductImageInput, CreateProductInput, CreateVariantInput, ProductImageResponse,
    ProductResponse, SetVariantAxesInput, UpdateProductImageInput, UpdateProductInput,
    UpdateVariantInput, VariantAxesPreviewResponse, VariantAxisConfigResponse, VariantResponse,
};
use crate::types::{
    AdminCatalogFacetsRequest, AdminProductsRequest, FilteredPublishedProductsRequest,
    LegacyAdminProductsRequest, LegacyStorefrontProductList, LegacyStorefrontProductsRequest,
    ProductProjectionRequest, ProductVariantIdentitiesRequest, ProductVariantIdentityListRequest,
    ProductVariantIdentitySnapshot, PublishedProductsRequest, StorefrontCatalogFacet,
    StorefrontCatalogFacetsRequest, StorefrontProductList, StorefrontProductProjectionRequest,
    StorefrontVariantProductProjectionRequest, VariantProductProjectionRequest,
};

/// Transport-neutral owner boundary for product catalog read projections.
#[async_trait]
pub trait ProductCatalogReadPort: Send + Sync {
    async fn read_product_projection(
        &self,
        context: PortContext,
        request: ProductProjectionRequest,
    ) -> Result<ProductResponse, PortError>;

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

    async fn load_storefront_catalog_facets(
        &self,
        _context: PortContext,
        _request: StorefrontCatalogFacetsRequest,
    ) -> Result<Vec<StorefrontCatalogFacet>, PortError> {
        Err(PortError::unavailable(
            "product.catalog_facets_unavailable",
            "product catalog facets are unavailable",
        ))
    }

    async fn load_admin_catalog_facets(
        &self,
        _context: PortContext,
        _request: AdminCatalogFacetsRequest,
    ) -> Result<Vec<StorefrontCatalogFacet>, PortError> {
        Err(PortError::unavailable(
            "product.admin_catalog_facets_unavailable",
            "product admin catalog facets are unavailable",
        ))
    }

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

    async fn read_storefront_product_projection(
        &self,
        _context: PortContext,
        _request: StorefrontProductProjectionRequest,
    ) -> Result<ProductResponse, PortError> {
        Err(PortError::unavailable(
            "product.storefront_projection_unavailable",
            "storefront product projection is unavailable",
        ))
    }

    async fn read_storefront_variant_product_projection(
        &self,
        _context: PortContext,
        _request: StorefrontVariantProductProjectionRequest,
    ) -> Result<ProductResponse, PortError> {
        Err(PortError::unavailable(
            "product.storefront_variant_projection_unavailable",
            "storefront variant product projection is unavailable",
        ))
    }

    async fn list_admin_products(
        &self,
        _context: PortContext,
        _request: AdminProductsRequest,
    ) -> Result<crate::types::AdminProductList, PortError> {
        Err(PortError::unavailable(
            "product.admin_list_unavailable",
            "admin product listing is unavailable",
        ))
    }

    async fn list_legacy_admin_products(
        &self,
        _context: PortContext,
        _request: LegacyAdminProductsRequest,
    ) -> Result<crate::types::AdminProductList, PortError> {
        Err(PortError::unavailable(
            "product.legacy_admin_list_unavailable",
            "legacy admin product listing is unavailable",
        ))
    }
}

/// Small Product-owned SKU identity projection for price/identity consumers.
#[async_trait]
pub trait ProductVariantIdentityReadPort: Send + Sync {
    async fn read_variant_identities(
        &self,
        context: PortContext,
        request: ProductVariantIdentitiesRequest,
    ) -> Result<Vec<ProductVariantIdentitySnapshot>, PortError>;

    async fn list_product_variant_identities(
        &self,
        context: PortContext,
        request: ProductVariantIdentityListRequest,
    ) -> Result<Vec<ProductVariantIdentitySnapshot>, PortError>;
}

/// Transport-neutral owner boundary for Product catalog lifecycle commands.
#[async_trait]
pub trait ProductCatalogCommandPort: Send + Sync {
    async fn create_product(
        &self,
        context: PortContext,
        input: CreateProductInput,
    ) -> Result<ProductResponse, PortError>;

    async fn preview_variant_axes(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: SetVariantAxesInput,
    ) -> Result<VariantAxesPreviewResponse, PortError>;

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

pub fn product_catalog_command_error_is_terminal(error: &PortError) -> bool {
    matches!(
        error.kind,
        rustok_api::PortErrorKind::Validation
            | rustok_api::PortErrorKind::Conflict
            | rustok_api::PortErrorKind::NotFound
            | rustok_api::PortErrorKind::InvariantViolation
    ) || matches!(
        error.code.as_str(),
        "product.validation"
            | "product.variant_axis_policy_conflict"
            | "product.schema_validation"
            | "product.conflict"
            | "product.dependency_not_found"
            | "product.duplicate_handle"
            | "product.duplicate_sku"
            | "product.revision_conflict"
            | "product.variant_axes_preview_required"
            | "product.variant_axes_stale"
            | "product.variant_axes_conflict"
            | "product.variant_axes_limit_exceeded"
            | "product.no_variants"
            | "product.lifecycle_conflict"
            | "product.cannot_delete_only_variant"
            | "product.product_not_found"
            | "product.variant_not_found"
            | "product.image_not_found"
    )
}
