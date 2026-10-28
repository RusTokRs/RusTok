use crate::i18n;
use crate::model::CacheHealthPayload;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheModeOption {
    InMemory,
    Redis,
    Hybrid,
}

impl CacheModeOption {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InMemory => "in-memory",
            Self::Redis => "redis",
            Self::Hybrid => "hybrid",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "redis" => Self::Redis,
            "hybrid" => Self::Hybrid,
            _ => Self::InMemory,
        }
    }

    pub fn requires_redis_fields(&self) -> bool {
        matches!(self, Self::Redis | Self::Hybrid)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheAdminFormDraft {
    pub mode: String,
    pub redis_host: String,
    pub redis_port: String,
    pub redis_password: String,
    pub redis_db: String,
    pub redis_url: String,
}

impl Default for CacheAdminFormDraft {
    fn default() -> Self {
        Self {
            mode: "in-memory".to_string(),
            redis_host: "127.0.0.1".to_string(),
            redis_port: "6379".to_string(),
            redis_password: String::new(),
            redis_db: "0".to_string(),
            redis_url: String::new(),
        }
    }
}

pub fn parse_cache_settings_json(json_str: &str) -> Option<CacheAdminFormDraft> {
    let val: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let mut draft = CacheAdminFormDraft::default();
    if let Some(m) = val.get("mode").and_then(|v| v.as_str()) {
        draft.mode = m.to_string();
    }
    if let Some(h) = val.get("redis_host").and_then(|v| v.as_str()) {
        draft.redis_host = h.to_string();
    }
    if let Some(p) = val.get("redis_port").and_then(|v| v.as_u64()) {
        draft.redis_port = p.to_string();
    }
    if let Some(pw) = val.get("redis_password").and_then(|v| v.as_str()) {
        draft.redis_password = pw.to_string();
    }
    if let Some(db) = val.get("redis_db").and_then(|v| v.as_u64()) {
        draft.redis_db = db.to_string();
    }
    if let Some(u) = val.get("redis_url").and_then(|v| v.as_str()) {
        draft.redis_url = u.to_string();
    }
    Some(draft)
}

/// Build the settings payload, refusing to silently replace invalid input with defaults.
///
/// A typo in the port or database used to be swallowed by `unwrap_or`, so the UI reported success
/// while a different target than the typed one was saved (AGENTS §14: no silent substitution).
pub fn build_cache_settings_payload(
    draft: &CacheAdminFormDraft,
) -> Result<serde_json::Value, String> {
    let port: u16 = draft.redis_port.trim().parse().map_err(|_| {
        format!(
            "redis_port must be a number between 1 and 65535, got '{}'",
            draft.redis_port.trim()
        )
    })?;
    if port == 0 {
        return Err("redis_port must be greater than 0".to_string());
    }
    let db: u32 = draft.redis_db.trim().parse().map_err(|_| {
        format!(
            "redis_db must be a number between 0 and 255, got '{}'",
            draft.redis_db.trim()
        )
    })?;
    if db > 255 {
        return Err("redis_db must be between 0 and 255".to_string());
    }
    if !matches!(draft.mode.as_str(), "in-memory" | "redis" | "hybrid") {
        return Err(format!(
            "mode must be one of in-memory, redis, hybrid; got '{}'",
            draft.mode
        ));
    }
    // The remaining rules mirror `rustok_cache::CacheSettings::validate`, which is the owner of the
    // document contract. The form cannot call it directly (this crate also builds for `hydrate`,
    // where the server-side cache crate is not a dependency), so it refuses the same documents
    // instead of sending a payload the server will reject without explaining which field is wrong.
    let mode = CacheModeOption::from_str(draft.mode.as_str());
    let redis_host = draft.redis_host.trim().to_string();
    let redis_url = draft.redis_url.trim().to_string();
    if mode.requires_redis_fields() && redis_host.is_empty() && redis_url.is_empty() {
        return Err(format!(
            "mode '{}' requires a Redis target: set redis_url or redis_host",
            draft.mode
        ));
    }
    if !mode.requires_redis_fields() && !redis_url.is_empty() {
        return Err(
            "mode is 'in-memory' but a redis_url is set; a Redis target must not be ignored"
                .to_string(),
        );
    }

    let mut map = serde_json::Map::new();
    map.insert(
        "mode".to_string(),
        serde_json::Value::String(draft.mode.clone()),
    );
    map.insert(
        "redis_host".to_string(),
        serde_json::Value::String(redis_host),
    );
    map.insert("redis_port".to_string(), serde_json::json!(port));
    map.insert("redis_db".to_string(), serde_json::json!(db));
    map.insert(
        "redis_url".to_string(),
        serde_json::Value::String(redis_url),
    );
    if !draft.redis_password.is_empty() {
        map.insert(
            "redis_password".to_string(),
            serde_json::Value::String(draft.redis_password.clone()),
        );
    }
    Ok(serde_json::Value::Object(map))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheDiagnosticsViewModel {
    pub backend_name: String,
    pub is_redis_configured: bool,
    pub is_redis_healthy: bool,
    pub error_message: Option<String>,
}

impl CacheDiagnosticsViewModel {
    pub fn from_payload(payload: &CacheHealthPayload) -> Self {
        Self {
            backend_name: payload.backend.clone(),
            is_redis_configured: payload.redis_configured,
            is_redis_healthy: payload.redis_healthy,
            error_message: payload.redis_error.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheAdminLabels {
    pub title: String,
    pub subtitle: String,
    pub eyebrow: String,
    pub health_title: String,
    pub health_backend: String,
    pub health_configured: String,
    pub health_healthy: String,
    pub health_error: String,
    pub yes: String,
    pub no: String,
    pub settings_title: String,
    pub settings_description: String,
    pub mode_label: String,
    pub mode_inmemory: String,
    pub mode_redis: String,
    pub mode_hybrid: String,
    pub redis_title: String,
    pub redis_host: String,
    pub redis_port: String,
    pub redis_password: String,
    pub redis_password_placeholder: String,
    pub redis_db: String,
    pub redis_url: String,
    pub redis_url_placeholder: String,
    pub save: String,
    pub saving: String,
    pub saved: String,
}

impl CacheAdminLabels {
    pub fn resolve(locale: Option<&str>) -> Self {
        Self {
            title: i18n::t(locale, "cache-title", "Cache Management"),
            subtitle: i18n::t(
                locale,
                "cache-subtitle",
                "Diagnostics and caching system configuration",
            ),
            eyebrow: i18n::t(locale, "cache-eyebrow", "System"),
            health_title: i18n::t(locale, "cache-health-title", "Diagnostics"),
            health_backend: i18n::t(locale, "cache-health-backend", "Backend"),
            health_configured: i18n::t(locale, "cache-health-configured", "Redis Configured"),
            health_healthy: i18n::t(locale, "cache-health-healthy", "Redis Available"),
            health_error: i18n::t(locale, "cache-health-error", "Error"),
            yes: i18n::t(locale, "cache-yes", "Yes"),
            no: i18n::t(locale, "cache-no", "No"),
            settings_title: i18n::t(locale, "cache-settings-title", "Cache Configuration"),
            settings_description: i18n::t(
                locale,
                "cache-settings-description",
                "Select caching layer and Redis connection settings",
            ),
            mode_label: i18n::t(locale, "cache-settings-mode", "Cache Mode"),
            mode_inmemory: i18n::t(
                locale,
                "cache-settings-mode-inmemory",
                "In-Memory (Local RAM)",
            ),
            mode_redis: i18n::t(locale, "cache-settings-mode-redis", "Redis (Distributed)"),
            mode_hybrid: i18n::t(
                locale,
                "cache-settings-mode-hybrid",
                "Hybrid (L1 RAM + L2 Redis)",
            ),
            redis_title: i18n::t(locale, "cache-settings-redis-title", "Redis Settings"),
            redis_host: i18n::t(locale, "cache-settings-redis-host", "Host"),
            redis_port: i18n::t(locale, "cache-settings-redis-port", "Port"),
            redis_password: i18n::t(locale, "cache-settings-redis-password", "Password"),
            redis_password_placeholder: i18n::t(
                locale,
                "cache-settings-redis-password-placeholder",
                "Leave blank to keep current",
            ),
            redis_db: i18n::t(locale, "cache-settings-redis-db", "Database (DB)"),
            redis_url: i18n::t(
                locale,
                "cache-settings-redis-url",
                "Connection URL (optional)",
            ),
            redis_url_placeholder: i18n::t(
                locale,
                "cache-settings-redis-url-placeholder",
                "redis://:password@host:port/db",
            ),
            save: i18n::t(locale, "cache-settings-save", "Save Settings"),
            saving: i18n::t(locale, "cache-settings-saving", "Saving..."),
            saved: i18n::t(
                locale,
                "cache-settings-saved",
                "Cache settings updated successfully",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_options() {
        assert_eq!(CacheModeOption::from_str("redis"), CacheModeOption::Redis);
        assert_eq!(CacheModeOption::from_str("hybrid"), CacheModeOption::Hybrid);
        assert_eq!(
            CacheModeOption::from_str("unknown"),
            CacheModeOption::InMemory
        );
        assert!(!CacheModeOption::InMemory.requires_redis_fields());
        assert!(CacheModeOption::Redis.requires_redis_fields());
        assert!(CacheModeOption::Hybrid.requires_redis_fields());
    }

    #[test]
    fn test_payload_builder() {
        let draft = CacheAdminFormDraft {
            mode: "redis".to_string(),
            redis_host: "10.0.0.1".to_string(),
            redis_port: "6380".to_string(),
            redis_password: "secret".to_string(),
            redis_db: "2".to_string(),
            redis_url: "redis://10.0.0.1:6380".to_string(),
        };
        let payload = build_cache_settings_payload(&draft).expect("valid draft builds a payload");
        assert_eq!(payload["mode"], "redis");
        assert_eq!(payload["redis_host"], "10.0.0.1");
        assert_eq!(payload["redis_port"], 6380);
        assert_eq!(payload["redis_password"], "secret");
        assert_eq!(payload["redis_db"], 2);
    }

    #[test]
    fn payload_builder_mirrors_the_cache_settings_target_rules() {
        // `in-memory` plus a Redis target is rejected by `CacheSettings::validate`; the form must
        // not submit a document the model refuses.
        let mut draft = CacheAdminFormDraft::default();
        draft.redis_url = "redis://cache.internal:6379/0".to_string();
        let error = build_cache_settings_payload(&draft)
            .expect_err("an ignored Redis target must not be submitted");
        assert!(error.contains("in-memory"), "{error}");

        // A Redis mode without any target is equally invalid.
        let draft = CacheAdminFormDraft {
            mode: "redis".to_string(),
            redis_host: "   ".to_string(),
            redis_url: String::new(),
            ..CacheAdminFormDraft::default()
        };
        let error = build_cache_settings_payload(&draft).expect_err("a Redis mode needs a target");
        assert!(error.contains("requires a Redis target"), "{error}");

        // A URL alone is enough, and surrounding whitespace is trimmed rather than stored.
        let draft = CacheAdminFormDraft {
            mode: "redis".to_string(),
            redis_host: String::new(),
            redis_url: "  redis://cache.internal:6379/0  ".to_string(),
            ..CacheAdminFormDraft::default()
        };
        let payload = build_cache_settings_payload(&draft).expect("a URL is a valid target");
        assert_eq!(payload["redis_url"], "redis://cache.internal:6379/0");
        assert_eq!(payload["redis_host"], "");
    }

    #[test]
    fn payload_builder_rejects_invalid_port_and_db_instead_of_defaulting() {
        let mut draft = CacheAdminFormDraft::default();
        draft.mode = "redis".to_string();
        draft.redis_port = "not-a-port".to_string();
        let port_error =
            build_cache_settings_payload(&draft).expect_err("invalid port must not be defaulted");
        assert!(port_error.contains("redis_port"));

        draft.redis_port = "0".to_string();
        assert!(build_cache_settings_payload(&draft).is_err());

        draft.redis_port = "6379".to_string();
        draft.redis_db = "900".to_string();
        let db_error = build_cache_settings_payload(&draft)
            .expect_err("invalid database must not be defaulted");
        assert!(db_error.contains("redis_db"));

        draft.redis_db = "0".to_string();
        draft.mode = "memcached".to_string();
        assert!(build_cache_settings_payload(&draft).is_err());
    }

    #[test]
    fn test_parse_cache_settings_json() {
        assert!(parse_cache_settings_json("not valid json").is_none());

        let empty_json = "{}";
        let parsed_default = parse_cache_settings_json(empty_json).expect("parsed default");
        assert_eq!(parsed_default.mode, "in-memory");
        assert_eq!(parsed_default.redis_host, "127.0.0.1");

        let full_json = r#"{
            "mode": "hybrid",
            "redis_host": "cache.internal",
            "redis_port": 6385,
            "redis_password": "redis_secure_pass",
            "redis_db": 3,
            "redis_url": "redis://cache.internal:6385/3"
        }"#;
        let parsed = parse_cache_settings_json(full_json).expect("parsed full");
        assert_eq!(parsed.mode, "hybrid");
        assert_eq!(parsed.redis_host, "cache.internal");
        assert_eq!(parsed.redis_port, "6385");
        assert_eq!(parsed.redis_password, "redis_secure_pass");
        assert_eq!(parsed.redis_db, "3");
        assert_eq!(parsed.redis_url, "redis://cache.internal:6385/3");
    }

    #[test]
    fn test_diagnostics_view_model_from_payload() {
        let payload = CacheHealthPayload {
            redis_configured: true,
            redis_healthy: false,
            redis_error: Some("connection timed out".to_string()),
            backend: "redis".to_string(),
        };
        let vm = CacheDiagnosticsViewModel::from_payload(&payload);
        assert_eq!(vm.backend_name, "redis");
        assert!(vm.is_redis_configured);
        assert!(!vm.is_redis_healthy);
        assert_eq!(vm.error_message.as_deref(), Some("connection timed out"));
    }

    #[test]
    fn test_labels_resolution() {
        let labels_en = CacheAdminLabels::resolve(Some("en"));
        assert_eq!(labels_en.title, "Cache Management");
        assert_eq!(labels_en.mode_inmemory, "In-Memory (Local RAM)");

        let labels_default = CacheAdminLabels::resolve(None);
        assert_eq!(labels_default.title, "Cache Management");
    }
}
