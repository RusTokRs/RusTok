use std::path::PathBuf;

use async_trait::async_trait;
use base64::Engine;
use reqwest::Url;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, de::DeserializeOwned};

use crate::{SecretError, SecretResolver};

const MAX_SECRET_RESPONSE_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
pub enum VaultAuth {
    Token(SecretString),
    Kubernetes {
        role: String,
        auth_mount: String,
        service_account_token_path: PathBuf,
    },
}

impl std::fmt::Debug for VaultAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Token(_) => f.write_str("Token(<redacted>)"),
            Self::Kubernetes {
                role, auth_mount, ..
            } => f
                .debug_struct("Kubernetes")
                .field("role", role)
                .field("auth_mount", auth_mount)
                .finish_non_exhaustive(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct VaultResolver {
    client: reqwest::Client,
    endpoint: Url,
    namespace: Option<String>,
    kv_mount: String,
    auth: VaultAuth,
}

impl VaultResolver {
    pub fn new(
        endpoint: impl Into<String>,
        namespace: Option<String>,
        kv_mount: impl Into<String>,
        auth: VaultAuth,
    ) -> Result<Self, SecretError> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(vault_error)?;
        Self::with_client(client, endpoint, namespace, kv_mount, auth)
    }

    pub fn with_client(
        client: reqwest::Client,
        endpoint: impl Into<String>,
        namespace: Option<String>,
        kv_mount: impl Into<String>,
        auth: VaultAuth,
    ) -> Result<Self, SecretError> {
        let endpoint = Url::parse(endpoint.into().trim_end_matches('/')).map_err(vault_error)?;
        let kv_mount = kv_mount.into();
        require_secure_remote_endpoint(&endpoint, "Vault")?;
        validate_optional_header_value(namespace.as_deref(), "Vault namespace")?;
        validate_path(&kv_mount, "Vault KV mount")?;
        if let VaultAuth::Kubernetes {
            role, auth_mount, ..
        } = &auth
        {
            validate_plain_value(role, "Vault Kubernetes role")?;
            validate_path(auth_mount, "Vault auth mount")?;
        }
        Ok(Self {
            client,
            endpoint,
            namespace,
            kv_mount,
            auth,
        })
    }

    async fn token(&self) -> Result<SecretString, SecretError> {
        match &self.auth {
            VaultAuth::Token(token) => Ok(token.clone()),
            VaultAuth::Kubernetes {
                role,
                auth_mount,
                service_account_token_path,
            } => {
                let jwt = tokio::fs::read_to_string(service_account_token_path)
                    .await
                    .map_err(vault_error)?;
                let url = append_url_path(
                    &self.endpoint,
                    &["v1", "auth"],
                    validate_path(auth_mount, "Vault auth mount")?,
                    &["login"],
                )?;
                let response = self
                    .client
                    .post(url)
                    .json(&serde_json::json!({"role": role, "jwt": jwt.trim()}))
                    .send()
                    .await
                    .map_err(vault_error)?
                    .error_for_status()
                    .map_err(vault_error)?;
                let response: VaultLoginResponse = read_bounded_json(response, "vault").await?;
                Ok(SecretString::from(response.auth.client_token))
            }
        }
    }
}

#[derive(Deserialize)]
struct VaultLoginResponse {
    auth: VaultLoginAuth,
}

#[derive(Deserialize)]
struct VaultLoginAuth {
    client_token: String,
}

#[derive(Deserialize)]
struct VaultReadResponse {
    data: VaultReadData,
}

#[derive(Deserialize)]
struct VaultReadData {
    data: serde_json::Map<String, serde_json::Value>,
}

