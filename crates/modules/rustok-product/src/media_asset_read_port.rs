/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Product-owned consumer boundary for Media-owned asset references.
//!
//! Product stores only the canonical Media asset UUID on `product_images.media_id`. The Media
//! owner remains the single authority for asset existence, tenancy and lifecycle state, so a
//! deployment that composes a Media provider receives tenant-scoped validation before an image row
//! is written. Deployments without a provider keep accepting opaque references; that deployment
//! state stays visible through [`ProductMediaReferencePolicy`] instead of being silently assumed.

use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError};
use uuid::Uuid;

use crate::ProductCatalogCommandPort;
use crate::dto::{
    AddProductImageInput, CreateProductInput, CreateVariantInput, ProductImageResponse,
    ProductResponse, SetVariantAxesInput, UpdateProductImageInput, UpdateProductInput,
    UpdateVariantInput, VariantAxisConfigResponse, VariantResponse,
};

/// Owner operation name reported to the Media provider for asset validation reads.
pub const ENSURE_PRODUCT_IMAGE_MEDIA_ASSET_OPERATION: &str = "ensure_product_image_media_asset";

/// Deadline the Product owner requests for one Media asset validation read.
pub const PRODUCT_MEDIA_ASSET_VALIDATION_DEADLINE: Duration = Duration::from_millis(2_000);

/// Deployment-visible statement about what Product can guarantee for a media reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductMediaReferencePolicy {
    /// A Media provider is composed: every referenced asset must exist inside the caller tenant.
    ValidatedByMediaOwner,
    /// No Media provider is composed: Product stores the canonical UUID without existence proof.
    OpaqueReferences,
}

impl ProductMediaReferencePolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ValidatedByMediaOwner => "validated_by_media_owner",
            Self::OpaqueReferences => "opaque_references",
        }
    }
}

/// Product-owned requirement on the Media owner.
///
/// The Product owner never imports Media persistence: the host composes an adapter over the
/// Media-owned read port, so a remote Media deployment validates references through the same
/// contract as the embedded one.
#[async_trait]
pub trait ProductMediaAssetReadPort: Send + Sync {
    /// Fails closed when the canonical Media asset is missing or belongs to another tenant.
    async fn ensure_asset_exists(
        &self,
        context: PortContext,
        media_id: Uuid,
    ) -> Result<(), PortError>;
}

/// Builds the tenant-scoped read context Product uses for one Media asset validation.
pub fn product_media_asset_validation_context(tenant_id: Uuid, actor_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "und",
        format!("product-image-media-{}", Uuid::new_v4()),
    )
    .with_deadline(PRODUCT_MEDIA_ASSET_VALIDATION_DEADLINE)
}

/// Command-port decorator that validates Media-owned asset references before delegating writes.
///
/// Only `add_product_image` introduces a new `media_id`; image updates keep the already validated
/// reference. Validation runs before the owner transaction, so an unknown asset can never reach
/// `product_images`.
pub struct ProductMediaValidatedCommandPort {
    inner: std::sync::Arc<dyn ProductCatalogCommandPort>,
    media_asset_read_port: std::sync::Arc<dyn ProductMediaAssetReadPort>,
}

impl ProductMediaValidatedCommandPort {
    pub fn new(
        inner: std::sync::Arc<dyn ProductCatalogCommandPort>,
        media_asset_read_port: std::sync::Arc<dyn ProductMediaAssetReadPort>,
    ) -> Self {
        Self {
            inner,
            media_asset_read_port,
        }
    }

