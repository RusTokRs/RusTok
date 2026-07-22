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
        validate_aws_secret_id(key)?;
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
    /// Validates a deployment-owned GCP project identifier without creating an
    /// ADC client. Runtime composition uses this to fail closed at startup.
    pub fn validate_project(project: &str) -> Result<(), SecretError> {
        validate_gcp_project(project)
    }

    pub async fn from_adc(project: impl Into<String>) -> Result<Self, SecretError> {
        let project = project.into();
        Self::validate_project(&project)?;
        let client = google_cloud_secretmanager_v1::client::SecretManagerService::builder()
            .build()
            .await
            .map_err(|error| SecretError::Resolver {
                resolver: "gcp_secret_manager".to_string(),
                message: error.to_string(),
            })?;
        Ok(Self { client, project })
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
        Self::validate_project(&self.project)?;
        validate_gcp_secret_id(key)?;
        let name = format!("projects/{}/secrets/{key}/versions/latest", self.project);
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
        Self::validate_endpoint(endpoint)?;
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
        Self::validate_endpoint(endpoint)?;
        let endpoint = reqwest::Url::parse(endpoint).map_err(azure_error)?;
        let client =
            azure_security_keyvault_secrets::SecretClient::new(endpoint.as_str(), credential, None)
                .map_err(azure_error)?;
        Ok(Self { client })
    }

    /// Validates a deployment-owned Key Vault endpoint before credential discovery.
    pub fn validate_endpoint(endpoint: &str) -> Result<(), SecretError> {
        let endpoint = reqwest::Url::parse(endpoint).map_err(azure_error)?;
        if endpoint.scheme() != "https"
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
        {
            return Err(provider_policy_error(
                "azure_key_vault",
                "Azure Key Vault endpoint must be a plain HTTPS URL",
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl SecretResolver for AzureKeyVaultResolver {
    async fn resolve(&self, key: &str) -> Result<SecretString, SecretError> {
        validate_azure_secret_name(key)?;
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

fn validate_aws_secret_id(value: &str) -> Result<(), SecretError> {
    let valid =
        !value.trim().is_empty() && value.len() <= 2048 && !value.chars().any(char::is_control);
    if valid {
        Ok(())
    } else {
        Err(provider_policy_error(
            "aws_secrets_manager",
            "AWS secret id is invalid",
        ))
    }
}

fn validate_gcp_project(value: &str) -> Result<(), SecretError> {
    let bytes = value.as_bytes();
    let valid = (6..=30).contains(&bytes.len())
        && bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes
            .last()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-');
    if valid {
        Ok(())
    } else {
        Err(provider_policy_error(
            "gcp_secret_manager",
            "configured GCP project id is invalid",
        ))
    }
}

fn validate_gcp_secret_id(value: &str) -> Result<(), SecretError> {
    let valid = !value.is_empty()
        && value.len() <= 255
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    if valid {
        Ok(())
    } else {
        Err(provider_policy_error(
            "gcp_secret_manager",
            "GCP secret key must be a secret id, not a fully-qualified resource name",
        ))
    }
}

fn validate_azure_secret_name(value: &str) -> Result<(), SecretError> {
    let valid = !value.is_empty()
        && value.len() <= 127
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    if valid {
        Ok(())
    } else {
        Err(provider_policy_error(
            "azure_key_vault",
            "Azure secret name contains unsupported characters",
        ))
    }
}

fn provider_policy_error(resolver: &str, message: &str) -> SecretError {
    SecretError::Resolver {
        resolver: resolver.to_string(),
        message: message.to_string(),
    }
}

fn azure_error(error: impl std::fmt::Display) -> SecretError {
    SecretError::Resolver {
        resolver: "azure_key_vault".to_string(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AzureKeyVaultResolver, validate_azure_secret_name, validate_gcp_project,
        validate_gcp_secret_id,
    };

    #[test]
    fn gcp_resolver_rejects_cross_project_resource_names() {
        assert!(validate_gcp_project("rustok-prod1").is_ok());
        assert!(validate_gcp_secret_id("openai-api-key").is_ok());
        assert!(
            validate_gcp_secret_id("projects/other-project/secrets/key/versions/latest").is_err()
        );
    }

    #[test]
    fn azure_resolver_accepts_only_vault_secret_names() {
        assert!(validate_azure_secret_name("openai-api-key").is_ok());
        assert!(validate_azure_secret_name("../certificates/admin").is_err());
    }

    #[test]
    fn azure_resolver_rejects_unsafe_endpoint_before_credential_discovery() {
        assert!(AzureKeyVaultResolver::validate_endpoint("https://rustok.vault.azure.net").is_ok());
        assert!(AzureKeyVaultResolver::validate_endpoint("http://rustok.vault.azure.net").is_err());
        assert!(
            AzureKeyVaultResolver::validate_endpoint("https://user@rustok.vault.azure.net")
                .is_err()
        );
        assert!(
            AzureKeyVaultResolver::validate_endpoint(
                "https://rustok.vault.azure.net?token=forbidden"
            )
            .is_err()
        );
    }
}
