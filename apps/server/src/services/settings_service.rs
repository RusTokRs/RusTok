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
                if n <= 0.0 {
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

        if let Some(from) = settings.get("from").and_then(|v| v.as_str())
            && !from.contains('@')
        {
            errors.push("email.from must be a valid email address".to_string());
        }

        if let Some(provider) = settings.get("provider").and_then(|v| v.as_str())
            && !matches!(provider, "smtp" | "sendgrid" | "mailgun" | "ses" | "none")
        {
            errors.push(format!(
                "email.provider must be one of: smtp, sendgrid, mailgun, ses, none; got '{provider}'"
            ));
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

        if let Some(mode) = settings.get("mode").and_then(|v| v.as_str()) {
            if !matches!(mode, "in-memory" | "redis" | "hybrid") {
                errors.push(format!(
                    "cache.mode must be one of: in-memory, redis, hybrid; got '{mode}'"
                ));
            }
        }

        if let Some(port) = settings.get("redis_port") {
            if let Some(n) = port.as_u64() {
                if !(1..=65535).contains(&n) {
                    errors.push("cache.redis_port must be between 1 and 65535".to_string());
                }
            } else {
                errors.push("cache.redis_port must be a valid port number".to_string());
            }
        }

        if let Some(db) = settings.get("redis_db") {
            if let Some(n) = db.as_u64() {
                if n > 255 {
                    errors.push("cache.redis_db must be between 0 and 255".to_string());
                }
            } else {
                errors.push("cache.redis_db must be a valid database number".to_string());
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
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
        ensure_supported_category(cat)?;

        // 1. DB row
        if let Some(row) = Entity::find_by_category(ctx.db(), tenant_id, cat).await? {
            return Ok(redact_secrets(cat, row.settings));
        }

        // 2. YAML
        let yaml_value = Self::yaml_defaults_for(ctx, cat);
        if !yaml_value.is_null() {
            return Ok(yaml_value);
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
                result.push((
                    cat.to_string(),
                    if v.is_null() {
                        serde_json::json!({})
                    } else {
                        v
                    },
                ));
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
        ensure_supported_category(cat)?;

        let settings = preserve_secrets(ctx, tenant_id, cat, settings).await?;

        validators
            .validate(cat, &settings)
            .map_err(SettingsError::ValidationFailed)?;

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
            category::CACHE => {
                let default_mode = if rs.cache.redis_url.is_some() {
                    "redis"
                } else {
                    "in-memory"
                };
                serde_json::json!({
                    "mode": default_mode,
                    "redis_url": rs.cache.redis_url.clone().unwrap_or_default(),
                    "redis_host": "127.0.0.1",
                    "redis_port": 6379,
                    "redis_password": "",
                    "redis_db": 0
                })
            }
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

    if incoming_object
        .get("redis_password")
        .and_then(Value::as_str)
        .is_some_and(str::is_empty)
        && let Some(existing_password) = existing_object
            .get("redis_password")
            .or_else(|| existing_object.get("redisPassword"))
            .filter(|value| !value.as_str().unwrap_or_default().is_empty())
    {
        incoming_object.insert("redis_password".to_string(), existing_password.clone());
    }

    if incoming_object
        .get("redisPassword")
        .and_then(Value::as_str)
        .is_some_and(str::is_empty)
        && let Some(existing_password) = existing_object
            .get("redis_password")
            .or_else(|| existing_object.get("redisPassword"))
            .filter(|value| !value.as_str().unwrap_or_default().is_empty())
    {
        incoming_object.insert("redisPassword".to_string(), existing_password.clone());
    }

    incoming
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

fn ensure_supported_category(cat: &str) -> Result<(), SettingsError> {
    if category::ALL.contains(&cat) {
        Ok(())
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
}
