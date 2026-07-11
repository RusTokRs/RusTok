use std::sync::Arc;

use async_trait::async_trait;
use azure_core::credentials::TokenCredential;
use secrecy::SecretString;

use crate::{SecretError, SecretResolver};

#[derive(Clone, Debug)]
pub struct AwsSecretsManagerResolver {
    client: aws_sdk_secretsmanager::Client,
}

impl AwsSecretsManagerResolver {
    pub async fn from_default_chain() -> Self {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        Self {
            client: aws_sdk_secretsmanager::Client::new(&config),
        }
    }

    pub fn from_client(client: aws_sdk_secretsmanager::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl SecretResolver for AwsSecretsManagerResolver {
    async fn resolve(&self, key: &str) -> Result<SecretString, SecretError> {
        let response = self
            .client
            .get_secret_value()
            .secret_id(key)
            .send()
            .await
            .map_err(|error| SecretError::Resolver {
                resolver: "aws_secrets_manager".to_string(),
                message: error.to_string(),
            })?;
        if let Some(value) = response.secret_string() {
            return Ok(SecretString::from(value.to_string()));
        }
        if let Some(value) = response.secret_binary() {
            let value = String::from_utf8(value.as_ref().to_vec()).map_err(|error| {
                SecretError::Resolver {
                    resolver: "aws_secrets_manager".to_string(),
                    message: error.to_string(),
                }
            })?;
            return Ok(SecretString::from(value));
        }
        Err(SecretError::NotFound {
            resolver: "aws_secrets_manager".to_string(),
            key: key.to_string(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct GcpSecretManagerResolver {
    client: google_cloud_secretmanager_v1::client::SecretManagerService,
    project: String,
}

impl GcpSecretManagerResolver {
    pub async fn from_adc(project: impl Into<String>) -> Result<Self, SecretError> {
        let client = google_cloud_secretmanager_v1::client::SecretManagerService::builder()
            .build()
            .await
            .map_err(|error| SecretError::Resolver {
                resolver: "gcp_secret_manager".to_string(),
                message: error.to_string(),
            })?;
        Ok(Self {
            client,
            project: project.into(),
        })
    }

    pub fn from_client(
        project: impl Into<String>,
        client: google_cloud_secretmanager_v1::client::SecretManagerService,
    ) -> Self {
        Self {
            client,
            project: project.into(),
        }
    }
}

#[async_trait]
impl SecretResolver for GcpSecretManagerResolver {
    async fn resolve(&self, key: &str) -> Result<SecretString, SecretError> {
        let name = if key.starts_with("projects/") {
            key.to_string()
        } else {
            format!("projects/{}/secrets/{key}/versions/latest", self.project)
        };
        let response = self
            .client
            .access_secret_version()
            .set_name(name)
            .send()
            .await
            .map_err(|error| SecretError::Resolver {
                resolver: "gcp_secret_manager".to_string(),
                message: error.to_string(),
            })?;
        let payload = response.payload.ok_or_else(|| SecretError::NotFound {
            resolver: "gcp_secret_manager".to_string(),
            key: key.to_string(),
        })?;
        let value =
            String::from_utf8(payload.data.to_vec()).map_err(|error| SecretError::Resolver {
                resolver: "gcp_secret_manager".to_string(),
                message: error.to_string(),
            })?;
        Ok(SecretString::from(value))
    }
}

pub struct AzureKeyVaultResolver {
    client: azure_security_keyvault_secrets::SecretClient,
}

impl std::fmt::Debug for AzureKeyVaultResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzureKeyVaultResolver")
            .finish_non_exhaustive()
    }
}

impl AzureKeyVaultResolver {
    pub fn from_default_credential(endpoint: &str) -> Result<Self, SecretError> {
        let credential: Arc<dyn TokenCredential> =
            if std::env::var_os("AZURE_FEDERATED_TOKEN_FILE").is_some() {
                azure_identity::WorkloadIdentityCredential::new(None).map_err(azure_error)?
            } else if std::env::var_os("IDENTITY_ENDPOINT").is_some()
                || std::env::var_os("MSI_ENDPOINT").is_some()
            {
                azure_identity::ManagedIdentityCredential::new(None).map_err(azure_error)?
            } else {
                azure_identity::DeveloperToolsCredential::new(None).map_err(azure_error)?
            };
        Self::from_credential(endpoint, credential)
    }

    pub fn from_credential(
        endpoint: &str,
        credential: Arc<dyn TokenCredential>,
    ) -> Result<Self, SecretError> {
        let client = azure_security_keyvault_secrets::SecretClient::new(endpoint, credential, None)
            .map_err(azure_error)?;
        Ok(Self { client })
    }
}

#[async_trait]
impl SecretResolver for AzureKeyVaultResolver {
    async fn resolve(&self, key: &str) -> Result<SecretString, SecretError> {
        let response = self
            .client
            .get_secret(key, None)
            .await
            .map_err(azure_error)?;
        let secret = response.into_model().map_err(azure_error)?;
        secret
            .value
            .map(SecretString::from)
            .ok_or_else(|| SecretError::NotFound {
                resolver: "azure_key_vault".to_string(),
                key: key.to_string(),
            })
    }
}

fn azure_error(error: impl std::fmt::Display) -> SecretError {
    SecretError::Resolver {
        resolver: "azure_key_vault".to_string(),
        message: error.to_string(),
    }
}
