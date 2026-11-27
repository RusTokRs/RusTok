use std::fmt;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
};
use rustok_core::tenant_validation::TenantIdentifierValidator;
use uuid::Uuid;

use crate::common::{
    extract_effective_host, peer_ip_from_extensions,
    settings::{RustokSettings, TenantFallbackMode, TenantResolutionMode, TenantRuntimeProfile},
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum TenantIdentifierKind {
    Uuid,
    Slug,
    Host,
}

impl TenantIdentifierKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Uuid => "uuid",
            Self::Slug => "slug",
            Self::Host => "host",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum ResolvedTenantIdentifier {
    Uuid(Uuid),
    Slug(String),
    Host(String),
}

impl ResolvedTenantIdentifier {
    pub(crate) const fn kind(&self) -> TenantIdentifierKind {
        match self {
            Self::Uuid(_) => TenantIdentifierKind::Uuid,
            Self::Slug(_) => TenantIdentifierKind::Slug,
            Self::Host(_) => TenantIdentifierKind::Host,
        }
    }

    pub(crate) fn value(&self) -> String {
        match self {
            Self::Uuid(value) => value.to_string(),
            Self::Slug(value) | Self::Host(value) => value.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum TenantResolutionSource {
    SingleTenantDefault,
    Header,
    CompatibilitySlugHeader,
    SelfResolvingHandshake,
    Host,
    Domain,
    Subdomain,
    DevelopmentFallback,
}

impl TenantResolutionSource {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::SingleTenantDefault => "single_tenant_default",
            Self::Header => "header",
            Self::CompatibilitySlugHeader => "compatibility_slug_header",
            Self::SelfResolvingHandshake => "self_resolving_handshake",
            Self::Host => "host",
            Self::Domain => "domain",
            Self::Subdomain => "subdomain",
            Self::DevelopmentFallback => "development_fallback",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct TenantResolution {
    pub(crate) identifier: ResolvedTenantIdentifier,
    pub(crate) source: TenantResolutionSource,
    pub(crate) asserted_slug: Option<String>,
}

impl TenantResolution {
    pub(crate) fn validate_resolved_slug(
        &self,
        resolved_slug: &str,
    ) -> Result<(), TenantResolutionError> {
        let Some(asserted_slug) = self.asserted_slug.as_deref() else {
            return Ok(());
        };
        if asserted_slug == resolved_slug {
            return Ok(());
        }
        Err(TenantResolutionError::ConflictingTenantAssertions {
            asserted_slug: asserted_slug.to_string(),
            resolved_slug: resolved_slug.to_string(),
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum TenantResolutionError {
    InvalidPolicy(String),
    MissingHeader {
        header_name: String,
    },
    InvalidHeaderValue {
        header_name: String,
    },
    ConflictingHeaderValues {
        header_name: String,
    },
    MissingHost,
    InvalidHost {
        value: String,
        reason: String,
    },
    InvalidIdentifier {
        value: String,
        reason: String,
    },
    BaseDomainRequiresTenantSlug {
        host: String,
        base_domain: String,
    },
    NestedSubdomain {
        host: String,
        base_domain: String,
    },
    NoBaseDomainMatch {
        host: String,
    },
    ConflictingTenantAssertions {
        asserted_slug: String,
        resolved_slug: String,
    },
}

impl TenantResolutionError {
    pub(crate) const fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidPolicy(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::NoBaseDomainMatch { .. } => StatusCode::NOT_FOUND,
            Self::MissingHeader { .. }
            | Self::InvalidHeaderValue { .. }
            | Self::ConflictingHeaderValues { .. }
            | Self::MissingHost
            | Self::InvalidHost { .. }
            | Self::InvalidIdentifier { .. }
            | Self::BaseDomainRequiresTenantSlug { .. }
            | Self::NestedSubdomain { .. }
            | Self::ConflictingTenantAssertions { .. } => StatusCode::BAD_REQUEST,
        }
    }

    pub(crate) const fn error_code(&self) -> &'static str {
        match self {
            Self::InvalidPolicy(_) => "tenant_policy_invalid",
            Self::NoBaseDomainMatch { .. } => "tenant_not_found",
            Self::MissingHeader { .. } => "missing_tenant_header",
            Self::InvalidHeaderValue { .. } => "invalid_tenant_header",
            Self::ConflictingHeaderValues { .. } => "conflicting_tenant_headers",
            Self::MissingHost => "missing_tenant_host",
            Self::InvalidHost { .. } => "invalid_tenant_host",
            Self::InvalidIdentifier { .. } => "invalid_tenant_identifier",
            Self::BaseDomainRequiresTenantSlug { .. } => "base_domain_requires_slug",
            Self::NestedSubdomain { .. } => "nested_subdomain_forbidden",
            Self::ConflictingTenantAssertions { .. } => "conflicting_tenant_assertions",
        }
    }

    /// Public errors must never echo a client-controlled Host/header/slug
    /// value or an internal validation detail. Keep stable machine codes and
    /// HTTP statuses for clients; the diagnostic Display is for internal use.
    pub(crate) const fn error_message(&self) -> &'static str {
        match self {
            Self::InvalidPolicy(_) => "Internal tenant configuration error",
            Self::MissingHeader { .. } => "Required tenant header is missing",
            Self::InvalidHeaderValue { .. } => "Tenant header is invalid",
            Self::ConflictingHeaderValues { .. } => "Conflicting tenant header values",
            Self::MissingHost => "Request host is missing or untrusted",
            Self::InvalidHost { .. } => "Invalid tenant host",
            Self::InvalidIdentifier { .. } => "Invalid tenant identifier",
            Self::BaseDomainRequiresTenantSlug { .. } => {
                "Tenant slug is required for the base domain"
            }
            Self::NestedSubdomain { .. } => "Nested tenant subdomains are not allowed",
            Self::NoBaseDomainMatch { .. } => "Tenant not found",
            Self::ConflictingTenantAssertions { .. } => "Conflicting tenant assertions",
        }
    }

    pub(crate) fn to_response(&self) -> Response {
        rustok_web::HttpError::new(self.status_code(), self.error_code(), self.error_message())
            .into_response()
    }
}

impl fmt::Display for TenantResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPolicy(reason) => {
                write!(formatter, "invalid tenant routing policy: {reason}")
            }
            Self::MissingHeader { header_name } => {
                write!(formatter, "missing required tenant header `{header_name}`")
            }
            Self::InvalidHeaderValue { header_name } => {
                write!(
                    formatter,
                    "tenant header `{header_name}` is not valid UTF-8"
                )
            }
            Self::ConflictingHeaderValues { header_name } => {
                write!(
                    formatter,
                    "multiple tenant values supplied for header `{header_name}`"
                )
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
                write!(
                    formatter,
                    "tenant host `{host}` matches no configured base domain"
                )
            }
            Self::ConflictingTenantAssertions {
                asserted_slug,
                resolved_slug,
            } => write!(
                formatter,
                "tenant slug assertion `{asserted_slug}` conflicts with resolved tenant `{resolved_slug}`"
            ),
        }
    }
}

impl std::error::Error for TenantResolutionError {}

pub(crate) fn resolve_request(
    req: &Request<Body>,
    settings: &RustokSettings,
) -> Result<TenantResolution, TenantResolutionError> {
    settings
        .tenant
        .validate()
        .map_err(|error| TenantResolutionError::InvalidPolicy(error.to_string()))?;

    if settings.tenant.profile == TenantRuntimeProfile::SingleTenant {
        return Ok(TenantResolution {
            identifier: ResolvedTenantIdentifier::Uuid(settings.tenant.default_id),
            source: TenantResolutionSource::SingleTenantDefault,
            asserted_slug: None,
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
    let compatibility_slug = if settings
        .tenant
        .header_name
        .eq_ignore_ascii_case("X-Tenant-Slug")
    {
        None
    } else {
        header_value(req, "X-Tenant-Slug")?
    };

    if let Some(identifier) = primary {
        let asserted_slug = compatibility_slug.map(validate_slug).transpose()?;
        return Ok(TenantResolution {
            identifier: classify_identifier(identifier)?,
            source: TenantResolutionSource::Header,
            asserted_slug,
        });
    }

    if let Some(slug) = compatibility_slug {
        let slug = validate_slug(slug)?;
        return Ok(TenantResolution {
            identifier: ResolvedTenantIdentifier::Slug(slug),
            source: TenantResolutionSource::CompatibilitySlugHeader,
            asserted_slug: None,
        });
    }

    if settings.tenant.fallback_mode == TenantFallbackMode::DefaultTenant {
        return Ok(TenantResolution {
            identifier: ResolvedTenantIdentifier::Uuid(settings.tenant.default_id),
            source: TenantResolutionSource::DevelopmentFallback,
            asserted_slug: None,
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
    let mut values = req.headers().get_all(header_name).iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(TenantResolutionError::ConflictingHeaderValues {
            header_name: header_name.to_string(),
        });
    }

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
        asserted_slug: None,
    })
}

fn resolve_subdomain(
    req: &Request<Body>,
    settings: &RustokSettings,
) -> Result<TenantResolution, TenantResolutionError> {
    let host = effective_host(req, settings)?;
    let identifier = subdomain_identifier(&host, &settings.tenant.base_domains)?;
    Ok(TenantResolution {
        identifier: ResolvedTenantIdentifier::Slug(validate_slug(&identifier)?),
        source: TenantResolutionSource::Subdomain,
        asserted_slug: None,
    })
}

fn effective_host(
    req: &Request<Body>,
    settings: &RustokSettings,
) -> Result<String, TenantResolutionError> {
    if req
        .headers()
        .get_all(axum::http::header::HOST)
        .iter()
        .nth(1)
        .is_some()
    {
        return Err(TenantResolutionError::ConflictingHeaderValues {
            header_name: "host".to_string(),
        });
    }

    let peer_ip = peer_ip_from_extensions(req.extensions());
    // Unlike Host, X-Forwarded-Host is a proxy chain: each trusted hop may
    // append its own header field. The trust resolver deliberately selects
    // the rightmost value, so rejecting repeated fields here breaks legitimate
    // multi-proxy requests and contradicts the canonical trust policy.
    let host = extract_effective_host(req.headers(), peer_ip, &settings.runtime.request_trust)
        .ok_or(TenantResolutionError::MissingHost)?;
    let authority = host
        .parse::<axum::http::uri::Authority>()
        .map_err(|error| TenantResolutionError::InvalidHost {
            value: host.clone(),
            reason: error.to_string(),
        })?;
    let host_without_port = authority.host();
    let canonical_host = host_without_port
        .strip_suffix('.')
        .unwrap_or(host_without_port);
    TenantIdentifierValidator::validate_host(canonical_host).map_err(|error| {
        TenantResolutionError::InvalidHost {
            value: canonical_host.to_string(),
            reason: error.to_string(),
        }
    })
}

pub(crate) fn subdomain_identifier(
    host: &str,
    base_domains: &[String],
) -> Result<String, TenantResolutionError> {
    for base_domain in base_domains {
        let normalized_base = base_domain
            .trim()
            .strip_suffix('.')
            .unwrap_or(base_domain.trim());
        if host.eq_ignore_ascii_case(normalized_base) {
            return Err(TenantResolutionError::BaseDomainRequiresTenantSlug {
                host: host.to_string(),
                base_domain: base_domain.clone(),
            });
        }

        let suffix = format!(".{normalized_base}");
        // Never byte-slice an untrusted host at a potentially invalid UTF-8
        // boundary. The normal host validator restricts production input,
        // but this helper must remain safe even for a raw malformed string.
        let Some(suffix_start) = host.len().checked_sub(suffix.len()) else {
            continue;
        };
        let Some(tail) = host.get(suffix_start..) else {
            continue;
        };
        if suffix_start > 0 && tail.eq_ignore_ascii_case(&suffix) {
            let candidate = &host[..suffix_start];
            if candidate.is_empty() || candidate.contains('.') {
                return Err(TenantResolutionError::NestedSubdomain {
                    host: host.to_string(),
                    base_domain: base_domain.clone(),
                });
            }
            return Ok(candidate.to_ascii_lowercase());
        }
    }

    Err(TenantResolutionError::NoBaseDomainMatch {
        host: host.to_string(),
    })
}

pub(crate) fn resolve_explicit_slug(
    value: &str,
) -> Result<TenantResolution, TenantResolutionError> {
    Ok(TenantResolution {
        identifier: ResolvedTenantIdentifier::Slug(validate_slug(value)?),
        source: TenantResolutionSource::SelfResolvingHandshake,
        asserted_slug: None,
    })
}

fn validate_slug(value: &str) -> Result<String, TenantResolutionError> {
    TenantIdentifierValidator::validate_slug(value).map_err(|error| {
        TenantResolutionError::InvalidIdentifier {
            value: bounded_identifier_for_error(value),
            reason: error.to_string(),
        }
    })
}

fn classify_identifier(value: &str) -> Result<ResolvedTenantIdentifier, TenantResolutionError> {
    if let Ok(uuid) = TenantIdentifierValidator::validate_uuid(value) {
        return Ok(ResolvedTenantIdentifier::Uuid(uuid));
    }

    validate_slug(value).map(ResolvedTenantIdentifier::Slug)
}

fn bounded_identifier_for_error(value: &str) -> String {
    const MAX_CHARS: usize = 64;

    let mut bounded: String = value.chars().take(MAX_CHARS).collect();
    if value.chars().nth(MAX_CHARS).is_some() {
        bounded.push('…');
    }
    bounded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::settings::TenantSettings;

    fn request(path: &str) -> Request<Body> {
        Request::builder()
            .uri(path)
            .body(Body::empty())
            .expect("request")
    }

    #[test]
    fn explicit_slug_resolution_has_typed_handshake_source() {
        let resolution = resolve_explicit_slug("demo").expect("explicit slug resolution");
        assert_eq!(
            resolution.identifier,
            ResolvedTenantIdentifier::Slug("demo".to_string())
        );
        assert_eq!(
            resolution.source,
            TenantResolutionSource::SelfResolvingHandshake
        );
        assert_eq!(resolution.asserted_slug, None);
    }

    #[test]
    fn explicit_single_tenant_profile_resolves_default_without_request_assertion() {
        let mut settings = RustokSettings::default();
        settings.tenant.profile = TenantRuntimeProfile::SingleTenant;
        settings.tenant.enabled = false;
        let resolution = resolve_request(&request("/api/users"), &settings).expect("single tenant");
        assert_eq!(
            resolution.source,
            TenantResolutionSource::SingleTenantDefault
        );
        assert_eq!(
            resolution.identifier,
            ResolvedTenantIdentifier::Uuid(settings.tenant.default_id)
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
        settings.tenant.profile = TenantRuntimeProfile::Development;
        settings.tenant.fallback_mode = TenantFallbackMode::DefaultTenant;
        let resolution = resolve_request(&request("/api/users"), &settings).expect("fallback");
        assert_eq!(
            resolution.source,
            TenantResolutionSource::DevelopmentFallback
        );
        assert_eq!(resolution.asserted_slug, None);
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
        assert_eq!(resolution.asserted_slug, None);
        assert_eq!(
            resolution.identifier,
            ResolvedTenantIdentifier::Slug("demo".to_string())
        );
    }

    #[test]
    fn duplicate_primary_tenant_header_values_are_rejected() {
        let mut request = request("/api/users");
        request
            .headers_mut()
            .append("X-Tenant-ID", axum::http::HeaderValue::from_static("first"));
        request.headers_mut().append(
            "X-Tenant-ID",
            axum::http::HeaderValue::from_static("second"),
        );

        let error = resolve_request(&request, &RustokSettings::default())
            .expect_err("duplicate tenant header values must fail closed");
        assert!(matches!(
            error,
            TenantResolutionError::ConflictingHeaderValues { ref header_name }
                if header_name == "X-Tenant-ID"
        ));
    }

    #[test]
    fn duplicate_compatibility_tenant_header_values_are_rejected() {
        let mut request = request("/api/users");
        request.headers_mut().append(
            "X-Tenant-Slug",
            axum::http::HeaderValue::from_static("first"),
        );
        request.headers_mut().append(
            "X-Tenant-Slug",
            axum::http::HeaderValue::from_static("second"),
        );

        let error = resolve_request(&request, &RustokSettings::default())
            .expect_err("duplicate compatibility header values must fail closed");
        assert!(matches!(
            error,
            TenantResolutionError::ConflictingHeaderValues { ref header_name }
                if header_name == "X-Tenant-Slug"
        ));
    }

    #[tokio::test]
    async fn tenant_resolution_http_errors_never_reflect_untrusted_identifiers() {
        let secret = "super-secret-host-and-token-".repeat(512);
        let variants = [
            TenantResolutionError::InvalidPolicy(secret.clone()),
            TenantResolutionError::MissingHeader {
                header_name: secret.clone(),
            },
            TenantResolutionError::InvalidHeaderValue {
                header_name: secret.clone(),
            },
            TenantResolutionError::ConflictingHeaderValues {
                header_name: secret.clone(),
            },
            TenantResolutionError::InvalidHost {
                value: secret.clone(),
                reason: secret.clone(),
            },
            TenantResolutionError::InvalidIdentifier {
                value: secret.clone(),
                reason: secret.clone(),
            },
            TenantResolutionError::BaseDomainRequiresTenantSlug {
                host: secret.clone(),
                base_domain: secret.clone(),
            },
            TenantResolutionError::NestedSubdomain {
                host: secret.clone(),
                base_domain: secret.clone(),
            },
            TenantResolutionError::NoBaseDomainMatch {
                host: secret.clone(),
            },
            TenantResolutionError::ConflictingTenantAssertions {
                asserted_slug: secret.clone(),
                resolved_slug: secret.clone(),
            },
        ];
        for error in variants {
            let response = error.to_response();
            assert_eq!(response.status(), error.status_code());
            let body = axum::body::to_bytes(response.into_body(), 4096)
                .await
                .expect("bounded tenant error response");
            let text = std::str::from_utf8(&body).expect("JSON response body");
            assert!(!text.contains("super-secret-host-and-token-"));
            assert!(
                text.len() < 512,
                "untrusted user identifiers must not expand the public error envelope"
            );
            assert!(
                text.contains(error.error_code()),
                "stable error code must survive response redaction"
            );
        }
    }

    #[test]
    fn overlong_identifier_error_is_bounded_for_logs() {
        let value = "a".repeat(4_096);
        let error = validate_slug(&value).expect_err("identifier must be rejected");
        match error {
            TenantResolutionError::InvalidIdentifier { value, .. } => {
                assert_eq!(value.chars().count(), 65);
                assert!(value.ends_with('…'));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn subdomain_uuid_shaped_label_remains_a_slug() {
        let domains = vec!["example.test".to_string()];
        let identifier = subdomain_identifier(
            "550e8400-e29b-41d4-a716-446655440000.example.test",
            &domains,
        )
        .expect("uuid-shaped tenant slug should be accepted");
        assert_eq!(identifier, "550e8400-e29b-41d4-a716-446655440000");

        let settings = RustokSettings {
            tenant: TenantSettings {
                profile: TenantRuntimeProfile::MultiTenant,
                enabled: true,
                resolution: TenantResolutionMode::Subdomain,
                header_name: "X-Tenant-ID".to_string(),
                default_id: Uuid::from_u128(1),
                fallback_mode: TenantFallbackMode::Disabled,
                base_domains: domains,
            },
            ..RustokSettings::default()
        };
        let request = Request::builder()
            .uri("/api/users")
            .header("Host", "550E8400-E29B-41D4-A716-446655440000.Example.Test")
            .body(Body::empty())
            .expect("request");
        let resolution = resolve_request(&request, &settings).expect("subdomain resolution");
        assert_eq!(
            resolution.identifier,
            ResolvedTenantIdentifier::Slug("550e8400-e29b-41d4-a716-446655440000".to_string())
        );
    }

    #[test]
    fn trailing_dot_is_removed_from_effective_host_before_tenant_resolution() {
        let settings = RustokSettings {
            tenant: TenantSettings {
                profile: TenantRuntimeProfile::MultiTenant,
                enabled: true,
                resolution: TenantResolutionMode::Subdomain,
                header_name: "X-Tenant-ID".to_string(),
                default_id: Uuid::from_u128(1),
                fallback_mode: TenantFallbackMode::Disabled,
                base_domains: vec!["example.test".to_string()],
            },
            ..RustokSettings::default()
        };
        let request = Request::builder()
            .uri("/api/users")
            .header("Host", "tenant.example.test.")
            .body(Body::empty())
            .expect("request");
        let resolution =
            resolve_request(&request, &settings).expect("trailing dot must canonicalize");
        assert_eq!(
            resolution.identifier,
            ResolvedTenantIdentifier::Slug("tenant".to_string())
        );
    }

    #[test]
    fn dual_headers_are_correlated_after_tenant_lookup() {
        let settings = RustokSettings::default();
        let request = Request::builder()
            .uri("/api/users")
            .header("X-Tenant-ID", Uuid::from_u128(7).to_string())
            .header("X-Tenant-Slug", "expected-slug")
            .body(Body::empty())
            .expect("request");
        let resolution = resolve_request(&request, &settings).expect("header resolution");
        assert_eq!(resolution.asserted_slug.as_deref(), Some("expected-slug"));
        assert!(resolution.validate_resolved_slug("expected-slug").is_ok());
        assert!(matches!(
            resolution.validate_resolved_slug("other-slug"),
            Err(TenantResolutionError::ConflictingTenantAssertions { .. })
        ));
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

    #[test]
    fn subdomain_matching_never_panics_on_a_non_character_boundary() {
        // The base-domain suffix starts midway through the multibyte 'é'.
        // A byte-indexed slice used to panic here instead of rejecting the host.
        let domains = vec!["x.a.test".to_string()];
        assert!(matches!(
            subdomain_identifier("éx.a.test", &domains),
            Err(TenantResolutionError::NoBaseDomainMatch { .. })
        ));
    }

    #[test]
    fn duplicate_host_header_values_are_rejected() {
        let settings = RustokSettings {
            tenant: TenantSettings {
                profile: TenantRuntimeProfile::MultiTenant,
                enabled: true,
                resolution: TenantResolutionMode::Host,
                header_name: "X-Tenant-ID".to_string(),
                default_id: Uuid::from_u128(1),
                fallback_mode: TenantFallbackMode::Disabled,
                base_domains: vec![],
            },
            ..RustokSettings::default()
        };

        let mut req = request("/api/users");
        req.headers_mut()
            .append("Host", axum::http::HeaderValue::from_static("legit.com"));
        req.headers_mut()
            .append("Host", axum::http::HeaderValue::from_static("evil.com"));

        let error = resolve_request(&req, &settings)
            .expect_err("duplicate host headers must fail closed per RFC 9110");
        assert!(matches!(
            error,
            TenantResolutionError::ConflictingHeaderValues { ref header_name } if header_name == "host"
        ));
    }

    #[test]
    fn trusted_proxy_chain_uses_last_forwarded_host_field_for_tenant_resolution() {
        use crate::common::settings::{ForwardedHeadersMode, RequestTrustSettings};
        use std::net::{IpAddr, SocketAddr};

        let mut settings = RustokSettings {
            tenant: TenantSettings {
                profile: TenantRuntimeProfile::MultiTenant,
                enabled: true,
                resolution: TenantResolutionMode::Host,
                header_name: "X-Tenant-ID".to_string(),
                default_id: Uuid::from_u128(1),
                fallback_mode: TenantFallbackMode::Disabled,
                base_domains: vec![],
            },
            ..RustokSettings::default()
        };
        settings.runtime.request_trust = RequestTrustSettings {
            forwarded_headers_mode: ForwardedHeadersMode::TrustedOnly,
            trusted_proxy_cidrs: vec!["10.0.0.0/8".to_string()],
        };

        let mut req = request("/api/users");
        req.headers_mut().insert(
            "Host",
            axum::http::HeaderValue::from_static("internal.test"),
        );
        req.headers_mut().append(
            "X-Forwarded-Host",
            axum::http::HeaderValue::from_static("spoofed.example.test"),
        );
        req.headers_mut().append(
            "X-Forwarded-Host",
            axum::http::HeaderValue::from_static("tenant.example.test"),
        );
        req.extensions_mut()
            .insert(SocketAddr::new(IpAddr::from([10, 0, 0, 7]), 443));

        let resolution = resolve_request(&req, &settings)
            .expect("last host added by trusted proxy should determine tenant");
        assert_eq!(
            resolution.identifier,
            ResolvedTenantIdentifier::Host("tenant.example.test".to_string())
        );

        // A client outside the configured proxy network cannot supply a
        // trusted tenant host, even with the same repeated header values.
        req.extensions_mut()
            .insert(SocketAddr::new(IpAddr::from([192, 0, 2, 7]), 443));
        let untrusted =
            resolve_request(&req, &settings).expect("untrusted forwarding should be ignored");
        assert_eq!(
            untrusted.identifier,
            ResolvedTenantIdentifier::Host("internal.test".to_string())
        );
    }

    #[test]
    fn subdomain_matching_is_case_insensitive_for_base_domain() {
        let domains = vec!["Example.Test.".to_string()];
        assert_eq!(
            subdomain_identifier("STORE.example.test", &domains).expect("slug"),
            "store"
        );
        assert_eq!(
            subdomain_identifier("store.EXAMPLE.TEST", &domains).expect("slug"),
            "store"
        );
        assert!(matches!(
            subdomain_identifier("EXAMPLE.TEST", &domains),
            Err(TenantResolutionError::BaseDomainRequiresTenantSlug { .. })
        ));
    }

    #[test]
    fn tenant_resolution_error_code_mapping() {
        let missing = TenantResolutionError::MissingHeader {
            header_name: "X-Tenant-ID".to_string(),
        };
        assert_eq!(missing.error_code(), "missing_tenant_header");
        assert_eq!(missing.status_code(), StatusCode::BAD_REQUEST);

        let conflict = TenantResolutionError::ConflictingHeaderValues {
            header_name: "host".to_string(),
        };
        assert_eq!(conflict.error_code(), "conflicting_tenant_headers");
        assert_eq!(conflict.status_code(), StatusCode::BAD_REQUEST);

        let not_found = TenantResolutionError::NoBaseDomainMatch {
            host: "unknown.com".to_string(),
        };
        assert_eq!(not_found.error_code(), "tenant_not_found");
        assert_eq!(not_found.status_code(), StatusCode::NOT_FOUND);
    }
}
