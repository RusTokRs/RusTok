use sea_orm::{ActiveModelTrait, Set};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::models::platform_settings::{self, ActiveModel, Entity};
use crate::services::server_runtime_context::ServerRuntimeContext;

/// Known setting categories.
pub mod category {
    pub const GENERAL: &str = "general";
    pub const EMAIL: &str = "email";
    pub const SEARCH: &str = "search";
    pub const RATE_LIMIT: &str = "rate_limit";
    pub const FEATURES: &str = "features";
    pub const I18N: &str = "i18n";
    pub const OAUTH: &str = "oauth";

    pub const ALL: &[&str] = &[GENERAL, EMAIL, SEARCH, RATE_LIMIT, FEATURES, I18N, OAUTH];
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

/// Built-in validator for the tenant-visible, non-secret `email` category.
///
/// SMTP credentials remain process-level secrets. Tenant settings may select
/// non-secret delivery metadata, but they may never persist or echo passwords,
/// tokens, API keys, or generic secret fields.
pub struct EmailSettingsValidator;

impl SettingsValidator for EmailSettingsValidator {
    fn category(&self) -> &str {
        category::EMAIL
    }

    fn validate(&self, settings: &Value) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if let Some(path) = find_email_secret_path(settings, "email") {
            errors.push(format!(
                "{path} is a runtime secret and cannot be stored in tenant platform settings"
            ));
        }

        if let Some(from) = email_string(settings, "from_address", &["smtp", "from"])
            .or_else(|| settings.get("from").and_then(Value::as_str))
        {
            if !from.contains('@') {
                errors.push("email.from_address must be a valid email address".to_string());
            }
        }

        if let Some(provider) = settings.get("provider").and_then(Value::as_str) {
            if !matches!(provider, "smtp" | "none") {
                errors.push(format!(
                    "email.provider must be one of: smtp, none; got '{provider}'"
                ));
            }
        }

        if let Some(port) = email_u64(settings, "smtp_port", &["smtp", "port"]) {
            if port == 0 || port > u64::from(u16::MAX) {
                errors.push("email.smtp_port must be in range 1..=65535".to_string());
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
    /// Get settings for a single category with fallback.
    pub async fn get(
        ctx: &ServerRuntimeContext,
        tenant_id: Uuid,
        cat: &str,
    ) -> Result<Value, SettingsError> {
        // 1. DB row
        if let Some(row) = Entity::find_by_category(ctx.db(), tenant_id, cat).await? {
            return Ok(public_settings_value(cat, row.settings));
        }

        // 2. YAML
        let yaml_value = Self::yaml_defaults_for(ctx, cat);
        if !yaml_value.is_null() {
            return Ok(public_settings_value(cat, yaml_value));
        }

        // 3. Empty object default
        Ok(serde_json::json!({}))
    }

    /// List all categories for a tenant, filling gaps with fallbacks.
    pub async fn get_all(
        ctx: &ServerRuntimeContext,
        tenant_id: Uuid,
    ) -> Result<Vec<(String, Value)>, SettingsError> {
        let db_rows = Entity::find_all_for_tenant(ctx.db(), tenant_id).await?;
        let mut result: Vec<(String, Value)> = db_rows
            .into_iter()
            .map(|r| {
                let category = r.category;
                let settings = public_settings_value(&category, r.settings);
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
                        public_settings_value(cat, v)
                    },
                ));
            }
        }

        result.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(result)
    }

    /// Upsert settings for a category.
    ///
    /// Returns the stored, public-safe `Value`.
    pub async fn update(
        ctx: &ServerRuntimeContext,
        tenant_id: Uuid,
        cat: &str,
        settings: Value,
        actor_id: Option<Uuid>,
        validators: &ValidatorRegistry,
    ) -> Result<Value, SettingsError> {
        if !category::ALL.contains(&cat) {
            return Err(SettingsError::InvalidCategory(cat.to_string()));
        }

        validators
            .validate(cat, &settings)
            .map_err(SettingsError::ValidationFailed)?;

        let stored_settings = if cat == category::EMAIL {
            canonical_email_override(&settings)
        } else {
            settings
        };

        match Entity::find_by_category(ctx.db(), tenant_id, cat).await? {
            Some(existing) => {
                let mut active: platform_settings::ActiveModel = existing.into();
                active.settings = Set(stored_settings.clone());
                active.updated_by = Set(actor_id);
                active.schema_version = Set(1);
                active.update(ctx.db()).await?;
            }
            None => {
                ActiveModel::new(tenant_id, cat, stored_settings.clone(), actor_id)
                    .insert(ctx.db())
                    .await?;
            }
        }

        Ok(public_settings_value(cat, stored_settings))
    }

    // ── Private helpers ────────────────────────────────────────────────────

    fn yaml_defaults_for(ctx: &ServerRuntimeContext, cat: &str) -> Value {
        let rs = ctx.settings();
        match cat {
            category::EMAIL => email_public_projection(&serde_json::json!({
                "enabled": rs.email.enabled,
                "provider": match rs.email.provider {
                    crate::common::settings::EmailProvider::Smtp => "smtp",
                    crate::common::settings::EmailProvider::None => "none",
                },
                "smtp": {
                    "host": rs.email.smtp.host,
                    "port": rs.email.smtp.port,
                    "username": rs.email.smtp.username,
                    "password": rs.email.smtp.password,
                },
                "from": rs.email.from,
                "reset_base_url": rs.email.reset_base_url,
            })),
            category::SEARCH => serde_json::to_value(&rs.search).unwrap_or(Value::Null),
            category::RATE_LIMIT => serde_json::to_value(&rs.rate_limit).unwrap_or(Value::Null),
            category::FEATURES => serde_json::to_value(&rs.features).unwrap_or(Value::Null),
            _ => Value::Null,
        }
    }
}

