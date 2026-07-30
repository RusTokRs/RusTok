use sea_orm::{ActiveModelTrait, Set};
use serde_json::Value;
use uuid::Uuid;

use crate::models::platform_settings::{self, ActiveModel, Entity};
use crate::services::server_runtime_context::ServerRuntimeContext;

const SEARCH_API_KEY_FIELD: &str = "api_key";
const SEARCH_API_KEY_CONFIGURED_FIELD: &str = "api_key_configured";

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

/// Search connector secrets are bootstrap-only and never tenant-managed through
/// the generic settings API.
pub struct SearchSettingsValidator;

impl SettingsValidator for SearchSettingsValidator {
    fn category(&self) -> &str {
        category::SEARCH
    }

    fn validate(&self, settings: &Value) -> Result<(), Vec<String>> {
        if settings.get(SEARCH_API_KEY_FIELD).is_some() {
            return Err(vec![
                "search.api_key is bootstrap-only and cannot be stored in tenant platform settings"
                    .to_string(),
            ]);
        }
        Ok(())
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

        if let Some(from) = settings.get("from").and_then(|v| v.as_str()) {
            if !from.contains('@') {
                errors.push("email.from must be a valid email address".to_string());
            }
        }

        if let Some(provider) = settings.get("provider").and_then(|v| v.as_str()) {
            if !matches!(provider, "smtp" | "sendgrid" | "mailgun" | "ses" | "none") {
                errors.push(format!(
                    "email.provider must be one of: smtp, sendgrid, mailgun, ses, none; got '{provider}'"
                ));
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
        reg.register(SearchSettingsValidator);
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
///
/// Public methods always return a category-aware safe projection. Raw values are
/// confined to private storage helpers so bootstrap credentials cannot cross the
/// generic settings GraphQL boundary.
pub struct SettingsService;

impl SettingsService {
    /// Get a secret-safe public projection for a single category with fallback.
    pub async fn get(
        ctx: &ServerRuntimeContext,
        tenant_id: Uuid,
        cat: &str,
    ) -> Result<Value, SettingsError> {
        let raw = Self::load_raw(ctx, tenant_id, cat).await?;
        Ok(Self::public_projection(cat, raw))
    }

    /// List all categories for a tenant, filling gaps with secret-safe fallbacks.
    pub async fn get_all(
        ctx: &ServerRuntimeContext,
        tenant_id: Uuid,
    ) -> Result<Vec<(String, Value)>, SettingsError> {
        let db_rows = Entity::find_all_for_tenant(ctx.db(), tenant_id).await?;
        let mut result: Vec<(String, Value)> = db_rows
            .into_iter()
            .map(|row| {
                let category = row.category;
                let settings = Self::public_projection(&category, row.settings);
                (category, settings)
            })
            .collect();

        // Fill in categories that are not yet in the DB.
        let existing: std::collections::HashSet<String> =
            result.iter().map(|(c, _)| c.clone()).collect();

        for &cat in category::ALL {
            if !existing.contains(cat) {
                let raw = Self::yaml_defaults_for(ctx, cat);
                let value = if raw.is_null() {
                    serde_json::json!({})
                } else {
                    Self::public_projection(cat, raw)
                };
                result.push((cat.to_string(), value));
            }
        }

        result.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(result)
    }

    /// Upsert settings for a category and return only its secret-safe public projection.
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

        let settings = Self::normalize_for_storage(cat, settings)?;
        validators
            .validate(cat, &settings)
            .map_err(SettingsError::ValidationFailed)?;

        match Entity::find_by_category(ctx.db(), tenant_id, cat).await? {
            Some(existing) => {
                let mut active: platform_settings::ActiveModel = existing.into();
                active.settings = Set(settings.clone());
                active.updated_by = Set(actor_id);
                active.schema_version = Set(1);
                active.update(ctx.db()).await?;
            }
            None => {
                ActiveModel::new(tenant_id, cat, settings.clone(), actor_id)
                    .insert(ctx.db())
                    .await?;
            }
        }

        Ok(Self::public_projection(cat, settings))
    }

    async fn load_raw(
        ctx: &ServerRuntimeContext,
        tenant_id: Uuid,
        cat: &str,
    ) -> Result<Value, SettingsError> {
        if let Some(row) = Entity::find_by_category(ctx.db(), tenant_id, cat).await? {
            return Ok(row.settings);
        }

        let yaml_value = Self::yaml_defaults_for(ctx, cat);
        if !yaml_value.is_null() {
            return Ok(yaml_value);
        }

        Ok(serde_json::json!({}))
    }

    fn normalize_for_storage(cat: &str, mut settings: Value) -> Result<Value, SettingsError> {
        if cat != category::SEARCH {
            return Ok(settings);
        }

        if settings.get(SEARCH_API_KEY_FIELD).is_some() {
            return Err(SettingsError::ValidationFailed(vec![
                "search.api_key is bootstrap-only and cannot be stored in tenant platform settings"
                    .to_string(),
            ]));
        }
        if let Some(object) = settings.as_object_mut() {
            object.remove(SEARCH_API_KEY_CONFIGURED_FIELD);
        }
        Ok(settings)
    }

    fn public_projection(cat: &str, mut settings: Value) -> Value {
        if cat != category::SEARCH {
            return settings;
        }

        if let Some(object) = settings.as_object_mut() {
            let configured = object
                .remove(SEARCH_API_KEY_FIELD)
                .and_then(|value| value.as_str().map(str::to_owned))
                .is_some_and(|value| !value.trim().is_empty());
            object.insert(
                SEARCH_API_KEY_CONFIGURED_FIELD.to_string(),
                Value::Bool(configured),
            );
        }
        settings
    }

    fn yaml_defaults_for(ctx: &ServerRuntimeContext, cat: &str) -> Value {
        let rs = ctx.settings();
        match cat {
            category::EMAIL => serde_json::to_value(&rs.email).unwrap_or(Value::Null),
            category::SEARCH => serde_json::to_value(&rs.search).unwrap_or(Value::Null),
            category::RATE_LIMIT => serde_json::to_value(&rs.rate_limit).unwrap_or(Value::Null),
            category::FEATURES => serde_json::to_value(&rs.features).unwrap_or(Value::Null),
            _ => Value::Null,
        }
    }
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
    fn search_validator_rejects_api_key_storage() {
        let validator = SearchSettingsValidator;
        let errors = validator
            .validate(&json!({ "api_key": "must-not-be-stored" }))
            .unwrap_err();
        assert!(errors.iter().any(|error| error.contains("bootstrap-only")));
    }

    #[test]
    fn search_public_projection_redacts_api_key_and_reports_configuration() {
        let projected = SettingsService::public_projection(
            category::SEARCH,
            json!({
                "enabled": true,
                "driver": "meilisearch",
                "api_key": "top-secret"
            }),
        );
        assert_eq!(projected.get("api_key"), None);
        assert_eq!(
            projected.get("api_key_configured"),
            Some(&Value::Bool(true))
        );
        assert_eq!(projected.get("driver"), Some(&json!("meilisearch")));
    }

    #[test]
    fn search_storage_normalization_rejects_secret_and_drops_public_marker() {
        let error = SettingsService::normalize_for_storage(
            category::SEARCH,
            json!({ "api_key": "top-secret" }),
        )
        .unwrap_err();
        assert!(error.to_string().contains("bootstrap-only"));

        let normalized = SettingsService::normalize_for_storage(
            category::SEARCH,
            json!({
                "enabled": true,
                "api_key_configured": true
            }),
        )
        .unwrap();
        assert_eq!(normalized.get("api_key_configured"), None);
        assert_eq!(normalized.get("enabled"), Some(&Value::Bool(true)));
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
    fn validator_registry_default_includes_rate_limit_search_and_email() {
        let reg = ValidatorRegistry::default();
        assert!(reg.validate("rate_limit", &json!({})).is_ok());
        assert!(
            reg.validate("search", &json!({ "api_key": "secret" }))
                .is_err()
        );
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
