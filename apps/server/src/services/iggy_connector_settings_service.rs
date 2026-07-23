use std::path::{Path, PathBuf};

use rustok_iggy::config::{IggyConfig, IggyMode};
use rustok_iggy_connector::{IggyConnectorConfigurationSnapshot, IggyConnectorSettingsInput};
use rustok_secrets::{
    EnvResolver, MountedFileResolver, SecretAccessPolicy, SecretRef, SecretResolverRegistry,
};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use url::Url;
use uuid::Uuid;

use crate::models::_entities::iggy_connector_settings::{self, Entity, Model};
use crate::services::event_transport_factory::EventRuntime;
use crate::services::server_runtime_context::ServerRuntimeContext;

const SINGLETON_ID: i32 = 1;
const ENV_RESOLVER: &str = "env";
const MOUNTED_FILE_RESOLVER: &str = "mounted_file";
const SECRET_MOUNT_ROOT_ENV: &str = "RUSTOK_IGGY_SECRET_MOUNT_ROOT";

#[derive(Debug, thiserror::Error)]
pub enum IggyConnectorSettingsError {
    #[error("Iggy connector mode must be bundled or external")]
    InvalidMode,
    #[error("{0}")]
    InvalidConfiguration(String),
    #[error("Iggy connector settings database error: {0}")]
    Database(#[from] sea_orm::DbErr),
}

pub struct IggyConnectorSettingsService;

impl IggyConnectorSettingsService {
    pub async fn configuration(
        ctx: &ServerRuntimeContext,
    ) -> Result<IggyConnectorConfigurationSnapshot, IggyConnectorSettingsError> {
        let stored = Entity::find_by_id(SINGLETON_ID).one(ctx.db()).await?;
        let active_mode = ctx
            .shared_get::<std::sync::Arc<EventRuntime>>()
            .and_then(|runtime| runtime.iggy_mode.clone());
        let desired_mode = match stored.as_ref() {
            Some(settings) => parse_mode(&settings.mode)?,
            None => ctx.settings().events.iggy.mode.clone(),
        };
        let bundled_available = bundled_available(&ctx.settings().events.iggy);
        let (addresses, username, resolver, key, tls_enabled, tls_domain) =
            external_snapshot(ctx, stored.as_ref());
        let configuration_error = Self::configuration_error(ctx, stored.as_ref()).await;

        Ok(IggyConnectorConfigurationSnapshot {
            active_mode: active_mode
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "inactive".to_string()),
            desired_mode: desired_mode.to_string(),
            bundled_available,
            external_addresses: addresses,
            external_username: username,
            password_resolver: resolver,
            password_key: key.clone(),
            password_configured: !key.trim().is_empty(),
            tls_enabled,
            tls_domain,
            configured: configuration_error.is_none(),
            configuration_error,
            restart_required: active_mode
                .as_ref()
                .is_some_and(|active| active != &desired_mode),
        })
    }

