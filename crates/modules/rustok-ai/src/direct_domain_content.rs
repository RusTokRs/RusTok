#![cfg(feature = "server")]

use std::sync::Arc;

use rustok_ai_content::{
    BLOG_DRAFT_TASK_SLUG, CONTENT_MODERATION_TASK_SLUG, register_content_ai_vertical_handlers,
};

use super::direct_content_moderation::ContentModerationHandler;
use super::{BlogDraftHandler, DirectExecutionRegistry, DirectTaskHandler};

/// Registers content-owned AI direct handlers through content crate adapter APIs.
///
/// `rustok-ai` keeps the executable handler, but consumes the domain-owned
/// vertical descriptors instead of hard-coding content task registration.
pub fn register_content_direct_handlers(registry: &mut DirectExecutionRegistry) {
    register_content_ai_vertical_handlers(|vertical| match vertical.task_slug {
        CONTENT_MODERATION_TASK_SLUG => {
            registry.register(Arc::new(ContentModerationHandler) as Arc<dyn DirectTaskHandler>)
        }
        BLOG_DRAFT_TASK_SLUG => {
            registry.register(Arc::new(BlogDraftHandler) as Arc<dyn DirectTaskHandler>)
        }
        _ => {}
    });
}
