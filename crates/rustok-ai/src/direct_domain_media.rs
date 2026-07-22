#![cfg(feature = "server")]

use std::sync::Arc;

use rustok_ai_media::{IMAGE_ASSET_TASK_SLUG, register_media_ai_vertical_handlers};

use super::{DirectExecutionRegistry, DirectTaskHandler, MediaImageAssetHandler};

/// Registers media-owned AI direct handlers through media crate adapter APIs.
pub fn register_media_direct_handlers(registry: &mut DirectExecutionRegistry) {
    register_media_ai_vertical_handlers(|vertical| {
        if vertical.task_slug == IMAGE_ASSET_TASK_SLUG {
            registry.register(Arc::new(MediaImageAssetHandler) as Arc<dyn DirectTaskHandler>);
        }
    });
}
