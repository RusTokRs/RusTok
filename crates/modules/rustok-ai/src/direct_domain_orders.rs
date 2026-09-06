#![cfg(feature = "server")]

use std::sync::Arc;

use rustok_ai_order::{
    ORDER_ANALYTICS_TASK_SLUG, ORDER_OPS_ASSISTANT_TASK_SLUG, register_order_ai_vertical_handlers,
};

use super::direct_order_tasks::{OrderAnalyticsHandler, OrderOpsAssistantHandler};
use super::{DirectExecutionRegistry, DirectTaskHandler};

/// Registers order-owned AI direct handlers through order crate adapter APIs.
///
/// This keeps runtime composition in `rustok-ai` while task identity and
/// sensitivity metadata remain in the order AI support crate.
pub fn register_order_direct_handlers(registry: &mut DirectExecutionRegistry) {
    register_order_ai_vertical_handlers(|vertical| match vertical.task_slug {
        ORDER_ANALYTICS_TASK_SLUG => {
            registry.register(Arc::new(OrderAnalyticsHandler) as Arc<dyn DirectTaskHandler>)
        }
        ORDER_OPS_ASSISTANT_TASK_SLUG => {
            registry.register(Arc::new(OrderOpsAssistantHandler) as Arc<dyn DirectTaskHandler>)
        }
        _ => {}
    });
}
