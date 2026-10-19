//! Host adapter over Media's public read/write ports. Neither Product nor this
//! adapter accesses Media persistence. A pair must come from one selected owner.
use crate::services::server_runtime_context::ServerRuntimeContext;
use async_trait::async_trait;
use rustok_api::{HostRuntimeContext, PortContext, PortError};
use rustok_media::{
    MediaAssetPorts, MediaAssetReadPort, MediaAssetReferenceInput, MediaAssetReferenceListRequest,
    MediaAssetReferenceLookupRequest, MediaAssetWritePort,
};
use rustok_product::{
    ProductCatalogCommandProfile, ProductCatalogCommandRuntime, ProductMediaReference,
    ProductMediaReferencePage, ProductMediaReferencePort,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

pub struct ProductMediaReferenceProvider {
    read: Arc<dyn MediaAssetReadPort>,
    write: Arc<dyn MediaAssetWritePort>,
}
impl ProductMediaReferenceProvider {
    pub fn new(ports: MediaAssetPorts) -> Self {
        Self {
            read: ports.read(),
            write: ports.write(),
        }
    }
}
fn input(image_id: Uuid) -> MediaAssetReferenceInput {
    MediaAssetReferenceInput {
        owner_module: "product".into(),
        reference_id: image_id,
    }
}
fn command_context(
    mut context: PortContext,
    operation: &str,
    media: Uuid,
    image: Uuid,
) -> Result<PortContext, PortError> {
    let key = context
        .idempotency_key
        .as_deref()
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| {
            PortError::validation(
                "product.media_key_missing",
                "Product Media caller identity is required",
            )
        })?;
    // Includes the original caller identity, actor and asset; different explicit
    // maintenance operations do not alias a stale Media retain acknowledgement.
    let digest = Sha256::digest(
        serde_json::to_vec(&(
            key,
            operation,
            &context.tenant_id,
            &context.actor,
            media,
            image,
        ))
        .map_err(|_| {
            PortError::invariant_violation(
                "product.media_context_invalid",
                "Product Media context is invalid",
            )
        })?,
    );
    context.idempotency_key = Some(format!("product-media:{operation}:{}", hex::encode(digest)));
    Ok(context)
}
fn project(
    reference: rustok_media::MediaAssetReference,
    tenant: &str,
) -> Result<ProductMediaReference, PortError> {
    if reference.owner_module != "product"
        || reference.reference_id.is_nil()
        || reference.media_id.is_nil()
        || reference.tenant_id.is_nil()
        || Uuid::parse_str(tenant).ok() != Some(reference.tenant_id)
    {
        return Err(PortError::invariant_violation(
            "product.media_reference_invalid",
            "Media reference scope is invalid",
        ));
    }
    Ok(ProductMediaReference {
        tenant_id: reference.tenant_id,
        media_id: reference.media_id,
        image_id: reference.reference_id,
    })
}
#[async_trait]
impl ProductMediaReferencePort for ProductMediaReferenceProvider {
    async fn retain(
        &self,
        context: PortContext,
        media_id: Uuid,
        image_id: Uuid,
    ) -> Result<(), PortError> {
        let media_context = command_context(context.clone(), "retain", media_id, image_id)?;
        let retained = self
            .write
            .retain_asset_reference(media_context, media_id, input(image_id))
            .await?;
        let retained = project(retained, &context.tenant_id)?;
        if retained.image_id != image_id || retained.media_id != media_id {
            return Err(PortError::invariant_violation(
                "product.media_reference_mismatch",
                "Media reference does not match the Product image",
            ));
        }
        // Media may replay an old successful retain receipt. A historical receipt
        // is not proof that its hold still exists now (e.g. operator backfill).
        let holds = self.lookup(context, vec![image_id]).await?;
        if holds.len() != 1 || holds[0] != retained {
            return Err(PortError::unavailable(
                "product.media_hold_unconfirmed",
                "Media reference retention could not be confirmed",
            ));
        }
        Ok(())
    }
    async fn release(
        &self,
        context: PortContext,
        media_id: Uuid,
        image_id: Uuid,
    ) -> Result<(), PortError> {
        self.write
            .release_asset_reference(
                command_context(context, "release", media_id, image_id)?,
                media_id,
                input(image_id),
            )
            .await
    }
    async fn lookup(
        &self,
        context: PortContext,
        image_ids: Vec<Uuid>,
    ) -> Result<Vec<ProductMediaReference>, PortError> {
        if image_ids.is_empty() {
            return Ok(Vec::new());
        }
        let tenant = context.tenant_id.clone();
        let result = self
            .read
            .lookup_asset_references(
                context,
                MediaAssetReferenceLookupRequest {
                    owner_module: "product".into(),
                    reference_ids: image_ids,
                },
            )
            .await?;
        result
            .references
            .into_iter()
            .map(|reference| project(reference, &tenant))
            .collect()
    }
    async fn list(
        &self,
        context: PortContext,
        after: Option<Uuid>,
        limit: u64,
    ) -> Result<ProductMediaReferencePage, PortError> {
        let tenant = context.tenant_id.clone();
        let page = self
            .read
            .list_asset_references(
                context,
                MediaAssetReferenceListRequest {
                    owner_module: "product".into(),
                    after_reference_id: after,
                    limit,
                },
            )
            .await?;
        Ok(ProductMediaReferencePage {
            references: page
                .references
                .into_iter()
                .map(|reference| project(reference, &tenant))
                .collect::<Result<_, _>>()?,
            next_image_id: page.next_reference_id,
            has_more: page.has_more,
        })
    }
}

