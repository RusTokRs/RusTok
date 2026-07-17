from pathlib import Path
import re

PATH = Path("apps/server/src/middleware/tenant.rs")
source = PATH.read_text()


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, got {count}")
    return text.replace(old, new, 1)


def sub_once(text: str, pattern: str, replacement: str, label: str) -> str:
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.S)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, got {count}")
    return updated

source = replace_once(
    source,
    "use std::sync::Arc;",
    "use std::{fmt, sync::Arc};",
    "fmt import",
)
source = sub_once(
    source,
    r"use super::tenant_resolution::\{.*?\};",
    """use super::tenant_resolution::{
    resolve_request, tenant_route_scope, validated_slug_identifier, ResolvedTenantIdentifier,
    TenantIdentifierKind, TenantResolutionSource, TenantRouteScope,
};""",
    "tenant resolution imports",
)

source = sub_once(
    source,
    r"impl CachedTenantMiss \{.*?\n\}\n\nfn tenant_context_from_projection",
    """#[derive(Debug)]
pub(crate) enum TenantContextLoadError {
    InvalidIdentifier(String),
    InfrastructureUnavailable,
    NotFound,
    Disabled,
    CacheUnavailable(String),
    ClockUnavailable(String),
    BackendUnavailable(String),
}

impl TenantContextLoadError {
    pub(crate) const fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidIdentifier(_) => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Disabled => StatusCode::FORBIDDEN,
            Self::InfrastructureUnavailable
            | Self::CacheUnavailable(_)
            | Self::ClockUnavailable(_)
            | Self::BackendUnavailable(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub(crate) const fn client_message(&self) -> &'static str {
        match self {
            Self::InvalidIdentifier(_) => "Invalid tenant identifier",
            Self::NotFound => "Tenant not found",
            Self::Disabled => "Tenant is disabled",
            Self::InfrastructureUnavailable
            | Self::CacheUnavailable(_)
            | Self::ClockUnavailable(_)
            | Self::BackendUnavailable(_) => "Failed to resolve tenant",
        }
    }
}

impl fmt::Display for TenantContextLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier(reason) => write!(formatter, "invalid tenant identifier: {reason}"),
            Self::InfrastructureUnavailable => formatter.write_str("tenant cache infrastructure is unavailable"),
            Self::NotFound => formatter.write_str("tenant not found"),
            Self::Disabled => formatter.write_str("tenant is disabled"),
            Self::CacheUnavailable(reason) => write!(formatter, "tenant cache unavailable: {reason}"),
            Self::ClockUnavailable(reason) => write!(formatter, "tenant clock unavailable: {reason}"),
            Self::BackendUnavailable(reason) => write!(formatter, "tenant backend unavailable: {reason}"),
        }
    }
}

impl std::error::Error for TenantContextLoadError {}

impl From<CachedTenantMiss> for TenantContextLoadError {
    fn from(value: CachedTenantMiss) -> Self {
        match value {
            CachedTenantMiss::NotFound => Self::NotFound,
            CachedTenantMiss::Disabled => Self::Disabled,
        }
    }
}

fn tenant_context_from_projection""",
    "typed tenant load error",
)

source = sub_once(
    source,
    r"    async fn check_negative\(.*?\n    \}\n\n    async fn set_negative",
    """    async fn check_negative(
        &self,
        cache_key: &str,
    ) -> Result<Option<CachedTenantMiss>, TenantContextLoadError> {
        let cached = self
            .cache_service
            .get_negative::<CachedTenantMiss>(
                Arc::clone(&self.tenant_negative_cache),
                cache_key,
                &self.negative_policy,
            )
            .await
            .map_err(|error| TenantContextLoadError::CacheUnavailable(error.to_string()))?;

        if let Some(hit) = cached {
            self.metrics
                .incr("negative_hits", &self.metrics.local_negative_hits)
                .await;
            return Ok(Some(hit.reason));
        }

        self.metrics
            .incr("negative_misses", &self.metrics.local_negative_misses)
            .await;
        Ok(None)
    }

    async fn set_negative""",
    "typed negative cache read",
)

source = sub_once(
    source,
    r"    async fn set_negative\(.*?\n    \}\n\n    async fn get_or_load_with_coalescing",
    """    async fn set_negative(
        &self,
        cache_key: String,
        reason: CachedTenantMiss,
    ) -> Result<(), TenantContextLoadError> {
        self.cache_service
            .store_negative(
                Arc::clone(&self.tenant_negative_cache),
                cache_key,
                reason,
                current_unix_ms()
                    .map_err(|error| TenantContextLoadError::ClockUnavailable(error.to_string()))?,
                None,
                &self.negative_policy,
            )
            .await
            .map_err(|error| TenantContextLoadError::CacheUnavailable(error.to_string()))?;
        self.metrics
            .incr("negative_inserts", &self.metrics.local_negative_inserts)
            .await;
        Ok(())
    }

    async fn get_or_load_with_coalescing""",
    "typed negative cache write",
)

