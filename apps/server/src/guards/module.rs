use axum::{
    extract::{FromRef, FromRequestParts},
    http::{StatusCode, request::Parts},
};
use std::marker::PhantomData;

use rustok_core::ModuleRegistry;

use crate::context::TenantContextExt;
use crate::services::effective_module_policy::EffectiveModulePolicyService;
use crate::services::server_runtime_context::ServerRuntimeContext;

pub trait ModuleSlug {
    const SLUG: &'static str;
}

pub struct RequireModule<M: ModuleSlug>(PhantomData<M>);

impl<S, M: ModuleSlug> FromRequestParts<S> for RequireModule<M>
where
    S: Send + Sync,
    ServerRuntimeContext: FromRef<S>,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let tenant_id = parts
            .tenant_context()
            .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Tenant context missing"))?
            .id;
        let ctx = ServerRuntimeContext::from_ref(state);
        let registry = ctx.shared_get::<ModuleRegistry>().ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Static module registry unavailable",
        ))?;
        let is_enabled =
            EffectiveModulePolicyService::is_enabled(ctx.db(), &registry, tenant_id, M::SLUG)
                .await
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

        if is_enabled {
            Ok(Self(PhantomData))
        } else {
            Err((StatusCode::NOT_FOUND, "Module is disabled or not found"))
        }
    }
}
