/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Host composition of the Product-owned Media asset validation boundary.
//!
//! Product stores only the canonical Media asset UUID on `product_images.media_id`; the Media owner
//! remains the authority for existence, tenancy and lifecycle state. This adapter resolves the
//! Product-owned requirement through the Media-owned read port, so image writes fail closed when
//! the referenced asset is missing or belongs to another tenant. Deployments without a Media
//! provider keep the explicit `opaque_references` policy instead of silently claiming validation.

use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{HostRuntimeContext, PortContext, PortError};
use rustok_media::MediaAssetReadPort;
use rustok_product::{
    ProductCatalogCommandRuntime, ProductMediaAssetReadPort as ProductMediaAssetValidationPort,
};
use uuid::Uuid;

use crate::services::server_runtime_context::ServerRuntimeContext;

/// Product-owned adapter over the Media-owned read port.
pub struct ProductMediaAssetProvider {
    media_asset_read_port: Arc<dyn MediaAssetReadPort>,
}

impl ProductMediaAssetProvider {
    pub fn new(media_asset_read_port: Arc<dyn MediaAssetReadPort>) -> Self {
        Self {
            media_asset_read_port,
        }
    }
}

#[async_trait]
impl ProductMediaAssetValidationPort for ProductMediaAssetProvider {
    async fn ensure_asset_exists(
        &self,
        context: PortContext,
        media_id: Uuid,
    ) -> Result<(), PortError> {
        self.media_asset_read_port
            .get_asset(context, media_id)
            .await
            .map(|_| ())
    }
}

/// Composes Media asset validation into the host-selected Product command runtime.
///
/// A runtime that already carries a Product media provider keeps it: an outer host composition
/// stays authoritative, exactly like every other composed owner runtime. Otherwise the Media
/// provider published by the host is reused before the embedded Media service is constructed from
/// initialized durable storage. When neither is available the runtime keeps
/// `opaque_references` and the host records why.
pub fn compose_product_media_asset_validation(
    runtime: ProductCatalogCommandRuntime,
    server: &ServerRuntimeContext,
    host: &HostRuntimeContext,
) -> ProductCatalogCommandRuntime {
    if runtime.media_asset_read_port().is_some() {
        return runtime;
    }

    let Some(media_asset_read_port) = resolve_media_asset_read_port(server, host) else {
        tracing::warn!(
            policy = runtime.media_reference_policy().as_str(),
            "Product image media asset validation is not composed; Media references stay opaque"
        );
        return runtime;
    };

    runtime.with_media_asset_read_port(Arc::new(ProductMediaAssetProvider::new(
        media_asset_read_port,
    )))
}

fn resolve_media_asset_read_port(
    server: &ServerRuntimeContext,
    host: &HostRuntimeContext,
) -> Option<Arc<dyn MediaAssetReadPort>> {
    if let Some(provider) = host.shared_get::<Arc<dyn MediaAssetReadPort>>() {
        return Some(provider);
    }
    if let Some(provider) = server.shared_get::<Arc<dyn MediaAssetReadPort>>() {
        return Some(provider);
    }

    let Some(storage) = server.shared_get::<rustok_storage::StorageRuntime>() else {
        return None;
    };

    let provider: Arc<dyn MediaAssetReadPort> = Arc::new(rustok_media::MediaService::new(
        server.db_clone(),
        storage,
    ));
    server.shared_insert(provider.clone());
    Some(provider)
}

#[cfg(all(test, feature = "mod-product", feature = "mod-media"))]
mod product_media_asset_validation_tests {
    use std::sync::Arc;

    use rustok_api::HostRuntimeContext;
    use rustok_media::{MediaAssetReadPort, MediaService};
    use rustok_product::{
        CatalogService, ProductCatalogCommandProfile, ProductCatalogCommandRuntime,
        ProductMediaReferencePolicy,
    };
    use rustok_storage::{LocalStorageConfig, StorageRuntime};
    use rustok_test_utils::mock_transactional_event_bus;
    use sea_orm::Database;

    use super::compose_product_media_asset_validation;
    use crate::common::settings::RustokSettings;
    use crate::services::server_runtime_context::ServerRuntimeContext;

    fn local_storage(label: &str) -> StorageRuntime {
        StorageRuntime::local(&LocalStorageConfig {
            base_dir: std::env::temp_dir()
                .join(format!(
                    "rustok-product-media-validation-{}-{}",
                    label,
                    uuid::Uuid::new_v4()
                ))
                .display()
                .to_string(),
            base_url: String::new(),
            fsync: false,
        })
        .expect("local storage should initialize")
    }

    fn embedded_runtime(db: sea_orm::DatabaseConnection) -> ProductCatalogCommandRuntime {
        ProductCatalogCommandRuntime::new(
            Arc::new(CatalogService::new(db, mock_transactional_event_bus())),
            ProductCatalogCommandProfile::EmbeddedNative,
        )
    }

    #[tokio::test]
    async fn host_published_media_provider_is_composed_into_the_product_runtime() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        let ctx = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        let provider: Arc<dyn MediaAssetReadPort> =
            Arc::new(MediaService::new(db.clone(), local_storage("host")));
        let host = HostRuntimeContext::new(db.clone()).with_shared_value(provider);

        let runtime =
            compose_product_media_asset_validation(embedded_runtime(db), &ctx, &host);

        assert_eq!(
            runtime.media_reference_policy(),
            ProductMediaReferencePolicy::ValidatedByMediaOwner
        );
        assert!(runtime.media_asset_read_port().is_some());
    }

    #[tokio::test]
    async fn embedded_storage_composes_the_media_provider_when_the_host_published_none() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        let ctx = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        ctx.shared_insert(local_storage("embedded"));
        let host = HostRuntimeContext::new(db.clone());

        let runtime =
            compose_product_media_asset_validation(embedded_runtime(db), &ctx, &host);

        assert_eq!(
            runtime.media_reference_policy(),
            ProductMediaReferencePolicy::ValidatedByMediaOwner
        );
        assert!(
            ctx.shared_get::<Arc<dyn MediaAssetReadPort>>().is_some(),
            "the embedded Media provider must stay visible to other consumers"
        );
    }

    #[tokio::test]
    async fn missing_media_provider_keeps_opaque_reference_policy() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        let ctx = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        let host = HostRuntimeContext::new(db.clone());

        let runtime =
            compose_product_media_asset_validation(embedded_runtime(db), &ctx, &host);

        assert_eq!(
            runtime.media_reference_policy(),
            ProductMediaReferencePolicy::OpaqueReferences
        );
        assert!(runtime.media_asset_read_port().is_none());
    }

    #[tokio::test]
    async fn already_validated_runtime_keeps_its_own_media_provider() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        let ctx = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        let host = HostRuntimeContext::new(db.clone());
        let runtime = embedded_runtime(db.clone()).with_media_asset_read_port(Arc::new(
            super::ProductMediaAssetProvider::new(Arc::new(MediaService::new(
                db,
                local_storage("preexisting"),
            ))),
        ));
        let expected = runtime
            .media_asset_read_port()
            .expect("pre-composed provider must be present");

        let composed = compose_product_media_asset_validation(runtime, &ctx, &host);
        let selected = composed
            .media_asset_read_port()
            .expect("composed runtime must keep a provider");

        assert_eq!(
            composed.media_reference_policy(),
            ProductMediaReferencePolicy::ValidatedByMediaOwner
        );
        assert!(
            Arc::ptr_eq(&expected, &selected),
            "an outer host composition must stay authoritative"
        );
    }
}