pub fn compose_product_media_references(
    runtime: ProductCatalogCommandRuntime,
    server: &ServerRuntimeContext,
    host: &HostRuntimeContext,
) -> ProductCatalogCommandRuntime {
    // Never decorate an external command proxy with live reads before owner replay.
    if runtime.profile() == ProductCatalogCommandProfile::External {
        return runtime;
    }
    if let Some(port) = runtime.media_reference_port() {
        server.shared_insert(port);
        return runtime;
    }
    let Some(port) = resolve_product_media_reference_port(server, host) else {
        tracing::warn!("Product Media read/write pair is unavailable; adding images fails closed");
        return runtime;
    };
    match runtime.clone().with_media_reference_port(port.clone()) {
        Ok(configured) => {
            if let Some(bounded) = configured.media_reference_port() {
                server.shared_insert(bounded);
            }
            configured
        }
        Err(error) => {
            tracing::error!(code = %error.code, "Product owner rejected Media retention configuration");
            runtime
        }
    }
}

pub fn resolve_product_media_reference_port(
    server: &ServerRuntimeContext,
    host: &HostRuntimeContext,
) -> Option<Arc<dyn ProductMediaReferencePort>> {
    if let Some(port) = host.shared_get::<Arc<dyn ProductMediaReferencePort>>() {
        return Some(port);
    }
    if let Some(ports) = host.shared_get::<MediaAssetPorts>() {
        return Some(Arc::new(ProductMediaReferenceProvider::new(ports)));
    }
    if host.shared_get::<Arc<dyn MediaAssetReadPort>>().is_some()
        || host.shared_get::<Arc<dyn MediaAssetWritePort>>().is_some()
    {
        return None; // incomplete/unpaired host publication must not fall back
    }
    if let Some(port) = server.shared_get::<Arc<dyn ProductMediaReferencePort>>() {
        return Some(port);
    }
    if let Some(ports) = server.shared_get::<MediaAssetPorts>() {
        return Some(Arc::new(ProductMediaReferenceProvider::new(ports)));
    }
    if server.shared_get::<Arc<dyn MediaAssetReadPort>>().is_some()
        || server
            .shared_get::<Arc<dyn MediaAssetWritePort>>()
            .is_some()
    {
        return None;
    }
    let storage = server.shared_get::<rustok_storage::StorageRuntime>()?;
    let ports = MediaAssetPorts::new(Arc::new(rustok_media::MediaService::new(
        server.db_clone(),
        storage,
    )));
    server.shared_insert(ports.read());
    server.shared_insert(ports.write());
    server.shared_insert(ports.clone());
    Some(Arc::new(ProductMediaReferenceProvider::new(ports)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::settings::RustokSettings;
    use rustok_product::ProductMediaReferencePolicy;
    use rustok_storage::{LocalStorageConfig, StorageRuntime};
    fn storage() -> StorageRuntime {
        StorageRuntime::local(&LocalStorageConfig {
            base_dir: std::env::temp_dir()
                .join(format!("product-media-{}", Uuid::new_v4()))
                .display()
                .to_string(),
            base_url: String::new(),
            fsync: false,
        })
        .unwrap()
    }
    fn runtime(db: sea_orm::DatabaseConnection) -> ProductCatalogCommandRuntime {
        ProductCatalogCommandRuntime::in_process(
            db,
            rustok_test_utils::mock_transactional_event_bus(),
        )
    }
    fn ports(db: sea_orm::DatabaseConnection) -> MediaAssetPorts {
        MediaAssetPorts::new(Arc::new(rustok_media::MediaService::new(db, storage())))
    }
    #[tokio::test]
    async fn composed_pair_is_installed_inside_the_embedded_owner() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let server = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        let host = HostRuntimeContext::new(db.clone()).with_shared_value(ports(db.clone()));
        let configured = compose_product_media_references(runtime(db), &server, &host);
        assert_eq!(
            configured.media_reference_policy(),
            ProductMediaReferencePolicy::DurableRetention
        );
        assert!(configured.media_maintenance_port().is_some());
        assert!(
            server
                .shared_get::<Arc<dyn ProductMediaReferencePort>>()
                .is_some()
        );
    }
    #[tokio::test]
    async fn unpaired_host_read_cannot_fall_back_to_embedded_write() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let server = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        server.shared_insert(storage());
        server.shared_insert(ports(db.clone()));
        let read = ports(db.clone()).read();
        let host = HostRuntimeContext::new(db.clone()).with_shared_value(read);
        let configured = compose_product_media_references(runtime(db), &server, &host);
        assert_eq!(
            configured.media_reference_policy(),
            ProductMediaReferencePolicy::Unavailable
        );
        assert!(configured.media_maintenance_port().is_some());
    }
    #[tokio::test]
    async fn no_media_provider_is_explicitly_unavailable() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let server = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        let host = HostRuntimeContext::new(db.clone());
        assert_eq!(
            compose_product_media_references(runtime(db), &server, &host).media_reference_policy(),
            ProductMediaReferencePolicy::Unavailable
        );
    }
    #[tokio::test]
    async fn configured_product_owner_cannot_be_silently_rebound_to_another_media_authority() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let server = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        let configured = runtime(db.clone())
            .with_media_reference_port(Arc::new(ProductMediaReferenceProvider::new(ports(
                db.clone(),
            ))))
            .unwrap();
        let original = configured.media_reference_port().unwrap();
        let host = HostRuntimeContext::new(db.clone()).with_shared_value(ports(db));
        let composed = compose_product_media_references(configured, &server, &host);
        assert!(Arc::ptr_eq(
            &original,
            &composed.media_reference_port().unwrap()
        ));
    }
    #[test]
    fn media_child_keys_are_stable_bounded_and_preserve_propagated_context() {
        let tenant = Uuid::new_v4();
        let actor = Uuid::new_v4();
        let media = Uuid::new_v4();
        let image = Uuid::new_v4();
        let context = PortContext::new(
            tenant.to_string(),
            rustok_api::PortActor::user(actor.to_string()),
            "ru",
            "correlation",
        )
        .with_idempotency_key("caller".repeat(1000))
        .with_deadline(std::time::Duration::from_secs(1))
        .with_causation_id("cause")
        .with_traceparent("trace")
        .with_claim("products:update")
        .with_role("admin");
        let retained = command_context(context.clone(), "retain", media, image).unwrap();
        assert_eq!(
            retained,
            command_context(context.clone(), "retain", media, image).unwrap()
        );
        assert_ne!(
            retained.idempotency_key,
            command_context(context.clone(), "release", media, image)
                .unwrap()
                .idempotency_key
        );
        assert_ne!(
            retained.idempotency_key,
            command_context(context.clone(), "retain", media, Uuid::new_v4())
                .unwrap()
                .idempotency_key
        );
        assert!(retained.idempotency_key.as_ref().unwrap().len() <= 96);
        let mut expected = context;
        expected.idempotency_key = retained.idempotency_key.clone();
        assert_eq!(retained, expected);
    }
    #[test]
    fn projected_reference_cannot_escape_product_namespace_or_tenant() {
        let tenant = Uuid::new_v4();
        let reference = rustok_media::MediaAssetReference {
            tenant_id: tenant,
            media_id: Uuid::new_v4(),
            owner_module: "product".into(),
            reference_id: Uuid::new_v4(),
        };
        assert!(project(reference.clone(), &tenant.to_string()).is_ok());
        assert!(project(reference.clone(), &Uuid::new_v4().to_string()).is_err());
        let mut wrong = reference;
        wrong.owner_module = "forum".into();
        assert!(project(wrong, &tenant.to_string()).is_err());
    }
    #[cfg(any(feature = "mod-profiles", feature = "media-grpc"))]
    #[tokio::test]
    async fn grpc_control_client_can_publish_a_single_owner_pair_without_connecting() {
        let channel = tonic::transport::Endpoint::from_static("http://media-owner.invalid:50051")
            .connect_lazy();
        let pair = MediaAssetPorts::new(Arc::new(
            rustok_media_transport::GrpcMediaProvider::from_channel(channel),
        ));
        // Compilation/composition proof only; intentionally no claim of live RPC.
        let _provider = ProductMediaReferenceProvider::new(pair);
    }
}

#[cfg(test)]
#[path = "product_media_reference_integration_tests.rs"]
mod owner_integration_tests;
