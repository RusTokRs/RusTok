use sea_orm::{ActiveModelTrait, Set, TransactionTrait};
use serde_json::Value;
use uuid::Uuid;

use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::models::platform_settings::{self, ActiveModel, Entity};
use crate::services::server_runtime_context::ServerRuntimeContext;

/// Known generic platform-setting categories.
///
/// Search settings are intentionally absent. `rustok-search` owns its settings
/// through `SearchSettingsService`; the generic platform table must not expose or
/// persist runtime connector credentials such as `search.api_key`.
pub mod category {
    pub const GENERAL: &str = "general";
    pub const EMAIL: &str = "email";
    pub const RATE_LIMIT: &str = "rate_limit";
    pub const FEATURES: &str = "features";
    pub const I18N: &str = "i18n";
    pub const OAUTH: &str = "oauth";
    pub const CACHE: &str = "cache";

    pub const ALL: &[&str] = &[GENERAL, EMAIL, RATE_LIMIT, FEATURES, I18N, OAUTH, CACHE];
}

#[derive(Debug)]
pub enum SettingsError {
    InvalidCategory(String),
    ValidationFailed(Vec<String>),
    Db(sea_orm::DbErr),
    Json(serde_json::Error),
}

impl From<sea_orm::DbErr> for SettingsError {
    fn from(e: sea_orm::DbErr) -> Self {
        Self::Db(e)
    }
}

impl From<serde_json::Error> for SettingsError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCategory(c) => write!(f, "Invalid settings category: {c}"),
            Self::ValidationFailed(errs) => write!(f, "Validation failed: {}", errs.join("; ")),
            Self::Db(e) => write!(f, "Database error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
        }
    }
}

/// Trait for validating a specific settings category.
pub trait SettingsValidator: Send + Sync {
    fn category(&self) -> &str;
    fn validate(&self, settings: &Value) -> Result<(), Vec<String>>;
}

/// Built-in validator for the `rate_limit` category.
pub struct RateLimitSettingsValidator;

impl SettingsValidator for RateLimitSettingsValidator {
    fn category(&self) -> &str {
        category::RATE_LIMIT
    }

    fn validate(&self, settings: &Value) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if let Some(rps) = settings.get("requests_per_second") {
            if let Some(n) = rps.as_f64() {
                if n <= 0.0 || !n.is_finite() {
                    errors.push("requests_per_second must be positive".to_string());
                }
            } else {
                errors.push("requests_per_second must be a number".to_string());
            }
        }

