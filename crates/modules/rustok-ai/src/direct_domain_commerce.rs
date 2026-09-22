#![cfg(feature = "server")]

use std::sync::Arc;

use rustok_ai_product::{
    PRODUCT_ATTRIBUTES_TASK_SLUG, PRODUCT_COPY_TASK_SLUG, register_product_ai_vertical_handlers,
};

use super::direct_product_attributes::ProductAttributesHandler;
use super::{DirectExecutionRegistry, DirectTaskHandler, ProductCopyHandler};

/// Registers commerce-owned AI direct handlers through product crate adapter APIs.
///
/// `rustok-ai` still owns runtime composition, while `rustok-ai-product` owns
/// the vertical descriptor list and task identity used by this binding seam.
pub fn register_commerce_direct_handlers(registry: &mut DirectExecutionRegistry) {
    register_product_ai_vertical_handlers(|vertical| match vertical.task_slug {
        PRODUCT_COPY_TASK_SLUG => {
            registry.register(Arc::new(ProductCopyHandler) as Arc<dyn DirectTaskHandler>)
        }
        PRODUCT_ATTRIBUTES_TASK_SLUG => {
            registry.register(Arc::new(ProductAttributesHandler) as Arc<dyn DirectTaskHandler>)
        }
        _ => {}
    });
}
