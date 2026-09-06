use std::net::IpAddr;
use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use rustok_media::{MediaImageDescriptor, MediaPublicImageAsset, MediaPublicImageReadPort};
use thiserror::Error;
use tonic::transport::{ClientTlsConfig, Endpoint};
use url::{Host, Url};
use uuid::Uuid;

use crate::GrpcMediaProvider;

const MAX_CONNECT_TIMEOUT_MS: u64 = 30_000;

/// Deployment-owned connection settings for the extracted Media public-image control plane.
///
/// The gRPC endpoint carries descriptor metadata only. `public_origin` is used solely to resolve
/// root-relative Media-owned descriptor URLs when the byte endpoint is exposed on a different
/// public origin from the consuming storefront/API host.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GrpcMediaPublicImageConnectionConfig {
    endpoint: String,
    public_origin: Option<String>,
    tls_domain: Option<String>,
    connect_timeout: Duration,
    allow_insecure_loopback: bool,
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

    pub async fn connect(
        &self,
    ) -> Result<GrpcMediaPublicImageProvider, GrpcMediaPublicImageConnectionError> {
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

        let inner = GrpcMediaProvider::connect(endpoint)
            .await
            .map_err(GrpcMediaPublicImageConnectionError::Connection)?;
        Ok(GrpcMediaPublicImageProvider {
            inner,
            public_origin: validated.public_origin,
        })
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

/// Public-image-only remote adapter. Binary image delivery remains on the Media HTTP route.
pub struct GrpcMediaPublicImageProvider {
    inner: GrpcMediaProvider,
    public_origin: Option<String>,
}

#[async_trait]
impl MediaPublicImageReadPort for GrpcMediaPublicImageProvider {
    async fn get_public_image_asset(
        &self,
        context: PortContext,
        media_id: Uuid,
        alt: Option<String>,
    ) -> Result<MediaPublicImageAsset, PortError> {
        let mut asset = self
            .inner
            .get_public_image_asset(context, media_id, alt)
            .await?;
        rebase_public_descriptor(asset.descriptor.as_mut(), self.public_origin.as_deref());
        Ok(asset)
    }
}

#[derive(Debug)]
struct ValidatedServiceUrl {
    url: String,
    uses_tls: bool,
}

#[derive(Debug, Clone, Copy)]
enum ServiceUrlKind {
    Endpoint,
    PublicOrigin,
}

fn validate_service_url(
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

fn is_loopback_host(host: Option<Host<&str>>) -> bool {
    match host {
        Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(address)) => IpAddr::V4(address).is_loopback(),
        Some(Host::Ipv6(address)) => IpAddr::V6(address).is_loopback(),
        None => false,
    }
}

fn rebase_public_descriptor(
    descriptor: Option<&mut MediaImageDescriptor>,
    public_origin: Option<&str>,
) {
    let (Some(descriptor), Some(public_origin)) = (descriptor, public_origin) else {
        return;
    };
    if descriptor.url.starts_with('/') && !descriptor.url.starts_with("//") {
        descriptor.url = format!("{public_origin}{}", descriptor.url);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GrpcMediaPublicImageConnectionConfig, GrpcMediaPublicImageConnectionError,
        rebase_public_descriptor,
    };
    use rustok_media::MediaImageDescriptor;
    use std::time::Duration;

    #[test]
    fn https_endpoint_and_origin_are_normalized() {
        let validated = GrpcMediaPublicImageConnectionConfig::new("https://media.internal:7443")
            .with_public_origin(Some("https://media.example.com/".to_string()))
            .with_tls_domain(Some("media.internal".to_string()))
            .validated()
            .expect("https remote media configuration should validate");

        assert!(validated.endpoint_uses_tls);
        assert_eq!(
            validated.public_origin.as_deref(),
            Some("https://media.example.com")
        );
        assert_eq!(validated.tls_domain.as_deref(), Some("media.internal"));
    }

    #[test]
    fn external_plaintext_endpoint_is_rejected() {
        let error = GrpcMediaPublicImageConnectionConfig::new("http://media.internal:50051")
            .validated()
            .expect_err("external plaintext media must fail closed");
        assert!(matches!(
            error,
            GrpcMediaPublicImageConnectionError::InsecureEndpointForbidden
        ));
    }

    #[test]
    fn plaintext_loopback_requires_explicit_opt_in() {
        let rejected = GrpcMediaPublicImageConnectionConfig::new("http://127.0.0.1:50051")
            .validated()
            .expect_err("loopback plaintext must still require explicit opt in");
        assert!(matches!(
            rejected,
            GrpcMediaPublicImageConnectionError::InsecureEndpointForbidden
        ));

        let accepted = GrpcMediaPublicImageConnectionConfig::new("http://127.0.0.1:50051")
            .allow_insecure_loopback(true)
            .validated()
            .expect("explicit loopback development transport should validate");
        assert!(!accepted.endpoint_uses_tls);
    }

    #[test]
    fn timeout_is_bounded() {
        let error = GrpcMediaPublicImageConnectionConfig::new("https://media.internal")
            .with_connect_timeout(Duration::from_secs(31))
            .validated()
            .expect_err("unbounded connect timeout must fail closed");
        assert!(matches!(
            error,
            GrpcMediaPublicImageConnectionError::InvalidConnectTimeout
        ));
    }

    #[test]
    fn public_origin_rebases_only_root_relative_descriptors() {
        let mut relative = MediaImageDescriptor::from_parts(
            "/api/media/public/images/asset/checksum",
            None,
            None,
            None,
            Some("image/png".to_string()),
        )
        .expect("relative descriptor");
        rebase_public_descriptor(Some(&mut relative), Some("https://media.example.com"));
        assert_eq!(
            relative.url,
            "https://media.example.com/api/media/public/images/asset/checksum"
        );

        let mut absolute = MediaImageDescriptor::from_parts(
            "https://cdn.example.com/image.png",
            None,
            None,
            None,
            Some("image/png".to_string()),
        )
        .expect("absolute descriptor");
        rebase_public_descriptor(Some(&mut absolute), Some("https://media.example.com"));
        assert_eq!(absolute.url, "https://cdn.example.com/image.png");
    }
}
