//! Shared mTLS listener configuration for isolated RusToK worker processes.

use std::{fs, net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use sha2::{Digest, Sha256};
#[cfg(unix)]
use tokio::signal::unix::SignalKind;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tonic::transport::{Certificate, ClientTlsConfig, Identity, Server, ServerTlsConfig};
use tonic::{Request, Status};

/// Stable SHA-256 identity of the verified leaf certificate presented by an
/// mTLS peer. The fingerprint identifies a deployment principal, never a user
/// or an application-level role.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PeerCertificateFingerprint(String);

impl PeerCertificateFingerprint {
    const PREFIX: &str = "sha256:";
    const HEX_LENGTH: usize = 64;

    /// Parses the canonical lowercase fingerprint representation used by
    /// deployment-owned peer identity maps.
    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        let Some(hex) = value.strip_prefix(Self::PREFIX) else {
            return Err("peer certificate fingerprint must use the sha256: prefix".to_string());
        };
        if hex.len() != Self::HEX_LENGTH
            || hex
                .chars()
                .any(|character| !character.is_ascii_digit() && !('a'..='f').contains(&character))
        {
            return Err(
                "peer certificate fingerprint must contain 64 lowercase hexadecimal characters"
                    .to_string(),
            );
        }
        Ok(Self(value))
    }

    /// Returns the canonical fingerprint suitable for a deployment-owned
    /// identity resolver.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn from_leaf_certificate_der(certificate_der: &[u8]) -> Result<Self, String> {
        if certificate_der.is_empty() {
            return Err("peer certificate must not be empty".to_string());
        }
        Self::parse(format!(
            "{}{}",
            Self::PREFIX,
            hex::encode(Sha256::digest(certificate_der))
        ))
    }
}

/// Extracts the leaf-certificate fingerprint from a request that was accepted
/// by a mutual-TLS listener. The listener establishes the certificate chain;
/// this helper only turns the verified peer certificate into a stable
/// deployment identity. Missing or malformed TLS evidence is never inferred
/// from request fields and fails closed.
pub fn peer_certificate_fingerprint<T>(
    request: &Request<T>,
) -> Result<PeerCertificateFingerprint, Status> {
    let certificates = request
        .peer_certs()
        .ok_or_else(|| Status::unauthenticated("mutual TLS peer certificate is required"))?;
    let certificate = certificates
        .first()
        .ok_or_else(|| Status::unauthenticated("mutual TLS peer certificate is required"))?;
    PeerCertificateFingerprint::from_leaf_certificate_der(certificate.as_ref())
        .map_err(Status::unauthenticated)
}

/// Deployment-owned mutually authenticated listener configuration. A worker
/// supplies its uppercase environment-variable prefix, such as
/// `RUSTOK_VERIFICATION` or `RUSTOK_MODULE_BUILD`.
pub struct MutualTlsListenerConfig {
    pub address: SocketAddr,
    pub certificate_pem: Vec<u8>,
    pub private_key_pem: Vec<u8>,
    pub client_ca_pem: Vec<u8>,
    pub request_timeout: Duration,
    pub admission_timeout: Duration,
    pub concurrency_limit: usize,
    pub max_message_size: usize,
    message_size_ceiling: usize,
}

impl MutualTlsListenerConfig {
    const DEFAULT_TIMEOUT_MS: u64 = 30_000;
    const DEFAULT_CONCURRENCY_LIMIT: usize = 16;
    const DEFAULT_MAX_MESSAGE_SIZE: usize = 128 * 1024;
    const ABSOLUTE_MESSAGE_SIZE_CEILING: usize = 128 * 1024 * 1024;
    pub const STANDARD_MESSAGE_SIZE_CEILING: usize = 1024 * 1024;

