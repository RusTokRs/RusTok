use crate::{ComponentPatch, FlyError, FlyResult, ProjectDocument};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Image,
    Video,
    Audio,
    Document,
    Font,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssetDescriptor {
    pub id: String,
    pub kind: AssetKind,
    pub source: String,
    pub name: Option<String>,
    pub mime_type: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub provider: Option<String>,
    pub provider_asset_id: Option<String>,
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetReference {
    pub asset_id: String,
    pub provider: Option<String>,
    pub provider_asset_id: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetPolicy {
    pub allow_http: bool,
    pub allow_https: bool,
    pub allow_data_images: bool,
    pub allow_relative: bool,
    pub maximum_assets: usize,
}

impl Default for AssetPolicy {
    fn default() -> Self {
        Self {
            allow_http: true,
            allow_https: true,
            allow_data_images: true,
            allow_relative: true,
            maximum_assets: 10_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AssetCatalog {
    pub assets: Vec<AssetDescriptor>,
    pub unknown_entries: Vec<Value>,
    pub duplicate_ids: BTreeSet<String>,
}

impl AssetCatalog {
    pub fn from_document(document: &ProjectDocument) -> Self {
        let mut catalog = Self::default();
        let mut ids = BTreeSet::new();
        for raw in &document.project.assets {
            match AssetDescriptor::from_value(raw.clone()) {
                Some(asset) => {
                    if !ids.insert(asset.id.clone()) {
                        catalog.duplicate_ids.insert(asset.id.clone());
                    }
                    catalog.assets.push(asset);
                }
                None => catalog.unknown_entries.push(raw.clone()),
            }
        }
        catalog
    }

    pub fn get(&self, id: &str) -> Option<&AssetDescriptor> {
        self.assets.iter().find(|asset| asset.id == id)
    }

    pub fn by_kind(&self, kind: AssetKind) -> impl Iterator<Item = &AssetDescriptor> {
        self.assets.iter().filter(move |asset| asset.kind == kind)
    }

    pub fn validate(&self, policy: &AssetPolicy) -> Vec<String> {
        let mut diagnostics = Vec::new();
        if self.assets.len() + self.unknown_entries.len() > policy.maximum_assets {
            diagnostics.push(format!(
                "project contains {} assets, exceeding maximum {}",
                self.assets.len() + self.unknown_entries.len(),
                policy.maximum_assets
            ));
        }
        for duplicate in &self.duplicate_ids {
            diagnostics.push(format!("asset id `{duplicate}` is duplicated"));
        }
        for asset in &self.assets {
            if !source_allowed(&asset.source, asset.kind, policy) {
                diagnostics.push(format!(
                    "asset `{}` uses a source rejected by policy",
                    asset.id
                ));
            }
        }
        diagnostics
    }
}

impl AssetDescriptor {
    pub fn from_value(raw: Value) -> Option<Self> {
        let object = raw.as_object()?;
        let source = string_field(object, &["src", "source", "url"])?;
        let id =
            string_field(object, &["id", "assetId"]).unwrap_or_else(|| stable_source_id(&source));
        let mime_type =
            string_field(object, &["mimeType", "mime", "type"]).filter(|value| value.contains('/'));
        let kind = object
            .get("kind")
            .and_then(Value::as_str)
            .and_then(parse_kind)
            .or_else(|| mime_type.as_deref().map(kind_from_mime))
            .unwrap_or_else(|| kind_from_source(&source));
        Some(Self {
            id,
            kind,
            source,
            name: string_field(object, &["name", "filename", "title"]),
            mime_type,
            width: integer_field(object, &["width"]),
            height: integer_field(object, &["height"]),
            provider: string_field(object, &["provider"]),
            provider_asset_id: string_field(object, &["providerAssetId", "externalId"]),
            raw,
        })
    }

    pub fn reference(&self) -> AssetReference {
        AssetReference {
            asset_id: self.id.clone(),
            provider: self.provider.clone(),
            provider_asset_id: self.provider_asset_id.clone(),
            source: self.source.clone(),
        }
    }

    pub fn component_patch(&self, source_attribute: &str) -> FlyResult<ComponentPatch> {
        if source_attribute.trim().is_empty() {
            return Err(FlyError::InvalidAssetReference(
                "asset source attribute must not be empty".to_string(),
            ));
        }
        let mut attributes = Map::new();
        attributes.insert(
            source_attribute.to_string(),
            Value::String(self.source.clone()),
        );
        attributes.insert(
            "data-fly-asset-id".to_string(),
            Value::String(self.id.clone()),
        );
        if let Some(provider) = &self.provider {
            attributes.insert(
                "data-fly-asset-provider".to_string(),
                Value::String(provider.clone()),
            );
        }
        if let Some(provider_asset_id) = &self.provider_asset_id {
            attributes.insert(
                "data-fly-provider-asset-id".to_string(),
                Value::String(provider_asset_id.clone()),
            );
        }
        Ok(ComponentPatch {
            attributes,
            ..ComponentPatch::default()
        })
    }
}

pub fn source_allowed(source: &str, kind: AssetKind, policy: &AssetPolicy) -> bool {
    let normalized = source.trim().to_ascii_lowercase();
    if normalized.starts_with("https://") {
        return policy.allow_https;
    }
    if normalized.starts_with("http://") {
        return policy.allow_http;
    }
    if normalized.starts_with("data:image/") {
        return policy.allow_data_images && kind == AssetKind::Image;
    }
    if normalized.starts_with('/') || normalized.starts_with("./") || normalized.starts_with("../")
    {
        return policy.allow_relative;
    }
    false
}

fn string_field(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
        .map(ToString::to_string)
}

fn integer_field(object: &Map<String, Value>, keys: &[&str]) -> Option<u32> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(Value::as_u64))
        .and_then(|value| u32::try_from(value).ok())
}

fn parse_kind(value: &str) -> Option<AssetKind> {
    match value.to_ascii_lowercase().as_str() {
        "image" => Some(AssetKind::Image),
        "video" => Some(AssetKind::Video),
        "audio" => Some(AssetKind::Audio),
        "document" => Some(AssetKind::Document),
        "font" => Some(AssetKind::Font),
        "other" => Some(AssetKind::Other),
        _ => None,
    }
}

fn kind_from_mime(mime: &str) -> AssetKind {
    if mime.starts_with("image/") {
        AssetKind::Image
    } else if mime.starts_with("video/") {
        AssetKind::Video
    } else if mime.starts_with("audio/") {
        AssetKind::Audio
    } else if mime.starts_with("font/") {
        AssetKind::Font
    } else if mime == "application/pdf" || mime.starts_with("text/") {
        AssetKind::Document
    } else {
        AssetKind::Other
    }
}

fn kind_from_source(source: &str) -> AssetKind {
    let source = source
        .split(['?', '#'])
        .next()
        .unwrap_or(source)
        .to_ascii_lowercase();
    if source.starts_with("data:image/")
        || [".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".avif"]
            .iter()
            .any(|extension| source.ends_with(extension))
    {
        AssetKind::Image
    } else if [".mp4", ".webm", ".mov"]
        .iter()
        .any(|extension| source.ends_with(extension))
    {
        AssetKind::Video
    } else if [".mp3", ".wav", ".ogg"]
        .iter()
        .any(|extension| source.ends_with(extension))
    {
        AssetKind::Audio
    } else if [".woff", ".woff2", ".ttf", ".otf"]
        .iter()
        .any(|extension| source.ends_with(extension))
    {
        AssetKind::Font
    } else if [".pdf", ".txt", ".doc", ".docx"]
        .iter()
        .any(|extension| source.ends_with(extension))
    {
        AssetKind::Document
    } else {
        AssetKind::Other
    }
}

fn stable_source_id(source: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in source.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("asset-{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsV1Codec;
    use serde_json::json;

    #[test]
    fn catalog_normalizes_grapesjs_asset_shapes_without_losing_raw_data() {
        let document = GrapesJsV1Codec::decode_value(json!({
            "assets": [{
                "id": "hero",
                "src": "https://cdn.example.com/hero.webp",
                "width": 1200,
                "customFutureField": { "enabled": true }
            }],
            "pages": []
        }))
        .expect("document");
        let catalog = AssetCatalog::from_document(&document);
        let asset = catalog.get("hero").expect("asset");
        assert_eq!(asset.kind, AssetKind::Image);
        assert_eq!(asset.raw["customFutureField"]["enabled"], true);
    }

    #[test]
    fn component_patch_keeps_provider_reference() {
        let asset = AssetDescriptor::from_value(json!({
            "id": "media-1",
            "src": "/media/1.webp",
            "provider": "rustok.media",
            "providerAssetId": "1"
        }))
        .expect("asset");
        let patch = asset.component_patch("src").expect("patch");
        assert_eq!(patch.attributes["data-fly-asset-provider"], "rustok.media");
    }
}
