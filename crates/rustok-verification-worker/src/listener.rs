use std::{fs, net::SocketAddr, path::PathBuf, time::Duration};

use tonic::transport::{Certificate, Identity, Server, ServerTlsConfig};

/// Deployment-owned listener boundary. The verification worker never binds an
/// unauthenticated plaintext endpoint: both its identity and its client CA are
/// required at startup.
pub struct ListenerConfig {
    pub address: SocketAddr,
    pub certificate_pem: Vec<u8>,
    pub private_key_pem: Vec<u8>,
    pub client_ca_pem: Vec<u8>,
    pub request_timeout: Duration,
    pub concurrency_limit: usize,
    pub max_message_size: usize,
}

impl ListenerConfig {
    const DEFAULT_TIMEOUT_MS: u64 = 30_000;
    const DEFAULT_CONCURRENCY_LIMIT: usize = 16;
    const DEFAULT_MAX_MESSAGE_SIZE: usize = 128 * 1024;
    const MAX_MESSAGE_SIZE: usize = 1024 * 1024;

    pub fn from_env() -> Result<Self, String> {
        let address = required_env("RUSTOK_VERIFICATION_LISTEN_ADDR")?
            .parse()
            .map_err(|error| format!("RUSTOK_VERIFICATION_LISTEN_ADDR is invalid: {error}"))?;
        let certificate_pem = read_required_file("RUSTOK_VERIFICATION_SERVER_CERT_PEM")?;
        let private_key_pem = read_required_file("RUSTOK_VERIFICATION_SERVER_KEY_PEM")?;
        let client_ca_pem = read_required_file("RUSTOK_VERIFICATION_CLIENT_CA_PEM")?;
        let request_timeout = Duration::from_millis(parse_env(
            "RUSTOK_VERIFICATION_REQUEST_TIMEOUT_MS",
            Self::DEFAULT_TIMEOUT_MS,
        )?);
        let concurrency_limit = parse_env(
            "RUSTOK_VERIFICATION_CONCURRENCY_LIMIT",
            Self::DEFAULT_CONCURRENCY_LIMIT,
        )?;
        let max_message_size = parse_env(
            "RUSTOK_VERIFICATION_MAX_MESSAGE_SIZE",
            Self::DEFAULT_MAX_MESSAGE_SIZE,
        )?;
        let config = Self {
            address,
            certificate_pem,
            private_key_pem,
            client_ca_pem,
            request_timeout,
            concurrency_limit,
            max_message_size,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.certificate_pem.is_empty()
            || self.private_key_pem.is_empty()
            || self.client_ca_pem.is_empty()
        {
            return Err("verification listener TLS material must not be empty".to_string());
        }
        if self.request_timeout.is_zero() {
            return Err("verification listener timeout must be positive".to_string());
        }
        if self.concurrency_limit == 0 {
            return Err("verification listener concurrency limit must be positive".to_string());
        }
        if self.max_message_size == 0 || self.max_message_size > Self::MAX_MESSAGE_SIZE {
            return Err(format!(
                "verification listener max message size must be between 1 and {} bytes",
                Self::MAX_MESSAGE_SIZE
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

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, time::Duration};

    use super::ListenerConfig;

    fn config() -> ListenerConfig {
        ListenerConfig {
            address: "127.0.0.1:9443".parse::<SocketAddr>().expect("address"),
            certificate_pem: b"certificate".to_vec(),
            private_key_pem: b"key".to_vec(),
            client_ca_pem: b"ca".to_vec(),
            request_timeout: Duration::from_secs(1),
            concurrency_limit: 1,
            max_message_size: 1024,
        }
    }

    #[test]
    fn listener_rejects_unbounded_message_size() {
        let mut config = config();
        config.max_message_size = ListenerConfig::MAX_MESSAGE_SIZE + 1;
        assert!(config.validate().is_err());
    }

    #[test]
    fn listener_rejects_empty_client_ca() {
        let mut config = config();
        config.client_ca_pem.clear();
        assert!(config.validate().is_err());
    }
}
