use std::{net::IpAddr, time::Duration};

use thiserror::Error;
use tonic::transport::{ClientTlsConfig, Endpoint};
use url::{Host, Url};

use crate::GrpcProductCatalogReadProvider;

const MAX_CONNECT_TIMEOUT_MS: u64 = 30_000;

/// Deployment-owned connection settings for the extracted Product catalog read service.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GrpcProductCatalogReadConnectionConfig {
    endpoint: String,
    tls_domain: Option<String>,
    connect_timeout: Duration,
    allow_insecure_loopback: bool,
}

impl GrpcProductCatalogReadConnectionConfig {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            tls_domain: None,
            connect_timeout: Duration::from_secs(5),
            allow_insecure_loopback: false,
        }
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
    ) -> Result<ValidatedGrpcProductCatalogReadConnection, GrpcProductCatalogReadConnectionError>
    {
        let endpoint = validate_endpoint(self.endpoint.as_str(), self.allow_insecure_loopback)?;
        let tls_domain = normalize_tls_domain(self.tls_domain.as_deref())?;
        let timeout_ms = u64::try_from(self.connect_timeout.as_millis()).unwrap_or(u64::MAX);
        if timeout_ms == 0 || timeout_ms > MAX_CONNECT_TIMEOUT_MS {
            return Err(GrpcProductCatalogReadConnectionError::InvalidConnectTimeout);
        }

        Ok(ValidatedGrpcProductCatalogReadConnection {
            endpoint: endpoint.url,
            endpoint_uses_tls: endpoint.uses_tls,
            tls_domain,
            connect_timeout: self.connect_timeout,
        })
    }

    pub async fn connect(
        &self,
    ) -> Result<GrpcProductCatalogReadProvider, GrpcProductCatalogReadConnectionError> {
        let validated = self.validated()?;
        let mut endpoint = Endpoint::from_shared(validated.endpoint)
            .map_err(|_| GrpcProductCatalogReadConnectionError::InvalidEndpoint)?
            .connect_timeout(validated.connect_timeout)
            .tcp_keepalive(Some(Duration::from_secs(30)));

        if validated.endpoint_uses_tls {
            let mut tls = ClientTlsConfig::new().with_webpki_roots();
            if let Some(domain) = validated.tls_domain {
                tls = tls.domain_name(domain);
            }
            endpoint = endpoint
                .tls_config(tls)
                .map_err(GrpcProductCatalogReadConnectionError::TransportConfiguration)?;
        }

        GrpcProductCatalogReadProvider::connect(endpoint)
            .await
            .map_err(GrpcProductCatalogReadConnectionError::Connection)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ValidatedGrpcProductCatalogReadConnection {
    pub endpoint: String,
    pub endpoint_uses_tls: bool,
    pub tls_domain: Option<String>,
    pub connect_timeout: Duration,
}

#[derive(Debug, Error)]
pub enum GrpcProductCatalogReadConnectionError {
    #[error(
        "Product catalog gRPC endpoint must be an absolute http(s) service URL without credentials, query, fragment, or path"
    )]
    InvalidEndpoint,
    #[error(
        "insecure Product catalog gRPC endpoint is allowed only for an explicitly enabled loopback host"
    )]
    InsecureEndpointForbidden,
    #[error("Product catalog gRPC TLS domain is invalid")]
    InvalidTlsDomain,
    #[error("Product catalog gRPC connect timeout must be between 1 and 30000 milliseconds")]
    InvalidConnectTimeout,
    #[error("Product catalog gRPC transport configuration failed")]
    TransportConfiguration(#[source] tonic::transport::Error),
    #[error("Product catalog gRPC connection failed")]
    Connection(#[source] tonic::transport::Error),
}

#[derive(Debug)]
struct ValidatedEndpoint {
    url: String,
    uses_tls: bool,
}

fn validate_endpoint(
    value: &str,
    allow_insecure_loopback: bool,
) -> Result<ValidatedEndpoint, GrpcProductCatalogReadConnectionError> {
    let parsed = Url::parse(value.trim())
        .map_err(|_| GrpcProductCatalogReadConnectionError::InvalidEndpoint)?;
    if parsed.host().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !matches!(parsed.path(), "" | "/")
    {
        return Err(GrpcProductCatalogReadConnectionError::InvalidEndpoint);
    }

    let uses_tls = match parsed.scheme() {
        "https" => true,
        "http" if allow_insecure_loopback && is_loopback_host(parsed.host()) => false,
        "http" => return Err(GrpcProductCatalogReadConnectionError::InsecureEndpointForbidden),
        _ => return Err(GrpcProductCatalogReadConnectionError::InvalidEndpoint),
    };

    Ok(ValidatedEndpoint {
        url: parsed.to_string(),
        uses_tls,
    })
}

fn normalize_tls_domain(
    value: Option<&str>,
) -> Result<Option<String>, GrpcProductCatalogReadConnectionError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.chars().any(char::is_control)
        || value.contains('/')
        || value.contains('@')
        || Host::parse(value).is_err()
    {
        return Err(GrpcProductCatalogReadConnectionError::InvalidTlsDomain);
    }
    Ok(Some(value.to_string()))
}