#[async_trait]
impl SecretResolver for VaultResolver {
    async fn resolve(&self, key: &str) -> Result<SecretString, SecretError> {
        let (secret_path, field) = key.split_once('#').unwrap_or((key, "value"));
        let secret_path = validate_path(secret_path, "Vault secret path")?;
        let mount = validate_path(&self.kv_mount, "Vault KV mount")?;
        validate_plain_value(field, "Vault secret field")?;

        let token = self.token().await?;
        let base = append_url_path(&self.endpoint, &["v1"], mount, &["data"])?;
        let url = append_url_path(&base, &[], secret_path, &[])?;
        let mut request = self
            .client
            .get(url)
            .header("X-Vault-Token", token.expose_secret());
        if let Some(namespace) = &self.namespace {
            request = request.header("X-Vault-Namespace", namespace);
        }
        let response = request
            .send()
            .await
            .map_err(vault_error)?
            .error_for_status()
            .map_err(vault_error)?;
        let response: VaultReadResponse = read_bounded_json(response, "vault").await?;
        response
            .data
            .data
            .get(field)
            .and_then(serde_json::Value::as_str)
            .map(|value| SecretString::from(value.to_string()))
            .ok_or_else(|| SecretError::NotFound {
                resolver: "vault".to_string(),
                key: key.to_string(),
            })
    }
}

#[derive(Clone, Debug)]
pub struct KubernetesSecretResolver {
    client: reqwest::Client,
    api_server: String,
    namespace: String,
    token_path: PathBuf,
}

impl KubernetesSecretResolver {
    pub fn in_cluster(namespace: impl Into<String>) -> Result<Self, SecretError> {
        let namespace = namespace.into();
        validate_dns_name(&namespace, "Kubernetes namespace")?;
        let host = std::env::var("KUBERNETES_SERVICE_HOST").map_err(kubernetes_error)?;
        let port =
            std::env::var("KUBERNETES_SERVICE_PORT_HTTPS").unwrap_or_else(|_| "443".to_string());
        let ca = std::fs::read("/var/run/secrets/kubernetes.io/serviceaccount/ca.crt")
            .map_err(kubernetes_error)?;
        let certificate = reqwest::Certificate::from_pem(&ca).map_err(kubernetes_error)?;
        let client = reqwest::Client::builder()
            .add_root_certificate(certificate)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(kubernetes_error)?;
        Ok(Self {
            client,
            api_server: format!("https://{host}:{port}"),
            namespace,
            token_path: PathBuf::from("/var/run/secrets/kubernetes.io/serviceaccount/token"),
        })
    }

    pub fn with_client(
        client: reqwest::Client,
        api_server: impl Into<String>,
        namespace: impl Into<String>,
        token_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            client,
            api_server: api_server.into().trim_end_matches('/').to_string(),
            namespace: namespace.into(),
            token_path: token_path.into(),
        }
    }
}

#[derive(Deserialize)]
struct KubernetesSecretResponse {
    data: std::collections::HashMap<String, String>,
}

#[async_trait]
impl SecretResolver for KubernetesSecretResolver {
    async fn resolve(&self, key: &str) -> Result<SecretString, SecretError> {
        let (name, field) = key.split_once('#').unwrap_or((key, "value"));
        validate_dns_name(&self.namespace, "Kubernetes namespace")?;
        validate_dns_name(name, "Kubernetes secret name")?;
        validate_plain_value(field, "Kubernetes secret field")?;

        let api_server = Url::parse(&self.api_server).map_err(kubernetes_error)?;
        require_secure_remote_endpoint(&api_server, "Kubernetes API")?;
        let url = append_url_path(
            &api_server,
            &["api", "v1", "namespaces", &self.namespace, "secrets", name],
            Vec::new(),
            &[],
        )?;
        let token = tokio::fs::read_to_string(&self.token_path)
            .await
            .map_err(kubernetes_error)?;
        let response = self
            .client
            .get(url)
            .bearer_auth(token.trim())
            .send()
            .await
            .map_err(kubernetes_error)?
            .error_for_status()
            .map_err(kubernetes_error)?;
        let response: KubernetesSecretResponse = read_bounded_json(response, "kubernetes").await?;
        let encoded = response
            .data
            .get(field)
            .ok_or_else(|| SecretError::NotFound {
                resolver: "kubernetes".to_string(),
                key: key.to_string(),
            })?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(kubernetes_error)?;
        let value = String::from_utf8(bytes).map_err(kubernetes_error)?;
        Ok(SecretString::from(value))
    }
}

