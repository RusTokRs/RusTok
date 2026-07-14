use axum::Router;
use rustok_api::HostRuntimeContext;

pub fn axum_router(runtime: &HostRuntimeContext) -> anyhow::Result<Router> {
    Ok(crate::controllers::axum_router(runtime)?
        .merge(crate::provider_event_recovery_controller::axum_router(runtime)?))
}

pub fn axum_webhook_router(runtime: &HostRuntimeContext) -> anyhow::Result<Router> {
    crate::controllers::axum_webhook_router(runtime)
}
