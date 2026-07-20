use leptos::prelude::*;
#[cfg(feature = "ssr")]
use serde::Serialize;
#[cfg(feature = "ssr")]
use serde::de::DeserializeOwned;

use super::types::*;
#[allow(unused_imports)]
use crate::entities::module::model::{
    MarketplaceModuleVersion, RegistryFollowUpGateLifecycle, RegistryGovernanceActionLifecycle,
    RegistryGovernanceEventLifecycle, RegistryGovernanceEventPayloadLifecycle,
    RegistryModuleLifecycle, RegistryOwnerLifecycle, RegistryPublishRequestLifecycle,
    RegistryReleaseLifecycle, RegistryValidationStageLifecycle,
    registry_principal_label_from_value,
};
use crate::entities::module::{
    BuildJob, InstalledModule, MarketplaceModule, ModuleInfo, ReleaseInfo, TenantModule,
};
#[cfg(feature = "ssr")]
use crate::shared::api::api_base_url;

#[cfg(feature = "ssr")]
use super::manifest::*;

#[cfg(feature = "ssr")]
pub async fn registry_governance_get_native<T>(
    path: String,
    token: String,
    tenant: String,
) -> Result<T, ServerFnError>
where
    T: DeserializeOwned,
{
    registry_governance_http_request_native::<(), T>(
        reqwest::Method::GET,
        path,
        token,
        tenant,
        None,
    )
    .await
}

#[cfg(feature = "ssr")]
pub async fn registry_governance_request_native<B, T>(
    method: reqwest::Method,
    path: String,
    token: String,
    tenant: String,
    body: &B,
) -> Result<T, ServerFnError>
where
    B: Serialize + ?Sized,
    T: DeserializeOwned,
{
    registry_governance_http_request_native(method, path, token, tenant, Some(body)).await
}

#[cfg(feature = "ssr")]
pub async fn registry_governance_http_request_native<B, T>(
    method: reqwest::Method,
    path: String,
    token: String,
    tenant: String,
    body: Option<&B>,
) -> Result<T, ServerFnError>
where
    B: Serialize + ?Sized,
    T: DeserializeOwned,
{
    let url = format!(
        "{}{}",
        api_base_url(),
        if path.starts_with('/') {
            path
        } else {
            format!("/{path}")
        }
    );
    let client = reqwest::Client::new();
    let mut request = client
        .request(method, url)
        .bearer_auth(token)
        .header("X-Tenant-ID", tenant);

    if let Some(body) = body {
        request = request.json(body);
    }

    let response = request
        .send()
        .await
        .map_err(|err| ServerFnError::new(err.to_string()))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| ServerFnError::new(err.to_string()))?;

    if !status.is_success() {
        return Err(ServerFnError::new(format!(
            "registry governance request failed with status {status}: {text}"
        )));
    }

    serde_json::from_str(&text).map_err(|err| ServerFnError::new(err.to_string()))
}

#[cfg(feature = "ssr")]
#[derive(Clone, Debug, Serialize)]
pub struct RegistryValidationRequestPayload {
    #[serde(rename = "schema_version")]
    pub schema_version: u32,
    #[serde(rename = "dry_run")]
    pub dry_run: bool,
}

#[cfg(feature = "ssr")]
#[derive(Clone, Debug, Serialize)]
pub struct RegistryDecisionRequestPayload {
    #[serde(rename = "schema_version")]
    pub schema_version: u32,
    #[serde(rename = "dry_run")]
    pub dry_run: bool,
    pub reason: Option<String>,
    pub reason_code: Option<String>,
}

#[cfg(feature = "ssr")]
#[derive(Clone, Debug, Serialize)]
pub struct RegistryOwnerTransferPayload {
    #[serde(rename = "schema_version")]
    pub schema_version: u32,
    #[serde(rename = "dry_run")]
    pub dry_run: bool,
    pub slug: String,
    #[serde(rename = "new_owner_user_id")]
    pub new_owner_user_id: String,
    pub reason: Option<String>,
    pub reason_code: Option<String>,
}

#[cfg(feature = "ssr")]
#[derive(Clone, Debug, Serialize)]
pub struct RegistryYankPayload {
    #[serde(rename = "schema_version")]
    pub schema_version: u32,
    #[serde(rename = "dry_run")]
    pub dry_run: bool,
    pub slug: String,
    pub version: String,
    pub reason: Option<String>,
    pub reason_code: Option<String>,
}

#[cfg(feature = "ssr")]
pub fn server_error(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError(message.into())
}

#[cfg(feature = "ssr")]
pub fn default_module_ownership() -> String {
    "third_party".to_string()
}

#[cfg(feature = "ssr")]
pub fn default_module_trust_level() -> String {
    "unverified".to_string()
}

#[cfg(feature = "ssr")]
#[derive(Clone)]
struct ModulesServerRuntime {
    db: sea_orm::DatabaseConnection,
}

#[cfg(feature = "ssr")]
async fn modules_server_context() -> Result<
    (
        ModulesServerRuntime,
        rustok_api::AuthContext,
        rustok_api::TenantContext,
    ),
    ServerFnError,
> {
    use leptos::prelude::expect_context;
    use leptos_axum::extract;
    use rustok_api::Permission;
    use rustok_api::{
        AuthContext, HostRuntimeContext, TenantContext, has_any_effective_permission,
    };

    let runtime_ctx = expect_context::<HostRuntimeContext>();
    let auth = extract::<AuthContext>()
        .await
        .map_err(|err| server_error(err.to_string()))?;
    let tenant = extract::<TenantContext>()
        .await
        .map_err(|err| server_error(err.to_string()))?;

    if !has_any_effective_permission(
        &auth.permissions,
        &[
            Permission::MODULES_READ,
            Permission::MODULES_LIST,
            Permission::MODULES_MANAGE,
        ],
    ) {
        return Err(ServerFnError::new(
            "modules:read, modules:list, or modules:manage required",
        ));
    }

    Ok((
        ModulesServerRuntime {
            db: runtime_ctx.db_clone(),
        },
        auth,
        tenant,
    ))
}

#[cfg(feature = "ssr")]
pub fn upper_snake(value: &str) -> String {
    value
        .replace('-', "_")
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_uppercase())
        .collect::<Vec<_>>()
        .join("_")
}

#[cfg(feature = "ssr")]
pub fn build_modules_delta_summary(value: Option<&serde_json::Value>) -> String {
    let Some(value) = value else {
        return String::new();
    };

    if let Some(summary) = value.as_str() {
        return summary.to_string();
    }

    if let Some(summary) = value.get("summary").and_then(serde_json::Value::as_str) {
        return summary.to_string();
    }

    if let Some(object) = value.as_object() {
        let mut slugs = object.keys().cloned().collect::<Vec<_>>();
        slugs.sort();
        return slugs.join(",");
    }

    value.to_string()
}

#[cfg(feature = "ssr")]
pub fn platform_state_select_sql(backend: sea_orm::DbBackend) -> &'static str {
    match backend {
        sea_orm::DbBackend::Sqlite => {
            "SELECT revision, manifest_json, manifest_hash FROM platform_state WHERE id = ?1 LIMIT 1"
        }
        _ => {
            "SELECT revision, manifest_json, manifest_hash FROM platform_state WHERE id = $1 LIMIT 1"
        }
    }
}

#[cfg(feature = "ssr")]
pub fn platform_state_insert_sql(backend: sea_orm::DbBackend) -> &'static str {
    match backend {
        sea_orm::DbBackend::Sqlite => {
            "INSERT INTO platform_state (id, revision, manifest_json, manifest_hash, active_release_id, updated_by, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7)"
        }
        _ => {
            "INSERT INTO platform_state (id, revision, manifest_json, manifest_hash, active_release_id, updated_by, created_at, updated_at) VALUES ($1, $2, $3, $4, NULL, $5, $6, $7)"
        }
    }
}

#[cfg(feature = "ssr")]
pub async fn active_runtime_platform_snapshot(
    db: &sea_orm::DatabaseConnection,
) -> Result<RuntimePlatformSnapshot, ServerFnError> {
    use sea_orm::{ConnectionTrait, Statement};

    let backend = db.get_database_backend();
    let row = db
        .query_one(Statement::from_sql_and_values(
            backend,
            platform_state_select_sql(backend),
            vec!["active".into()],
        ))
        .await
        .map_err(|err| server_error(err.to_string()))?;

    if let Some(row) = row {
        let manifest_json = row
            .try_get::<serde_json::Value>("", "manifest_json")
            .map_err(|err| server_error(err.to_string()))?;
        return Ok(RuntimePlatformSnapshot {
            revision: row
                .try_get("", "revision")
                .map_err(|err| server_error(err.to_string()))?,
            manifest: serde_json::from_value(manifest_json).map_err(|err| {
                server_error(format!("failed to decode platform manifest: {err}"))
            })?,
        });
    }

    let manifest = bootstrap_runtime_modules_manifest()?;
    let manifest_hash = runtime_manifest_hash(&manifest);
    let now = chrono::Utc::now();
    let manifest_json = rustok_api::manifest_hash::canonical_manifest_snapshot_json(&manifest)
        .map_err(|err| server_error(format!("failed to encode platform manifest: {err}")))?;
    let insert = Statement::from_sql_and_values(
        backend,
        platform_state_insert_sql(backend),
        vec![
            "active".into(),
            1.into(),
            manifest_json.into(),
            manifest_hash.clone().into(),
            "bootstrap".into(),
            now.into(),
            now.into(),
        ],
    );
    match db.execute(insert).await {
        Ok(_) => {}
        Err(err) => {
            let msg = err.to_string().to_lowercase();
            let is_duplicate = msg.contains("duplicate")
                || msg.contains("unique")
                || msg.contains("already exists");
            if !is_duplicate {
                return Err(server_error(format!(
                    "failed to bootstrap runtime platform state: {err}"
                )));
            }
        }
    }
    Ok(RuntimePlatformSnapshot {
        revision: 1,
        manifest,
    })
}

