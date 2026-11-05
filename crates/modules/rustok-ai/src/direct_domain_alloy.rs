#![cfg(feature = "server")]

use std::sync::Arc;

use rustok_ai_alloy::{ALLOY_CODE_TASK_SLUG, register_alloy_ai_vertical_handlers};

use super::{AlloyScriptAssistHandler, DirectExecutionRegistry, DirectTaskHandler};

/// Registers alloy-owned AI direct handlers through alloy crate adapter APIs.
pub fn register_alloy_direct_handlers(registry: &mut DirectExecutionRegistry) {
    register_alloy_ai_vertical_handlers(|vertical| {
        if vertical.task_slug == ALLOY_CODE_TASK_SLUG {
            registry.register(Arc::new(AlloyScriptAssistHandler) as Arc<dyn DirectTaskHandler>);
        }
    });
}
