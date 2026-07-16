//! Shared mTLS listener configuration for isolated RusToK worker processes.

use std::{fs, net::SocketAddr, path::PathBuf, time::Duration};

use tonic::transport::{Certificate, ClientTlsConfig, Identity, Server, ServerTlsConfig};

/// Deployment-owned mutually authenticated listener configuration. A worker
/// supplies its uppercase environment-variable prefix, such as
/// `RUSTOK_VERIFICATION` or `RUSTOK_MODULE_BUILD`.
pub struct MutualTlsListenerConfig {
    pub address: SocketAddr,
    pub certificate_pem: Vec<u8>,
    pub private_key_pem: Vec<u8>,
    pub client_ca_pem: Vec<u8>,
    pub request_timeout: Duration,
    pub concurrency_limit: usize,
    pub max_message_size: usize,
}

impl MutualTlsListenerConfig {
    const DEFAULT_TIMEOUT_MS: u64 = 30_000;
    const DEFAULT_CONCURRENCY_LIMIT: usize = 16;
    const DEFAULT_MAX_MESSAGE_SIZE: usize = 128 * 1024;
    pub const MAX_MESSAGE_SIZE: usize = 1024 * 1024;

    pub fn from_env_prefix(prefix: &str) -> Result<Self, String> {
        validate_prefix(prefix)?;
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
        let concurrency_limit = parse_env(
            &env_name(prefix, "CONCURRENCY_LIMIT"),
            Self::DEFAULT_CONCURRENCY_LIMIT,
        )?;
        let max_message_size = parse_env(
            &env_name(prefix, "MAX_MESSAGE_SIZE"),
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
            return Err("worker listener TLS material must not be empty".to_string());
        }
        if self.request_timeout.is_zero() {
            return Err("worker listener timeout must be positive".to_string());
        }
        if self.concurrency_limit == 0 {
            return Err("worker listener concurrency limit must be positive".to_string());
        }
        if self.max_message_size == 0 || self.max_message_size > Self::MAX_MESSAGE_SIZE {
            return Err(format!(
                "worker listener max message size must be between 1 and {} bytes",
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

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, time::Duration};

    use super::{MutualTlsClientConfig, MutualTlsListenerConfig};

    fn config() -> MutualTlsListenerConfig {
        MutualTlsListenerConfig {
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
        config.max_message_size = MutualTlsListenerConfig::MAX_MESSAGE_SIZE + 1;
        assert!(config.validate().is_err());
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
}