    pub async fn save(
        ctx: &ServerRuntimeContext,
        input: IggyConnectorSettingsInput,
        actor_id: Uuid,
        actor_tenant_id: Uuid,
    ) -> Result<(), IggyConnectorSettingsError> {
        let mode = parse_mode(&input.mode)?;
        let normalized_addresses = normalize_addresses(input.external_addresses)?;
        let username = input.external_username.trim().to_string();
        let resolver = input.password_resolver.trim().to_string();
        let key = input.password_key.trim().to_string();
        let tls_domain = normalize_optional(input.tls_domain);

        if mode == IggyMode::Bundled {
            validate_bundled(&ctx.settings().events.iggy)?;
        } else {
            validate_external_fields(
                &normalized_addresses,
                &username,
                &resolver,
                &key,
                input.tls_enabled,
                tls_domain.as_deref(),
            )?;
            resolve_password(ctx, actor_tenant_id, &resolver, &key).await?;
        }

        let now: chrono::DateTime<chrono::FixedOffset> = chrono::Utc::now().into();
        let addresses_json = serde_json::to_value(&normalized_addresses)
            .map_err(|error| IggyConnectorSettingsError::InvalidConfiguration(error.to_string()))?;
        match Entity::find_by_id(SINGLETON_ID).one(ctx.db()).await? {
            Some(row) => {
                let mut active: iggy_connector_settings::ActiveModel = row.into();
                active.mode = Set(mode.to_string());
                active.external_addresses = Set(addresses_json);
                active.external_username = Set(username);
                active.password_resolver = Set(nonempty(resolver));
                active.password_key = Set(nonempty(key));
                active.secret_tenant_id = Set(Some(actor_tenant_id));
                active.tls_enabled = Set(input.tls_enabled);
                active.tls_domain = Set(tls_domain);
                active.updated_by = Set(Some(actor_id));
                active.updated_at = Set(now);
                active.update(ctx.db()).await?;
            }
            None => {
                iggy_connector_settings::ActiveModel {
                    id: Set(SINGLETON_ID),
                    mode: Set(mode.to_string()),
                    external_addresses: Set(addresses_json),
                    external_username: Set(username),
                    password_resolver: Set(nonempty(resolver)),
                    password_key: Set(nonempty(key)),
                    secret_tenant_id: Set(Some(actor_tenant_id)),
                    tls_enabled: Set(input.tls_enabled),
                    tls_domain: Set(tls_domain),
                    updated_by: Set(Some(actor_id)),
                    created_at: Set(now),
                    updated_at: Set(chrono::Utc::now().into()),
                }
                .insert(ctx.db())
                .await?;
            }
        }
        Ok(())
    }

    pub async fn resolved_config(
        ctx: &ServerRuntimeContext,
    ) -> Result<IggyConfig, IggyConnectorSettingsError> {
        let stored = Entity::find_by_id(SINGLETON_ID).one(ctx.db()).await?;
        let Some(stored) = stored else {
            validate_deployment_config(&ctx.settings().events.iggy)?;
            return Ok(ctx.settings().events.iggy.clone());
        };

        let mut config = ctx.settings().events.iggy.clone();
        config.mode = parse_mode(&stored.mode)?;
        match config.mode {
            IggyMode::Bundled => validate_bundled(&config)?,
            IggyMode::External => {
                let addresses = addresses_from_model(&stored)?;
                let resolver = stored.password_resolver.as_deref().unwrap_or_default();
                let key = stored.password_key.as_deref().unwrap_or_default();
                let tenant_id = stored.secret_tenant_id.ok_or_else(|| {
                    IggyConnectorSettingsError::InvalidConfiguration(
                        "external password secret owner is missing".to_string(),
                    )
                })?;
                validate_external_fields(
                    &addresses,
                    &stored.external_username,
                    resolver,
                    key,
                    stored.tls_enabled,
                    stored.tls_domain.as_deref(),
                )?;
                let password = resolve_password(ctx, tenant_id, resolver, key).await?;
                config.external.addresses = addresses;
                config.external.protocol = "tcp".to_string();
                config.external.username = stored.external_username;
                config.external.password = password;
                config.external.tls_enabled = stored.tls_enabled;
                config.external.tls_domain = stored.tls_domain;
            }
        }
        Ok(config)
    }

    pub async fn readiness_error(ctx: &ServerRuntimeContext) -> Option<String> {
        let stored = match Entity::find_by_id(SINGLETON_ID).one(ctx.db()).await {
            Ok(stored) => stored,
            Err(error) => return Some(format!("cannot read connector settings: {error}")),
        };
        Self::configuration_error(ctx, stored.as_ref()).await
    }

    async fn configuration_error(
        ctx: &ServerRuntimeContext,
        stored: Option<&Model>,
    ) -> Option<String> {
        let result = match stored {
            Some(settings) => validate_stored(ctx, settings).await,
            None => validate_deployment_config(&ctx.settings().events.iggy),
        };
        result.err().map(|error| error.to_string())
    }
}

