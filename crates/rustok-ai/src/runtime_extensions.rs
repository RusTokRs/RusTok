//! Deployment-owned runtime values published through the generic module
//! extension registry. Tenant profiles never influence this construction.

use std::{collections::BTreeSet, path::PathBuf, sync::Arc, time::Duration};

use rustok_secrets::{
    AwsSecretsManagerResolver, AzureKeyVaultResolver, EnvResolver, GcpSecretManagerResolver,
    KubernetesSecretResolver, MountedFileResolver, SecretAccessPolicy, SecretError, SecretResolver,
    SecretResolverRegistry, VaultAuth, VaultResolver,
};
use secrecy::SecretString;
use serde::Deserialize;
use tokio::sync::OnceCell;

use crate::{
    AiProviderTargetCatalog, ProviderEgressPolicy, SharedAiEgressPolicy,
    SharedAiProviderTargetCatalog, SharedAiSecretResolverRegistry,
};

pub(crate) struct AiDeploymentRuntime {
    pub secret_registry: SharedAiSecretResolverRegistry,
    pub egress_policy: SharedAiEgressPolicy,
    pub provider_targets: SharedAiProviderTargetCatalog,
}

impl AiDeploymentRuntime {
    pub fn from_environment() -> Result<Self, String> {
        let egress_policy = ProviderEgressPolicy {
            allowed_origins: json_string_list("RUSTOK_AI_EGRESS_ALLOWED_ORIGINS_JSON")?,
            allow_local_origins: environment_bool("RUSTOK_AI_EGRESS_ALLOW_LOCAL_ORIGINS")?,
        };
        let provider_targets =
            AiProviderTargetCatalog::from_environment_with_egress_policy(&egress_policy)?;
        let secrets = secret_registry_from_environment()?;

        Ok(Self {
            secret_registry: SharedAiSecretResolverRegistry(secrets),
            egress_policy: SharedAiEgressPolicy(egress_policy),
            provider_targets: SharedAiProviderTargetCatalog(provider_targets),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DeploymentSecretResolverConfig {
    Env {
        alias: String,
        key_prefixes: Vec<String>,
    },
    MountedFile {
        alias: String,
        root: PathBuf,
        key_prefixes: Vec<String>,
    },
    Vault {
        alias: String,
        endpoint: String,
        namespace: Option<String>,
        kv_mount: String,
        key_prefixes: Vec<String>,
        token_env: Option<String>,
        token_file: Option<PathBuf>,
        kubernetes_role: Option<String>,
        kubernetes_auth_mount: Option<String>,
        kubernetes_token_path: Option<PathBuf>,
    },
    Kubernetes {
        alias: String,
        namespace: String,
        key_prefixes: Vec<String>,
    },
    AwsSecretsManager {
        alias: String,
        key_prefixes: Vec<String>,
    },
    GcpSecretManager {
        alias: String,
        project: String,
        key_prefixes: Vec<String>,
    },
    AzureKeyVault {
        alias: String,
        endpoint: String,
        key_prefixes: Vec<String>,
    },
}

impl DeploymentSecretResolverConfig {
    fn alias(&self) -> &str {
        match self {
            Self::Env { alias, .. }
            | Self::MountedFile { alias, .. }
            | Self::Vault { alias, .. }
            | Self::Kubernetes { alias, .. }
            | Self::AwsSecretsManager { alias, .. }
            | Self::GcpSecretManager { alias, .. }
            | Self::AzureKeyVault { alias, .. } => alias,
        }
    }
}

fn secret_registry_from_environment() -> Result<SecretResolverRegistry, String> {
    let mut builder = SecretResolverRegistry::builder().cache_ttl(Duration::from_secs(60));
    let configs = match std::env::var_os("RUSTOK_AI_SECRET_RESOLVERS_JSON") {
        Some(raw) => {
            serde_json::from_str::<Vec<DeploymentSecretResolverConfig>>(&raw.to_string_lossy())
                .map_err(|error| format!("invalid RUSTOK_AI_SECRET_RESOLVERS_JSON: {error}"))?
        }
        None => legacy_secret_resolver_configs()?,
    };
    validate_config_aliases(&configs)?;
    for config in configs {
        builder = register_deployment_resolver(builder, config)?;
    }
    Ok(builder.build())
}

fn validate_config_aliases(configs: &[DeploymentSecretResolverConfig]) -> Result<(), String> {
    let mut aliases = BTreeSet::new();
    for config in configs {
        let alias = config.alias().trim();
        if alias.is_empty() || !aliases.insert(alias.to_string()) {
            return Err("secret resolver aliases must be unique and non-empty".to_string());
        }
    }
    Ok(())
}

fn legacy_secret_resolver_configs() -> Result<Vec<DeploymentSecretResolverConfig>, String> {
    let mut configs = vec![DeploymentSecretResolverConfig::Env {
        alias: "env".to_string(),
        key_prefixes: secret_prefixes()?,
    }];
    if let Some(root) = std::env::var_os("RUSTOK_AI_SECRET_MOUNT_ROOT") {
        configs.push(DeploymentSecretResolverConfig::MountedFile {
            alias: "mounted_file".to_string(),
            root: PathBuf::from(root),
            key_prefixes: vec!["ai/".to_string()],
        });
    }
    Ok(configs)
}

fn register_deployment_resolver(
    builder: rustok_secrets::SecretResolverRegistryBuilder,
    config: DeploymentSecretResolverConfig,
) -> Result<rustok_secrets::SecretResolverRegistryBuilder, String> {
    match config {
        DeploymentSecretResolverConfig::Env {
            alias,
            key_prefixes,
        } => Ok(builder.resolver(alias, EnvResolver, policy(key_prefixes)?)),
        DeploymentSecretResolverConfig::MountedFile {
            alias,
            root,
            key_prefixes,
        } => Ok(builder.resolver(alias, MountedFileResolver::new(root), policy(key_prefixes)?)),
        DeploymentSecretResolverConfig::Vault {
            alias,
            endpoint,
            namespace,
            kv_mount,
            key_prefixes,
            token_env,
            token_file,
            kubernetes_role,
            kubernetes_auth_mount,
            kubernetes_token_path,
        } => {
            let auth = vault_auth(
                token_env,
                token_file,
                kubernetes_role,
                kubernetes_auth_mount,
                kubernetes_token_path,
            )?;
            let resolver = VaultResolver::new(endpoint, namespace, kv_mount, auth)
                .map_err(|error| error.to_string())?;
            Ok(builder.resolver(alias, resolver, policy(key_prefixes)?))
        }
        DeploymentSecretResolverConfig::Kubernetes {
            alias,
            namespace,
            key_prefixes,
        } => {
            let resolver = KubernetesSecretResolver::in_cluster(namespace)
                .map_err(|error| error.to_string())?;
            Ok(builder.resolver(alias, resolver, policy(key_prefixes)?))
        }
        DeploymentSecretResolverConfig::AwsSecretsManager {
            alias,
            key_prefixes,
        } => Ok(builder.resolver(alias, LazyAwsResolver::default(), policy(key_prefixes)?)),
        DeploymentSecretResolverConfig::GcpSecretManager {
            alias,
            project,
            key_prefixes,
        } => {
            GcpSecretManagerResolver::validate_project(&project)
                .map_err(|error| error.to_string())?;
            Ok(builder.resolver(alias, LazyGcpResolver::new(project), policy(key_prefixes)?))
        }
        DeploymentSecretResolverConfig::AzureKeyVault {
            alias,
            endpoint,
            key_prefixes,
        } => {
            let resolver = AzureKeyVaultResolver::from_default_credential(&endpoint)
                .map_err(|error| error.to_string())?;
            Ok(builder.resolver(alias, resolver, policy(key_prefixes)?))
        }
    }
}

fn policy(prefixes: Vec<String>) -> Result<SecretAccessPolicy, String> {
    if prefixes.is_empty() || prefixes.iter().any(|prefix| prefix.trim().is_empty()) {
        return Err(
            "secret resolver key_prefixes must contain non-empty deployment-owned prefixes"
                .to_string(),
        );
    }
    Ok(SecretAccessPolicy::Prefix(prefixes))
}

fn vault_auth(
    token_env: Option<String>,
    token_file: Option<PathBuf>,
    kubernetes_role: Option<String>,
    kubernetes_auth_mount: Option<String>,
    kubernetes_token_path: Option<PathBuf>,
) -> Result<VaultAuth, String> {
    let token = match (token_env, token_file) {
        (Some(name), None) => {
            Some(std::env::var(name).map(SecretString::from).map_err(|_| {
                "configured Vault token environment variable is missing".to_string()
            })?)
        }
        (None, Some(path)) => Some(SecretString::from(
            std::fs::read_to_string(path)
                .map_err(|error| format!("configured Vault token file cannot be read: {error}"))?
                .trim()
                .to_string(),
        )),
        (None, None) => None,
        _ => {
            return Err(
                "Vault configuration accepts exactly one token_env or token_file".to_string(),
            );
        }
    };
    if let Some(token) = token {
        if kubernetes_role.is_some()
            || kubernetes_auth_mount.is_some()
            || kubernetes_token_path.is_some()
        {
            return Err(
                "Vault token auth cannot be combined with Kubernetes auth fields".to_string(),
            );
        }
        return Ok(VaultAuth::Token(token));
    }
    Ok(VaultAuth::Kubernetes {
        role: kubernetes_role
            .ok_or_else(|| "Vault Kubernetes auth requires kubernetes_role".to_string())?,
        auth_mount: kubernetes_auth_mount
            .ok_or_else(|| "Vault Kubernetes auth requires kubernetes_auth_mount".to_string())?,
        service_account_token_path: kubernetes_token_path
            .ok_or_else(|| "Vault Kubernetes auth requires kubernetes_token_path".to_string())?,
    })
}

#[derive(Clone, Default)]
struct LazyAwsResolver(Arc<OnceCell<AwsSecretsManagerResolver>>);

#[async_trait::async_trait]
impl SecretResolver for LazyAwsResolver {
    async fn resolve(&self, key: &str) -> Result<SecretString, SecretError> {
        self.0
            .get_or_init(AwsSecretsManagerResolver::from_default_chain)
            .await
            .resolve(key)
            .await
    }
}

#[derive(Clone)]
struct LazyGcpResolver {
    project: String,
    resolver: Arc<OnceCell<GcpSecretManagerResolver>>,
}
impl LazyGcpResolver {
    fn new(project: String) -> Self {
        Self {
            project,
            resolver: Arc::new(OnceCell::new()),
        }
    }
}

#[async_trait::async_trait]
impl SecretResolver for LazyGcpResolver {
    async fn resolve(&self, key: &str) -> Result<SecretString, SecretError> {
        let project = self.project.clone();
        self.resolver
            .get_or_try_init(|| async move { GcpSecretManagerResolver::from_adc(project).await })
            .await?
            .resolve(key)
            .await
    }
}

fn secret_prefixes() -> Result<Vec<String>, String> {
    let prefixes = json_string_list("RUSTOK_AI_SECRET_ENV_PREFIXES_JSON")?;
    if prefixes.is_empty() {
        Ok(vec!["RUSTOK_AI_".to_string()])
    } else {
        Ok(prefixes)
    }
}

fn json_string_list(name: &str) -> Result<Vec<String>, String> {
    let Some(raw) = std::env::var_os(name) else {
        return Ok(Vec::new());
    };
    serde_json::from_str::<Vec<String>>(&raw.to_string_lossy())
        .map_err(|error| format!("invalid {name}: {error}"))
}

fn environment_bool(name: &str) -> Result<bool, String> {
    let Some(raw) = std::env::var_os(name) else {
        return Ok(false);
    };
    raw.to_string_lossy()
        .parse::<bool>()
        .map_err(|error| format!("invalid {name}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{
        DeploymentSecretResolverConfig, environment_bool, policy, register_deployment_resolver,
        secret_prefixes, secret_registry_from_environment, validate_config_aliases, vault_auth,
    };

    static DEPLOYMENT_RESOLVER_CONFIG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct ScopedEnvironmentVariable {
        name: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    impl ScopedEnvironmentVariable {
        fn set(name: &'static str, value: &str) -> Self {
            let previous = std::env::var_os(name);
            unsafe { std::env::set_var(name, value) };
            Self { name, previous }
        }
    }

    impl Drop for ScopedEnvironmentVariable {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => unsafe { std::env::set_var(self.name, value) },
                None => unsafe { std::env::remove_var(self.name) },
            }
        }
    }

    #[test]
    fn defaults_to_ai_scoped_environment_secret_prefix() {
        unsafe { std::env::remove_var("RUSTOK_AI_SECRET_ENV_PREFIXES_JSON") };
        assert_eq!(secret_prefixes().unwrap(), vec!["RUSTOK_AI_"]);
    }

    #[test]
    fn invalid_boolean_is_rejected() {
        unsafe { std::env::set_var("RUSTOK_AI_EGRESS_ALLOW_LOCAL_ORIGINS", "not-a-bool") };
        assert!(environment_bool("RUSTOK_AI_EGRESS_ALLOW_LOCAL_ORIGINS").is_err());
        unsafe { std::env::remove_var("RUSTOK_AI_EGRESS_ALLOW_LOCAL_ORIGINS") };
    }

    #[test]
    fn resolver_policy_requires_a_deployment_owned_prefix() {
        assert!(policy(Vec::new()).is_err());
        assert!(policy(vec![" ".to_string()]).is_err());
    }

    #[test]
    fn vault_auth_rejects_ambiguous_token_and_kubernetes_configuration() {
        unsafe { std::env::set_var("RUSTOK_AI_TEST_VAULT_TOKEN", "test-token") };
        assert!(
            vault_auth(
                Some("RUSTOK_AI_TEST_VAULT_TOKEN".to_string()),
                None,
                Some("role".to_string()),
                Some("kubernetes".to_string()),
                Some(std::path::PathBuf::from("/token")),
            )
            .is_err()
        );
        unsafe { std::env::remove_var("RUSTOK_AI_TEST_VAULT_TOKEN") };
    }

    #[test]
    fn resolver_config_exposes_its_deployment_alias() {
        let config = DeploymentSecretResolverConfig::Env {
            alias: "production_env".to_string(),
            key_prefixes: vec!["RUSTOK_AI_".to_string()],
        };
        assert_eq!(config.alias(), "production_env");
    }

    #[test]
    fn resolver_configs_reject_duplicate_deployment_aliases() {
        let configs = vec![
            DeploymentSecretResolverConfig::Env {
                alias: "shared".to_string(),
                key_prefixes: vec!["RUSTOK_AI_".to_string()],
            },
            DeploymentSecretResolverConfig::AwsSecretsManager {
                alias: "shared".to_string(),
                key_prefixes: vec!["ai/".to_string()],
            },
        ];
        assert!(validate_config_aliases(&configs).is_err());
    }

    #[test]
    fn explicit_resolver_json_registers_deployment_aliases_without_legacy_fallback() {
        let _lock = DEPLOYMENT_RESOLVER_CONFIG_LOCK
            .lock()
            .expect("deployment resolver configuration lock must not be poisoned");
        let _config = ScopedEnvironmentVariable::set(
            "RUSTOK_AI_SECRET_RESOLVERS_JSON",
            r#"[
                {"kind":"env","alias":"deployment_env","key_prefixes":["RUSTOK_AI_"]},
                {"kind":"mounted_file","alias":"deployment_file","root":"/var/run/secrets/rustok","key_prefixes":["ai/"]},
                {"kind":"vault","alias":"deployment_vault","endpoint":"https://vault.example.test","kv_mount":"secret","key_prefixes":["ai/"],"token_env":"RUSTOK_AI_TEST_DEPLOYMENT_VAULT_TOKEN"},
                {"kind":"aws_secrets_manager","alias":"deployment_aws","key_prefixes":["ai/"]},
                {"kind":"gcp_secret_manager","alias":"deployment_gcp","project":"rustok-prod1","key_prefixes":["ai/"]}
            ]"#,
        );
        let _vault_token =
            ScopedEnvironmentVariable::set("RUSTOK_AI_TEST_DEPLOYMENT_VAULT_TOKEN", "test-token");

        let registry = secret_registry_from_environment()
            .expect("explicit deployment resolver configuration must register");
        for alias in [
            "deployment_env",
            "deployment_file",
            "deployment_vault",
            "deployment_aws",
            "deployment_gcp",
        ] {
            assert!(registry.contains(alias), "missing configured alias {alias}");
        }
        assert!(
            !registry.contains("env"),
            "explicit deployment configuration must not add the legacy env resolver"
        );
    }

    #[test]
    fn explicit_resolver_json_fails_closed_for_duplicate_aliases() {
        let _lock = DEPLOYMENT_RESOLVER_CONFIG_LOCK
            .lock()
            .expect("deployment resolver configuration lock must not be poisoned");
        let _config = ScopedEnvironmentVariable::set(
            "RUSTOK_AI_SECRET_RESOLVERS_JSON",
            r#"[
                {"kind":"env","alias":"duplicate","key_prefixes":["RUSTOK_AI_"]},
                {"kind":"aws_secrets_manager","alias":"duplicate","key_prefixes":["ai/"]}
            ]"#,
        );

        let error = match secret_registry_from_environment() {
            Ok(_) => panic!("duplicate deployment aliases must fail closed"),
            Err(error) => error,
        };
        assert!(error.contains("aliases must be unique and non-empty"));
    }

    #[test]
    fn every_deployment_resolver_kind_deserializes_without_initializing_clients() {
        let configs = serde_json::from_str::<Vec<DeploymentSecretResolverConfig>>(
            r#"[
                {"kind":"env","alias":"env","key_prefixes":["RUSTOK_AI_"]},
                {"kind":"mounted_file","alias":"file","root":"/var/run/secrets/rustok","key_prefixes":["ai/"]},
                {"kind":"vault","alias":"vault","endpoint":"https://vault.example.test","kv_mount":"secret","key_prefixes":["ai/"],"token_env":"RUSTOK_AI_VAULT_TOKEN"},
                {"kind":"kubernetes","alias":"kubernetes","namespace":"rustok","key_prefixes":["ai/"]},
                {"kind":"aws_secrets_manager","alias":"aws","key_prefixes":["ai/"]},
                {"kind":"gcp_secret_manager","alias":"gcp","project":"rustok-prod1","key_prefixes":["ai/"]},
                {"kind":"azure_key_vault","alias":"azure","endpoint":"https://rustok.vault.azure.net","key_prefixes":["ai/"]}
            ]"#,
        )
        .expect("deployment resolver JSON must deserialize");
        assert_eq!(configs.len(), 7);
        assert!(validate_config_aliases(&configs).is_ok());
    }

    #[test]
    fn offline_and_lazy_resolver_configs_register_without_network_calls() {
        let configs = vec![
            DeploymentSecretResolverConfig::Env {
                alias: "env".to_string(),
                key_prefixes: vec!["RUSTOK_AI_".to_string()],
            },
            DeploymentSecretResolverConfig::MountedFile {
                alias: "file".to_string(),
                root: std::path::PathBuf::from("/var/run/secrets/rustok"),
                key_prefixes: vec!["ai/".to_string()],
            },
            DeploymentSecretResolverConfig::Vault {
                alias: "vault".to_string(),
                endpoint: "https://vault.example.test".to_string(),
                namespace: None,
                kv_mount: "secret".to_string(),
                key_prefixes: vec!["ai/".to_string()],
                token_env: None,
                token_file: None,
                kubernetes_role: Some("rustok-ai".to_string()),
                kubernetes_auth_mount: Some("kubernetes".to_string()),
                kubernetes_token_path: Some(std::path::PathBuf::from("/token")),
            },
            DeploymentSecretResolverConfig::AwsSecretsManager {
                alias: "aws".to_string(),
                key_prefixes: vec!["ai/".to_string()],
            },
            DeploymentSecretResolverConfig::GcpSecretManager {
                alias: "gcp".to_string(),
                project: "rustok-prod1".to_string(),
                key_prefixes: vec!["ai/".to_string()],
            },
        ];

        let builder = configs.into_iter().try_fold(
            rustok_secrets::SecretResolverRegistry::builder(),
            register_deployment_resolver,
        );
        let registry = builder
            .expect("offline and lazy resolver setup must be deployable")
            .build();

        for alias in ["env", "file", "vault", "aws", "gcp"] {
            assert!(registry.contains(alias), "missing resolver alias {alias}");
        }
    }

    #[test]
    fn cloud_and_kubernetes_resolver_configs_fail_closed_when_invalid() {
        let invalid_kubernetes = DeploymentSecretResolverConfig::Kubernetes {
            alias: "kubernetes".to_string(),
            namespace: "invalid namespace".to_string(),
            key_prefixes: vec!["ai/".to_string()],
        };
        assert!(
            register_deployment_resolver(
                rustok_secrets::SecretResolverRegistry::builder(),
                invalid_kubernetes,
            )
            .is_err(),
            "Kubernetes resolver registration must fail without valid in-cluster configuration"
        );

        let invalid_gcp = DeploymentSecretResolverConfig::GcpSecretManager {
            alias: "gcp".to_string(),
            project: "INVALID".to_string(),
            key_prefixes: vec!["ai/".to_string()],
        };
        assert!(
            register_deployment_resolver(
                rustok_secrets::SecretResolverRegistry::builder(),
                invalid_gcp,
            )
            .is_err(),
            "GCP resolver registration must validate the deployment-owned project"
        );

        let invalid_azure = DeploymentSecretResolverConfig::AzureKeyVault {
            alias: "azure".to_string(),
            endpoint: "http://vault.example.test".to_string(),
            key_prefixes: vec!["ai/".to_string()],
        };
        assert!(
            register_deployment_resolver(
                rustok_secrets::SecretResolverRegistry::builder(),
                invalid_azure,
            )
            .is_err(),
            "Azure Key Vault resolver registration must reject non-HTTPS endpoints"
        );
    }
}