#[cfg(feature = "ssr")]
pub async fn effective_enabled_modules_native(
    db: &sea_orm::DatabaseConnection,
    registry: &rustok_core::ModuleRegistry,
    tenant_id: uuid::Uuid,
) -> Result<std::collections::HashSet<String>, ServerFnError> {
    use rustok_tenant::entities::tenant_module;
    use rustok_tenant::entities::tenant_module::Entity as TenantModuleEntity;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let manifest = active_runtime_platform_snapshot(db).await?.manifest;
    let mut enabled = registry
        .list()
        .into_iter()
        .filter(|module| registry.is_core(module.slug()))
        .map(|module| module.slug().to_string())
        .collect::<std::collections::HashSet<_>>();

    for slug in manifest.settings.default_enabled {
        if registry.get(&slug).is_some() {
            enabled.insert(slug);
        }
    }

    let overrides = TenantModuleEntity::find()
        .filter(tenant_module::Column::TenantId.eq(tenant_id))
        .all(db)
        .await
        .map_err(|err| server_error(err.to_string()))?;
    for module in overrides {
        if module.enabled {
            enabled.insert(module.module_slug);
        } else {
            enabled.remove(&module.module_slug);
        }
    }

    Ok(enabled)
}

#[cfg(feature = "ssr")]
pub fn humanize_module_slug(slug: &str) -> String {
    slug.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(feature = "ssr")]
pub fn fallback_module_category(slug: &str) -> &'static str {
    match slug {
        "content" | "blog" | "forum" | "pages" => "content",
        "commerce" | "pricing" | "product" | "inventory" => "commerce",
        "tenant" | "rbac" | "index" | "outbox" => "platform",
        _ => "extensions",
    }
}

#[cfg(feature = "ssr")]
pub fn runtime_setting_fields(
    schema: &std::collections::BTreeMap<String, RuntimeModuleSettingSpec>,
) -> Vec<crate::entities::module::ModuleSettingField> {
    schema
        .iter()
        .map(|(key, spec)| crate::entities::module::ModuleSettingField {
            key: key.clone(),
            value_type: spec.value_type.clone(),
            required: spec.required,
            default_value: spec.default.clone(),
            description: spec.description.clone(),
            min: spec.min,
            max: spec.max,
            options: spec.options.clone(),
            object_keys: runtime_setting_object_keys(spec),
            item_type: runtime_setting_item_type(spec),
            shape: runtime_setting_shape(spec),
        })
        .collect()
}

#[cfg(feature = "ssr")]
#[allow(dead_code)]
pub fn runtime_build_job_insert_sql(backend: sea_orm::DbBackend) -> &'static str {
    match backend {
        sea_orm::DbBackend::Sqlite => {
            r#"
            INSERT INTO builds (
                id, status, stage, progress, profile, manifest_ref, manifest_hash,
                manifest_revision, manifest_snapshot, modules_delta,
                requested_by, reason, release_id, logs_url, error_message, started_at, finished_at,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL, NULL, NULL, NULL, NULL, ?13, ?14)
            "#
        }
        _ => {
            r#"
            INSERT INTO builds (
                id, status, stage, progress, profile, manifest_ref, manifest_hash,
                manifest_revision, manifest_snapshot, modules_delta,
                requested_by, reason, release_id, logs_url, error_message, started_at, finished_at,
                created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NULL, NULL, NULL, NULL, NULL, $13, $14)
            "#
        }
    }
}

#[cfg(feature = "ssr")]
#[allow(dead_code)]
pub fn runtime_build_job_select_sql(backend: sea_orm::DbBackend) -> &'static str {
    match backend {
        sea_orm::DbBackend::Sqlite => {
            r#"
            SELECT
                id, status, stage, progress, profile, manifest_ref, manifest_hash,
                manifest_revision, modules_delta,
                requested_by, reason, release_id, logs_url, error_message, started_at,
                created_at, updated_at, finished_at
            FROM builds
            WHERE id = ?1
            LIMIT 1
            "#
        }
        _ => {
            r#"
            SELECT
                id, status, stage, progress, profile, manifest_ref, manifest_hash,
                manifest_revision, modules_delta,
                requested_by, reason, release_id, logs_url, error_message, started_at,
                created_at, updated_at, finished_at
            FROM builds
            WHERE id = $1
            LIMIT 1
            "#
        }
    }
}

pub fn runtime_setting_value_matches_type(value_type: &str, value: &serde_json::Value) -> bool {
    match value_type {
        "string" => value.is_string(),
        "integer" => {
            value.as_i64().is_some()
                || value.as_u64().is_some()
                || value
                    .as_f64()
                    .is_some_and(|number| number.fract().abs() < f64::EPSILON)
        }
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "json" | "any" => true,
        _ => false,
    }
}

#[cfg(feature = "ssr")]
pub fn validate_runtime_setting_value(
    module_slug: &str,
    key: &str,
    spec: &RuntimeModuleSettingSpec,
    value: &serde_json::Value,
) -> Result<(), ServerFnError> {
    let value_type = spec.value_type.trim();
    if !runtime_setting_value_matches_type(value_type, value) {
        return Err(server_error(format!(
            "Module '{module_slug}' setting '{key}' is invalid: expected {value_type}"
        )));
    }

    if !spec.options.is_empty() && !spec.options.iter().any(|option| option == value) {
        let allowed = spec
            .options
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(server_error(format!(
            "Module '{module_slug}' setting '{key}' is invalid: must be one of: {allowed}"
        )));
    }

    match value_type {
        "integer" | "number" => {
            let numeric_value = value.as_f64().ok_or_else(|| {
                server_error(format!(
                    "Module '{module_slug}' setting '{key}' is invalid: expected {value_type}"
                ))
            })?;

            if let Some(min) = spec.min {
                if numeric_value < min {
                    return Err(server_error(format!(
                        "Module '{module_slug}' setting '{key}' is invalid: must be >= {min}"
                    )));
                }
            }
            if let Some(max) = spec.max {
                if numeric_value > max {
                    return Err(server_error(format!(
                        "Module '{module_slug}' setting '{key}' is invalid: must be <= {max}"
                    )));
                }
            }
        }
        "string" => {
            let length = value
                .as_str()
                .map(|item| item.chars().count())
                .unwrap_or_default() as f64;
            if let Some(min) = spec.min {
                if length < min {
                    return Err(server_error(format!(
                        "Module '{module_slug}' setting '{key}' is invalid: length must be >= {min}"
                    )));
                }
            }
            if let Some(max) = spec.max {
                if length > max {
                    return Err(server_error(format!(
                        "Module '{module_slug}' setting '{key}' is invalid: length must be <= {max}"
                    )));
                }
            }
        }
        "array" => {
            let length = value
                .as_array()
                .map(|items| items.len())
                .unwrap_or_default() as f64;
            if let Some(min) = spec.min {
                if length < min {
                    return Err(server_error(format!(
                        "Module '{module_slug}' setting '{key}' is invalid: length must be >= {min}"
                    )));
                }
            }
            if let Some(max) = spec.max {
                if length > max {
                    return Err(server_error(format!(
                        "Module '{module_slug}' setting '{key}' is invalid: length must be <= {max}"
                    )));
                }
            }
        }
        _ => {}
    }

    Ok(())
}

#[cfg(feature = "ssr")]
pub fn normalize_runtime_module_settings(
    module_slug: &str,
    schema: &std::collections::BTreeMap<String, RuntimeModuleSettingSpec>,
    settings: serde_json::Value,
) -> Result<serde_json::Value, ServerFnError> {
    let mut settings_object = settings.as_object().cloned().ok_or_else(|| {
        server_error(format!(
            "Module '{module_slug}' setting '$root' is invalid: module settings must be a JSON object"
        ))
    })?;

    if schema.is_empty() {
        return Ok(serde_json::Value::Object(settings_object));
    }

    let allowed_keys = schema.keys().cloned().collect::<Vec<_>>();
    let mut unknown_keys = settings_object
        .keys()
        .filter(|key| !schema.contains_key(*key))
        .cloned()
        .collect::<Vec<_>>();
    unknown_keys.sort();
    if let Some(key) = unknown_keys.first() {
        return Err(server_error(format!(
            "Module '{module_slug}' setting '{key}' is invalid: unknown setting; allowed keys: {}",
            allowed_keys.join(", ")
        )));
    }

    let mut normalized = serde_json::Map::new();
    for key in allowed_keys {
        let spec = schema
            .get(&key)
            .expect("runtime settings key must exist in schema");

        match settings_object.remove(&key) {
            Some(value) => {
                validate_runtime_setting_value(module_slug, &key, spec, &value)?;
                normalized.insert(key, value);
            }
            None if spec.required && spec.default.is_none() => {
                return Err(server_error(format!(
                    "Module '{module_slug}' setting '{key}' is invalid: required setting is missing"
                )));
            }
            None => {
                if let Some(default) = spec.default.clone() {
                    if !runtime_setting_value_matches_type(spec.value_type.trim(), &default) {
                        return Err(server_error(format!(
                            "Module '{module_slug}' setting '{key}' has invalid schema: default does not match declared type"
                        )));
                    }
                    if !spec.options.is_empty()
                        && !spec.options.iter().any(|option| option == &default)
                    {
                        return Err(server_error(format!(
                            "Module '{module_slug}' setting '{key}' has invalid schema: default must be one of the declared options"
                        )));
                    }
                    normalized.insert(key, default);
                }
            }
        }
    }

    Ok(serde_json::Value::Object(normalized))
}