fn is_loopback_host(host: Option<Host<&str>>) -> bool {
    match host {
        Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(address)) => IpAddr::V4(address).is_loopback(),
        Some(Host::Ipv6(address)) => IpAddr::V6(address).is_loopback(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{GrpcProductCatalogReadConnectionConfig, GrpcProductCatalogReadConnectionError};

    #[test]
    fn https_endpoint_is_accepted() {
        let validated =
            GrpcProductCatalogReadConnectionConfig::new("https://product-catalog.internal:7443")
                .with_tls_domain(Some("product-catalog.internal".to_string()))
                .with_connect_timeout(Duration::from_millis(2_500))
                .validated()
                .expect("HTTPS Product endpoint should validate");

        assert!(validated.endpoint_uses_tls);
        assert_eq!(
            validated.tls_domain.as_deref(),
            Some("product-catalog.internal")
        );
        assert_eq!(validated.connect_timeout, Duration::from_millis(2_500));
    }

    #[test]
    fn plaintext_requires_explicit_loopback() {
        assert!(matches!(
            GrpcProductCatalogReadConnectionConfig::new("http://127.0.0.1:7443").validated(),
            Err(GrpcProductCatalogReadConnectionError::InsecureEndpointForbidden)
        ));
        assert!(
            GrpcProductCatalogReadConnectionConfig::new("http://127.0.0.1:7443")
                .allow_insecure_loopback(true)
                .validated()
                .is_ok()
        );
    }

    #[test]
    fn non_loopback_plaintext_is_forbidden() {
        assert!(matches!(
            GrpcProductCatalogReadConnectionConfig::new("http://product.internal:7443")
                .allow_insecure_loopback(true)
                .validated(),
            Err(GrpcProductCatalogReadConnectionError::InsecureEndpointForbidden)
        ));
    }

    #[test]
    fn credentials_and_paths_are_rejected() {
        for endpoint in [
            "https://user:secret@product.internal:7443",
            "https://product.internal:7443/catalog",
            "https://product.internal:7443?tenant=a",
        ] {
            assert!(matches!(
                GrpcProductCatalogReadConnectionConfig::new(endpoint).validated(),
                Err(GrpcProductCatalogReadConnectionError::InvalidEndpoint)
            ));
        }
    }

    #[test]
    fn connect_timeout_is_bounded() {
        assert!(matches!(
            GrpcProductCatalogReadConnectionConfig::new("https://product.internal:7443")
                .with_connect_timeout(Duration::ZERO)
                .validated(),
            Err(GrpcProductCatalogReadConnectionError::InvalidConnectTimeout)
        ));
        assert!(matches!(
            GrpcProductCatalogReadConnectionConfig::new("https://product.internal:7443")
                .with_connect_timeout(Duration::from_millis(30_001))
                .validated(),
            Err(GrpcProductCatalogReadConnectionError::InvalidConnectTimeout)
        ));
    }
}
