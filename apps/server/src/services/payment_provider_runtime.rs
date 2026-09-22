#[cfg(feature = "payment-stripe")]
use crate::error::Error;
use crate::error::Result;
use crate::services::server_runtime_context::ServerRuntimeContext;

/// Build the process-owned payment provider registry once for all transports.
///
/// The manual provider remains the baseline. Optional external providers are
/// registered only from deployment-owned configuration; tenant requests and
/// persisted commerce records cannot supply secret resolver endpoints or keys.
pub fn build_payment_provider_registry(
    server: &ServerRuntimeContext,
) -> Result<rustok_payment::providers::PaymentProviderRegistry> {
    #[cfg(feature = "payment-stripe")]
    {
        let mut registry =
            rustok_payment::providers::PaymentProviderRegistry::with_manual_provider();
        register_deployment_stripe_provider(server, &mut registry)?;
        Ok(registry)
    }

    #[cfg(not(feature = "payment-stripe"))]
    {
        let _ = server;
        Ok(rustok_payment::providers::PaymentProviderRegistry::with_manual_provider())
    }
}

#[cfg(feature = "payment-stripe")]
mod stripe {
    use std::{
        collections::{HashMap, HashSet},
        path::PathBuf,
        sync::Arc,
    };

    use async_trait::async_trait;
    use rustok_payment::{
        PaymentError, PaymentResult, STRIPE_PAYMENT_PROVIDER_ID, StripeCredentialProvider,
        StripeCredentials, StripePaymentProvider, StripePaymentProviderConfig,
        providers::{
            ExternalPaymentProviderRegistration, PaymentProvider, PaymentProviderHealth,
            PaymentProviderRegistry,
        },
    };
    use rustok_secrets::{
        EnvResolver, MountedFileResolver, SecretAccessPolicy, SecretRef, SecretResolverRegistry,
    };
    use serde::Deserialize;
    use uuid::Uuid;

    use super::{Error, Result, ServerRuntimeContext};

    const TENANT_CREDENTIALS_ENV: &str = "RUSTOK_STRIPE_TENANT_CREDENTIALS_JSON";
    const SECRET_MOUNT_ROOT_ENV: &str = "RUSTOK_STRIPE_SECRET_MOUNT_ROOT";
    const API_BASE_ENV: &str = "RUSTOK_STRIPE_API_BASE";
    const REQUEST_TIMEOUT_ENV: &str = "RUSTOK_STRIPE_REQUEST_TIMEOUT_SECONDS";
    const WEBHOOK_TOLERANCE_ENV: &str = "RUSTOK_STRIPE_WEBHOOK_TOLERANCE_SECONDS";
    const ENV_RESOLVER_ALIAS: &str = "env";
    const MOUNTED_FILE_RESOLVER_ALIAS: &str = "mounted_file";

    #[derive(Clone, Debug, Deserialize)]
    struct StripeTenantCredentialRefs {
        tenant_id: Uuid,
        secret_key: SecretRef,
        webhook_secret: SecretRef,
    }

    #[derive(Clone)]
    struct DeploymentStripeCredentialProvider {
        registry: SecretResolverRegistry,
        refs_by_tenant: Arc<HashMap<Uuid, StripeTenantCredentialRefs>>,
    }

    impl DeploymentStripeCredentialProvider {
        fn new(
            registry: SecretResolverRegistry,
            configs: Vec<StripeTenantCredentialRefs>,
        ) -> Result<Self> {
            validate_configs(&configs)?;
            for config in &configs {
                validate_reference(&registry, config.tenant_id, &config.secret_key)?;
                validate_reference(&registry, config.tenant_id, &config.webhook_secret)?;
            }
            Ok(Self {
                registry,
                refs_by_tenant: Arc::new(
                    configs
                        .into_iter()
                        .map(|config| (config.tenant_id, config))
                        .collect(),
                ),
            })
        }
    }

    #[async_trait]
    impl StripeCredentialProvider for DeploymentStripeCredentialProvider {
        async fn credentials_for_tenant(
            &self,
            tenant_id: Uuid,
        ) -> PaymentResult<StripeCredentials> {
            let refs = self
                .refs_by_tenant
                .get(&tenant_id)
                .ok_or_else(|| PaymentError::provider_configuration(STRIPE_PAYMENT_PROVIDER_ID))?;
            let secret_key = self
                .registry
                .resolve_for_tenant(tenant_id, &refs.secret_key)
                .await
                .map_err(|_| PaymentError::provider_configuration(STRIPE_PAYMENT_PROVIDER_ID))?;
            let webhook_secret = self
                .registry
                .resolve_for_tenant(tenant_id, &refs.webhook_secret)
                .await
                .map_err(|_| PaymentError::provider_configuration(STRIPE_PAYMENT_PROVIDER_ID))?;
            StripeCredentials::new(secret_key, webhook_secret)
        }
    }

