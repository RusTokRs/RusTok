use axum::Router;
use rustok_api::HostRuntimeContext;

use crate::error::PaymentResult;

pub fn axum_router(runtime: &HostRuntimeContext) -> PaymentResult<Router> {
    Ok(crate::controllers::axum_router(runtime)?.merge(
        crate::provider_event_recovery_controller::axum_router(runtime)?,
    ))
}

pub fn axum_webhook_router(runtime: &HostRuntimeContext) -> PaymentResult<Router> {
    crate::controllers::axum_webhook_router(runtime)
}