#[cfg(feature = "ssr")]
pub fn map_build_job_row(row: sea_orm::QueryResult) -> Result<BuildJob, ServerFnError> {
    let modules_delta = row
        .try_get::<Option<serde_json::Value>>("", "modules_delta")
        .map_err(|err| server_error(err.to_string()))?;

    Ok(BuildJob {
        id: row
            .try_get::<uuid::Uuid>("", "id")
            .map(|value| value.to_string())
            .map_err(|err| server_error(err.to_string()))?,
        status: upper_snake(
            &row.try_get::<String>("", "status")
                .map_err(|err| server_error(err.to_string()))?,
        ),
        stage: upper_snake(
            &row.try_get::<String>("", "stage")
                .map_err(|err| server_error(err.to_string()))?,
        ),
        progress: row
            .try_get("", "progress")
            .map_err(|err| server_error(err.to_string()))?,
        profile: upper_snake(
            &row.try_get::<String>("", "profile")
                .map_err(|err| server_error(err.to_string()))?,
        ),
        manifest_ref: row
            .try_get("", "manifest_ref")
            .map_err(|err| server_error(err.to_string()))?,
        manifest_hash: row
            .try_get("", "manifest_hash")
            .map_err(|err| server_error(err.to_string()))?,
        manifest_revision: row.try_get("", "manifest_revision").unwrap_or_default(),
        modules_delta: build_modules_delta_summary(modules_delta.as_ref()),
        requested_by: row
            .try_get("", "requested_by")
            .map_err(|err| server_error(err.to_string()))?,
        reason: row
            .try_get("", "reason")
            .map_err(|err| server_error(err.to_string()))?,
        release_id: row
            .try_get("", "release_id")
            .map_err(|err| server_error(err.to_string()))?,
        logs_url: row
            .try_get("", "logs_url")
            .map_err(|err| server_error(err.to_string()))?,
        error_message: row
            .try_get("", "error_message")
            .map_err(|err| server_error(err.to_string()))?,
        started_at: row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "started_at")
            .map(|value| value.map(|value| value.to_rfc3339()))
            .map_err(|err| server_error(err.to_string()))?,
        created_at: row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "created_at")
            .map(|value| value.to_rfc3339())
            .map_err(|err| server_error(err.to_string()))?,
        updated_at: row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "updated_at")
            .map(|value| value.to_rfc3339())
            .map_err(|err| server_error(err.to_string()))?,
        finished_at: row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "finished_at")
            .map(|value| value.map(|value| value.to_rfc3339()))
            .map_err(|err| server_error(err.to_string()))?,
    })
}

#[cfg(feature = "ssr")]
pub fn map_release_info_row(row: sea_orm::QueryResult) -> Result<ReleaseInfo, ServerFnError> {
    let modules = row
        .try_get::<serde_json::Value>("", "modules")
        .ok()
        .and_then(|value| serde_json::from_value::<Vec<String>>(value).ok())
        .unwrap_or_default();

    Ok(ReleaseInfo {
        id: row
            .try_get("", "id")
            .map_err(|err| server_error(err.to_string()))?,
        build_id: row
            .try_get::<uuid::Uuid>("", "build_id")
            .map(|value| value.to_string())
            .map_err(|err| server_error(err.to_string()))?,
        status: upper_snake(
            &row.try_get::<String>("", "status")
                .map_err(|err| server_error(err.to_string()))?,
        ),
        environment: row
            .try_get("", "environment")
            .map_err(|err| server_error(err.to_string()))?,
        manifest_hash: row
            .try_get("", "manifest_hash")
            .map_err(|err| server_error(err.to_string()))?,
        manifest_revision: row.try_get("", "manifest_revision").unwrap_or_default(),
        modules,
        previous_release_id: row
            .try_get("", "previous_release_id")
            .map_err(|err| server_error(err.to_string()))?,
        deployed_at: row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "deployed_at")
            .map(|value| value.map(|value| value.to_rfc3339()))
            .map_err(|err| server_error(err.to_string()))?,
        rolled_back_at: row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "rolled_back_at")
            .map(|value| value.map(|value| value.to_rfc3339()))
            .map_err(|err| server_error(err.to_string()))?,
        created_at: row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "created_at")
            .map(|value| value.to_rfc3339())
            .map_err(|err| server_error(err.to_string()))?,
        updated_at: row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "updated_at")
            .map(|value| value.to_rfc3339())
            .map_err(|err| server_error(err.to_string()))?,
    })
}

#[cfg(feature = "ssr")]
pub fn json_message_list(value: Option<serde_json::Value>) -> Vec<String> {
    value
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| item.as_str().map(ToString::to_string))
        .collect()
}

#[cfg(feature = "ssr")]
pub fn required_registry_principal_label(
    row: &sea_orm::QueryResult,
    column: &str,
) -> Result<String, ServerFnError> {
    let value = row
        .try_get::<serde_json::Value>("", column)
        .map_err(|err| server_error(err.to_string()))?;
    registry_principal_label_from_value(&value).ok_or_else(|| {
        server_error(format!(
            "Registry principal column '{column}' is missing a displayable principal label"
        ))
    })
}

#[cfg(feature = "ssr")]
pub fn optional_registry_principal_label(
    row: &sea_orm::QueryResult,
    column: &str,
) -> Result<Option<String>, ServerFnError> {
    let value = row
        .try_get::<Option<serde_json::Value>>("", column)
        .map_err(|err| server_error(err.to_string()))?;
    Ok(value.as_ref().and_then(registry_principal_label_from_value))
}

#[cfg(feature = "ssr")]
pub fn map_registry_publish_request_row(
    row: sea_orm::QueryResult,
) -> Result<RegistryPublishRequestLifecycle, ServerFnError> {
    Ok(RegistryPublishRequestLifecycle {
        id: row
            .try_get("", "id")
            .map_err(|err| server_error(err.to_string()))?,
        status: upper_snake(
            &row.try_get::<String>("", "status")
                .map_err(|err| server_error(err.to_string()))?,
        ),
        requested_by: required_registry_principal_label(&row, "requested_by")?,
        publisher: optional_registry_principal_label(&row, "publisher")?,
        approved_by: optional_registry_principal_label(&row, "approved_by")?,
        rejected_by: optional_registry_principal_label(&row, "rejected_by")?,
        rejection_reason: row
            .try_get("", "rejection_reason")
            .map_err(|err| server_error(err.to_string()))?,
        changes_requested_by: optional_registry_principal_label(&row, "changes_requested_by")?,
        changes_requested_reason: row
            .try_get("", "changes_requested_reason")
            .map_err(|err| server_error(err.to_string()))?,
        changes_requested_reason_code: row
            .try_get("", "changes_requested_reason_code")
            .map_err(|err| server_error(err.to_string()))?,
        changes_requested_at: row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "changes_requested_at")
            .map(|value| value.map(|value| value.to_rfc3339()))
            .map_err(|err| server_error(err.to_string()))?,
        held_by: optional_registry_principal_label(&row, "held_by")?,
        held_reason: row
            .try_get("", "held_reason")
            .map_err(|err| server_error(err.to_string()))?,
        held_reason_code: row
            .try_get("", "held_reason_code")
            .map_err(|err| server_error(err.to_string()))?,
        held_at: row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "held_at")
            .map(|value| value.map(|value| value.to_rfc3339()))
            .map_err(|err| server_error(err.to_string()))?,
        held_from_status: row
            .try_get("", "held_from_status")
            .map_err(|err| server_error(err.to_string()))?,
        warnings: json_message_list(
            row.try_get::<Option<serde_json::Value>>("", "validation_warnings")
                .map_err(|err| server_error(err.to_string()))?,
        ),
        errors: json_message_list(
            row.try_get::<Option<serde_json::Value>>("", "validation_errors")
                .map_err(|err| server_error(err.to_string()))?,
        ),
        created_at: row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "created_at")
            .map(|value| value.to_rfc3339())
            .map_err(|err| server_error(err.to_string()))?,
        updated_at: row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "updated_at")
            .map(|value| value.to_rfc3339())
            .map_err(|err| server_error(err.to_string()))?,
        published_at: row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "published_at")
            .map(|value| value.map(|value| value.to_rfc3339()))
            .map_err(|err| server_error(err.to_string()))?,
    })
}

#[cfg(feature = "ssr")]
pub fn map_registry_release_row(
    row: sea_orm::QueryResult,
) -> Result<RegistryReleaseLifecycle, ServerFnError> {
    Ok(RegistryReleaseLifecycle {
        version: row
            .try_get("", "version")
            .map_err(|err| server_error(err.to_string()))?,
        status: upper_snake(
            &row.try_get::<String>("", "status")
                .map_err(|err| server_error(err.to_string()))?,
        ),
        publisher: required_registry_principal_label(&row, "publisher")?,
        checksum_sha256: row
            .try_get("", "checksum_sha256")
            .map_err(|err| server_error(err.to_string()))?,
        published_at: row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "published_at")
            .map(|value| value.to_rfc3339())
            .map_err(|err| server_error(err.to_string()))?,
        yanked_reason: row
            .try_get("", "yanked_reason")
            .map_err(|err| server_error(err.to_string()))?,
        yanked_by: optional_registry_principal_label(&row, "yanked_by")?,
        yanked_at: row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "yanked_at")
            .map(|value| value.map(|value| value.to_rfc3339()))
            .map_err(|err| server_error(err.to_string()))?,
    })
}

#[cfg(feature = "ssr")]
pub fn map_registry_owner_row(
    row: sea_orm::QueryResult,
) -> Result<RegistryOwnerLifecycle, ServerFnError> {
    Ok(RegistryOwnerLifecycle {
        owner: required_registry_principal_label(&row, "owner")?,
        bound_by: required_registry_principal_label(&row, "bound_by")?,
        bound_at: row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "bound_at")
            .map(|value| value.to_rfc3339())
            .map_err(|err| server_error(err.to_string()))?,
        updated_at: row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "updated_at")
            .map(|value| value.to_rfc3339())
            .map_err(|err| server_error(err.to_string()))?,
    })
}