    pub fn from_env_prefix(prefix: &str, message_size_ceiling: usize) -> Result<Self, String> {
        validate_prefix(prefix)?;
        validate_message_size_ceiling(message_size_ceiling)?;
        let address_name = env_name(prefix, "LISTEN_ADDR");
        let address = required_env(&address_name)?
            .parse()
            .map_err(|error| format!("{address_name} is invalid: {error}"))?;
        let certificate_pem = read_required_file(&env_name(prefix, "SERVER_CERT_PEM"))?;
        let private_key_pem = read_required_file(&env_name(prefix, "SERVER_KEY_PEM"))?;
        let client_ca_pem = read_required_file(&env_name(prefix, "CLIENT_CA_PEM"))?;
        let request_timeout = Duration::from_millis(parse_env(
            &env_name(prefix, "REQUEST_TIMEOUT_MS"),
            Self::DEFAULT_TIMEOUT_MS,
        )?);
        let admission_timeout = parse_duration_ms(prefix, "ADMISSION_TIMEOUT_MS", 250)?;
        let concurrency_limit = parse_env(
            &env_name(prefix, "CONCURRENCY_LIMIT"),
            Self::DEFAULT_CONCURRENCY_LIMIT,
        )?;
        let max_message_size = parse_env(
            &env_name(prefix, "MAX_MESSAGE_SIZE"),
            Self::DEFAULT_MAX_MESSAGE_SIZE.min(message_size_ceiling),
        )?;
        let config = Self {
            address,
            certificate_pem,
            private_key_pem,
            client_ca_pem,
            request_timeout,
            admission_timeout,
            concurrency_limit,
            max_message_size,
            message_size_ceiling,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.certificate_pem.is_empty()
            || self.private_key_pem.is_empty()
            || self.client_ca_pem.is_empty()
        {
            return Err("worker listener TLS material must not be empty".to_string());
        }
        if self.request_timeout.is_zero() {
            return Err("worker listener timeout must be positive".to_string());
        }
        if self.admission_timeout.is_zero() || self.admission_timeout > self.request_timeout {
            return Err(
                "worker listener ADMISSION_TIMEOUT_MS must be positive and must not exceed REQUEST_TIMEOUT_MS"
                    .to_string(),
            );
        }
        if self.concurrency_limit == 0 {
            return Err("worker listener concurrency limit must be positive".to_string());
        }
        validate_message_size_ceiling(self.message_size_ceiling)?;
        if self.max_message_size == 0 || self.max_message_size > self.message_size_ceiling {
            return Err(format!(
                "worker listener max message size must be between 1 and {} bytes",
                self.message_size_ceiling
            ));
        }
        Ok(())
    }

    pub fn server(&self) -> Result<Server, tonic::transport::Error> {
        Server::builder().tls_config(
            ServerTlsConfig::new()
                .identity(Identity::from_pem(
                    self.certificate_pem.clone(),
                    self.private_key_pem.clone(),
                ))
                .client_ca_root(Certificate::from_pem(self.client_ca_pem.clone())),
        )
    }
}

/// Process-wide admission shared by every connection to one worker. Tonic's
/// per-connection concurrency limit remains a transport safeguard; this permit
/// is the canonical global bound across all authenticated clients.
#[derive(Clone)]
pub struct WorkerAdmission {
    permits: Arc<Semaphore>,
    admission_timeout: Duration,
}

pub type WorkerPermit = OwnedSemaphorePermit;

impl WorkerAdmission {
    pub fn from_listener(listener: &MutualTlsListenerConfig) -> Result<Self, String> {
        listener.validate()?;
        Self::new(listener.concurrency_limit, listener.admission_timeout)
    }

    fn new(concurrency_limit: usize, admission_timeout: Duration) -> Result<Self, String> {
        if concurrency_limit == 0 || admission_timeout.is_zero() {
            return Err("worker admission limits must be positive".to_string());
        }
        Ok(Self {
            permits: Arc::new(Semaphore::new(concurrency_limit)),
            admission_timeout,
        })
    }

    pub async fn acquire(&self) -> Result<WorkerPermit, Status> {
        match tokio::time::timeout(
            self.admission_timeout,
            Arc::clone(&self.permits).acquire_owned(),
        )
        .await
        {
            Ok(Ok(permit)) => Ok(permit),
            Ok(Err(_)) => Err(Status::unavailable("worker is shutting down")),
            Err(_) => Err(Status::resource_exhausted(
                "worker concurrency is saturated",
            )),
        }
    }