fn validate_path<'a>(value: &'a str, label: &str) -> Result<Vec<&'a str>, SecretError> {
    let value = value.trim_matches('/');
    if value.is_empty() {
        return Err(remote_policy_error(label, "must not be empty"));
    }
    let segments = value.split('/').collect::<Vec<_>>();
    if segments.iter().any(|segment| {
        segment.is_empty()
            || matches!(*segment, "." | "..")
            || segment.chars().any(|character| {
                matches!(character, '\\' | '?' | '#' | '%') || character.is_control()
            })
    }) {
        return Err(remote_policy_error(
            label,
            "contains an unsafe URL path segment",
        ));
    }
    Ok(segments)
}

fn validate_plain_value(value: &str, label: &str) -> Result<(), SecretError> {
    if value.trim().is_empty()
        || value.chars().any(|character| {
            matches!(character, '/' | '\\' | '#' | '%' | '\r' | '\n') || character.is_control()
        })
    {
        return Err(remote_policy_error(
            label,
            "contains unsupported characters",
        ));
    }
    Ok(())
}

fn validate_optional_header_value(value: Option<&str>, label: &str) -> Result<(), SecretError> {
    if value.is_some_and(|value| value.chars().any(char::is_control)) {
        return Err(remote_policy_error(label, "contains control characters"));
    }
    Ok(())
}

fn validate_dns_name(value: &str, label: &str) -> Result<(), SecretError> {
    let value = value.trim();
    let valid = !value.is_empty()
        && value.len() <= 253
        && value.split('.').all(|part| {
            !part.is_empty()
                && part.len() <= 63
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part.chars().all(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
                })
        });
    if valid {
        Ok(())
    } else {
        Err(remote_policy_error(label, "must be a DNS-1123 name"))
    }
}

fn require_secure_remote_endpoint(url: &Url, label: &str) -> Result<(), SecretError> {
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(remote_policy_error(
            label,
            "endpoint must not contain userinfo, query, or fragment",
        ));
    }
    if url.scheme() != "https" && !cfg!(test) {
        return Err(remote_policy_error(label, "endpoint must use HTTPS"));
    }
    Ok(())
}

fn append_url_path(
    base: &Url,
    prefix: &[&str],
    dynamic: Vec<&str>,
    suffix: &[&str],
) -> Result<Url, SecretError> {
    let mut url = base.clone();
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|_| remote_policy_error("remote secret endpoint", "cannot be a base URL"))?;
        path.pop_if_empty();
        for segment in prefix.iter().chain(dynamic.iter()).chain(suffix.iter()) {
            path.push(segment);
        }
    }
    Ok(url)
}

async fn read_bounded_json<T: DeserializeOwned>(
    response: reqwest::Response,
    resolver: &str,
) -> Result<T, SecretError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_SECRET_RESPONSE_BYTES as u64)
    {
        return Err(SecretError::Resolver {
            resolver: resolver.to_string(),
            message: "remote secret response exceeds size limit".to_string(),
        });
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| SecretError::Resolver {
            resolver: resolver.to_string(),
            message: error.to_string(),
        })?;
    if bytes.len() > MAX_SECRET_RESPONSE_BYTES {
        return Err(SecretError::Resolver {
            resolver: resolver.to_string(),
            message: "remote secret response exceeds size limit".to_string(),
        });
    }
    serde_json::from_slice(&bytes).map_err(|error| SecretError::Resolver {
        resolver: resolver.to_string(),
        message: error.to_string(),
    })
}

fn remote_policy_error(label: &str, message: &str) -> SecretError {
    SecretError::Resolver {
        resolver: "remote".to_string(),
        message: format!("{label} {message}"),
    }
}

