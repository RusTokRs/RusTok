use std::net::IpAddr;
use std::time::Duration;

use thiserror::Error;
use tonic::transport::{ClientTlsConfig, Endpoint};
use url::{Host, Url};

use super::auth::MediaGrpcBearerToken;

const MAX_CONNECT_TIMEOUT_MS: u64 = 30_000;

/// Configuration for general Media gRPC connections.
#[derive(Debug, Clone)]
pub struct GrpcMediaConnectionConfig {
    pub endpoint: String,
    pub bearer_token: Option<MediaGrpcBearerToken>,
    pub tls_domain: Option<String>,
    pub connect_timeout: Duration,
    pub allow_insecure_loopback: bool,
}

impl GrpcMediaConnectionConfig {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            bearer_token: None,
            tls_domain: None,
            connect_timeout: Duration::from_secs(5),
            allow_insecure_loopback: false,
        }
    }

    pub fn with_bearer_token(mut self, token: MediaGrpcBearerToken) -> Self {
        self.bearer_token = Some(token);
        self
    }

    pub fn with_tls_domain(mut self, tls_domain: Option<String>) -> Self {
        self.tls_domain = tls_domain;
        self
    }

    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    pub fn allow_insecure_loopback(mut self, allow: bool) -> Self {
        self.allow_insecure_loopback = allow;
        self
    }
}

/// Deployment-owned connection settings for the extracted Media public-image control plane.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GrpcMediaPublicImageConnectionConfig {
    pub(crate) endpoint: String,
    pub(crate) public_origin: Option<String>,
    pub(crate) tls_domain: Option<String>,
    pub(crate) connect_timeout: Duration,
    pub(crate) allow_insecure_loopback: bool,
}

