//! Host-supplied media library integration for the builder Assets panel.
//!
//! The Page Builder editor stays transport-free: a host composition binds an
//! [`AssetProviderPort`] implementation to the media module and carries the
//! authentication context. Registered provider declarations come from
//! [`builtin_asset_provider_definitions`] and are consumed by the Assets panel,
//! so a declaration without a bound runtime never renders upload UI.

use std::future::Future;
use std::pin::Pin;

use fly::AssetProviderDefinition;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Provider id under which `rustok-media` assets are registered and referenced.
pub const RUSTOK_MEDIA_ASSET_PROVIDER: &str = "rustok.media";

/// Asset provider declarations installed into the canvas registry.
pub fn builtin_asset_provider_definitions() -> Vec<AssetProviderDefinition> {
    vec![AssetProviderDefinition {
        id: RUSTOK_MEDIA_ASSET_PROVIDER.to_string(),
        supported_kinds: ["image", "video", "audio", "document"]
            .map(str::to_string)
            .to_vec(),
    }]
}

/// One library entry surfaced by an asset provider host binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetProviderItem {
    pub id: String,
    pub public_url: String,
    pub original_name: String,
    pub mime_type: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

/// One page of an asset provider library listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetProviderLibraryPage {
    pub items: Vec<AssetProviderItem>,
    pub total: u64,
}

/// Failure surfaced by an asset provider host binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{0}")]
pub struct AssetProviderError(String);

impl AssetProviderError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    pub fn message(&self) -> &str {
        &self.0
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub type AssetProviderLibraryFuture = Pin<
    Box<
        dyn Future<Output = Result<AssetProviderLibraryPage, AssetProviderError>>
            + Send
            + 'static,
    >,
>;

#[cfg(target_arch = "wasm32")]
pub type AssetProviderLibraryFuture = Pin<
    Box<dyn Future<Output = Result<AssetProviderLibraryPage, AssetProviderError>> + 'static>,
>;

#[cfg(not(target_arch = "wasm32"))]
pub type AssetProviderUploadFuture =
    Pin<Box<dyn Future<Output = Result<AssetProviderItem, AssetProviderError>> + Send + 'static>>;

#[cfg(target_arch = "wasm32")]
pub type AssetProviderUploadFuture =
    Pin<Box<dyn Future<Output = Result<AssetProviderItem, AssetProviderError>> + 'static>>;

/// Host-supplied media library access for one registered asset provider.
pub trait AssetProviderPort: Send + Sync {
    fn library_page(&self, page: u32, per_page: u32) -> AssetProviderLibraryFuture;

    fn upload(
        &self,
        file_name: String,
        content_type: String,
        data: Vec<u8>,
    ) -> AssetProviderUploadFuture;
}

/// Maps one library entry to the Fly asset catalog shape used by the Assets panel.
///
/// The media item id stays a provider-side reference (`providerAssetId`); the
/// document `src` remains the verifiable public URL of the media module.
pub fn media_provider_asset_value(item: &AssetProviderItem) -> Value {
    let mut asset = Map::new();
    asset.insert(
        "id".to_string(),
        Value::String(format!("media-{}", item.id)),
    );
    asset.insert("src".to_string(), Value::String(item.public_url.clone()));
    asset.insert(
        "name".to_string(),
        Value::String(item.original_name.clone()),
    );
    asset.insert(
        "mimeType".to_string(),
        Value::String(item.mime_type.clone()),
    );
    if let Some(width) = item.width {
        asset.insert("width".to_string(), Value::Number(width.into()));
    }
    if let Some(height) = item.height {
        asset.insert("height".to_string(), Value::Number(height.into()));
    }
    asset.insert(
        "provider".to_string(),
        Value::String(RUSTOK_MEDIA_ASSET_PROVIDER.to_string()),
    );
    asset.insert(
        "providerAssetId".to_string(),
        Value::String(item.id.clone()),
    );
    Value::Object(asset)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item() -> AssetProviderItem {
        AssetProviderItem {
            id: "0d4f2f64-6f4e-4f60-9c1f-3a5d2f6c9b21".to_string(),
            public_url: "/api/media/public/images/0d4f2f64/abc".to_string(),
            original_name: "hero.webp".to_string(),
            mime_type: "image/webp".to_string(),
            width: Some(1200),
            height: Some(630),
        }
    }

    #[test]
    fn media_provider_asset_value_keeps_provider_reference() {
        let value = media_provider_asset_value(&item());
        assert_eq!(value["id"], "media-0d4f2f64-6f4e-4f60-9c1f-3a5d2f6c9b21");
        assert_eq!(value["src"], "/api/media/public/images/0d4f2f64/abc");
        assert_eq!(value["provider"], RUSTOK_MEDIA_ASSET_PROVIDER);
        assert_eq!(value["providerAssetId"], "0d4f2f64-6f4e-4f60-9c1f-3a5d2f6c9b21");
        assert_eq!(value["mimeType"], "image/webp");
        assert_eq!(value["width"], 1200);
        assert_eq!(value["height"], 630);
    }

    #[test]
    fn media_provider_asset_value_omits_unknown_dimensions() {
        let mut entry = item();
        entry.width = None;
        entry.height = None;
        let value = media_provider_asset_value(&entry);
        assert!(value.get("width").is_none());
        assert!(value.get("height").is_none());
    }

    #[test]
    fn builtin_asset_provider_definitions_cover_media_kinds() {
        let definitions = builtin_asset_provider_definitions();
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].id, RUSTOK_MEDIA_ASSET_PROVIDER);
        assert!(definitions[0].supported_kinds.contains(&"image".to_string()));
    }
}