#[cfg(feature = "ssr")]
pub fn map_registry_governance_event_row(
    row: sea_orm::QueryResult,
) -> Result<RegistryGovernanceEventLifecycle, ServerFnError> {
    let details = row
        .try_get::<Option<serde_json::Value>>("", "details")
        .map_err(|err| server_error(err.to_string()))?
        .unwrap_or(serde_json::Value::Null);
    Ok(RegistryGovernanceEventLifecycle {
        id: row
            .try_get("", "id")
            .map_err(|err| server_error(err.to_string()))?,
        event_type: row
            .try_get("", "event_type")
            .map_err(|err| server_error(err.to_string()))?,
        actor: required_registry_principal_label(&row, "actor")?,
        publisher: optional_registry_principal_label(&row, "publisher")?,
        payload: RegistryGovernanceEventPayloadLifecycle::from_details(&details),
        created_at: row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "created_at")
            .map(|value| value.to_rfc3339())
            .map_err(|err| server_error(err.to_string()))?,
    })
}

#[cfg(feature = "ssr")]
pub fn map_registry_validation_stage_row(
    row: sea_orm::QueryResult,
) -> Result<RegistryValidationStageLifecycle, ServerFnError> {
    let key: String = row
        .try_get("", "stage_key")
        .map_err(|err| server_error(err.to_string()))?;
    Ok(RegistryValidationStageLifecycle {
        key: key.clone(),
        status: row
            .try_get("", "status")
            .map_err(|err| server_error(err.to_string()))?,
        detail: row
            .try_get("", "detail")
            .map_err(|err| server_error(err.to_string()))?,
        attempt_number: row
            .try_get("", "attempt_number")
            .map_err(|err| server_error(err.to_string()))?,
        updated_at: row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "updated_at")
            .map(|value| value.to_rfc3339())
            .map_err(|err| server_error(err.to_string()))?,
        started_at: row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "started_at")
            .map(|value| value.map(|value| value.to_rfc3339()))
            .map_err(|err| server_error(err.to_string()))?,
        finished_at: row
            .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "finished_at")
            .map(|value| value.map(|value| value.to_rfc3339()))
            .map_err(|err| server_error(err.to_string()))?,
        execution_mode: fallback_validation_stage_execution_mode(&key).to_string(),
        runnable: matches!(
            key.as_str(),
            "compile_smoke" | "targeted_tests" | "security_policy_review"
        ),
        requires_manual_confirmation: key == "security_policy_review",
        allowed_terminal_reason_codes: fallback_validation_stage_reason_codes(),
        suggested_pass_reason_code: fallback_validation_stage_pass_reason_code(&key)
            .map(str::to_string),
        suggested_failure_reason_code: fallback_validation_stage_failure_reason_code(&key)
            .map(str::to_string),
        suggested_blocked_reason_code: fallback_validation_stage_blocked_reason_code(&key)
            .map(str::to_string),
    })
}

#[cfg(feature = "ssr")]
pub fn registry_follow_up_gate_detail(key: &str) -> &'static str {
    match key {
        "compile_smoke" => "Compile smoke still runs outside the current registry validator.",
        "targeted_tests" => {
            "Targeted module tests still run outside the current registry validator."
        }
        "security_policy_review" => {
            "Security and policy review still require an external gate before production approval."
        }
        _ => "External follow-up gate is still pending.",
    }
}

#[cfg(feature = "ssr")]
pub fn fallback_validation_stage_execution_mode(key: &str) -> &'static str {
    match key {
        "security_policy_review" => "manual_review",
        "compile_smoke" | "targeted_tests" => "remote",
        _ => "external",
    }
}

#[cfg(feature = "ssr")]
pub fn fallback_validation_stage_reason_codes() -> Vec<String> {
    [
        "local_runner_passed",
        "manual_review_complete",
        "build_failure",
        "test_failure",
        "policy_preflight_failed",
        "security_findings",
        "policy_exception",
        "license_issue",
        "manual_override",
        "other",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

#[cfg(feature = "ssr")]
pub fn fallback_validation_stage_pass_reason_code(key: &str) -> Option<&'static str> {
    match key {
        "compile_smoke" | "targeted_tests" => Some("local_runner_passed"),
        "security_policy_review" => Some("manual_review_complete"),
        _ => None,
    }
}

#[cfg(feature = "ssr")]
pub fn fallback_validation_stage_failure_reason_code(key: &str) -> Option<&'static str> {
    match key {
        "compile_smoke" => Some("build_failure"),
        "targeted_tests" => Some("test_failure"),
        "security_policy_review" => Some("policy_preflight_failed"),
        _ => None,
    }
}

#[cfg(feature = "ssr")]
pub fn fallback_validation_stage_blocked_reason_code(key: &str) -> Option<&'static str> {
    match key {
        "security_policy_review" => Some("security_findings"),
        "compile_smoke" | "targeted_tests" => Some("other"),
        _ => None,
    }
}

#[cfg(feature = "ssr")]
pub fn registry_governance_action(
    key: &str,
    reason_required: bool,
    reason_code_required: bool,
    reason_codes: &[&str],
    destructive: bool,
) -> RegistryGovernanceActionLifecycle {
    RegistryGovernanceActionLifecycle {
        key: key.to_string(),
        reason_required,
        reason_code_required,
        reason_codes: reason_codes
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        destructive,
    }
}

#[cfg(feature = "ssr")]
pub fn derive_registry_governance_actions(
    latest_request: Option<&RegistryPublishRequestLifecycle>,
    latest_release: Option<&RegistryReleaseLifecycle>,
    owner_binding: Option<&RegistryOwnerLifecycle>,
    _validation_stages: &[RegistryValidationStageLifecycle],
) -> Vec<RegistryGovernanceActionLifecycle> {
    // Summary lifecycle stays actor-agnostic. Request-level interactive actions
    // are loaded separately from the authenticated publish status endpoint.
    let mut actions = Vec::new();

    if let Some(request) = latest_request {
        if request
            .publisher
            .as_ref()
            .is_some_and(|publisher| owner_binding.is_none_or(|owner| owner.owner != *publisher))
            || owner_binding.is_some()
        {
            actions.push(registry_governance_action(
                "owner_transfer",
                true,
                true,
                REGISTRY_OWNER_TRANSFER_REASON_CODES,
                true,
            ));
        }
    } else if owner_binding.is_some() {
        actions.push(registry_governance_action(
            "owner_transfer",
            true,
            true,
            REGISTRY_OWNER_TRANSFER_REASON_CODES,
            true,
        ));
    }

    if latest_release.is_some_and(|release| {
        release.status.eq_ignore_ascii_case("published")
            || release.status.eq_ignore_ascii_case("active")
    }) {
        actions.push(registry_governance_action(
            "yank",
            true,
            true,
            REGISTRY_YANK_REASON_CODES,
            true,
        ));
    }

    let mut seen = std::collections::HashSet::new();
    actions
        .into_iter()
        .filter(|action| seen.insert(action.key.clone()))
        .collect()
}

#[cfg(feature = "ssr")]
pub fn derive_registry_validation_stages(
    latest_request: Option<&RegistryPublishRequestLifecycle>,
    recent_events: &[RegistryGovernanceEventLifecycle],
    stage_rows: &[RegistryValidationStageLifecycle],
) -> Vec<RegistryValidationStageLifecycle> {
    let gate_keys = ["compile_smoke", "targeted_tests", "security_policy_review"];
    let mut stages = Vec::new();

    for gate_key in gate_keys {
        if let Some(stage) = stage_rows.iter().find(|stage| stage.key == gate_key) {
            stages.push(stage.clone());
            continue;
        }

        let latest_event = recent_events.iter().find(|event| {
            matches!(
                event.event_type.as_str(),
                "follow_up_gate_queued" | "follow_up_gate_passed" | "follow_up_gate_failed"
            ) && event.payload.stage_key.as_deref() == Some(gate_key)
        });

        if let Some(event) = latest_event {
            let status = match event.event_type.as_str() {
                "follow_up_gate_passed" => "passed",
                "follow_up_gate_failed" => "failed",
                _ => "queued",
            };
            stages.push(RegistryValidationStageLifecycle {
                key: gate_key.to_string(),
                status: status.to_string(),
                detail: event
                    .payload
                    .detail
                    .as_deref()
                    .unwrap_or_else(|| registry_follow_up_gate_detail(gate_key))
                    .to_string(),
                attempt_number: 0,
                updated_at: event.created_at.clone(),
                started_at: None,
                finished_at: None,
                execution_mode: fallback_validation_stage_execution_mode(gate_key).to_string(),
                runnable: matches!(
                    gate_key,
                    "compile_smoke" | "targeted_tests" | "security_policy_review"
                ),
                requires_manual_confirmation: gate_key == "security_policy_review",
                allowed_terminal_reason_codes: fallback_validation_stage_reason_codes(),
                suggested_pass_reason_code: fallback_validation_stage_pass_reason_code(gate_key)
                    .map(str::to_string),
                suggested_failure_reason_code: fallback_validation_stage_failure_reason_code(
                    gate_key,
                )
                .map(str::to_string),
                suggested_blocked_reason_code: fallback_validation_stage_blocked_reason_code(
                    gate_key,
                )
                .map(str::to_string),
            });
            continue;
        }

        if latest_request
            .is_some_and(|request| matches!(request.status.as_str(), "approved" | "published"))
        {
            stages.push(RegistryValidationStageLifecycle {
                key: gate_key.to_string(),
                status: "queued".to_string(),
                detail: registry_follow_up_gate_detail(gate_key).to_string(),
                attempt_number: 0,
                updated_at: latest_request
                    .map(|request| request.updated_at.clone())
                    .unwrap_or_default(),
                started_at: None,
                finished_at: None,
                execution_mode: fallback_validation_stage_execution_mode(gate_key).to_string(),
                runnable: matches!(
                    gate_key,
                    "compile_smoke" | "targeted_tests" | "security_policy_review"
                ),
                requires_manual_confirmation: gate_key == "security_policy_review",
                allowed_terminal_reason_codes: fallback_validation_stage_reason_codes(),
                suggested_pass_reason_code: fallback_validation_stage_pass_reason_code(gate_key)
                    .map(str::to_string),
                suggested_failure_reason_code: fallback_validation_stage_failure_reason_code(
                    gate_key,
                )
                .map(str::to_string),
                suggested_blocked_reason_code: fallback_validation_stage_blocked_reason_code(
                    gate_key,
                )
                .map(str::to_string),
            });
        }
    }

    stages
}