async fn validate_stored(
    ctx: &ServerRuntimeContext,
    stored: &Model,
) -> Result<(), IggyConnectorSettingsError> {
    match parse_mode(&stored.mode)? {
        IggyMode::Bundled => validate_bundled(&ctx.settings().events.iggy),
        IggyMode::External => {
            let addresses = addresses_from_model(stored)?;
            let resolver = stored.password_resolver.as_deref().unwrap_or_default();
            let key = stored.password_key.as_deref().unwrap_or_default();
            validate_external_fields(
                &addresses,
                &stored.external_username,
                resolver,
                key,
                stored.tls_enabled,
                stored.tls_domain.as_deref(),
            )?;
            let tenant_id = stored.secret_tenant_id.ok_or_else(|| {
                IggyConnectorSettingsError::InvalidConfiguration(
                    "external password secret owner is missing".to_string(),
                )
            })?;
            resolve_password(ctx, tenant_id, resolver, key).await?;
            Ok(())
        }
    }
}

fn validate_deployment_config(config: &IggyConfig) -> Result<(), IggyConnectorSettingsError> {
    match config.mode {
        IggyMode::Bundled => validate_bundled(config),
        IggyMode::External => {
            let addresses = normalize_addresses(config.external.addresses.clone())?;
            if config.external.password.is_empty() {
                return Err(IggyConnectorSettingsError::InvalidConfiguration(
                    "external password is not configured".to_string(),
                ));
            }
            validate_external_fields(
                &addresses,
                &config.external.username,
                "deployment",
                "configured",
                config.external.tls_enabled,
                config.external.tls_domain.as_deref(),
            )
        }
    }
}

fn validate_bundled(config: &IggyConfig) -> Result<(), IggyConnectorSettingsError> {
    if !rustok_iggy_connector::bundled_runtime_supported() {
        return Err(IggyConnectorSettingsError::InvalidConfiguration(
            "bundled Iggy is unavailable in this build or operating system; select external mode"
                .to_string(),
        ));
    }
    if config.bundled.data_dir.trim().is_empty() {
        return Err(IggyConnectorSettingsError::InvalidConfiguration(
            "bundled Iggy data directory is not configured".to_string(),
        ));
    }
    if !executable_exists(&config.bundled.executable) {
        return Err(IggyConnectorSettingsError::InvalidConfiguration(format!(
            "bundled Iggy artifact `{}` is not installed",
            config.bundled.executable
        )));
    }
    Ok(())
}

fn bundled_available(config: &IggyConfig) -> bool {
    validate_bundled(config).is_ok()
}

fn executable_exists(executable: &str) -> bool {
    let executable = executable.trim();
    if executable.is_empty() {
        return false;
    }
    let path = Path::new(executable);
    if path.components().count() > 1 {
        return executable_file(path);
    }
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths)
                .map(|directory| directory.join(executable))
                .any(|candidate| executable_file(&candidate))
        })
        .unwrap_or(false)
}

fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn validate_external_fields(
    addresses: &[String],
    username: &str,
    resolver: &str,
    key: &str,
    tls_enabled: bool,
    tls_domain: Option<&str>,
) -> Result<(), IggyConnectorSettingsError> {
    if addresses.is_empty() {
        return Err(IggyConnectorSettingsError::InvalidConfiguration(
            "at least one external Iggy address is required".to_string(),
        ));
    }
    for address in addresses {
        validate_address(address)?;
    }
    if username.trim().is_empty() {
        return Err(IggyConnectorSettingsError::InvalidConfiguration(
            "external Iggy username is required".to_string(),
        ));
    }
    if resolver.trim().is_empty() || key.trim().is_empty() {
        return Err(IggyConnectorSettingsError::InvalidConfiguration(
            "external Iggy password secret reference is required".to_string(),
        ));
    }
    if tls_enabled && tls_domain.is_some_and(|domain| domain.trim().is_empty()) {
        return Err(IggyConnectorSettingsError::InvalidConfiguration(
            "TLS domain must not be blank".to_string(),
        ));
    }
    Ok(())
}

fn validate_address(address: &str) -> Result<(), IggyConnectorSettingsError> {
    let url = Url::parse(&format!("tcp://{}", address.trim())).map_err(|_| {
        IggyConnectorSettingsError::InvalidConfiguration(format!(
            "invalid external Iggy address `{address}`; expected host:port"
        ))
    })?;
    if url.host_str().is_none()
        || url.port().is_none()
        || url.username() != ""
        || url.password().is_some()
        || url.path() != ""
    {
        return Err(IggyConnectorSettingsError::InvalidConfiguration(format!(
            "invalid external Iggy address `{address}`; expected host:port"
        )));
    }
    Ok(())
}