impl GrpcMediaPublicImageConnectionConfig {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            public_origin: None,
            tls_domain: None,
            connect_timeout: Duration::from_secs(5),
            allow_insecure_loopback: false,
        }
    }

    pub fn with_public_origin(mut self, public_origin: Option<String>) -> Self {
        self.public_origin = public_origin;
        self
    }

    pub fn with_tls_domain(mut self, tls_domain: Option<String>) -> Self {
        self.tls_domain = tls_domain;
        self
    }

    pub fn with_connect_timeout(mut self, connect_timeout: Duration) -> Self {
        self.connect_timeout = connect_timeout;
        self
    }

    pub fn allow_insecure_loopback(mut self, allow: bool) -> Self {
        self.allow_insecure_loopback = allow;
        self
    }

    pub fn validated(
        &self,
    ) -> Result<ValidatedGrpcMediaPublicImageConnection, GrpcMediaPublicImageConnectionError> {
        let endpoint = validate_service_url(
            self.endpoint.as_str(),
            self.allow_insecure_loopback,
            ServiceUrlKind::Endpoint,
        )?;
        let public_origin = self
            .public_origin
            .as_deref()
            .map(|value| validate_public_origin(value, self.allow_insecure_loopback))
            .transpose()?;
        let tls_domain = normalize_tls_domain(self.tls_domain.as_deref())?;
        let timeout_ms = u64::try_from(self.connect_timeout.as_millis()).unwrap_or(u64::MAX);
        if timeout_ms == 0 || timeout_ms > MAX_CONNECT_TIMEOUT_MS {
            return Err(GrpcMediaPublicImageConnectionError::InvalidConnectTimeout);
        }

        Ok(ValidatedGrpcMediaPublicImageConnection {
            endpoint: endpoint.url,
            endpoint_uses_tls: endpoint.uses_tls,
            public_origin,
            tls_domain,
            connect_timeout: self.connect_timeout,
        })
    }

    #[cfg(feature = "grpc-client")]
    pub async fn connect(
        &self,
    ) -> Result<super::public_image_client::GrpcMediaPublicImageProvider, GrpcMediaPublicImageConnectionError>
    {
        let validated = self.validated()?;
        let mut endpoint = Endpoint::from_shared(validated.endpoint)
            .map_err(|_| GrpcMediaPublicImageConnectionError::InvalidEndpoint)?
            .connect_timeout(validated.connect_timeout)
            .tcp_keepalive(Some(Duration::from_secs(30)));

        if validated.endpoint_uses_tls {
            let mut tls = ClientTlsConfig::new().with_webpki_roots();
            if let Some(domain) = validated.tls_domain {
                tls = tls.domain_name(domain);
            }
            endpoint = endpoint
                .tls_config(tls)
                .map_err(GrpcMediaPublicImageConnectionError::TransportConfiguration)?;
        }

        let inner = super::client::GrpcMediaProvider::connect(endpoint)
            .await
            .map_err(GrpcMediaPublicImageConnectionError::Connection)?;
        Ok(super::public_image_client::GrpcMediaPublicImageProvider::new(
            inner,
            validated.public_origin,
        ))
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ValidatedGrpcMediaPublicImageConnection {
    pub endpoint: String,
    pub endpoint_uses_tls: bool,
    pub public_origin: Option<String>,
    pub tls_domain: Option<String>,
    pub connect_timeout: Duration,
}

#[derive(Debug, Error)]
pub enum GrpcMediaPublicImageConnectionError {
    #[error(
        "media gRPC endpoint must be an absolute http(s) service URL without credentials, query, fragment, or path"
    )]
    InvalidEndpoint,
    #[error("insecure media gRPC endpoint is allowed only for an explicitly enabled loopback host")]
    InsecureEndpointForbidden,
    #[error(
        "media public origin must be an absolute http(s) origin without credentials, query, fragment, or path"
    )]
    InvalidPublicOrigin,
    #[error("insecure media public origin is allowed only for an explicitly enabled loopback host")]
    InsecurePublicOriginForbidden,
    #[error("media gRPC TLS domain is invalid")]
    InvalidTlsDomain,
    #[error("media gRPC connect timeout must be between 1 and 30000 milliseconds")]
    InvalidConnectTimeout,
    #[error("media gRPC transport configuration failed")]
    TransportConfiguration(#[source] tonic::transport::Error),
    #[error("media gRPC connection failed")]
    Connection(#[source] tonic::transport::Error),
}

#[derive(Debug)]
pub(crate) struct ValidatedServiceUrl {
    pub(crate) url: String,
    pub(crate) uses_tls: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ServiceUrlKind {
    Endpoint,
    PublicOrigin,
}

pub(crate) fn validate_service_url(
    value: &str,
    allow_insecure_loopback: bool,
    kind: ServiceUrlKind,
) -> Result<ValidatedServiceUrl, GrpcMediaPublicImageConnectionError> {
    let parsed = Url::parse(value.trim()).map_err(|_| invalid_url_error(kind))?;
    if parsed.host().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !matches!(parsed.path(), "" | "/")
    {
        return Err(invalid_url_error(kind));
    }

    let uses_tls = match parsed.scheme() {
        "https" => true,
        "http" if allow_insecure_loopback && is_loopback_host(parsed.host()) => false,
        "http" => return Err(insecure_url_error(kind)),
        _ => return Err(invalid_url_error(kind)),
    };

    Ok(ValidatedServiceUrl {
        url: parsed.to_string(),
        uses_tls,
    })
}

fn invalid_url_error(kind: ServiceUrlKind) -> GrpcMediaPublicImageConnectionError {
    match kind {
        ServiceUrlKind::Endpoint => GrpcMediaPublicImageConnectionError::InvalidEndpoint,
        ServiceUrlKind::PublicOrigin => GrpcMediaPublicImageConnectionError::InvalidPublicOrigin,
    }
}

fn insecure_url_error(kind: ServiceUrlKind) -> GrpcMediaPublicImageConnectionError {
    match kind {
        ServiceUrlKind::Endpoint => GrpcMediaPublicImageConnectionError::InsecureEndpointForbidden,
        ServiceUrlKind::PublicOrigin => {
            GrpcMediaPublicImageConnectionError::InsecurePublicOriginForbidden
        }
    }
}

fn validate_public_origin(
    value: &str,
    allow_insecure_loopback: bool,
) -> Result<String, GrpcMediaPublicImageConnectionError> {
    let validated =
        validate_service_url(value, allow_insecure_loopback, ServiceUrlKind::PublicOrigin)?;
    let parsed = Url::parse(validated.url.as_str())
        .map_err(|_| GrpcMediaPublicImageConnectionError::InvalidPublicOrigin)?;
    Ok(parsed.origin().ascii_serialization())
}

fn normalize_tls_domain(
    value: Option<&str>,
) -> Result<Option<String>, GrpcMediaPublicImageConnectionError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.chars().any(char::is_control)
        || value.contains('/')
        || value.contains('@')
        || Host::parse(value).is_err()
    {
        return Err(GrpcMediaPublicImageConnectionError::InvalidTlsDomain);
    }
    Ok(Some(value.to_string()))
}

pub fn is_loopback_host(host: Option<Host<&str>>) -> bool {
    match host {
        Some(Host::Domain(domain)) => {
            domain.eq_ignore_ascii_case("localhost")
                || domain.to_ascii_lowercase().ends_with(".localhost")
        }
        Some(Host::Ipv4(address)) => IpAddr::V4(address).is_loopback(),
        Some(Host::Ipv6(address)) => IpAddr::V6(address).is_loopback(),
        None => false,
    }
}