#[cfg(feature = "ssr")]
pub fn derive_registry_follow_up_gates(
    validation_stages: &[RegistryValidationStageLifecycle],
    latest_request: Option<&RegistryPublishRequestLifecycle>,
    recent_events: &[RegistryGovernanceEventLifecycle],
) -> Vec<RegistryFollowUpGateLifecycle> {
    if !validation_stages.is_empty() {
        return validation_stages
            .iter()
            .map(|stage| RegistryFollowUpGateLifecycle {
                key: stage.key.clone(),
                status: match stage.status.as_str() {
                    "queued" => "pending".to_string(),
                    other => other.to_string(),
                },
                detail: stage.detail.clone(),
                updated_at: stage.updated_at.clone(),
            })
            .collect();
    }

    let gate_keys = ["compile_smoke", "targeted_tests", "security_policy_review"];
    let mut gates = Vec::new();

    for gate_key in gate_keys {
        let latest_event = recent_events.iter().find(|event| {
            matches!(
                event.event_type.as_str(),
                "follow_up_gate_queued" | "follow_up_gate_passed" | "follow_up_gate_failed"
            ) && event.payload.stage_key.as_deref() == Some(gate_key)
        });

        if let Some(event) = latest_event {
            gates.push(RegistryFollowUpGateLifecycle {
                key: gate_key.to_string(),
                status: match event.event_type.as_str() {
                    "follow_up_gate_passed" => "passed".to_string(),
                    "follow_up_gate_failed" => "failed".to_string(),
                    _ => "pending".to_string(),
                },
                detail: event
                    .payload
                    .detail
                    .as_deref()
                    .unwrap_or_else(|| registry_follow_up_gate_detail(gate_key))
                    .to_string(),
                updated_at: event.created_at.clone(),
            });
            continue;
        }

        if latest_request
            .is_some_and(|request| matches!(request.status.as_str(), "approved" | "published"))
        {
            gates.push(RegistryFollowUpGateLifecycle {
                key: gate_key.to_string(),
                status: "pending".to_string(),
                detail: registry_follow_up_gate_detail(gate_key).to_string(),
                updated_at: latest_request
                    .map(|request| request.updated_at.clone())
                    .unwrap_or_default(),
            });
        }
    }

    gates
}

#[cfg(feature = "ssr")]
async fn load_registry_module_lifecycle(
    app_ctx: &ModulesServerRuntime,
    slug: &str,
) -> Result<Option<RegistryModuleLifecycle>, ServerFnError> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let backend = app_ctx.db.get_database_backend();
    let request_statement = match backend {
        DbBackend::Sqlite => Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            SELECT
                id,
                status,
                requested_by_principal AS requested_by,
                publisher_principal AS publisher,
                approved_by_principal AS approved_by,
                rejected_by_principal AS rejected_by,
                rejection_reason,
                changes_requested_by_principal AS changes_requested_by,
                changes_requested_reason,
                changes_requested_reason_code,
                changes_requested_at,
                held_by_principal AS held_by,
                held_reason,
                held_reason_code,
                held_at,
                held_from_status,
                validation_warnings,
                validation_errors,
                created_at,
                updated_at,
                published_at
            FROM registry_publish_requests
            WHERE slug = ?
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            [slug.into()],
        ),
        _ => Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT
                id,
                status,
                requested_by_principal AS requested_by,
                publisher_principal AS publisher,
                approved_by_principal AS approved_by,
                rejected_by_principal AS rejected_by,
                rejection_reason,
                changes_requested_by_principal AS changes_requested_by,
                changes_requested_reason,
                changes_requested_reason_code,
                changes_requested_at,
                held_by_principal AS held_by,
                held_reason,
                held_reason_code,
                held_at,
                held_from_status,
                validation_warnings,
                validation_errors,
                created_at,
                updated_at,
                published_at
            FROM registry_publish_requests
            WHERE slug = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            [slug.into()],
        ),
    };
    let release_statement = match backend {
        DbBackend::Sqlite => Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            SELECT
                version,
                status,
                publisher_principal AS publisher,
                checksum_sha256,
                published_at,
                yanked_reason,
                yanked_by_principal AS yanked_by,
                yanked_at
            FROM registry_module_releases
            WHERE slug = ?
            ORDER BY published_at DESC
            LIMIT 1
            "#,
            [slug.into()],
        ),
        _ => Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT
                version,
                status,
                publisher_principal AS publisher,
                checksum_sha256,
                published_at,
                yanked_reason,
                yanked_by_principal AS yanked_by,
                yanked_at
            FROM registry_module_releases
            WHERE slug = $1
            ORDER BY published_at DESC
            LIMIT 1
            "#,
            [slug.into()],
        ),
    };
    let owner_statement = match backend {
        DbBackend::Sqlite => Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            SELECT
                owner_principal AS owner,
                bound_by_principal AS bound_by,
                bound_at,
                updated_at
            FROM registry_module_owners
            WHERE slug = ?
            LIMIT 1
            "#,
            [slug.into()],
        ),
        _ => Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT
                owner_principal AS owner,
                bound_by_principal AS bound_by,
                bound_at,
                updated_at
            FROM registry_module_owners
            WHERE slug = $1
            LIMIT 1
            "#,
            [slug.into()],
        ),
    };
    let events_statement = match backend {
        DbBackend::Sqlite => Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            SELECT
                id,
                event_type,
                actor_principal AS actor,
                publisher_principal AS publisher,
                details,
                created_at
            FROM registry_governance_events
            WHERE slug = ?
            ORDER BY created_at DESC
            LIMIT 10
            "#,
            [slug.into()],
        ),
        _ => Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT
                id,
                event_type,
                actor_principal AS actor,
                publisher_principal AS publisher,
                details,
                created_at
            FROM registry_governance_events
            WHERE slug = $1
            ORDER BY created_at DESC
            LIMIT 10
            "#,
            [slug.into()],
        ),
    };
    let owner_binding = app_ctx
        .db
        .query_one(owner_statement)
        .await
        .map_err(|err| server_error(err.to_string()))?
        .map(map_registry_owner_row)
        .transpose()?;
    let latest_request = app_ctx
        .db
        .query_one(request_statement)
        .await
        .map_err(|err| server_error(err.to_string()))?
        .map(map_registry_publish_request_row)
        .transpose()?;
    let stage_statement = latest_request.as_ref().map(|request| match backend {
        DbBackend::Sqlite => Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
                SELECT
                    stage_key,
                    status,
                    detail,
                    attempt_number,
                    updated_at,
                    started_at,
                    finished_at
                FROM registry_validation_stages
                WHERE request_id = ?
                ORDER BY attempt_number DESC, created_at DESC
                "#,
            [request.id.clone().into()],
        ),
        _ => Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
                SELECT
                    stage_key,
                    status,
                    detail,
                    attempt_number,
                    updated_at,
                    started_at,
                    finished_at
                FROM registry_validation_stages
                WHERE request_id = $1
                ORDER BY attempt_number DESC, created_at DESC
                "#,
            [request.id.clone().into()],
        ),
    });
    let latest_release = app_ctx
        .db
        .query_one(release_statement)
        .await
        .map_err(|err| server_error(err.to_string()))?
        .map(map_registry_release_row)
        .transpose()?;
    let recent_events = app_ctx
        .db
        .query_all(events_statement)
        .await
        .map_err(|err| server_error(err.to_string()))?
        .into_iter()
        .map(map_registry_governance_event_row)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let validation_stage_rows = if let Some(stage_statement) = stage_statement {
        app_ctx
            .db
            .query_all(stage_statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .into_iter()
            .map(map_registry_validation_stage_row)
            .collect::<std::result::Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };

    if owner_binding.is_none()
        && latest_request.is_none()
        && latest_release.is_none()
        && recent_events.is_empty()
        && validation_stage_rows.is_empty()
    {
        return Ok(None);
    }

    let validation_stages = derive_registry_validation_stages(
        latest_request.as_ref(),
        &recent_events,
        &validation_stage_rows,
    );
    let follow_up_gates = derive_registry_follow_up_gates(
        &validation_stages,
        latest_request.as_ref(),
        &recent_events,
    );
    let governance_actions = derive_registry_governance_actions(
        latest_request.as_ref(),
        latest_release.as_ref(),
        owner_binding.as_ref(),
        &validation_stages,
    );

    Ok(Some(RegistryModuleLifecycle {
        moderation_policy: crate::entities::module::model::RegistryModerationPolicyLifecycle {
            mode: "registry_v2".to_string(),
            live_publish_supported: true,
            live_governance_supported: true,
            manual_review_required: true,
            restriction_reason_code: None,
            restriction_reason: String::new(),
        },
        owner_binding,
        latest_request,
        latest_release,
        recent_events,
        follow_up_gates,
        validation_stages,
        governance_actions,
    }))
}