fn public_settings_value(category: &str, value: Value) -> Value {
    if category == self::category::EMAIL {
        email_public_projection(&value)
    } else {
        value
    }
}

fn canonical_email_override(value: &Value) -> Value {
    let public = email_public_projection(value);
    let mut stored = Map::new();
    for key in [
        "enabled",
        "provider",
        "smtp_host",
        "smtp_port",
        "smtp_username",
        "from_address",
        "reset_base_url",
    ] {
        if let Some(value) = public.get(key) {
            stored.insert(key.to_string(), value.clone());
        }
    }
    Value::Object(stored)
}

fn email_public_projection(value: &Value) -> Value {
    let enabled = value.get("enabled").and_then(Value::as_bool).unwrap_or(false);
    let provider = value
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("smtp");
    let smtp_host = email_string(value, "smtp_host", &["smtp", "host"]).unwrap_or("localhost");
    let smtp_port = email_u64(value, "smtp_port", &["smtp", "port"]).unwrap_or(1025);
    let smtp_username =
        email_string(value, "smtp_username", &["smtp", "username"]).unwrap_or("");
    let from_address = value
        .get("from_address")
        .and_then(Value::as_str)
        .or_else(|| value.get("from").and_then(Value::as_str))
        .unwrap_or("no-reply@rustok.local");
    let reset_base_url = value
        .get("reset_base_url")
        .and_then(Value::as_str)
        .unwrap_or("/reset-password");
    let password_configured = email_string(value, "smtp_password", &["smtp", "password"])
        .is_some_and(|secret| !secret.is_empty());

    serde_json::json!({
        "enabled": enabled,
        "provider": provider,
        "smtp_host": smtp_host,
        "smtp_port": smtp_port,
        "smtp_username": smtp_username,
        "from_address": from_address,
        "reset_base_url": reset_base_url,
        "password_configured": password_configured,
    })
}

fn email_string<'a>(value: &'a Value, flat_key: &str, nested_path: &[&str]) -> Option<&'a str> {
    value
        .get(flat_key)
        .and_then(Value::as_str)
        .or_else(|| value.pointer(&format!("/{}", nested_path.join("/"))).and_then(Value::as_str))
}

fn email_u64(value: &Value, flat_key: &str, nested_path: &[&str]) -> Option<u64> {
    value
        .get(flat_key)
        .and_then(Value::as_u64)
        .or_else(|| value.pointer(&format!("/{}", nested_path.join("/"))).and_then(Value::as_u64))
}

fn find_email_secret_path(value: &Value, path: &str) -> Option<String> {
    match value {
        Value::Object(object) => object.iter().find_map(|(key, child)| {
            let child_path = format!("{path}.{key}");
            if is_email_secret_key(key) {
                Some(child_path)
            } else {
                find_email_secret_path(child, &child_path)
            }
        }),
        Value::Array(values) => values
            .iter()
            .enumerate()
            .find_map(|(index, child)| find_email_secret_path(child, &format!("{path}[{index}]"))),
        _ => None,
    }
}

fn is_email_secret_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "password"
            | "smtp_password"
            | "smtp-password"
            | "secret"
            | "smtp_secret"
            | "api_key"
            | "apikey"
            | "token"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
        let errs = v
            .validate(&json!({ "from_address": "not-an-email" }))
            .unwrap_err();
        assert!(errs.iter().any(|e| e.contains("from_address")));
    }

    #[test]
    fn email_validator_rejects_unknown_provider() {
        let v = EmailSettingsValidator;
        let errs = v.validate(&json!({ "provider": "pigeon" })).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("provider")));
    }

    #[test]
    fn email_validator_accepts_runtime_supported_providers() {
        let v = EmailSettingsValidator;
        for provider in ["smtp", "none"] {
            assert!(
                v.validate(&json!({ "provider": provider })).is_ok(),
                "should accept provider '{provider}'"
            );
        }
    }

    #[test]
    fn email_validator_rejects_nested_and_flattened_secrets() {
        let v = EmailSettingsValidator;
        assert!(
            v.validate(&json!({ "smtp": { "password": "secret" } }))
                .is_err()
        );
        assert!(v.validate(&json!({ "smtp_password": "secret" })).is_err());
        assert!(v.validate(&json!({ "api_key": "secret" })).is_err());
    }

    #[test]
    fn email_projection_never_returns_the_smtp_password() {
        let projected = email_public_projection(&json!({
            "enabled": true,
            "provider": "smtp",
            "smtp": {
                "host": "smtp.example.test",
                "port": 587,
                "username": "mailer",
                "password": "top-secret",
            },
            "from": "no-reply@example.test",
            "reset_base_url": "https://admin.example.test/reset",
        }));

        let encoded = projected.to_string();
        assert!(!encoded.contains("top-secret"));
        assert!(!encoded.contains("password\""));
        assert_eq!(projected["smtp_host"], "smtp.example.test");
        assert_eq!(projected["smtp_port"], 587);
        assert_eq!(projected["password_configured"], true);
    }

    #[test]
    fn canonical_email_override_whitelists_non_secret_fields() {
        let stored = canonical_email_override(&json!({
            "smtp_host": "smtp.example.test",
            "smtp_port": 587,
            "smtp_username": "mailer",
            "from_address": "no-reply@example.test",
            "password_configured": true,
            "unknown": "drop-me",
        }));

        assert_eq!(stored["smtp_host"], "smtp.example.test");
        assert!(stored.get("password_configured").is_none());
        assert!(stored.get("unknown").is_none());
    }

    #[test]
    fn validator_registry_default_includes_rate_limit_and_email() {
        let reg = ValidatorRegistry::default();
        assert!(reg.validate("rate_limit", &json!({})).is_ok());
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
