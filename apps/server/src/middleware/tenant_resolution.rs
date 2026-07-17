use std::fmt;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use rustok_core::tenant_validation::TenantIdentifierValidator;
use uuid::Uuid;

use crate::common::{
    extract_effective_host, peer_ip_from_extensions,
    settings::{RustokSettings, TenantFallbackMode, TenantResolutionMode},
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TenantRouteScope {
    TenantBound,
    GlobalOperator,
    SelfResolvingHandshake,
}

pub fn tenant_route_scope(path: &str) -> TenantRouteScope {
    if path == "/api/graphql/ws" {
        return TenantRouteScope::SelfResolvingHandshake;
    }

    if matches!(path, "/metrics" | "/api/openapi.json" | "/api/openapi.yaml")
        || path == "/api/graphql/schema.graphql"
        || path == "/api/install"
        || path.starts_with("/api/install/")
        || path == "/v1/catalog"
        || path.starts_with("/v1/catalog/")
        || path == "/catalog"
        || path.starts_with("/catalog/")
        || path.starts_with("/health")
    {
        TenantRouteScope::GlobalOperator
    } else {
        TenantRouteScope::TenantBound
    }
}

pub fn tenant_path_requires_resolution(path: &str) -> bool {
    tenant_route_scope(path) == TenantRouteScope::TenantBound
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TenantIdentifierKind {
    Uuid,
    Slug,
    Host,
}

impl TenantIdentifierKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Uuid => "uuid",
            Self::Slug => "slug",
            Self::Host => "host",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ResolvedTenantIdentifier {
    Uuid(Uuid),
    Slug(String),
    Host(String),
}

impl ResolvedTenantIdentifier {
    pub const fn kind(&self) -> TenantIdentifierKind {
        match self {
            Self::Uuid(_) => TenantIdentifierKind::Uuid,
            Self::Slug(_) => TenantIdentifierKind::Slug,
            Self::Host(_) => TenantIdentifierKind::Host,
        }
    }

    pub fn value(&self) -> String {
        match self {
            Self::Uuid(value) => value.to_string(),
            Self::Slug(value) | Self::Host(value) => value.clone(),
        }
    }

    pub const fn uuid(&self) -> Option<Uuid> {
        match self {
            Self::Uuid(value) => Some(*value),
            Self::Slug(_) | Self::Host(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TenantResolutionSource {
    SingleTenantDefault,
    Header,
    Host,
    Domain,
    Subdomain,
    DevelopmentFallback,
}

impl TenantResolutionSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SingleTenantDefault => "single_tenant_default",
            Self::Header => "header",
            Self::Host => "host",
            Self::Domain => "domain",
            Self::Subdomain => "subdomain",
            Self::DevelopmentFallback => "development_fallback",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TenantResolutionSourceExtension(pub TenantResolutionSource);

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TenantResolution {
    pub identifier: ResolvedTenantIdentifier,
    pub source: TenantResolutionSource,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TenantResolutionError {
    InvalidPolicy(String),
    MissingHeader { header_name: String },
    InvalidHeaderValue { header_name: String },
    MissingHost,
    InvalidHost { value: String, reason: String },
    InvalidIdentifier { value: String, reason: String },
    BaseDomainRequiresTenantSlug { host: String, base_domain: String },
    NestedSubdomain { host: String, base_domain: String },
    NoBaseDomainMatch { host: String },
}

impl TenantResolutionError {
    pub const fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidPolicy(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::NoBaseDomainMatch { .. } => StatusCode::NOT_FOUND,
            Self::MissingHeader { .. }
            | Self::InvalidHeaderValue { .. }
            | Self::MissingHost
            | Self::InvalidHost { .. }
            | Self::InvalidIdentifier { .. }
            | Self::BaseDomainRequiresTenantSlug { .. }
            | Self::NestedSubdomain { .. } => StatusCode::BAD_REQUEST,
        }
    }
}

impl fmt::Display for TenantResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPolicy(reason) => write!(formatter, "invalid tenant routing policy: {reason}"),
            Self::MissingHeader { header_name } => {
                write!(formatter, "missing required tenant header `{header_name}`")
            }
            Self::InvalidHeaderValue { header_name } => {
                write!(formatter, "tenant header `{header_name}` is not valid UTF-8")
            }
            Self::MissingHost => formatter.write_str("request host is missing or untrusted"),
            Self::InvalidHost { value, reason } => {
                write!(formatter, "invalid tenant host `{value}`: {reason}")
            }
            Self::InvalidIdentifier { value, reason } => {
                write!(formatter, "invalid tenant identifier `{value}`: {reason}")
            }
            Self::BaseDomainRequiresTenantSlug { host, base_domain } => write!(
                formatter,
                "tenant host `{host}` equals base domain `{base_domain}` and has no tenant slug"
            ),
            Self::NestedSubdomain { host, base_domain } => write!(
                formatter,
                "tenant host `{host}` contains a nested subdomain before `{base_domain}`"
            ),
            Self::NoBaseDomainMatch { host } => {
                write!(formatter, "tenant host `{host}` matches no configured base domain")
            }
        }
    }
}

impl std::error::Error for TenantResolutionError {}

pub fn resolve_request(
    req: &Request<Body>,
    settings: &RustokSettings,
) -> Result<TenantResolution, TenantResolutionError> {
    settings
        .tenant
        .validate()
        .map_err(|error| TenantResolutionError::InvalidPolicy(error.to_string()))?;

    if !settings.tenant.enabled {
        return Ok(TenantResolution {
            identifier: ResolvedTenantIdentifier::Uuid(settings.tenant.default_id),
            source: TenantResolutionSource::SingleTenantDefault,
        });
    }

    match settings.tenant.resolution {
        TenantResolutionMode::Header => resolve_header(req, settings),
        TenantResolutionMode::Host => resolve_host(req, settings, TenantResolutionSource::Host),
        TenantResolutionMode::Domain => resolve_host(req, settings, TenantResolutionSource::Domain),
        TenantResolutionMode::Subdomain => resolve_subdomain(req, settings),
    }
}

fn resolve_header(
    req: &Request<Body>,
    settings: &RustokSettings,
) -> Result<TenantResolution, TenantResolutionError> {
    let primary = header_value(req, &settings.tenant.header_name)?;
    let slug = if settings.tenant.header_name.eq_ignore_ascii_case("X-Tenant-Slug") {
        None
    } else {
        header_value(req, "X-Tenant-Slug")?
    };

    if let Some(identifier) = primary.or(slug) {
        return Ok(TenantResolution {
            identifier: classify_identifier(identifier)?,
            source: TenantResolutionSource::Header,
        });
    }

    if settings.tenant.fallback_mode == TenantFallbackMode::DefaultTenant {
        return Ok(TenantResolution {
            identifier: ResolvedTenantIdentifier::Uuid(settings.tenant.default_id),
            source: TenantResolutionSource::DevelopmentFallback,
        });
    }

    Err(TenantResolutionError::MissingHeader {
        header_name: settings.tenant.header_name.clone(),
    })
}

fn header_value<'a>(
    req: &'a Request<Body>,
    header_name: &str,
) -> Result<Option<&'a str>, TenantResolutionError> {
    let Some(value) = req.headers().get(header_name) else {
        return Ok(None);
    };
    let value = value
        .to_str()
        .map_err(|_| TenantResolutionError::InvalidHeaderValue {
            header_name: header_name.to_string(),
        })?
        .trim();
    Ok((!value.is_empty()).then_some(value))
}

fn resolve_host(
    req: &Request<Body>,
    settings: &RustokSettings,
    source: TenantResolutionSource,
) -> Result<TenantResolution, TenantResolutionError> {
    let host = effective_host(req, settings)?;
    Ok(TenantResolution {
        identifier: ResolvedTenantIdentifier::Host(host),
        source,
    })
}

fn resolve_subdomain(
    req: &Request<Body>,
    settings: &RustokSettings,
) -> Result<TenantResolution, TenantResolutionError> {
    let host = effective_host(req, settings)?;
    let identifier = subdomain_identifier(&host, &settings.tenant.base_domains)?;
    Ok(TenantResolution {
        identifier: classify_identifier(&identifier)?,
        source: TenantResolutionSource::Subdomain,
    })
}

fn effective_host(
    req: &Request<Body>,
    settings: &RustokSettings,
) -> Result<String, TenantResolutionError> {
    let peer_ip = peer_ip_from_extensions(req.extensions());
    let host = extract_effective_host(req.headers(), peer_ip, &settings.runtime.request_trust)
        .ok_or(TenantResolutionError::MissingHost)?;
    let host_without_port = host.split(':').next().unwrap_or(host.as_str());
    TenantIdentifierValidator::validate_host(host_without_port).map_err(|error| {
        TenantResolutionError::InvalidHost {
            value: host_without_port.to_string(),
            reason: error.to_string(),
        }
    })
}

pub(crate) fn subdomain_identifier(
    host: &str,
    base_domains: &[String],
) -> Result<String, TenantResolutionError> {
    for base_domain in base_domains {
        if host == base_domain {
            return Err(TenantResolutionError::BaseDomainRequiresTenantSlug {
                host: host.to_string(),
                base_domain: base_domain.clone(),
            });
        }

        let suffix = format!(".{base_domain}");
        if let Some(candidate) = host.strip_suffix(&suffix) {
            if candidate.is_empty() || candidate.contains('.') {
                return Err(TenantResolutionError::NestedSubdomain {
                    host: host.to_string(),
                    base_domain: base_domain.clone(),
                });
            }
            return Ok(candidate.to_string());
        }
    }

    Err(TenantResolutionError::NoBaseDomainMatch {
        host: host.to_string(),
    })
}

fn classify_identifier(value: &str) -> Result<ResolvedTenantIdentifier, TenantResolutionError> {
    if let Ok(uuid) = TenantIdentifierValidator::validate_uuid(value) {
        return Ok(ResolvedTenantIdentifier::Uuid(uuid));
    }

    TenantIdentifierValidator::validate_slug(value)
        .map(ResolvedTenantIdentifier::Slug)
        .map_err(|error| TenantResolutionError::InvalidIdentifier {
            value: value.to_string(),
            reason: error.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(path: &str) -> Request<Body> {
        Request::builder()
            .uri(path)
            .body(Body::empty())
            .expect("request")
    }

    #[test]
    fn route_policy_distinguishes_global_and_self_resolving_surfaces() {
        assert_eq!(tenant_route_scope("/metrics"), TenantRouteScope::GlobalOperator);
        assert_eq!(
            tenant_route_scope("/api/graphql/ws"),
            TenantRouteScope::SelfResolvingHandshake
        );
        assert_eq!(
            tenant_route_scope("/api/graphql"),
            TenantRouteScope::TenantBound
        );
        assert_eq!(
            tenant_route_scope("/v2/catalog/publish"),
            TenantRouteScope::TenantBound
        );
    }

    #[test]
    fn strict_header_mode_rejects_missing_header() {
        let settings = RustokSettings::default();
        let error = resolve_request(&request("/api/users"), &settings).expect_err("missing header");
        assert!(matches!(error, TenantResolutionError::MissingHeader { .. }));
    }

    #[test]
    fn fallback_is_reported_as_actual_resolution_source() {
        let mut settings = RustokSettings::default();
        settings.tenant.fallback_mode = TenantFallbackMode::DefaultTenant;
        let resolution = resolve_request(&request("/api/users"), &settings).expect("fallback");
        assert_eq!(resolution.source, TenantResolutionSource::DevelopmentFallback);
        assert_eq!(
            resolution.identifier,
            ResolvedTenantIdentifier::Uuid(settings.tenant.default_id)
        );
    }

    #[test]
    fn supplied_header_is_not_reported_as_fallback() {
        let settings = RustokSettings::default();
        let request = Request::builder()
            .uri("/api/users")
            .header("X-Tenant-ID", "demo")
            .body(Body::empty())
            .expect("request");
        let resolution = resolve_request(&request, &settings).expect("header resolution");
        assert_eq!(resolution.source, TenantResolutionSource::Header);
        assert_eq!(
            resolution.identifier,
            ResolvedTenantIdentifier::Slug("demo".to_string())
        );
    }

    #[test]
    fn subdomain_requires_exactly_one_tenant_label() {
        let domains = vec!["example.test".to_string()];
        assert_eq!(
            subdomain_identifier("store.example.test", &domains).expect("slug"),
            "store"
        );
        assert!(matches!(
            subdomain_identifier("example.test", &domains),
            Err(TenantResolutionError::BaseDomainRequiresTenantSlug { .. })
        ));
        assert!(matches!(
            subdomain_identifier("a.b.example.test", &domains),
            Err(TenantResolutionError::NestedSubdomain { .. })
        ));
    }
}