fn vault_error(error: impl std::fmt::Display) -> SecretError {
    SecretError::Resolver {
        resolver: "vault".to_string(),
        message: error.to_string(),
    }
}

fn kubernetes_error(error: impl std::fmt::Display) -> SecretError {
    SecretError::Resolver {
        resolver: "kubernetes".to_string(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use secrecy::{ExposeSecret, SecretString};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    use uuid::Uuid;

    use super::{KubernetesSecretResolver, VaultAuth, VaultResolver};
    use crate::SecretResolver;

    async fn mock_server(response: &'static str) -> (SocketAddr, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 4096];
            let length = socket.read(&mut request).await.unwrap();
            socket.write_all(response.as_bytes()).await.unwrap();
            String::from_utf8(request[..length].to_vec()).unwrap()
        });
        (address, task)
    }

    #[tokio::test]
    async fn vault_resolver_uses_configured_mount_and_token() {
        let response = "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 42\r\nconnection: close\r\n\r\n{\"data\":{\"data\":{\"token\":\"vault-secret\"}}}";
        let (address, request) = mock_server(response).await;
        let resolver = VaultResolver::with_client(
            reqwest::Client::builder().no_proxy().build().unwrap(),
            format!("http://{address}"),
            Some("operator".to_string()),
            "kv",
            VaultAuth::Token(SecretString::from("server-token")),
        )
        .unwrap();

        let value = resolver.resolve("ai/openai#token").await.unwrap();
        assert_eq!(value.expose_secret(), "vault-secret");
        let request = request.await.unwrap();
        assert!(request.starts_with("GET /v1/kv/data/ai/openai "));
        assert!(request.contains("x-vault-token: server-token"));
        assert!(request.contains("x-vault-namespace: operator"));
    }

    #[tokio::test]
    async fn kubernetes_resolver_uses_service_account_token_and_namespace() {
        let response = "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 37\r\nconnection: close\r\n\r\n{\"data\":{\"token\":\"azhzLXNlY3JldA==\"}}";
        let (address, request) = mock_server(response).await;
        let token_path =
            std::env::temp_dir().join(format!("rustok-secrets-kubernetes-test-{}", Uuid::new_v4()));
        std::fs::write(&token_path, "workload-token\n").unwrap();
        let resolver = KubernetesSecretResolver::with_client(
            reqwest::Client::builder().no_proxy().build().unwrap(),
            format!("http://{address}"),
            "operator",
            &token_path,
        );

        let value = resolver.resolve("ai#token").await.unwrap();
        assert_eq!(value.expose_secret(), "k8s-secret");
        std::fs::remove_file(token_path).unwrap();
        let request = request.await.unwrap();
        assert!(request.starts_with("GET /api/v1/namespaces/operator/secrets/ai "));
        assert!(request.contains("authorization: Bearer workload-token"));
    }

    #[test]
    fn kubernetes_resolver_rejects_invalid_namespace_before_cluster_discovery() {
        assert!(KubernetesSecretResolver::in_cluster("invalid namespace").is_err());
    }

    #[tokio::test]
    async fn remote_resolvers_reject_path_traversal_before_network_io() {
        let vault = VaultResolver::with_client(
            reqwest::Client::builder().no_proxy().build().unwrap(),
            "http://127.0.0.1:9",
            None,
            "kv",
            VaultAuth::Token(SecretString::from("server-token")),
        )
        .unwrap();
        assert!(
            vault
                .resolve("tenants/allowed/../../sys#value")
                .await
                .is_err()
        );
        assert!(vault.resolve("tenants/%2e%2e/sys#value").await.is_err());

        let kubernetes = KubernetesSecretResolver::with_client(
            reqwest::Client::builder().no_proxy().build().unwrap(),
            "http://127.0.0.1:9",
            "operator",
            "/missing/token",
        );
        assert!(kubernetes.resolve("../pods#token").await.is_err());
    }
}