    pub(super) fn register_deployment_stripe_provider(
        server: &ServerRuntimeContext,
        registry: &mut PaymentProviderRegistry,
    ) -> Result<()> {
        let Some(configs) = configs_from_environment()? else {
            return Ok(());
        };
        let secret_registry = deployment_secret_registry(server, &configs)?;
        let credentials = Arc::new(DeploymentStripeCredentialProvider::new(
            secret_registry,
            configs,
        )?);
        let provider = Arc::new(
            StripePaymentProvider::new(provider_config_from_environment()?, credentials)
                .map_err(|_| configuration_error())?,
        );
        let registration = ExternalPaymentProviderRegistration {
            descriptor: provider.descriptor(),
            health: PaymentProviderHealth::Ready,
            degraded_mode: None,
        };
        registry
            .register_external(STRIPE_PAYMENT_PROVIDER_ID, provider, registration)
            .map_err(|_| configuration_error())?;
        Ok(())
    }

    fn configs_from_environment() -> Result<Option<Vec<StripeTenantCredentialRefs>>> {
        let Some(raw) = std::env::var_os(TENANT_CREDENTIALS_ENV) else {
            return Ok(None);
        };
        parse_configs(&raw.to_string_lossy()).map(Some)
    }

    fn parse_configs(raw: &str) -> Result<Vec<StripeTenantCredentialRefs>> {
        if raw.trim().is_empty() {
            return Err(configuration_error());
        }
        let configs = serde_json::from_str::<Vec<StripeTenantCredentialRefs>>(raw)
            .map_err(|_| configuration_error())?;
        validate_configs(&configs)?;
        Ok(configs)
    }

    fn validate_configs(configs: &[StripeTenantCredentialRefs]) -> Result<()> {
        if configs.is_empty() {
            return Err(configuration_error());
        }
        let mut tenants = HashSet::new();
        let mut reference_owners = HashMap::<(String, String), Uuid>::new();
        for config in configs {
            if config.tenant_id.is_nil() || !tenants.insert(config.tenant_id) {
                return Err(configuration_error());
            }
            if same_reference(&config.secret_key, &config.webhook_secret) {
                return Err(configuration_error());
            }
            for reference in [&config.secret_key, &config.webhook_secret] {
                if reference.resolver.trim().is_empty() || reference.key.trim().is_empty() {
                    return Err(configuration_error());
                }
                let identity = (reference.resolver.clone(), reference.key.clone());
                if reference_owners
                    .insert(identity, config.tenant_id)
                    .is_some_and(|owner| owner != config.tenant_id)
                {
                    return Err(configuration_error());
                }
            }
        }
        Ok(())
    }

    fn same_reference(left: &SecretRef, right: &SecretRef) -> bool {
        left.resolver == right.resolver && left.key == right.key
    }

    fn deployment_secret_registry(
        server: &ServerRuntimeContext,
        configs: &[StripeTenantCredentialRefs],
    ) -> Result<SecretResolverRegistry> {
        if let Some(registry) = server.shared_get::<SecretResolverRegistry>() {
            for config in configs {
                validate_reference(&registry, config.tenant_id, &config.secret_key)?;
                validate_reference(&registry, config.tenant_id, &config.webhook_secret)?;
            }
            return Ok(registry);
        }

        let mount_root = std::env::var_os(SECRET_MOUNT_ROOT_ENV).map(PathBuf::from);
        local_secret_registry(configs, mount_root)
    }