fn normalize_addresses(addresses: Vec<String>) -> Result<Vec<String>, IggyConnectorSettingsError> {
    let mut normalized = addresses
        .into_iter()
        .map(|address| address.trim().to_string())
        .filter(|address| !address.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    for address in &normalized {
        validate_address(address)?;
    }
    Ok(normalized)
}

fn addresses_from_model(model: &Model) -> Result<Vec<String>, IggyConnectorSettingsError> {
    serde_json::from_value::<Vec<String>>(model.external_addresses.clone()).map_err(|error| {
        IggyConnectorSettingsError::InvalidConfiguration(format!(
            "stored external Iggy addresses are invalid: {error}"
        ))
    })
}

fn external_snapshot(
    ctx: &ServerRuntimeContext,
    stored: Option<&Model>,
) -> (Vec<String>, String, String, String, bool, Option<String>) {
    match stored {
        Some(settings) => (
            addresses_from_model(settings).unwrap_or_default(),
            settings.external_username.clone(),
            settings.password_resolver.clone().unwrap_or_default(),
            settings.password_key.clone().unwrap_or_default(),
            settings.tls_enabled,
            settings.tls_domain.clone(),
        ),
        None => (
            ctx.settings().events.iggy.external.addresses.clone(),
            ctx.settings().events.iggy.external.username.clone(),
            "deployment".to_string(),
            if ctx.settings().events.iggy.external.password.is_empty() {
                String::new()
            } else {
                "configured".to_string()
            },
            ctx.settings().events.iggy.external.tls_enabled,
            ctx.settings().events.iggy.external.tls_domain.clone(),
        ),
    }
}

async fn resolve_password(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    resolver: &str,
    key: &str,
) -> Result<String, IggyConnectorSettingsError> {
    let reference = SecretRef {
        resolver: resolver.to_string(),
        key: key.to_string(),
    };
    let registry = if let Some(registry) = ctx.shared_get::<SecretResolverRegistry>() {
        registry
    } else {
        local_secret_registry(resolver, key)?
    };
    let password = registry
        .resolve_for_tenant(tenant_id, &reference)
        .await
        .map_err(|error| {
            IggyConnectorSettingsError::InvalidConfiguration(format!(
                "external Iggy password secret cannot be resolved: {error}"
            ))
        })?;
    use rustok_secrets::ExposeSecret;
    Ok(password.expose_secret().to_string())
}

fn local_secret_registry(
    resolver: &str,
    key: &str,
) -> Result<SecretResolverRegistry, IggyConnectorSettingsError> {
    let policy = SecretAccessPolicy::Exact(vec![key.to_string()]);
    let builder = SecretResolverRegistry::builder();
    let builder = match resolver {
        ENV_RESOLVER => builder.resolver(ENV_RESOLVER, EnvResolver, policy),
        MOUNTED_FILE_RESOLVER => {
            let root = std::env::var_os(SECRET_MOUNT_ROOT_ENV)
                .map(PathBuf::from)
                .ok_or_else(|| {
                    IggyConnectorSettingsError::InvalidConfiguration(format!(
                        "{SECRET_MOUNT_ROOT_ENV} is required for mounted_file secrets"
                    ))
                })?;
            builder.resolver(
                MOUNTED_FILE_RESOLVER,
                MountedFileResolver::new(root),
                policy,
            )
        }
        _ => {
            return Err(IggyConnectorSettingsError::InvalidConfiguration(format!(
                "secret resolver `{resolver}` is not registered"
            )));
        }
    };
    Ok(builder.build())
}

fn parse_mode(value: &str) -> Result<IggyMode, IggyConnectorSettingsError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "bundled" => Ok(IggyMode::Bundled),
        "external" => Ok(IggyMode::External),
        _ => Err(IggyConnectorSettingsError::InvalidMode),
    }
}

fn nonempty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| nonempty(value.trim().to_string()))
}