#[server(prefix = "/api/fn", endpoint = "admin/list-enabled-modules")]
pub async fn list_enabled_modules_native() -> Result<Vec<String>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use leptos_axum::extract;
        use rustok_api::Permission;
        use rustok_api::{
            AuthContext, HostRuntimeContext, TenantContext, has_any_effective_permission,
        };
        use rustok_core::ModuleRegistry;

        let app_ctx = ModulesServerRuntime {
            db: expect_context::<HostRuntimeContext>().db_clone(),
        };
        let auth = extract::<AuthContext>().await.map_err(ServerFnError::new)?;
        let tenant = extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        if !has_any_effective_permission(
            &auth.permissions,
            &[
                Permission::MODULES_READ,
                Permission::MODULES_LIST,
                Permission::MODULES_MANAGE,
            ],
        ) {
            return Err(ServerFnError::new(
                "modules:read, modules:list, or modules:manage required",
            ));
        }

        let registry = expect_context::<ModuleRegistry>();
        let mut modules = effective_enabled_modules_native(&app_ctx.db, &registry, tenant.id)
            .await?
            .into_iter()
            .collect::<Vec<_>>();

        modules.sort();
        Ok(modules)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/list-enabled-modules requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/module-registry")]
pub async fn list_module_registry_native() -> Result<Vec<ModuleInfo>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::app::modules::module_runtime_metadata;
        use leptos::prelude::expect_context;
        use rustok_core::ModuleRegistry;

        let (app_ctx, _auth, tenant) = modules_server_context().await?;
        let registry = expect_context::<ModuleRegistry>();
        let enabled_modules =
            effective_enabled_modules_native(&app_ctx.db, &registry, tenant.id).await?;

        Ok(registry
            .list()
            .into_iter()
            .map(|module| {
                let metadata = module_runtime_metadata(module.slug());
                ModuleInfo {
                    module_slug: module.slug().to_string(),
                    name: module.name().to_string(),
                    description: module.description().to_string(),
                    version: module.version().to_string(),
                    kind: if registry.is_core(module.slug()) {
                        "core".to_string()
                    } else {
                        "optional".to_string()
                    },
                    dependencies: module
                        .dependencies()
                        .iter()
                        .map(|dependency| dependency.to_string())
                        .collect(),
                    enabled: registry.is_core(module.slug())
                        || enabled_modules.contains(module.slug()),
                    ownership: metadata
                        .map(|metadata| metadata.ownership.to_string())
                        .unwrap_or_else(|| "third_party".to_string()),
                    trust_level: metadata
                        .map(|metadata| metadata.trust_level.to_string())
                        .unwrap_or_else(|| "unverified".to_string()),
                    has_admin_ui: false,
                    has_storefront_ui: false,
                    ui_classification: "no-ui".to_string(),
                    recommended_admin_surfaces: metadata
                        .map(|metadata| {
                            metadata
                                .recommended_admin_surfaces
                                .iter()
                                .map(|surface| surface.to_string())
                                .collect()
                        })
                        .unwrap_or_default(),
                    showcase_admin_surfaces: metadata
                        .map(|metadata| {
                            metadata
                                .showcase_admin_surfaces
                                .iter()
                                .map(|surface| surface.to_string())
                                .collect()
                        })
                        .unwrap_or_default(),
                }
            })
            .collect())
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/module-registry requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/installed-modules")]
pub async fn list_installed_modules_native() -> Result<Vec<InstalledModule>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (app_ctx, _auth, _tenant) = modules_server_context().await?;
        let manifest = active_runtime_platform_snapshot(&app_ctx.db)
            .await?
            .manifest;

        let mut modules = manifest
            .modules
            .into_iter()
            .map(|(slug, spec)| InstalledModule {
                slug,
                source: spec.source,
                crate_name: spec.crate_name,
                version: spec.version,
                required: spec.required,
                dependencies: spec.depends_on,
            })
            .collect::<Vec<_>>();
        modules.sort_by(|left, right| left.slug.cmp(&right.slug));
        Ok(modules)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/installed-modules requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/list-tenant-modules")]
pub async fn list_tenant_modules_native() -> Result<Vec<TenantModule>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_tenant::TenantService;

        let (app_ctx, _auth, tenant) = modules_server_context().await?;

        TenantService::new(app_ctx.db.clone())
            .list_tenant_modules(tenant.id)
            .await
            .map(|modules| {
                let mut modules = modules
                    .into_iter()
                    .map(|module| TenantModule {
                        module_slug: module.module_slug,
                        enabled: module.enabled,
                        settings: module.settings.to_string(),
                    })
                    .collect::<Vec<_>>();
                modules.sort_by(|left, right| left.module_slug.cmp(&right.module_slug));
                modules
            })
            .map_err(|err| server_error(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/list-tenant-modules requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/marketplace")]
pub async fn list_marketplace_modules_native(
    search: Option<String>,
    category: Option<String>,
    tag: Option<String>,
    source: Option<String>,
    trust_level: Option<String>,
    only_compatible: Option<bool>,
    installed_only: Option<bool>,
) -> Result<Vec<MarketplaceModule>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_core::ModuleRegistry;

        let (app_ctx, _auth, _tenant) = modules_server_context().await?;
        let registry = expect_context::<ModuleRegistry>();
        let search = search
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_lowercase);
        let category = category
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_lowercase);
        let tag = tag
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_lowercase);
        let source = source
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_lowercase);
        let trust_level = trust_level
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_lowercase);
        let only_compatible = only_compatible.unwrap_or(true);
        let installed_only = installed_only.unwrap_or(false);
        let snapshot = active_runtime_platform_snapshot(&app_ctx.db).await?;

        Ok(
            load_runtime_marketplace_modules(&registry, &snapshot.manifest)?
                .into_iter()
                .filter(|module| module.kind == "optional")
                .filter(|module| !only_compatible || module.compatible || module.installed)
                .filter(|module| !installed_only || module.installed)
                .filter(|module| {
                    trust_level
                        .as_ref()
                        .is_none_or(|value| module.trust_level.eq_ignore_ascii_case(value))
                })
                .filter(|module| {
                    source
                        .as_ref()
                        .is_none_or(|value| module.source.eq_ignore_ascii_case(value))
                })
                .filter(|module| {
                    category
                        .as_ref()
                        .is_none_or(|value| module.category.eq_ignore_ascii_case(value))
                })
                .filter(|module| {
                    tag.as_ref().is_none_or(|value| {
                        module
                            .tags
                            .iter()
                            .any(|module_tag| module_tag.eq_ignore_ascii_case(value))
                    })
                })
                .filter(|module| {
                    search.as_ref().is_none_or(|value| {
                        module.slug.to_lowercase().contains(value)
                            || module.name.to_lowercase().contains(value)
                            || module.description.to_lowercase().contains(value)
                            || module.crate_name.to_lowercase().contains(value)
                    })
                })
                .collect(),
        )
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            search,
            category,
            tag,
            source,
            trust_level,
            only_compatible,
            installed_only,
        );
        Err(ServerFnError::new(
            "admin/marketplace requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/marketplace-module")]
