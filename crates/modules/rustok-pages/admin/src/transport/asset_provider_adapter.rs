//! Host binding of the page builder `AssetProviderPort` to `rustok-media`
//! admin dispatchers.
//!
//! The port intentionally carries no token or tenant parameters: this adapter
//! captures the auth context the same way the builder facade does, so the
//! editor stays transport-free and a host without media support simply does
//! not bind the port.

use std::sync::Arc;

use rustok_media_admin::{MediaListItem, fetch_media_library, upload_media};
use rustok_page_builder_admin::{
    AssetProviderError, AssetProviderItem, AssetProviderLibraryFuture, AssetProviderLibraryPage,
    AssetProviderPort, AssetProviderUploadFuture,
};

/// Reads one auth-context value at call time so late sign-in stays visible to
/// media dispatches.
type AuthValue = Arc<dyn Fn() -> Option<String> + Send + Sync>;

pub struct PagesMediaAssetProvider {
    token: AuthValue,
    tenant_slug: AuthValue,
}

impl PagesMediaAssetProvider {
    pub fn new(token: AuthValue, tenant_slug: AuthValue) -> Self {
        Self { token, tenant_slug }
    }
}

/// Builds the media-backed provider bound by the pages builder host.
pub fn pages_media_asset_provider(
    token: impl Fn() -> Option<String> + Send + Sync + 'static,
    tenant_slug: impl Fn() -> Option<String> + Send + Sync + 'static,
) -> Arc<dyn AssetProviderPort> {
    Arc::new(PagesMediaAssetProvider::new(
        Arc::new(token),
        Arc::new(tenant_slug),
    ))
}

fn asset_item(item: MediaListItem) -> AssetProviderItem {
    AssetProviderItem {
        id: item.id,
        public_url: item.public_url,
        original_name: item.original_name,
        mime_type: item.mime_type,
        width: item.width,
        height: item.height,
    }
}

impl AssetProviderPort for PagesMediaAssetProvider {
    fn library_page(&self, page: u32, per_page: u32) -> AssetProviderLibraryFuture {
        let token = (self.token)();
        let tenant_slug = (self.tenant_slug)();
        Box::pin(async move {
            let page = i32::try_from(page)
                .map_err(|_| AssetProviderError::new("media library page out of range"))?;
            let per_page = i32::try_from(per_page)
                .map_err(|_| AssetProviderError::new("media library page size out of range"))?;
            let payload = fetch_media_library(page, per_page, token, tenant_slug)
                .await
                .map_err(|error| AssetProviderError::new(error.to_string()))?;
            Ok(AssetProviderLibraryPage {
                items: payload.items.into_iter().map(asset_item).collect(),
                total: payload.total,
            })
        })
    }

    fn upload(
        &self,
        file_name: String,
        content_type: String,
        data: Vec<u8>,
    ) -> AssetProviderUploadFuture {
        let token = (self.token)();
        let tenant_slug = (self.tenant_slug)();
        Box::pin(async move {
            let item = upload_media(file_name, content_type, data, token, tenant_slug)
                .await
                .map_err(|error| AssetProviderError::new(error.to_string()))?;
            Ok(asset_item(item))
        })
    }
}