        if let Some(burst) = settings.get("burst_size") {
            if let Some(n) = burst.as_u64() {
                if n == 0 {
                    errors.push("burst_size must be greater than 0".to_string());
                }
            } else {
                errors.push("burst_size must be a non-negative integer".to_string());
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Built-in validator for the `email` category.
pub struct EmailSettingsValidator;

impl SettingsValidator for EmailSettingsValidator {
    fn category(&self) -> &str {
        category::EMAIL
    }

    fn validate(&self, settings: &Value) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if let Some(from_val) = settings.get("from") {
            if let Some(from) = from_val.as_str() {
                let from = from.trim();
                if from.is_empty() || !from.contains('@') {
                    errors.push("email.from must be a valid email address".to_string());
                }
            } else {
                errors.push("email.from must be a string".to_string());
            }
        }

        if let Some(provider_val) = settings.get("provider") {
            if let Some(provider) = provider_val.as_str() {
                if !matches!(provider, "smtp" | "sendgrid" | "mailgun" | "ses" | "none") {
                    errors.push(format!(
                        "email.provider must be one of: smtp, sendgrid, mailgun, ses, none; got '{provider}'"
                    ));
                }
            } else {
                errors.push("email.provider must be a string".to_string());
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Built-in validator for the `cache` category.
pub struct CacheSettingsValidator;

impl SettingsValidator for CacheSettingsValidator {
    fn category(&self) -> &str {
        category::CACHE
    }

    fn validate(&self, settings: &Value) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if let Some(port) = settings.get("redis_port") {
            if let Some(n) = port.as_i64() {
                if !(1..=65535).contains(&n) {
                    errors.push("redis_port must be between 1 and 65535".to_string());
                }
            } else {
                errors.push("redis_port must be an integer".to_string());
            }
        }

        if let Some(db) = settings.get("redis_db") {
            if let Some(n) = db.as_i64() {
                if !(0..=255).contains(&n) {
                    errors.push("redis_db must be between 0 and 255".to_string());
                }
            } else {
                errors.push("redis_db must be an integer".to_string());
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        let parsed: rustok_cache::CacheSettings = serde_json::from_value(settings.clone())
            .map_err(|e| vec![format!("Invalid cache settings schema: {e}")])?;
        parsed.validate()
    }
}

/// Registry of validators indexed by category.
pub struct ValidatorRegistry {
    validators: Vec<Box<dyn SettingsValidator>>,
}

impl Default for ValidatorRegistry {
    fn default() -> Self {
        let mut reg = Self {
            validators: Vec::new(),
        };
        reg.register(RateLimitSettingsValidator);
        reg.register(EmailSettingsValidator);
        reg.register(CacheSettingsValidator);
        reg
    }
}

impl ValidatorRegistry {
    pub fn register(&mut self, v: impl SettingsValidator + 'static) {
        self.validators.push(Box::new(v));
    }

    pub fn validate(&self, cat: &str, settings: &Value) -> Result<(), Vec<String>> {
        for v in &self.validators {
            if v.category() == cat {
                return v.validate(settings);
            }
        }
        // Categories without a validator pass through
        Ok(())
    }
}

/// Platform settings service.
///
/// Reading uses a three-level fallback:
/// 1. `platform_settings` table (per-tenant DB override)
/// 2. YAML `settings.rustok.<category>` (bootstrap defaults from config)
/// 3. Compiled-in defaults (`serde_json::Value::Object {}`)
pub struct SettingsService;

impl SettingsService {
    /// Get settings for a single supported generic category with fallback.
    pub async fn get(
        ctx: &ServerRuntimeContext,
        tenant_id: Uuid,
        cat: &str,
    ) -> Result<Value, SettingsError> {
        let cat = ensure_supported_category(cat)?;

        tracing::debug!(
            service = "settings",
            tenant_id = %tenant_id,
            category = cat,
            "Retrieving platform settings"
        );

        // 1. DB row
        //
        // The `cache` category is the one exception: its Redis target is deployment
        // configuration (`rustok.cache.redis_url` / `RUSTOK_REDIS_URL` / `REDIS_URL`) that the
        // runtime resolved at startup, and the runtime never reads this table. Reporting a stored
        // row would therefore describe a target the process does not use, so the deployment
        // document is the only truthful answer (see `prepare_cache_settings_for_write`).
        if cat == category::CACHE {
            return Ok(redact_secrets(cat, Self::yaml_defaults_for(ctx, cat)));
        }

        if let Some(row) = Entity::find_by_category(ctx.db(), tenant_id, cat).await? {
            return Ok(redact_secrets(cat, row.settings));
        }

        // 2. YAML
        let yaml_value = Self::yaml_defaults_for(ctx, cat);
        if !yaml_value.is_null() {
            return Ok(redact_secrets(cat, yaml_value));
        }

        // 3. Empty object default
        Ok(serde_json::json!({}))
    }

    /// List all supported generic categories for a tenant, filling gaps with fallbacks.
    ///
    /// Historical rows for owner-specific categories are ignored rather than exposed.
    pub async fn get_all(
        ctx: &ServerRuntimeContext,
        tenant_id: Uuid,
    ) -> Result<Vec<(String, Value)>, SettingsError> {
        tracing::debug!(
            service = "settings",
            tenant_id = %tenant_id,
            "Listing all platform settings"
        );

        let db_rows = Entity::find_all_for_tenant(ctx.db(), tenant_id).await?;
        let mut result: Vec<(String, Value)> = db_rows
            .into_iter()
            .filter(|row| category::ALL.contains(&row.category.as_str()))
            .map(|row| {
                let category = row.category;
                let settings = redact_secrets(&category, row.settings);
                (category, settings)
            })
            .collect();

        // Fill in categories that are not yet in the DB
        let existing: std::collections::HashSet<String> =
            result.iter().map(|(c, _)| c.clone()).collect();

        for &cat in category::ALL {
            if !existing.contains(cat) {
                let v = Self::yaml_defaults_for(ctx, cat);
                let settings = if v.is_null() {
                    serde_json::json!({})
                } else {
                    redact_secrets(cat, v)
                };
                result.push((cat.to_string(), settings));
            }
        }

        // Keep the aggregate view consistent with `get`: the `cache` target is deployment-owned,
        // so a stored row must not be presented as the effective configuration.
        for entry in result.iter_mut() {
            if entry.0 == category::CACHE {
                entry.1 =
                    redact_secrets(category::CACHE, Self::yaml_defaults_for(ctx, category::CACHE));
            }
        }

        result.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(result)
    }

    /// Upsert settings for a supported generic category.
    ///
    /// Persists settings and outbox domain event atomically within a transaction.
    /// Returns the stored and redacted `Value`.
    pub async fn update(
        ctx: &ServerRuntimeContext,
        event_bus: &TransactionalEventBus,
        tenant_id: Uuid,
        cat: &str,
        settings: Value,
        actor_id: Option<Uuid>,
        validators: &ValidatorRegistry,
    ) -> Result<Value, SettingsError> {
        let cat = ensure_supported_category(cat)?;

        if !settings.is_object() {
            return Err(SettingsError::ValidationFailed(vec![
                "Settings payload must be a JSON object".to_string(),
            ]));
        }

        tracing::debug!(
            service = "settings",
            tenant_id = %tenant_id,
            category = cat,
            ?actor_id,
            "Updating platform settings"
        );

        let settings = preserve_secrets(ctx, tenant_id, cat, settings).await?;

        validators
            .validate(cat, &settings)
            .map_err(SettingsError::ValidationFailed)?;

        let settings = if cat == category::CACHE {
            prepare_cache_settings_for_write(ctx, settings)?
        } else {
            settings
        };

        let tx = ctx.db().begin().await?;

        match Entity::find_by_category(&tx, tenant_id, cat).await? {
            Some(existing) => {
                let next_version = existing.schema_version.saturating_add(1);
                let mut active: platform_settings::ActiveModel = existing.into();
                active.settings = Set(settings.clone());
                active.updated_by = Set(actor_id);
                active.schema_version = Set(next_version);
                active.update(&tx).await?;
            }
            None => {
                ActiveModel::new(tenant_id, cat, settings.clone(), actor_id)
                    .insert(&tx)
                    .await?;
            }
        }

        event_bus
            .publish_in_tx(
                &tx,
                tenant_id,
                actor_id,
                DomainEvent::PlatformSettingsChanged {
                    category: cat.to_string(),
                    changed_by: actor_id.unwrap_or_default(),
                },
            )
            .await
            .map_err(|e| {
                sea_orm::DbErr::Custom(format!("Failed to publish settings event to outbox: {e}"))
            })?;

        tx.commit().await?;

        Ok(redact_secrets(cat, settings))
    }

    // ── Private helpers ────────────────────────────────────────────────────

    fn yaml_defaults_for(ctx: &ServerRuntimeContext, cat: &str) -> Value {
        let rs = ctx.settings();
        match cat {
            category::EMAIL => serde_json::to_value(&rs.email).unwrap_or(Value::Null),
            category::RATE_LIMIT => serde_json::to_value(&rs.rate_limit).unwrap_or(Value::Null),
            category::FEATURES => serde_json::to_value(&rs.features).unwrap_or(Value::Null),
            category::CACHE => serde_json::to_value(rustok_cache::CacheSettings::for_deployment(
                rs.cache.redis_url.as_deref(),
            ))
            .unwrap_or(Value::Null),
            _ => Value::Null,
        }
    }
}

fn redact_secrets(cat: &str, mut settings: Value) -> Value {
    match cat {
        category::EMAIL => {
            if let Some(object) = settings.as_object_mut() {
                if object.contains_key("smtp_password") {
                    object.insert("smtp_password".to_string(), Value::String(String::new()));
                }
                if object.contains_key("smtpPassword") {
                    object.insert("smtpPassword".to_string(), Value::String(String::new()));
                }
                if let Some(smtp) = object.get_mut("smtp").and_then(Value::as_object_mut)
                    && smtp.contains_key("password")
                {
                    smtp.insert("password".to_string(), Value::String(String::new()));
                }
            }
        }
        category::CACHE => {
            if let Some(object) = settings.as_object_mut() {
                if object.contains_key("redis_password") {
                    object.insert("redis_password".to_string(), Value::String(String::new()));
                }
                if object.contains_key("redisPassword") {
                    object.insert("redisPassword".to_string(), Value::String(String::new()));
                }
                if let Some(redis_url) = object.get("redis_url").and_then(Value::as_str) {
                    // The crate owns how a Redis URL is stripped: its helpers fail closed (they
                    // return an empty URL rather than a URL that still carries the password).
                    let redacted = rustok_cache::redis_url_without_password(redis_url);
                    if redacted != redis_url {
                        object.insert("redis_url".to_string(), Value::String(redacted));
                    }
                }
            }
        }
        _ => {}
    }

    settings
}

async fn preserve_secrets(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    cat: &str,
    incoming: Value,
) -> Result<Value, SettingsError> {
    if cat != category::EMAIL && cat != category::CACHE {
        return Ok(incoming);
    }

    let Some(existing) = Entity::find_by_category(ctx.db(), tenant_id, cat).await? else {
        return Ok(incoming);
    };

    match cat {
        category::EMAIL => Ok(preserve_email_secret_fields(existing.settings, incoming)),
        category::CACHE => Ok(preserve_cache_secret_fields(existing.settings, incoming)),
        _ => Ok(incoming),
    }
}

fn preserve_cache_secret_fields(existing: Value, mut incoming: Value) -> Value {
    let (Some(existing_object), Some(incoming_object)) =
        (existing.as_object(), incoming.as_object_mut())
    else {
        return incoming;
    };

    // Secrets belong to a *target*, not to a document. `rustok_cache::same_redis_target` is the
    // canonical identity (the password is excluded, scheme/host/port/database/username are not), so
    // a save that redirects the cache to another target can never inherit the stored password and a
    // redaction mask is never persisted as if it were the credential.
    let existing_url = existing_object
        .get("redis_url")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let incoming_url = incoming_object
        .get("redis_url")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let same_target = !existing_url.is_empty()
        && !incoming_url.is_empty()
        && rustok_cache::same_redis_target(incoming_url, existing_url);

    if !same_target {
        let masked = Value::String(rustok_cache::REDACTED_SECRET.to_string());
        for key in ["redis_password", "redisPassword"] {
            if incoming_object.get(key) == Some(&masked) {
                incoming_object.insert(key.to_string(), Value::String(String::new()));
            }
        }
        return incoming;
    }

    // Reads redact the password inside `redis_url` (see `redact_secrets`). Without this step a
    // save that only touches another field would persist the redacted URL and destroy the secret.
    let restored_url = existing_object
        .get("redis_url")
        .and_then(Value::as_str)
        .and_then(|existing_url| {
            incoming_object
                .get("redis_url")
                .and_then(Value::as_str)
                .and_then(|incoming_url| {
                    rustok_cache::preserve_url_password(incoming_url, existing_url)
                })
        });
    if let Some(restored_url) = restored_url {
        incoming_object.insert("redis_url".to_string(), Value::String(restored_url));
    }

    let existing_password = existing_object
        .get("redis_password")
        .or_else(|| existing_object.get("redisPassword"))
        .filter(|value| !value.as_str().unwrap_or_default().is_empty())
        .filter(|value| value.as_str() != Some(rustok_cache::REDACTED_SECRET))
        .cloned();

    if let Some(existing_password) = existing_password {
        for key in ["redis_password", "redisPassword"] {
            let is_unset = incoming_object
                .get(key)
                .and_then(Value::as_str)
                .is_some_and(|value| {
                    value.is_empty() || value == rustok_cache::REDACTED_SECRET
                });
            if is_unset {
                incoming_object.insert(key.to_string(), existing_password.clone());
            }
        }
    } else {
        // Nothing to restore: clear a mask so it is not stored as a credential.
        for key in ["redis_password", "redisPassword"] {
            if incoming_object
                .get(key)
                .and_then(Value::as_str)
                .is_some_and(|value| value == rustok_cache::REDACTED_SECRET)
            {
                incoming_object.insert(key.to_string(), Value::String(String::new()));
            }
        }
    }

    incoming
}

/// Deployment-resolved cache settings: the only truthful description of the runtime target.
fn deployment_cache_settings(ctx: &ServerRuntimeContext) -> rustok_cache::CacheSettings {
    rustok_cache::CacheSettings::for_deployment(ctx.settings().cache.redis_url.as_deref())
}

/// Redis target of a settings document, with the password stripped.
///
/// Passwords are secrets, not identity: a document that keeps pointing at the same target may
/// carry a rotated password, but anything that changes scheme/host/port/database/mode is a
/// different target and must be rejected for the deployment-owned cache category.
fn cache_target_signature(settings: &rustok_cache::CacheSettings) -> (String, Option<String>) {
    let target = settings
        .resolved_url()
        .map(|url| rustok_cache::redis_url_without_credentials(&url));
    (settings.mode.clone(), target)
}

/// Restore the deployment password and refuse to redirect a running process.
///
/// The runtime resolves its Redis client once, at startup, from deployment configuration. A
/// `cache` settings write therefore may only mirror that target; anything else would be a silent
/// no-op, which is exactly the bug this helper removes.
fn prepare_cache_settings_for_write(
    ctx: &ServerRuntimeContext,
    mut incoming: Value,
) -> Result<Value, SettingsError> {
    let deployment = deployment_cache_settings(ctx);
    let deployment_value = serde_json::to_value(&deployment).ok();

    if let Some(incoming_object) = incoming.as_object_mut() {
        let deployment_url = deployment_value
            .as_ref()
            .and_then(|value| value.get("redis_url"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let restored = incoming_object
            .get("redis_url")
            .and_then(Value::as_str)
            .and_then(|incoming_url| {
                rustok_cache::preserve_url_password(incoming_url, &deployment_url)
            });
        if let Some(restored) = restored {
            incoming_object.insert("redis_url".to_string(), Value::String(restored));
        }
    }

    let parsed: rustok_cache::CacheSettings = serde_json::from_value(incoming.clone())
        .map_err(|error| {
            SettingsError::ValidationFailed(vec![format!(
                "Invalid cache settings schema: {error}"
            )])
        })?;

    if cache_target_signature(&parsed) != cache_target_signature(&deployment) {
        return Err(SettingsError::ValidationFailed(vec![
            "cache Redis target is deployment configuration (rustok.cache.redis_url / \
             RUSTOK_REDIS_URL / REDIS_URL): change it there and restart, the settings API cannot \
             redirect a running cache runtime"
                .to_string(),
        ]));
    }

    // The stored row is an audit mirror that the runtime never reads, so it must not become a
    // second copy of the deployment secret. The response is redacted by `redact_secrets` anyway;
    // this keeps the *stored* document free of credentials.
    let mut storable = parsed;
    storable.redis_password.clear();
    let storable_url = rustok_cache::redis_url_without_credentials(storable.redis_url.trim());
    storable.redis_url = storable_url;
    serde_json::to_value(&storable).map_err(|error| {
        SettingsError::ValidationFailed(vec![format!(
            "Invalid cache settings schema: {error}"
        )])
    })
}

fn preserve_email_secret_fields(existing: Value, mut incoming: Value) -> Value {
    let (Some(existing_object), Some(incoming_object)) =
        (existing.as_object(), incoming.as_object_mut())
    else {
        return incoming;
    };

    if incoming_object
        .get("smtp_password")
        .and_then(Value::as_str)
        .is_some_and(str::is_empty)
        && let Some(existing_password) = existing_object
            .get("smtp_password")
            .or_else(|| existing_object.get("smtpPassword"))
            .filter(|value| !value.as_str().unwrap_or_default().is_empty())
    {
        incoming_object.insert("smtp_password".to_string(), existing_password.clone());
    }

    if incoming_object
        .get("smtpPassword")
        .and_then(Value::as_str)
        .is_some_and(str::is_empty)
        && let Some(existing_password) = existing_object
            .get("smtp_password")
            .or_else(|| existing_object.get("smtpPassword"))
            .filter(|value| !value.as_str().unwrap_or_default().is_empty())
    {
        incoming_object.insert("smtpPassword".to_string(), existing_password.clone());
    }

    if let Some(incoming_smtp) = incoming_object
        .get_mut("smtp")
        .and_then(Value::as_object_mut)
        && incoming_smtp
            .get("password")
            .and_then(Value::as_str)
            .is_some_and(str::is_empty)
        && let Some(existing_password) = existing_object
            .get("smtp")
            .and_then(Value::as_object)
            .and_then(|smtp| smtp.get("password"))
            .filter(|value| !value.as_str().unwrap_or_default().is_empty())
    {
        incoming_smtp.insert("password".to_string(), existing_password.clone());
    }

    incoming
}

fn ensure_supported_category(cat: &str) -> Result<&str, SettingsError> {
    let trimmed = cat.trim();
    if category::ALL.contains(&trimmed) {
        Ok(trimmed)
    } else {
        Err(SettingsError::InvalidCategory(cat.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn generic_category_allowlist_excludes_search_owner_settings() {
        assert!(!category::ALL.contains(&"search"));
        assert!(matches!(
            ensure_supported_category("search"),
            Err(SettingsError::InvalidCategory(category)) if category == "search"
        ));
    }

    fn stored_cache_row(url: &str, password: &str) -> Value {
        json!({
            "mode": "redis",
            "redis_host": "cache.internal",
            "redis_port": 6379,
            "redis_db": 0,
            "redis_password": password,
            "redis_url": url,
        })
    }

    #[test]
    fn cache_save_restores_the_password_for_the_same_target() {
        let existing =
            stored_cache_row("redis://:url-secret@cache.internal:6379/0", "field-secret");
        // A read returns a redacted document: blank field, URL without the password.
        let incoming = stored_cache_row("redis://cache.internal:6379/0", "");

        let merged = preserve_cache_secret_fields(existing, incoming);

        assert_eq!(merged["redis_password"], json!("field-secret"));
        assert_eq!(
            merged["redis_url"],
            json!("redis://:url-secret@cache.internal:6379/0")
        );
    }

    #[test]
    fn cache_save_does_not_carry_a_password_to_another_target() {
        let existing =
            stored_cache_row("redis://:url-secret@cache.internal:6379/0", "field-secret");
        // The client edited the target and left the (masked) secret fields untouched.
        let incoming =
            stored_cache_row("redis://other.internal:6379/0", rustok_cache::REDACTED_SECRET);

        let merged = preserve_cache_secret_fields(existing, incoming);

        assert_eq!(
            merged["redis_password"],
            json!(""),
            "a password must not follow the document to another target"
        );
        assert_eq!(merged["redis_url"], json!("redis://other.internal:6379/0"));
        assert!(
            !merged.to_string().contains(rustok_cache::REDACTED_SECRET),
            "a redaction mask must never be stored as a credential: {merged}"
        );
    }

    #[test]
    fn cache_save_clears_a_mask_when_there_is_nothing_to_restore() {
        let existing = stored_cache_row("redis://cache.internal:6379/0", "");
        let incoming = stored_cache_row(
            "redis://cache.internal:6379/0",
            rustok_cache::REDACTED_SECRET,
        );

        let merged = preserve_cache_secret_fields(existing, incoming);

        assert_eq!(merged["redis_password"], json!(""));
    }

    #[test]
    fn rate_limit_validator_rejects_non_positive_rps() {
        let v = RateLimitSettingsValidator;
        let errs = v
            .validate(&json!({ "requests_per_second": 0 }))
            .unwrap_err();
        assert!(errs.iter().any(|e| e.contains("positive")));
    }

    #[test]
    fn rate_limit_validator_rejects_zero_burst() {
        let v = RateLimitSettingsValidator;
        let errs = v.validate(&json!({ "burst_size": 0 })).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("burst_size")));
    }

    #[test]
    fn rate_limit_validator_accepts_valid_settings() {
        let v = RateLimitSettingsValidator;
        assert!(
            v.validate(&json!({ "requests_per_second": 100.0, "burst_size": 200 }))
                .is_ok()
        );
    }

    #[test]
    fn email_validator_rejects_bad_from_address() {
        let v = EmailSettingsValidator;
        let errs = v.validate(&json!({ "from": "not-an-email" })).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("from")));
    }

    #[test]
    fn email_validator_rejects_unknown_provider() {
        let v = EmailSettingsValidator;
        let errs = v.validate(&json!({ "provider": "pigeon" })).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("provider")));
    }

    #[test]
    fn email_validator_accepts_known_providers() {
        let v = EmailSettingsValidator;
        for provider in ["smtp", "sendgrid", "mailgun", "ses", "none"] {
            assert!(
                v.validate(&json!({ "provider": provider })).is_ok(),
                "should accept provider '{provider}'"
            );
        }
    }

    #[test]
    fn validator_registry_default_includes_rate_limit_and_email() {
        let reg = ValidatorRegistry::default();
        // rate_limit: valid
        assert!(reg.validate("rate_limit", &json!({})).is_ok());
        // email: invalid provider
        assert!(
            reg.validate("email", &json!({ "provider": "pigeon" }))
                .is_err()
        );
    }

    #[test]
    fn validator_registry_passes_unknown_category() {
        let reg = ValidatorRegistry::default();
        assert!(reg.validate("general", &json!({ "any": "value" })).is_ok());
    }

    #[test]
    fn email_secrets_are_redacted_from_generic_settings_reads() {
        let value = redact_secrets(
            category::EMAIL,
            json!({
                "smtpPassword": "super-secret",
                "smtp": { "password": "nested-secret" },
                "from": "mail@example.com"
            }),
        );

        assert_eq!(value["smtpPassword"], "");
        assert_eq!(value["smtp"]["password"], "");
        assert_eq!(value["from"], "mail@example.com");
    }

    #[test]
    fn empty_email_password_preserves_existing_secret() {
        let value = preserve_email_secret_fields(
            json!({
                "smtpPassword": "super-secret",
                "smtp": { "password": "nested-secret" }
            }),
            json!({
                "smtpPassword": "",
                "smtp": { "password": "" }
            }),
        );

        assert_eq!(value["smtpPassword"], "super-secret");
        assert_eq!(value["smtp"]["password"], "nested-secret");
    }

    #[test]
    fn non_empty_email_password_replaces_existing_secret() {
        let value = preserve_email_secret_fields(
            json!({
                "smtpPassword": "old-secret",
                "smtp": { "password": "old-nested-secret" }
            }),
            json!({
                "smtpPassword": "new-secret",
                "smtp": { "password": "new-nested-secret" }
            }),
        );

        assert_eq!(value["smtpPassword"], "new-secret");
        assert_eq!(value["smtp"]["password"], "new-nested-secret");
    }

    #[test]
    fn cache_validator_accepts_valid_modes() {
        let v = CacheSettingsValidator;
        for mode in ["in-memory", "redis", "hybrid"] {
            assert!(
                v.validate(&json!({ "mode": mode, "redis_port": 6379, "redis_db": 0 }))
                    .is_ok(),
                "should accept mode '{mode}'"
            );
        }
    }

    #[test]
    fn cache_validator_rejects_unknown_mode() {
        let v = CacheSettingsValidator;
        let errs = v.validate(&json!({ "mode": "memcached" })).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("mode")));
    }

    #[test]
    fn cache_validator_rejects_invalid_port() {
        let v = CacheSettingsValidator;
        let errs = v.validate(&json!({ "redis_port": 70000 })).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("redis_port")));
    }

    #[test]
    fn cache_secrets_are_redacted_from_generic_settings_reads() {
        let value = redact_secrets(
            category::CACHE,
            json!({
                "mode": "redis",
                "redis_password": "super-secret-redis-password",
                "redis_host": "127.0.0.1"
            }),
        );

        assert_eq!(value["redis_password"], "");
        assert_eq!(value["redis_host"], "127.0.0.1");
    }

    #[test]
    fn empty_redis_password_preserves_existing_secret() {
        let value = preserve_cache_secret_fields(
            json!({ "redis_password": "super-secret" }),
            json!({ "redis_password": "", "mode": "redis" }),
        );

        assert_eq!(value["redis_password"], "super-secret");
    }

    #[test]
    fn settings_error_display_includes_category() {
        let err = SettingsError::InvalidCategory("bogus".into());
        assert!(err.to_string().contains("bogus"));
    }

    #[test]
    fn settings_error_display_validation() {
        let err = SettingsError::ValidationFailed(vec!["field required".into()]);
        assert!(err.to_string().contains("field required"));
    }

    #[test]
    fn ensure_supported_category_trims_whitespace() {
        assert_eq!(ensure_supported_category("  email  ").unwrap(), "email");
        assert_eq!(
            ensure_supported_category("\trate_limit\n").unwrap(),
            "rate_limit"
        );
        assert!(ensure_supported_category("   ").is_err());
    }

    #[test]
    fn cache_validator_rejects_zero_or_negative_port() {
        let v = CacheSettingsValidator;
        let errs_zero = v.validate(&json!({ "redis_port": 0 })).unwrap_err();
        assert!(errs_zero.iter().any(|e| e.contains("redis_port")));

        let errs_neg = v.validate(&json!({ "redis_port": -10 })).unwrap_err();
        assert!(errs_neg.iter().any(|e| e.contains("redis_port")));
    }

    #[test]
    fn cache_validator_rejects_out_of_bounds_db() {
        let v = CacheSettingsValidator;
        let errs = v.validate(&json!({ "redis_db": 300 })).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("redis_db")));
    }