    fn local_secret_registry(
        configs: &[StripeTenantCredentialRefs],
        mount_root: Option<PathBuf>,
    ) -> Result<SecretResolverRegistry> {
        let mut env_keys = Vec::new();
        let mut mounted_file_keys = Vec::new();
        for reference in configs
            .iter()
            .flat_map(|config| [&config.secret_key, &config.webhook_secret])
        {
            match reference.resolver.as_str() {
                ENV_RESOLVER_ALIAS => env_keys.push(reference.key.clone()),
                MOUNTED_FILE_RESOLVER_ALIAS => mounted_file_keys.push(reference.key.clone()),
                _ => return Err(configuration_error()),
            }
        }

        let mut builder = SecretResolverRegistry::builder();
        if !env_keys.is_empty() {
            env_keys.sort();
            env_keys.dedup();
            builder = builder.resolver(
                ENV_RESOLVER_ALIAS,
                EnvResolver,
                SecretAccessPolicy::Exact(env_keys),
            );
        }
        if !mounted_file_keys.is_empty() {
            let root = mount_root.ok_or_else(configuration_error)?;
            mounted_file_keys.sort();
            mounted_file_keys.dedup();
            builder = builder.resolver(
                MOUNTED_FILE_RESOLVER_ALIAS,
                MountedFileResolver::new(root),
                SecretAccessPolicy::Exact(mounted_file_keys),
            );
        }

        let registry = builder.build();
        for config in configs {
            validate_reference(&registry, config.tenant_id, &config.secret_key)?;
            validate_reference(&registry, config.tenant_id, &config.webhook_secret)?;
        }
        Ok(registry)
    }

    fn validate_reference(
        registry: &SecretResolverRegistry,
        tenant_id: Uuid,
        reference: &SecretRef,
    ) -> Result<()> {
        registry
            .validate_reference_for_tenant(tenant_id, reference)
            .map_err(|_| configuration_error())
    }

    fn provider_config_from_environment() -> Result<StripePaymentProviderConfig> {
        let mut config = StripePaymentProviderConfig::default();
        if let Some(value) = std::env::var_os(API_BASE_ENV) {
            config.api_base = value.to_string_lossy().trim().to_string();
        }
        if let Some(value) = parse_environment::<u64>(REQUEST_TIMEOUT_ENV)? {
            config.request_timeout_seconds = value;
        }
        if let Some(value) = parse_environment::<i64>(WEBHOOK_TOLERANCE_ENV)? {
            config.webhook_tolerance_seconds = value;
        }
        config.validate().map_err(|_| configuration_error())?;
        Ok(config)
    }

    fn parse_environment<T>(name: &str) -> Result<Option<T>>
    where
        T: std::str::FromStr,
    {
        let Some(raw) = std::env::var_os(name) else {
            return Ok(None);
        };
        raw.to_string_lossy()
            .trim()
            .parse::<T>()
            .map(Some)
            .map_err(|_| configuration_error())
    }

    fn configuration_error() -> Error {
        Error::BadRequest("Stripe payment provider deployment configuration is invalid".to_string())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn refs(tenant_id: Uuid, prefix: &str) -> StripeTenantCredentialRefs {
            StripeTenantCredentialRefs {
                tenant_id,
                secret_key: SecretRef {
                    resolver: ENV_RESOLVER_ALIAS.to_string(),
                    key: format!("{prefix}_SECRET_KEY"),
                },
                webhook_secret: SecretRef {
                    resolver: ENV_RESOLVER_ALIAS.to_string(),
                    key: format!("{prefix}_WEBHOOK_SECRET"),
                },
            }
        }

        #[test]
        fn deployment_config_requires_unique_tenants_and_secret_references() {
            let tenant = Uuid::new_v4();
            assert!(validate_configs(&[refs(tenant, "TENANT_A")]).is_ok());
            assert!(
                validate_configs(&[refs(tenant, "TENANT_A"), refs(tenant, "TENANT_B"),]).is_err()
            );

            let mut first = refs(Uuid::new_v4(), "SHARED");
            let mut second = refs(Uuid::new_v4(), "OTHER");
            second.secret_key = first.secret_key.clone();
            assert!(validate_configs(&[first.clone(), second]).is_err());

            first.webhook_secret = first.secret_key.clone();
            assert!(validate_configs(&[first]).is_err());
        }

        #[test]
        fn local_registry_accepts_only_deployment_owned_resolver_aliases() {
            let config = refs(Uuid::new_v4(), "TENANT_A");
            assert!(local_secret_registry(&[config], None).is_ok());

            let mut unsupported = refs(Uuid::new_v4(), "TENANT_B");
            unsupported.secret_key.resolver = "tenant-url".to_string();
            assert!(local_secret_registry(&[unsupported], None).is_err());
        }

        #[test]
        fn parser_rejects_empty_or_malformed_configuration() {
            assert!(parse_configs("").is_err());
            assert!(parse_configs("{}").is_err());
            assert!(parse_configs("[]").is_err());
        }
    }
}

#[cfg(feature = "payment-stripe")]
use stripe::register_deployment_stripe_provider;