source = replace_once(
    source,
    ") -> Result<TenantContext, StatusCode>\n    where",
    ") -> Result<TenantContext, TenantContextLoadError>\n    where",
    "typed coalesced loader signature",
)
source = replace_once(
    source,
    ".map_err(cache_load_error_to_status)?;",
    ".map_err(core_error_to_load_error)?;",
    "typed coalesced loader mapping",
)

marker = """fn tenant_infra(ctx: &ServerRuntimeContext) -> Option<Arc<TenantCacheInfrastructure>> {
    ctx.shared_get::<Arc<TenantCacheInfrastructure>>()
}

"""
loader = marker + """pub(crate) async fn load_tenant_context(
    ctx: &ServerRuntimeContext,
    identifier: &ResolvedTenantIdentifier,
) -> Result<TenantContext, TenantContextLoadError> {
    let Some(infra) = tenant_infra(ctx) else {
        return Err(TenantContextLoadError::InfrastructureUnavailable);
    };

    let identifier_value = identifier.value();
    let cache_key = infra
        .key_builder
        .kind_key(identifier.kind(), &identifier_value);
    let negative_key = infra
        .key_builder
        .kind_negative_key(identifier.kind(), &identifier_value);

    if let Some(reason) = infra.check_negative(&negative_key).await? {
        return Err(reason.into());
    }

    let tenant_service = TenantService::new(ctx.db_clone());
    let tenant_request = tenant_read_request(identifier);
    let tenant_port_context = tenant_read_context(identifier);
    let negative_key_clone = negative_key.clone();
    let infra_clone = infra.clone();

    infra
        .get_or_load_with_coalescing(&cache_key, || async move {
            let projection = match tenant_service
                .read_tenant(tenant_port_context, tenant_request)
                .await
            {
                Ok(projection) => projection,
                Err(error) if error.kind == PortErrorKind::NotFound => {
                    infra_clone
                        .set_negative(negative_key_clone.clone(), CachedTenantMiss::NotFound)
                        .await
                        .map_err(|error| CoreError::Cache(error.to_string()))?;
                    return Err(CoreError::NotFound(error.message));
                }
                Err(error) => return Err(tenant_port_error_to_core_error(error)),
            };

            match tenant_context_from_projection(projection) {
                Ok(context) => Ok(context),
                Err(CachedTenantMiss::Disabled) => {
                    infra_clone
                        .set_negative(negative_key_clone.clone(), CachedTenantMiss::Disabled)
                        .await
                        .map_err(|error| CoreError::Cache(error.to_string()))?;
                    Err(CoreError::Forbidden("tenant disabled".to_string()))
                }
                Err(CachedTenantMiss::NotFound) => {
                    infra_clone
                        .set_negative(negative_key_clone.clone(), CachedTenantMiss::NotFound)
                        .await
                        .map_err(|error| CoreError::Cache(error.to_string()))?;
                    Err(CoreError::NotFound("tenant not found".to_string()))
                }
            }
        })
        .await
}

pub(crate) async fn load_tenant_context_by_slug(
    ctx: &ServerRuntimeContext,
    slug: &str,
) -> Result<TenantContext, TenantContextLoadError> {
    let identifier = validated_slug_identifier(slug)
        .map_err(|error| TenantContextLoadError::InvalidIdentifier(error.to_string()))?;
    load_tenant_context(ctx, &identifier).await
}

"""
source = replace_once(source, marker, loader, "canonical tenant context loader")

source = sub_once(
    source,
    r"    let identifier = &resolution\.identifier;.*?        \.await\?;\n",
    """    let context = load_tenant_context(&ctx, &resolution.identifier)
        .await
        .map_err(|error| {
            tracing::warn!(
                path = req.uri().path(),
                error = %error,
                "Tenant context loading failed"
            );
            error.status_code()
        })?;
""",
    "delegate HTTP resolution to canonical loader",
)
source = replace_once(
    source,
    """    req.extensions_mut()
        .insert(TenantResolutionSourceExtension(resolution.source));
""",
    "",
    "remove source extension",
)
source = sub_once(
    source,
    r"fn cache_load_error_to_status\(error: CoreError\) -> StatusCode \{.*?\n\}\n",
    """fn core_error_to_load_error(error: CoreError) -> TenantContextLoadError {
    match error {
        CoreError::NotFound(_) => TenantContextLoadError::NotFound,
        CoreError::Forbidden(_) => TenantContextLoadError::Disabled,
        CoreError::Validation(reason) => TenantContextLoadError::InvalidIdentifier(reason),
        CoreError::Cache(reason) => TenantContextLoadError::CacheUnavailable(reason),
        other => TenantContextLoadError::BackendUnavailable(other.to_string()),
    }
}
""",
    "typed core error mapping",
)

PATH.write_text(source)
