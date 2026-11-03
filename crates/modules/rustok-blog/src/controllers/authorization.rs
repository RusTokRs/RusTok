use rustok_api::{AuthContext, Permission, TenantContext, has_any_effective_permission};
use rustok_web::{HttpError, HttpResult};

pub(super) fn security_context(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

pub(super) fn ensure_blog_permission(
    tenant: &TenantContext,
    auth: &AuthContext,
    permissions: &[Permission],
    message: &str,
) -> HttpResult<()> {
    if tenant.id.is_nil() || auth.tenant_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if auth.tenant_id != tenant.id {
        return Err(HttpError::forbidden(
            "blog_tenant_mismatch",
            "Authenticated principal is not bound to the current tenant",
        ));
    }
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(HttpError::forbidden("blog_permission_denied", message));
    }

    Ok(())
}
