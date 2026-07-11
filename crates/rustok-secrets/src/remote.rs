use std::path::PathBuf;

use async_trait::async_trait;
use base64::Engine;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use crate::{SecretError, SecretResolver};

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
    endpoint: String,
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
        Self::with_client(reqwest::Client::new(), endpoint, namespace, kv_mount, auth)
    }

    pub fn with_client(
        client: reqwest::Client,
        endpoint: impl Into<String>,
        namespace: Option<String>,
        kv_mount: impl Into<String>,
        auth: VaultAuth,
    ) -> Result<Self, SecretError> {
        let endpoint = endpoint.into().trim_end_matches('/').to_string();
        let url = reqwest::Url::parse(&endpoint).map_err(vault_error)?;
        if url.scheme() != "https" && !cfg!(test) {
            return Err(SecretError::Resolver {
                resolver: "vault".to_string(),
                message: "Vault endpoint must use HTTPS".to_string(),
            });
        }
        Ok(Self {
            client,
            endpoint,
            namespace,
            kv_mount: kv_mount.into(),
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
                let response: VaultLoginResponse = self
                    .client
                    .post(format!(
                        "{}/v1/auth/{}/login",
                        self.endpoint,
                        auth_mount.trim_matches('/')
                    ))
                    .json(&serde_json::json!({"role": role, "jwt": jwt.trim()}))
                    .send()
                    .await
                    .map_err(vault_error)?
                    .error_for_status()
                    .map_err(vault_error)?
                    .json()
                    .await
                    .map_err(vault_error)?;
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
        let (path, field) = key.split_once('#').unwrap_or((key, "value"));
        let token = self.token().await?;
        let mut request = self
            .client
            .get(format!(
                "{}/v1/{}/data/{}",
                self.endpoint,
                self.kv_mount.trim_matches('/'),
                path.trim_matches('/')
            ))
            .header("X-Vault-Token", token.expose_secret());
        if let Some(namespace) = &self.namespace {
            request = request.header("X-Vault-Namespace", namespace);
        }
        let response: VaultReadResponse = request
            .send()
            .await
            .map_err(vault_error)?
            .error_for_status()
            .map_err(vault_error)?
            .json()
            .await
            .map_err(vault_error)?;
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
        let host = std::env::var("KUBERNETES_SERVICE_HOST").map_err(kubernetes_error)?;
        let port =
            std::env::var("KUBERNETES_SERVICE_PORT_HTTPS").unwrap_or_else(|_| "443".to_string());
        let ca = std::fs::read("/var/run/secrets/kubernetes.io/serviceaccount/ca.crt")
            .map_err(kubernetes_error)?;
        let certificate = reqwest::Certificate::from_pem(&ca).map_err(kubernetes_error)?;
        let client = reqwest::Client::builder()
            .add_root_certificate(certificate)
            .build()
            .map_err(kubernetes_error)?;
        Ok(Self {
            client,
            api_server: format!("https://{host}:{port}"),
            namespace: namespace.into(),
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
        let token = tokio::fs::read_to_string(&self.token_path)
            .await
            .map_err(kubernetes_error)?;
        let response: KubernetesSecretResponse = self
            .client
            .get(format!(
                "{}/api/v1/namespaces/{}/secrets/{}",
                self.api_server, self.namespace, name
            ))
            .bearer_auth(token.trim())
            .send()
            .await
            .map_err(kubernetes_error)?
            .error_for_status()
            .map_err(kubernetes_error)?
            .json()
            .await
            .map_err(kubernetes_error)?;
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
    use std::{net::SocketAddr, path::PathBuf};

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
        let token_path = PathBuf::from(std::env::temp_dir())
            .join(format!("rustok-secrets-kubernetes-test-{}", Uuid::new_v4()));
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
}