pub async fn marketplace_module_native(
    slug: String,
) -> Result<Option<MarketplaceModule>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_core::ModuleRegistry;

        let (app_ctx, _auth, _tenant) = modules_server_context().await?;
        let registry = expect_context::<ModuleRegistry>();
        let slug = slug.trim().to_lowercase();
        let snapshot = active_runtime_platform_snapshot(&app_ctx.db).await?;
        let module = load_runtime_marketplace_modules(&registry, &snapshot.manifest)?
            .into_iter()
            .find(|module| module.slug.eq_ignore_ascii_case(&slug) && module.kind == "optional");

        match module {
            Some(mut module) => {
                module.registry_lifecycle =
                    load_registry_module_lifecycle(&app_ctx, &module.slug).await?;
                Ok(Some(module))
            }
            None => Ok(None),
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = slug;
        Err(ServerFnError::new(
            "admin/marketplace-module requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/active-build")]
pub async fn active_build_native() -> Result<Option<BuildJob>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use sea_orm::{ConnectionTrait, DbBackend, Statement};

        let (app_ctx, _auth, _tenant) = modules_server_context().await?;
        let backend = app_ctx.db.get_database_backend();
        let statement = match backend {
            DbBackend::Sqlite => Statement::from_string(
                DbBackend::Sqlite,
                r#"
                SELECT
                    id,
                    status,
                    stage,
                    progress,
                    profile,
                    manifest_ref,
                    manifest_hash,
                    modules_delta,
                    requested_by,
                    reason,
                    release_id,
                    logs_url,
                    error_message,
                    started_at,
                    created_at,
                    updated_at,
                    finished_at
                FROM builds
                WHERE status IN ('queued', 'running')
                ORDER BY created_at DESC
                LIMIT 1
                "#,
            ),
            _ => Statement::from_string(
                DbBackend::Postgres,
                r#"
                SELECT
                    id,
                    status,
                    stage,
                    progress,
                    profile,
                    manifest_ref,
                    manifest_hash,
                    modules_delta,
                    requested_by,
                    reason,
                    release_id,
                    logs_url,
                    error_message,
                    started_at,
                    created_at,
                    updated_at,
                    finished_at
                FROM builds
                WHERE status IN ('queued', 'running')
                ORDER BY created_at DESC
                LIMIT 1
                "#,
            ),
        };

        app_ctx
            .db
            .query_one(statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .map(map_build_job_row)
            .transpose()
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/active-build requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/active-release")]
pub async fn active_release_native() -> Result<Option<ReleaseInfo>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use sea_orm::{ConnectionTrait, DbBackend, Statement};

        let (app_ctx, _auth, _tenant) = modules_server_context().await?;
        let backend = app_ctx.db.get_database_backend();
        let statement = match backend {
            DbBackend::Sqlite => Statement::from_string(
                DbBackend::Sqlite,
                r#"
                SELECT
                    id,
                    build_id,
                    status,
                    environment,
                    manifest_hash,
                    modules,
                    previous_release_id,
                    deployed_at,
                    rolled_back_at,
                    created_at,
                    updated_at
                FROM releases
                WHERE status = 'active'
                ORDER BY updated_at DESC
                LIMIT 1
                "#,
            ),
            _ => Statement::from_string(
                DbBackend::Postgres,
                r#"
                SELECT
                    id,
                    build_id,
                    status,
                    environment,
                    manifest_hash,
                    modules,
                    previous_release_id,
                    deployed_at,
                    rolled_back_at,
                    created_at,
                    updated_at
                FROM releases
                WHERE status = 'active'
                ORDER BY updated_at DESC
                LIMIT 1
                "#,
            ),
        };

        app_ctx
            .db
            .query_one(statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .map(map_release_info_row)
            .transpose()
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/active-release requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/build-history")]
pub async fn build_history_native(limit: i32, offset: i32) -> Result<Vec<BuildJob>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use sea_orm::{ConnectionTrait, DbBackend, Statement};

        let (app_ctx, _auth, _tenant) = modules_server_context().await?;
        let backend = app_ctx.db.get_database_backend();
        let limit = limit.clamp(1, 100);
        let offset = offset.max(0);
        let statement = match backend {
            DbBackend::Sqlite => Statement::from_sql_and_values(
                DbBackend::Sqlite,
                r#"
                SELECT
                    id,
                    status,
                    stage,
                    progress,
                    profile,
                    manifest_ref,
                    manifest_hash,
                    modules_delta,
                    requested_by,
                    reason,
                    release_id,
                    logs_url,
                    error_message,
                    started_at,
                    created_at,
                    updated_at,
                    finished_at
                FROM builds
                ORDER BY created_at DESC
                LIMIT ?1
                OFFSET ?2
                "#,
                vec![limit.into(), offset.into()],
            ),
            _ => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    id,
                    status,
                    stage,
                    progress,
                    profile,
                    manifest_ref,
                    manifest_hash,
                    modules_delta,
                    requested_by,
                    reason,
                    release_id,
                    logs_url,
                    error_message,
                    started_at,
                    created_at,
                    updated_at,
                    finished_at
                FROM builds
                ORDER BY created_at DESC
                LIMIT $1
                OFFSET $2
                "#,
                vec![limit.into(), offset.into()],
            ),
        };

        app_ctx
            .db
            .query_all(statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .into_iter()
            .map(map_build_job_row)
            .collect()
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (limit, offset);
        Err(ServerFnError::new(
            "admin/build-history requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/update-module-settings")]
pub async fn update_module_settings_native(
    module_slug: String,
    settings: String,
) -> Result<TenantModule, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::Permission;
        use rustok_api::has_any_effective_permission;
        use rustok_core::ModuleRegistry;
        use rustok_tenant::entities::tenant_module;
        use rustok_tenant::entities::tenant_module::Entity as TenantModuleEntity;
        use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

        let (app_ctx, auth, tenant) = modules_server_context().await?;

        if !has_any_effective_permission(&auth.permissions, &[Permission::MODULES_MANAGE]) {
            return Err(ServerFnError::new("modules:manage required"));
        }

        let registry = expect_context::<ModuleRegistry>();
        if registry.get(&module_slug).is_none() {
            return Err(server_error("Unknown module"));
        }

        let raw_settings: serde_json::Value = serde_json::from_str(&settings)
            .map_err(|err| server_error(format!("invalid module settings JSON: {err}")))?;
        let snapshot = active_runtime_platform_snapshot(&app_ctx.db).await?;
        let schema =
            load_runtime_module_package_manifest_by_slug(&module_slug, &snapshot.manifest)?
                .map(|manifest| manifest.settings)
                .unwrap_or_default();
        let normalized_settings =
            normalize_runtime_module_settings(&module_slug, &schema, raw_settings)?;

        let existing = TenantModuleEntity::find()
            .filter(tenant_module::Column::TenantId.eq(tenant.id))
            .filter(tenant_module::Column::ModuleSlug.eq(&module_slug))
            .one(&app_ctx.db)
            .await
            .map_err(|err| server_error(err.to_string()))?;

        let is_core = registry.is_core(&module_slug);
        let model = match existing {
            Some(model) => {
                if !is_core && !model.enabled {
                    return Err(server_error(format!(
                        "Module '{module_slug}' is not enabled for this tenant"
                    )));
                }

                let was_enabled = model.enabled;
                let mut active: tenant_module::ActiveModel = model.into();
                active.enabled = Set(is_core || was_enabled);
                active.settings = Set(normalized_settings);
                active
                    .update(&app_ctx.db)
                    .await
                    .map_err(|err| server_error(err.to_string()))?
            }
            None if is_core => tenant_module::ActiveModel {
                id: Set(rustok_core::generate_id()),
                tenant_id: Set(tenant.id),
                module_slug: Set(module_slug.clone()),
                enabled: Set(true),
                settings: Set(normalized_settings),
                created_at: sea_orm::ActiveValue::NotSet,
                updated_at: sea_orm::ActiveValue::NotSet,
            }
            .insert(&app_ctx.db)
            .await
            .map_err(|err| server_error(err.to_string()))?,
            None => {
                return Err(server_error(format!(
                    "Module '{module_slug}' is not enabled for this tenant"
                )));
            }
        };

        Ok(TenantModule {
            module_slug: model.module_slug,
            enabled: model.enabled,
            settings: model.settings.to_string(),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (module_slug, settings);
        Err(ServerFnError::new(
            "admin/update-module-settings requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/rollback-build")]
pub async fn rollback_build_native(build_id: String) -> Result<BuildJob, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::Permission;
        use rustok_api::has_any_effective_permission;
        use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait};

        let (app_ctx, auth, _tenant) = modules_server_context().await?;

        if !has_any_effective_permission(&auth.permissions, &[Permission::MODULES_MANAGE]) {
            return Err(ServerFnError::new("modules:manage required"));
        }

        let build_id = uuid::Uuid::parse_str(build_id.trim())
            .map_err(|err| server_error(format!("invalid build id: {err}")))?;
        let backend = app_ctx.db.get_database_backend();

        let active_build_statement = match backend {
            DbBackend::Sqlite => Statement::from_string(
                DbBackend::Sqlite,
                r#"
                SELECT id
                FROM builds
                WHERE status IN ('queued', 'running')
                ORDER BY created_at DESC
                LIMIT 1
                "#,
            ),
            _ => Statement::from_string(
                DbBackend::Postgres,
                r#"
                SELECT id
                FROM builds
                WHERE status IN ('queued', 'running')
                ORDER BY created_at DESC
                LIMIT 1
                "#,
            ),
        };

        if app_ctx
            .db
            .query_one(active_build_statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .is_some()
        {
            return Err(server_error(
                "Cannot rollback while another build is still queued or running",
            ));
        }

        let build_query = match backend {
            DbBackend::Sqlite => Statement::from_sql_and_values(
                DbBackend::Sqlite,
                r#"
                SELECT
                    id,
                    status,
                    stage,
                    progress,
                    profile,
                    manifest_ref,
                    manifest_hash,
                    modules_delta,
                    requested_by,
                    reason,
                    release_id,
                    logs_url,
                    error_message,
                    started_at,
                    created_at,
                    updated_at,
                    finished_at
                FROM builds
                WHERE id = ?1
                LIMIT 1
                "#,
                vec![build_id.into()],
            ),
            _ => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    id,
                    status,
                    stage,
                    progress,
                    profile,
                    manifest_ref,
                    manifest_hash,
                    modules_delta,
                    requested_by,
                    reason,
                    release_id,
                    logs_url,
                    error_message,
                    started_at,
                    created_at,
                    updated_at,
                    finished_at
                FROM builds
                WHERE id = $1
                LIMIT 1
                "#,
                vec![build_id.into()],
            ),
        };

        let build_row = app_ctx
            .db
            .query_one(build_query)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .ok_or_else(|| server_error("Build not found"))?;

        let release_id = build_row
            .try_get::<Option<String>>("", "release_id")
            .map_err(|err| server_error(err.to_string()))?
            .ok_or_else(|| server_error("Build does not have a release to rollback"))?;

        let active_release_statement = match backend {
            DbBackend::Sqlite => Statement::from_string(
                DbBackend::Sqlite,
                r#"
                SELECT id, build_id, previous_release_id
                FROM releases
                WHERE status = 'active'
                LIMIT 1
                "#,
            ),
            _ => Statement::from_string(
                DbBackend::Postgres,
                r#"
                SELECT id, build_id, previous_release_id
                FROM releases
                WHERE status = 'active'
                LIMIT 1
                "#,
            ),
        };

        let active_release = app_ctx
            .db
            .query_one(active_release_statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .ok_or_else(|| server_error("No active release available for rollback"))?;

        let active_release_id = active_release
            .try_get::<String>("", "id")
            .map_err(|err| server_error(err.to_string()))?;
        if active_release_id != release_id {
            return Err(server_error(
                "Only the build that backs the current active release can be rolled back",
            ));
        }

        let previous_release_id = active_release
            .try_get::<Option<String>>("", "previous_release_id")
            .map_err(|err| server_error(err.to_string()))?
            .ok_or_else(|| server_error("No previous release available for rollback"))?;

        let previous_release_statement = match backend {
            DbBackend::Sqlite => Statement::from_sql_and_values(
                DbBackend::Sqlite,
                r#"
                SELECT id, build_id
                FROM releases
                WHERE id = ?1
                LIMIT 1
                "#,
                vec![previous_release_id.clone().into()],
            ),
            _ => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT id, build_id
                FROM releases
                WHERE id = $1
                LIMIT 1
                "#,
                vec![previous_release_id.clone().into()],
            ),
        };

        let previous_release = app_ctx
            .db
            .query_one(previous_release_statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .ok_or_else(|| server_error("Previous release not found"))?;
        let restored_build_id = previous_release
            .try_get::<uuid::Uuid>("", "build_id")
            .map_err(|err| server_error(err.to_string()))?;

        let now = chrono::Utc::now();
        app_ctx
            .db
            .transaction::<_, (), sea_orm::DbErr>(move |txn| {
                let active_release_id = active_release_id.clone();
                let previous_release_id = previous_release_id.clone();
                Box::pin(async move {
                    let rollback_current = match backend {
                        DbBackend::Sqlite => Statement::from_sql_and_values(
                            DbBackend::Sqlite,
                            r#"
                            UPDATE releases
                            SET status = 'rolled_back', rolled_back_at = ?2, updated_at = ?2
                            WHERE id = ?1
                            "#,
                            vec![active_release_id.into(), now.into()],
                        ),
                        _ => Statement::from_sql_and_values(
                            DbBackend::Postgres,
                            r#"
                            UPDATE releases
                            SET status = 'rolled_back', rolled_back_at = $2, updated_at = $2
                            WHERE id = $1
                            "#,
                            vec![active_release_id.into(), now.into()],
                        ),
                    };
                    txn.execute(rollback_current).await?;

                    let activate_previous = match backend {
                        DbBackend::Sqlite => Statement::from_sql_and_values(
                            DbBackend::Sqlite,
                            r#"
                            UPDATE releases
                            SET status = 'active', deployed_at = ?2, updated_at = ?2
                            WHERE id = ?1
                            "#,
                            vec![previous_release_id.into(), now.into()],
                        ),
                        _ => Statement::from_sql_and_values(
                            DbBackend::Postgres,
                            r#"
                            UPDATE releases
                            SET status = 'active', deployed_at = $2, updated_at = $2
                            WHERE id = $1
                            "#,
                            vec![previous_release_id.into(), now.into()],
                        ),
                    };
                    txn.execute(activate_previous).await?;
                    Ok(())
                })
            })
            .await
            .map_err(|err| match err {
                sea_orm::TransactionError::Connection(db_err) => server_error(db_err.to_string()),
                sea_orm::TransactionError::Transaction(db_err) => server_error(db_err.to_string()),
            })?;

        let restored_build_statement = match backend {
            DbBackend::Sqlite => Statement::from_sql_and_values(
                DbBackend::Sqlite,
                r#"
                SELECT
                    id,
                    status,
                    stage,
                    progress,
                    profile,
                    manifest_ref,
                    manifest_hash,
                    modules_delta,
                    requested_by,
                    reason,
                    release_id,
                    logs_url,
                    error_message,
                    started_at,
                    created_at,
                    updated_at,
                    finished_at
                FROM builds
                WHERE id = ?1
                LIMIT 1
                "#,
                vec![restored_build_id.into()],
            ),
            _ => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    id,
                    status,
                    stage,
                    progress,
                    profile,
                    manifest_ref,
                    manifest_hash,
                    modules_delta,
                    requested_by,
                    reason,
                    release_id,
                    logs_url,
                    error_message,
                    started_at,
                    created_at,
                    updated_at,
                    finished_at
                FROM builds
                WHERE id = $1
                LIMIT 1
                "#,
                vec![restored_build_id.into()],
            ),
        };

        let restored_build = app_ctx
            .db
            .query_one(restored_build_statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .ok_or_else(|| server_error("restored release is missing its build record"))?;

        map_build_job_row(restored_build)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = build_id;
        Err(ServerFnError::new(
            "admin/rollback-build requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "admin/registry-fetch-publish-request-status"
)]
pub async fn fetch_registry_publish_request_status_native(
    token: String,
    tenant: String,
    request_id: String,
) -> Result<RegistryPublishStatusContract, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        registry_governance_get_native(format!("/v2/catalog/publish/{request_id}"), token, tenant)
            .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant, request_id);
        Err(ServerFnError::new(
            "admin/registry-fetch-publish-request-status requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "admin/registry-validate-publish-request"
)]
pub async fn validate_registry_publish_request_native(
    token: String,
    tenant: String,
    request_id: String,
    dry_run: bool,
) -> Result<RegistryMutationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        registry_governance_request_native(
            reqwest::Method::POST,
            format!("/v2/catalog/publish/{request_id}/validate"),
            token,
            tenant,
            &RegistryValidationRequestPayload {
                schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
                dry_run,
            },
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant, request_id, dry_run);
        Err(ServerFnError::new(
            "admin/registry-validate-publish-request requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "admin/registry-approve-publish-request"
)]
pub async fn approve_registry_publish_request_native(
    token: String,
    tenant: String,
    request_id: String,
    reason: Option<String>,
    reason_code: Option<String>,
    dry_run: bool,
) -> Result<RegistryMutationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        registry_governance_request_native(
            reqwest::Method::POST,
            format!("/v2/catalog/publish/{request_id}/approve"),
            token,
            tenant,
            &RegistryDecisionRequestPayload {
                schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
                dry_run,
                reason,
                reason_code,
            },
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant, request_id, reason, reason_code, dry_run);
        Err(ServerFnError::new(
            "admin/registry-approve-publish-request requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/registry-reject-publish-request")]
pub async fn reject_registry_publish_request_native(
    token: String,
    tenant: String,
    request_id: String,
    reason: String,
    reason_code: String,
    dry_run: bool,
) -> Result<RegistryMutationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        registry_governance_request_native(
            reqwest::Method::POST,
            format!("/v2/catalog/publish/{request_id}/reject"),
            token,
            tenant,
            &RegistryDecisionRequestPayload {
                schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
                dry_run,
                reason: Some(reason),
                reason_code: Some(reason_code),
            },
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant, request_id, reason, reason_code, dry_run);
        Err(ServerFnError::new(
            "admin/registry-reject-publish-request requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "admin/registry-request-changes-publish-request"
)]
pub async fn request_changes_registry_publish_request_native(
    token: String,
    tenant: String,
    request_id: String,
    reason: String,
    reason_code: String,
    dry_run: bool,
) -> Result<RegistryMutationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        registry_governance_request_native(
            reqwest::Method::POST,
            format!("/v2/catalog/publish/{request_id}/request-changes"),
            token,
            tenant,
            &RegistryDecisionRequestPayload {
                schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
                dry_run,
                reason: Some(reason),
                reason_code: Some(reason_code),
            },
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant, request_id, reason, reason_code, dry_run);
        Err(ServerFnError::new(
            "admin/registry-request-changes-publish-request requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/registry-hold-publish-request")]
pub async fn hold_registry_publish_request_native(
    token: String,
    tenant: String,
    request_id: String,
    reason: String,
    reason_code: String,
    dry_run: bool,
) -> Result<RegistryMutationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        registry_governance_request_native(
            reqwest::Method::POST,
            format!("/v2/catalog/publish/{request_id}/hold"),
            token,
            tenant,
            &RegistryDecisionRequestPayload {
                schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
                dry_run,
                reason: Some(reason),
                reason_code: Some(reason_code),
            },
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant, request_id, reason, reason_code, dry_run);
        Err(ServerFnError::new(
            "admin/registry-hold-publish-request requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/registry-resume-publish-request")]
pub async fn resume_registry_publish_request_native(
    token: String,
    tenant: String,
    request_id: String,
    reason: String,
    reason_code: String,
    dry_run: bool,
) -> Result<RegistryMutationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        registry_governance_request_native(
            reqwest::Method::POST,
            format!("/v2/catalog/publish/{request_id}/resume"),
            token,
            tenant,
            &RegistryDecisionRequestPayload {
                schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
                dry_run,
                reason: Some(reason),
                reason_code: Some(reason_code),
            },
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant, request_id, reason, reason_code, dry_run);
        Err(ServerFnError::new(
            "admin/registry-resume-publish-request requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/registry-transfer-owner")]
pub async fn transfer_registry_owner_native(
    token: String,
    tenant: String,
    slug: String,
    new_owner_user_id: String,
    reason: String,
    reason_code: String,
    dry_run: bool,
) -> Result<RegistryMutationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        registry_governance_request_native(
            reqwest::Method::POST,
            "/v2/catalog/owner-transfer".to_string(),
            token,
            tenant,
            &RegistryOwnerTransferPayload {
                schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
                dry_run,
                slug,
                new_owner_user_id,
                reason: Some(reason),
                reason_code: Some(reason_code),
            },
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            token,
            tenant,
            slug,
            new_owner_user_id,
            reason,
            reason_code,
            dry_run,
        );
        Err(ServerFnError::new(
            "admin/registry-transfer-owner requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/registry-yank-release")]
pub async fn yank_registry_release_native(
    token: String,
    tenant: String,
    slug: String,
    version: String,
    reason: String,
    reason_code: String,
    dry_run: bool,
) -> Result<RegistryMutationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        registry_governance_request_native(
            reqwest::Method::POST,
            "/v2/catalog/yank".to_string(),
            token,
            tenant,
            &RegistryYankPayload {
                schema_version: REGISTRY_MUTATION_SCHEMA_VERSION,
                dry_run,
                slug,
                version,
                reason: Some(reason),
                reason_code: Some(reason_code),
            },
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant, slug, version, reason, reason_code, dry_run);
        Err(ServerFnError::new(
            "admin/registry-yank-release requires the `ssr` feature",
        ))
    }
}