    #[test]
    fn email_validator_rejects_non_string_types() {
        let v = EmailSettingsValidator;
        let errs_from = v.validate(&json!({ "from": 12345 })).unwrap_err();
        assert!(
            errs_from
                .iter()
                .any(|e| e.contains("email.from must be a string"))
        );

        let errs_prov = v.validate(&json!({ "provider": true })).unwrap_err();
        assert!(
            errs_prov
                .iter()
                .any(|e| e.contains("email.provider must be a string"))
        );
    }

    #[test]
    fn cache_secrets_preserve_the_url_password_on_save() {
        let existing = json!({
            "mode": "redis",
            "redis_url": "redis://:super-secret@cache.internal:6379/0",
            "redis_host": "cache.internal",
            "redis_port": 6379,
            "redis_db": 0
        });
        // What the client submits after a redacted read: no password inside the URL.
        let incoming = json!({
            "mode": "redis",
            "redis_url": "redis://cache.internal:6379/0",
            "redis_host": "cache.internal",
            "redis_port": 6379,
            "redis_db": 0
        });

        let merged = preserve_cache_secret_fields(existing, incoming);
        assert_eq!(
            merged["redis_url"], "redis://:super-secret@cache.internal:6379/0",
            "a save that does not touch the URL must keep its password"
        );
    }

