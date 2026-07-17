from pathlib import Path

path = Path("apps/server/src/middleware/tenant.rs")
source = path.read_text()


def replace_once(old: str, new: str, label: str) -> None:
    global source
    count = source.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, got {count}")
    source = source.replace(old, new, 1)


replace_once(
    '''use super::tenant_resolution::{
    resolve_request, tenant_route_scope, validated_slug_identifier, ResolvedTenantIdentifier,
    TenantIdentifierKind, TenantResolutionSource, TenantRouteScope,
};''',
    '''use super::{
    tenant_resolution::{
        resolve_explicit_slug, resolve_request, ResolvedTenantIdentifier, TenantIdentifierKind,
        TenantResolution, TenantResolutionSource,
    },
    tenant_route_policy::{tenant_route_scope, TenantRouteScope},
};''',
    "split resolver and route-policy imports",
)
replace_once(
    '''    InvalidIdentifier(String),
    InfrastructureUnavailable,''',
    '''    InvalidIdentifier(String),
    InvalidAssertion(String),
    InfrastructureUnavailable,''',
    "add invalid assertion error",
)
replace_once(
    '''            Self::InvalidIdentifier(_) => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,''',
    '''            Self::InvalidIdentifier(_) | Self::InvalidAssertion(_) => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,''',
    "map assertion status",
)
replace_once(
    '''            Self::InvalidIdentifier(_) => "Invalid tenant identifier",
            Self::NotFound => "Tenant not found",''',
    '''            Self::InvalidIdentifier(_) => "Invalid tenant identifier",
            Self::InvalidAssertion(_) => "Conflicting tenant assertions",
            Self::NotFound => "Tenant not found",''',
    "map assertion client message",
)
replace_once(
    '''            Self::InvalidIdentifier(reason) => {
                write!(formatter, "invalid tenant identifier: {reason}")
            }
            Self::InfrastructureUnavailable => {''',
    '''            Self::InvalidIdentifier(reason) => {
                write!(formatter, "invalid tenant identifier: {reason}")
            }
            Self::InvalidAssertion(reason) => {
                write!(formatter, "invalid tenant assertion: {reason}")
            }
            Self::InfrastructureUnavailable => {''',
    "display assertion error",
)
replace_once(
    '''pub(crate) async fn load_tenant_context_by_slug(
    ctx: &ServerRuntimeContext,
    slug: &str,
) -> Result<TenantContext, TenantContextLoadError> {
    let identifier = validated_slug_identifier(slug)
        .map_err(|error| TenantContextLoadError::InvalidIdentifier(error.to_string()))?;
    load_tenant_context(ctx, &identifier).await
}

pub async fn resolve(''',
    '''fn record_resolution_source(source: TenantResolutionSource) {
    rustok_telemetry::metrics::record_cache_operation(
        "tenant_resolution",
        "resolve",
        source.as_str(),
    );
}

async fn load_resolved_tenant_context(
    ctx: &ServerRuntimeContext,
    resolution: &TenantResolution,
) -> Result<TenantContext, TenantContextLoadError> {
    let context = load_tenant_context(ctx, &resolution.identifier).await?;
    resolution
        .validate_resolved_slug(&context.slug)
        .map_err(|error| TenantContextLoadError::InvalidAssertion(error.to_string()))?;
    Ok(context)
}

pub(crate) async fn resolve_tenant_context_by_slug(
    ctx: &ServerRuntimeContext,
    slug: &str,
) -> Result<TenantContext, TenantContextLoadError> {
    let resolution = resolve_explicit_slug(slug)
        .map_err(|error| TenantContextLoadError::InvalidIdentifier(error.to_string()))?;
    record_resolution_source(resolution.source);
    load_resolved_tenant_context(ctx, &resolution).await
}

pub async fn resolve(''',
    "replace raw slug loader with typed pipeline",
)
replace_once(
    '''    rustok_telemetry::metrics::record_cache_operation(
        "tenant_resolution",
        "resolve",
        resolution.source.as_str(),
    );''',
    '''    record_resolution_source(resolution.source);''',
    "centralize source telemetry",
)
replace_once(
    '''    let context = load_tenant_context(&ctx, &resolution.identifier)
        .await
        .map_err(|error| {
            tracing::warn!(
                path = req.uri().path(),
                error = %error,
                "Tenant context loading failed"
            );
            error.status_code()
        })?;

    resolution
        .validate_resolved_slug(&context.slug)
        .map_err(|error| {
            tracing::warn!(
                tenant_id = %context.id,
                resolved_slug = %context.slug,
                error = %error,
                "Conflicting tenant assertions rejected"
            );
            error.status_code()
        })?;''',
    '''    let context = load_resolved_tenant_context(&ctx, &resolution)
        .await
        .map_err(|error| {
            tracing::warn!(
                path = req.uri().path(),
                error = %error,
                "Tenant context loading failed"
            );
            error.status_code()
        })?;''',
    "centralize context loading and assertion validation",
)

path.write_text(source)