    pub fn close(&self) {
        self.permits.close();
    }
}

/// Resolves on SIGTERM or Ctrl+C so worker hosts stop accepting new RPCs and
/// tonic can drain bounded in-flight requests. Failure to install a termination
/// handler is itself a stop condition rather than an excuse to run unsupervised.
pub async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate = match tokio::signal::unix::signal(SignalKind::terminate()) {
            Ok(signal) => signal,
            Err(error) => {
                tracing::error!(%error, "failed to install SIGTERM handler; stopping worker");
                return;
            }
        };
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                if let Err(error) = result {
                    tracing::error!(%error, "failed to await Ctrl+C; stopping worker");
                }
            }
            _ = terminate.recv() => {}
        }
    }

    #[cfg(not(unix))]
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "failed to await Ctrl+C; stopping worker");
    }
}

fn validate_message_size_ceiling(message_size_ceiling: usize) -> Result<(), String> {
    if message_size_ceiling == 0
        || message_size_ceiling > MutualTlsListenerConfig::ABSOLUTE_MESSAGE_SIZE_CEILING
    {
        return Err(format!(
            "worker listener message-size ceiling must be between 1 and {} bytes",
            MutualTlsListenerConfig::ABSOLUTE_MESSAGE_SIZE_CEILING
        ));
    }
    Ok(())
}

/// Deployment-owned client identity for a mutually authenticated worker
/// connection. The same prefix convention keeps client and listener material
/// scoped to one named worker without relying on ambient TLS settings.
pub struct MutualTlsClientConfig {
    certificate_pem: Vec<u8>,
    private_key_pem: Vec<u8>,
    server_ca_pem: Vec<u8>,
    server_domain: String,
}

impl MutualTlsClientConfig {
    pub fn from_env_prefix(prefix: &str) -> Result<Self, String> {
        validate_prefix(prefix)?;
        let certificate_pem = read_required_file(&env_name(prefix, "CLIENT_CERT_PEM"))?;
        let private_key_pem = read_required_file(&env_name(prefix, "CLIENT_KEY_PEM"))?;
        let server_ca_pem = read_required_file(&env_name(prefix, "SERVER_CA_PEM"))?;
        let server_domain = required_env(&env_name(prefix, "SERVER_DOMAIN"))?;
        let config = Self {
            certificate_pem,
            private_key_pem,
            server_ca_pem,
            server_domain,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.certificate_pem.is_empty()
            || self.private_key_pem.is_empty()
            || self.server_ca_pem.is_empty()
            || self.server_domain.trim().is_empty()
        {
            return Err("worker client TLS configuration must not be empty".to_string());
        }
        Ok(())
    }

    pub fn tls_config(&self) -> ClientTlsConfig {
        ClientTlsConfig::new()
            .identity(Identity::from_pem(
                self.certificate_pem.clone(),
                self.private_key_pem.clone(),
            ))
            .ca_certificate(Certificate::from_pem(self.server_ca_pem.clone()))
            .domain_name(self.server_domain.clone())
    }
}

fn env_name(prefix: &str, suffix: &str) -> String {
    format!("{prefix}_{suffix}")
}

fn validate_prefix(prefix: &str) -> Result<(), String> {
    if prefix.is_empty()
        || prefix.starts_with('_')
        || prefix.ends_with('_')
        || prefix.chars().any(|character| {
            !character.is_ascii_uppercase() && !character.is_ascii_digit() && character != '_'
        })
    {
        return Err("worker environment prefix must be uppercase snake case".to_string());
    }
    Ok(())
}

fn required_env(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("{name} must be configured"))
}

fn read_required_file(name: &str) -> Result<Vec<u8>, String> {
    let path = PathBuf::from(required_env(name)?);
    fs::read(&path).map_err(|error| format!("could not read {}: {error}", path.display()))
}

fn parse_env<T>(name: &str, default: T) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match std::env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|error| format!("{name} is invalid: {error}")),
        Err(_) => Ok(default),
    }
}