    #[test]
    fn cache_secrets_do_not_move_a_password_to_another_host() {
        let existing = json!({
            "mode": "redis",
            "redis_url": "redis://:super-secret@cache.internal:6379/0"
        });
        let incoming = json!({
            "mode": "redis",
            "redis_url": "redis://other.internal:6379/0"
        });

        let merged = preserve_cache_secret_fields(existing, incoming);
        assert_eq!(merged["redis_url"], "redis://other.internal:6379/0");
    }

    #[test]
    fn cache_secrets_redacts_redis_url_password() {
        let value = redact_secrets(
            category::CACHE,
            json!({
                "mode": "redis",
                "redis_url": "redis://:super-secret@127.0.0.1:6379/0"
            }),
        );
        assert_eq!(value["redis_url"], "redis://127.0.0.1:6379/0");
    }

    async fn cache_context(deployment_url: Option<&str>) -> ServerRuntimeContext {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("sqlite connects");
        let mut settings = crate::common::settings::RustokSettings::default();
        settings.cache.redis_url = deployment_url.map(str::to_string);
        ServerRuntimeContext::new(db, settings)
    }

    #[tokio::test]
    async fn cache_settings_write_must_mirror_the_deployment_target() {
        let ctx = cache_context(Some("redis://:deploy-secret@cache.internal:6379/0")).await;

        let deployment_document = json!({
            "mode": "redis",
            "redis_host": "cache.internal",
            "redis_port": 6379,
            "redis_db": 0,
            "redis_url": "redis://cache.internal:6379/0"
        });
        let accepted = prepare_cache_settings_for_write(&ctx, deployment_document.clone())
            .expect("mirroring the deployment target is allowed");
        let parsed: rustok_cache::CacheSettings =
            serde_json::from_value(accepted).expect("settings parse");
        assert_eq!(
            parsed.resolved_url().as_deref(),
            Some("redis://cache.internal:6379/0"),
            "the stored mirror keeps the target but never the deployment password"
        );
        assert!(parsed.redis_password.is_empty());

        let redirected = json!({
            "mode": "redis",
            "redis_host": "other.internal",
            "redis_port": 6379,
            "redis_db": 0,
            "redis_url": "redis://other.internal:6379/0"
        });
        let error = prepare_cache_settings_for_write(&ctx, redirected)
            .expect_err("a different target must not be persisted");
        assert!(
            error.to_string().contains("deployment configuration"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn cache_settings_read_reports_the_deployment_target() {
        let ctx = cache_context(Some("redis://cache.internal:6380/2")).await;
        let value = SettingsService::get(&ctx, Uuid::new_v4(), category::CACHE)
            .await
            .expect("cache settings read");

        assert_eq!(value["mode"], "redis");
        assert_eq!(value["redis_host"], "cache.internal");
        assert_eq!(value["redis_port"], 6380);
        assert_eq!(value["redis_db"], 2);
        assert_eq!(value["redis_url"], "redis://cache.internal:6380/2");
        assert_eq!(value["redis_password"], "");
    }

    #[tokio::test]
    async fn cache_settings_read_never_exposes_the_deployment_password() {
        let ctx = cache_context(Some("redis://:deploy-secret@cache.internal:6380/2")).await;
        let value = SettingsService::get(&ctx, Uuid::new_v4(), category::CACHE)
            .await
            .expect("cache settings read");

        assert_eq!(
            value["redis_url"], "redis://cache.internal:6380/2",
            "the deployment target is reported without its credential"
        );
        assert_eq!(value["redis_password"], "");
        assert!(
            !value.to_string().contains("deploy-secret"),
            "no field of a settings read may carry the deployment password: {value}"
        );
    }
}