    async fn ensure_media_asset(
        &self,
        context: &PortContext,
        product_id: Uuid,
        media_id: Uuid,
    ) -> Result<(), PortError> {
        let tenant_id = Uuid::parse_str(context.tenant_id.as_str()).map_err(|_| {
            PortError::validation(
                "product.tenant_id_invalid",
                "product request context is invalid",
            )
        })?;
        let actor_id = Uuid::parse_str(context.actor.id.as_str()).map_err(|_| {
            PortError::validation(
                "product.actor_id_invalid",
                "product request context is invalid",
            )
        })?;

        let validation_context = product_media_asset_validation_context(tenant_id, actor_id);
        self.media_asset_read_port
            .ensure_asset_exists(validation_context, media_id)
            .await
            .map_err(|error| {
                tracing::warn!(
                    correlation_id = %context.correlation_id,
                    product_id = %product_id,
                    media_id = %media_id,
                    operation = ENSURE_PRODUCT_IMAGE_MEDIA_ASSET_OPERATION,
                    code = %error.code,
                    "product image media asset validation failed"
                );
                match error.kind {
                    rustok_api::PortErrorKind::NotFound => PortError::validation(
                        "product.media_asset_not_found",
                        "product image media asset was not found",
                    ),
                    rustok_api::PortErrorKind::Forbidden => PortError::validation(
                        "product.media_asset_not_found",
                        "product image media asset was not found",
                    ),
                    _ => PortError::unavailable(
                        "product.media_asset_unavailable",
                        "product image media asset could not be validated",
                    ),
                }
            })
    }
}

#[async_trait]
impl ProductCatalogCommandPort for ProductMediaValidatedCommandPort {
    async fn create_product(
        &self,
        context: PortContext,
        input: CreateProductInput,
    ) -> Result<ProductResponse, PortError> {
        self.inner.create_product(context, input).await
    }

    async fn set_variant_axes(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: SetVariantAxesInput,
    ) -> Result<Vec<VariantAxisConfigResponse>, PortError> {
        self.inner
            .set_variant_axes(context, product_id, input)
            .await
    }

    async fn update_product(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: UpdateProductInput,
    ) -> Result<ProductResponse, PortError> {
        self.inner.update_product(context, product_id, input).await
    }

    async fn delete_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<(), PortError> {
        self.inner.delete_product(context, product_id).await
    }

    async fn publish_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<ProductResponse, PortError> {
        self.inner.publish_product(context, product_id).await
    }

    async fn unpublish_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<ProductResponse, PortError> {
        self.inner.unpublish_product(context, product_id).await
    }

    async fn archive_product(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<ProductResponse, PortError> {
        self.inner.archive_product(context, product_id).await
    }

    async fn create_variant(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: CreateVariantInput,
    ) -> Result<VariantResponse, PortError> {
        self.inner.create_variant(context, product_id, input).await
    }

    async fn update_variant(
        &self,
        context: PortContext,
        variant_id: Uuid,
        input: UpdateVariantInput,
    ) -> Result<VariantResponse, PortError> {
        self.inner.update_variant(context, variant_id, input).await
    }

    async fn delete_variant(
        &self,
        context: PortContext,
        variant_id: Uuid,
    ) -> Result<(), PortError> {
        self.inner.delete_variant(context, variant_id).await
    }

    async fn add_product_image(
        &self,
        context: PortContext,
        product_id: Uuid,
        input: AddProductImageInput,
    ) -> Result<ProductImageResponse, PortError> {
        self.ensure_media_asset(&context, product_id, input.media_id)
            .await?;
        self.inner
            .add_product_image(context, product_id, input)
            .await
    }

    async fn update_product_image(
        &self,
        context: PortContext,
        product_id: Uuid,
        image_id: Uuid,
        input: UpdateProductImageInput,
    ) -> Result<ProductImageResponse, PortError> {
        self.inner
            .update_product_image(context, product_id, image_id, input)
            .await
    }

    async fn delete_product_image(
        &self,
        context: PortContext,
        product_id: Uuid,
        image_id: Uuid,
    ) -> Result<(), PortError> {
        self.inner
            .delete_product_image(context, product_id, image_id)
            .await
    }

    async fn reorder_product_images(
        &self,
        context: PortContext,
        product_id: Uuid,
        image_ids: Vec<Uuid>,
    ) -> Result<(), PortError> {
        self.inner
            .reorder_product_images(context, product_id, image_ids)
            .await
    }
}