fn parse_duration_ms(prefix: &str, suffix: &str, default_ms: u64) -> Result<Duration, String> {
    let name = env_name(prefix, suffix);
    let milliseconds = parse_env(&name, default_ms)?;
    if milliseconds == 0 {
        return Err(format!("{name} must be positive"));
    }
    Ok(Duration::from_millis(milliseconds))
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, time::Duration};

    use tonic::Request;

    use super::{
        MutualTlsClientConfig, MutualTlsListenerConfig, PeerCertificateFingerprint,
        WorkerAdmission, peer_certificate_fingerprint,
    };

    fn config() -> MutualTlsListenerConfig {
        MutualTlsListenerConfig {
            address: "127.0.0.1:9443".parse::<SocketAddr>().expect("address"),
            certificate_pem: b"certificate".to_vec(),
            private_key_pem: b"key".to_vec(),
            client_ca_pem: b"ca".to_vec(),
            request_timeout: Duration::from_secs(1),
            admission_timeout: Duration::from_millis(50),
            concurrency_limit: 1,
            max_message_size: 1024,
            message_size_ceiling: MutualTlsListenerConfig::STANDARD_MESSAGE_SIZE_CEILING,
        }
    }

    #[test]
    fn listener_rejects_unbounded_message_size() {
        let mut config = config();
        config.max_message_size = config.message_size_ceiling + 1;
        assert!(config.validate().is_err());
    }

    #[test]
    fn listener_rejects_an_unbounded_caller_ceiling() {
        assert!(
            super::validate_message_size_ceiling(
                MutualTlsListenerConfig::ABSOLUTE_MESSAGE_SIZE_CEILING + 1
            )
            .is_err()
        );
    }

    #[test]
    fn listener_rejects_empty_client_ca() {
        let mut config = config();
        config.client_ca_pem.clear();
        assert!(config.validate().is_err());
    }

    #[test]
    fn listener_rejects_unsafe_environment_prefix() {
        assert!(super::validate_prefix("rustok-build").is_err());
    }

    #[test]
    fn client_rejects_empty_material() {
        let client = MutualTlsClientConfig {
            certificate_pem: Vec::new(),
            private_key_pem: b"key".to_vec(),
            server_ca_pem: b"ca".to_vec(),
            server_domain: "worker.internal".to_string(),
        };
        assert!(client.validate().is_err());
    }

    #[test]
    fn peer_certificate_fingerprint_requires_canonical_lowercase_sha256() {
        assert!(PeerCertificateFingerprint::parse(format!("sha256:{}", "a".repeat(64))).is_ok());
        assert!(PeerCertificateFingerprint::parse(format!("sha256:{}", "A".repeat(64))).is_err());
        assert!(PeerCertificateFingerprint::parse("sha256:abc").is_err());
    }

    #[test]
    fn request_without_a_verified_peer_certificate_is_rejected() {
        let error = peer_certificate_fingerprint(&Request::new(())).expect_err("peer certificate");
        assert_eq!(error.code(), tonic::Code::Unauthenticated);
    }

    #[test]
    fn certificate_fingerprint_is_stable_and_never_uses_raw_certificate_text() {
        let first = PeerCertificateFingerprint::from_leaf_certificate_der(b"certificate-a")
            .expect("first certificate fingerprint");
        let repeated = PeerCertificateFingerprint::from_leaf_certificate_der(b"certificate-a")
            .expect("repeated certificate fingerprint");
        let second = PeerCertificateFingerprint::from_leaf_certificate_der(b"certificate-b")
            .expect("second certificate fingerprint");
        assert_eq!(first, repeated);
        assert_ne!(first, second);
        assert_ne!(first.as_str(), "certificate-a");
    }

    #[tokio::test]
    async fn admission_sheds_after_bounded_wait() {
        let admission = WorkerAdmission::new(1, Duration::from_millis(5)).expect("valid admission");
        let _permit = admission.acquire().await.expect("first permit");
        let error = admission
            .acquire()
            .await
            .expect_err("second request must be shed");
        assert_eq!(error.code(), tonic::Code::ResourceExhausted);
    }

    #[test]
    fn admission_timeout_must_not_exceed_request_timeout() {
        let mut config = config();
        config.admission_timeout = config.request_timeout + Duration::from_millis(1);
        assert!(config.validate().is_err());
    }
}
