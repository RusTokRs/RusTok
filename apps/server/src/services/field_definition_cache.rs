use rustok_core::EventBus;

use crate::services::server_runtime_context::ServerRuntimeContext;

#[path = "field_definition_cache_base.rs"]
mod base;
#[path = "field_definition_cache_reconciliation.rs"]
mod reconciliation;

pub use base::{
    FieldDefinitionCache, FieldDefinitionCacheInvalidationHandle, SharedFieldDefinitionCache,
};
pub use reconciliation::FieldDefinitionCacheGenerationReconciliationHandle;

pub fn field_definition_cache_from_context(
    ctx: &ServerRuntimeContext,
    bus: EventBus,
) -> FieldDefinitionCache {
    let cache = base::field_definition_cache_from_context(ctx, bus);
    reconciliation::start_field_definition_cache_generation_reconciliation(ctx, cache.clone());
    cache
}
