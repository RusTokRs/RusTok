//! Canonical cache settings model.
//!
//! This module is the single owner of four decisions that used to be spread across the crate, the
//! host settings service and the admin UI:
//!
//! 1. **Which Redis target is effective** — [`CacheSettings::resolved_url`] (explicit `redis_url`
//!    first, then the structured `redis_host`/`redis_port`/`redis_db`/`redis_password` fields).
//!    Deployment precedence (`settings file` → `RUSTOK_REDIS_URL` → `REDIS_URL`) is resolved by
//!    [`crate::CacheService::from_url`], which calls [`crate::resolve_redis_url`].
//! 2. **Whether a Redis target is well-formed** — [`CacheSettings::validate`] parses the URL,
//!    enforces the `redis`/`rediss` schemes, the `redis_db` range and the mode ↔ target agreement.
//! 3. **What a client may see** — [`CacheSettings::redacted_for_client`] masks the password both in
//!    the dedicated field and inside the URL, so a read-modify-write cycle cannot persist the mask.
//! 4. **How a redacted read is merged back** — [`CacheSettings::merge_with_existing`] restores the
//!    password only when the incoming value still points at the same Redis target.
//!
//! [`preserve_url_password`] exposes decision 4 for callers that store the raw JSON document (the
//! host settings service does), so URL passwords cannot be destroyed by a save.

use serde::{Deserialize, Serialize};

pub const DEFAULT_CACHE_MODE: &str = "in-memory";
pub const DEFAULT_REDIS_MODE: &str = "redis";
pub const DEFAULT_REDIS_HOST: &str = "127.0.0.1";
pub const DEFAULT_REDIS_PORT: u16 = 6379;
pub const DEFAULT_REDIS_DB: u32 = 0;
/// Placeholder a client receives instead of a real secret.
pub const REDACTED_SECRET: &str = "********";

const VALID_MODES: [&str; 3] = ["in-memory", "redis", "hybrid"];

/// Environment variables that may carry the deployment Redis target, in precedence order.
pub const REDIS_URL_ENV_NAMES: [&str; 2] = ["RUSTOK_REDIS_URL", "REDIS_URL"];

/// Deployment-provided Redis target.
///
/// Precedence is `RUSTOK_REDIS_URL` → `REDIS_URL`; a blank value is treated as absent so an
/// exported-but-empty variable cannot shadow a real one. This is the only environment-based
/// resolver in the workspace: the host calls it instead of keeping a second copy, so the runtime
/// target and the target reported by the settings API cannot diverge.
pub fn resolve_redis_url() -> Option<String> {
    resolve_redis_url_from(None, |name| std::env::var(name).ok())
}

/// Pure form of [`resolve_redis_url`], so precedence and blank handling are testable without
/// mutating process-wide environment state.
fn resolve_redis_url_from(
    explicit_url: Option<&str>,
    lookup: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    explicit_url
        .into_iter()
        .map(str::to_string)
        .chain(REDIS_URL_ENV_NAMES.into_iter().filter_map(lookup))
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
}
const VALID_URL_SCHEMES: [&str; 2] = ["redis", "rediss"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheSettings {
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_host")]
    pub redis_host: String,
    #[serde(default = "default_port")]
    pub redis_port: u16,
    #[serde(default)]
    pub redis_password: String,
    #[serde(default)]
    pub redis_db: u32,
    #[serde(default)]
    pub redis_url: String,
}

fn default_mode() -> String {
    DEFAULT_CACHE_MODE.to_string()
}

fn default_host() -> String {
    DEFAULT_REDIS_HOST.to_string()
}

fn default_port() -> u16 {
    DEFAULT_REDIS_PORT
}

impl Default for CacheSettings {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            redis_host: default_host(),
            redis_port: default_port(),
            redis_password: String::new(),
            redis_db: DEFAULT_REDIS_DB,
            redis_url: String::new(),
        }
    }
}

/// Identity of a Redis target, with the password deliberately excluded.
///
/// Two settings documents that identify the same target may share a secret; anything else must not,
/// because restoring a password across a different host would leak it to the new target.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RedisTargetIdentity {
    scheme: String,
    host: String,
    port: u16,
    db: u32,
    username: Option<String>,
}

fn parse_redis_url(url: &str) -> Result<url::Url, String> {
    let parsed = url::Url::parse(url)
        .map_err(|error| format!("redis_url '{url}' is not a valid URL: {error}"))?;
    if !VALID_URL_SCHEMES.contains(&parsed.scheme()) {
        return Err(format!(
            "redis_url must use one of the schemes {}, got '{}'",
            VALID_URL_SCHEMES.join(", "),
            parsed.scheme()
        ));
    }
    if parsed.host_str().is_none_or(str::is_empty) {
        return Err("redis_url must include a host".to_string());
    }
    Ok(parsed)
}

fn url_db(parsed: &url::Url) -> Result<u32, String> {
    let path = parsed.path().trim_start_matches('/');
    if path.is_empty() {
        return Ok(DEFAULT_REDIS_DB);
    }
    let db = path
        .parse::<u32>()
        .map_err(|_| format!("redis_url database '{path}' is not a number"))?;
    if db > 255 {
        return Err("redis_url database must be between 0 and 255".to_string());
    }
    Ok(db)
}

fn identity_of_url(url: &str) -> Option<RedisTargetIdentity> {
    let parsed = parse_redis_url(url).ok()?;
    Some(RedisTargetIdentity {
        scheme: parsed.scheme().to_string(),
        host: parsed.host_str()?.to_ascii_lowercase(),
        port: parsed.port().unwrap_or(DEFAULT_REDIS_PORT),
        db: url_db(&parsed).ok()?,
        username: (!parsed.username().is_empty()).then(|| parsed.username().to_string()),
    })
}

/// Restore the password of `existing_url` into `incoming_url`.
///
/// `incoming_url` is expected to have arrived from a redacted read: it parses, it has no password,
/// and it still points at the same target as `existing_url`. Returns `None` when the two documents
/// do not describe the same target, when either URL is unusable, or when `incoming_url` already
/// carries a password.
pub fn preserve_url_password(incoming_url: &str, existing_url: &str) -> Option<String> {
    let incoming = parse_redis_url(incoming_url).ok()?;
    // A redacted read hands back the mask, not a secret; treat it as "no password" so the real one
    // can be restored. Any other value is a deliberately supplied new password and wins.
    if incoming
        .password()
        .is_some_and(|password| !password.is_empty() && password != REDACTED_SECRET)
    {
        return None;
    }
    let existing = parse_redis_url(existing_url).ok()?;
    existing.password()?;
    if identity_of_url(incoming_url)? != identity_of_url(existing_url)? {
        return None;
    }
    let mut restored = incoming;
    restored.set_password(existing.password()).ok()?;
    Some(restored.to_string())
}

/// Strip the password from a Redis URL and report whether one was present.
fn redact_url_password(url: &str) -> (String, bool) {
    match parse_redis_url(url) {
        Ok(mut parsed) => {
            let had_password = parsed.password().is_some_and(|password| !password.is_empty());
            if had_password {
                let _ = parsed.set_password(Some(REDACTED_SECRET));
            }
            (parsed.to_string(), had_password)
        }
        Err(_) => (url.to_string(), false),
    }
}

impl CacheSettings {
    /// Effective Redis target of this document.
    ///
    /// `None` means "no Redis target configured by this document"; the runtime then falls back to
    /// `RUSTOK_REDIS_URL` / `REDIS_URL` (see [`crate::CacheService::from_url`]).
    pub fn resolved_url(&self) -> Option<String> {
        let explicit = self.redis_url.trim();
        if !explicit.is_empty() {
            return Some(explicit.to_string());
        }
        if self.mode == DEFAULT_CACHE_MODE {
            return None;
        }
        let host = self.redis_host.trim();
        if host.is_empty() || self.redis_port == 0 {
            return None;
        }
        let mut url = url::Url::parse("redis://localhost").expect("static Redis URL parses");
        let _ = url.set_host(Some(host));
        let _ = url.set_port(Some(self.redis_port));
        url.set_path(&format!("/{}", self.redis_db));
        if !self.redis_password.is_empty() {
            let _ = url.set_password(Some(self.redis_password.as_str()));
        }
        Some(url.to_string())
    }

    /// Settings document describing the deployment-provided Redis target.
    ///
    /// Used for the settings API defaults so that the admin surface shows the same target the
    /// runtime actually uses (including `RUSTOK_REDIS_URL` / `REDIS_URL` setups, which used to be
    /// rendered as "in-memory" while the process talked to Redis).
    pub fn for_deployment(explicit_url: Option<&str>) -> Self {
        let resolved = explicit_url
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(str::to_string)
            .or_else(crate::resolve_redis_url);
        let Some(url) = resolved else {
            return Self::default();
        };
        let mut settings = Self {
            mode: DEFAULT_REDIS_MODE.to_string(),
            redis_url: url.clone(),
            ..Self::default()
        };
        if let Ok(parsed) = parse_redis_url(&url) {
            if let Some(host) = parsed.host_str() {
                settings.redis_host = host.to_string();
            }
            settings.redis_port = parsed.port().unwrap_or(DEFAULT_REDIS_PORT);
            settings.redis_db = url_db(&parsed).unwrap_or(DEFAULT_REDIS_DB);
            settings.redis_password = parsed.password().unwrap_or_default().to_string();
        }
        settings
    }

    /// Whether this document carries a Redis password (field or URL).
    pub fn has_password(&self) -> bool {
        if !self.redis_password.is_empty() && self.redis_password != REDACTED_SECRET {
            return true;
        }
        parse_redis_url(self.redis_url.trim())
            .ok()
            .and_then(|parsed| parsed.password().map(|password| !password.is_empty()))
            .unwrap_or(false)
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if !VALID_MODES.contains(&self.mode.as_str()) {
            errors.push(format!(
                "Invalid cache mode '{}'. Must be one of: {}",
                self.mode,
                VALID_MODES.join(", ")
            ));
        }

        if self.redis_port == 0 {
            errors.push("redis_port must be greater than 0".to_string());
        }

        if self.redis_db > 255 {
            errors.push("redis_db must be between 0 and 255".to_string());
        }

        let explicit_url = self.redis_url.trim();
        if !explicit_url.is_empty() {
            match parse_redis_url(explicit_url).and_then(|parsed| url_db(&parsed)) {
                Ok(url_db_value) => {
                    if self.redis_db != url_db_value && self.redis_db != DEFAULT_REDIS_DB {
                        errors.push(format!(
                            "redis_db ({}) contradicts the database in redis_url ({url_db_value})",
                            self.redis_db
                        ));
                    }
                }
                Err(error) => errors.push(error),
            }
            if self.mode == DEFAULT_CACHE_MODE {
                errors.push(
                    "mode is 'in-memory' but redis_url is set; a Redis target must not be ignored"
                        .to_string(),
                );
            }
        } else if self.mode != DEFAULT_CACHE_MODE && self.redis_host.trim().is_empty() {
            errors.push(format!(
                "mode '{}' requires redis_url or a non-empty redis_host",
                self.mode
            ));
        }

        errors.sort();
        errors.dedup();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn redacted_for_client(&self) -> Self {
        let mut clone = self.clone();
        if !clone.redis_password.is_empty() {
            clone.redis_password = REDACTED_SECRET.to_string();
        }
        let (url, had_password) = redact_url_password(clone.redis_url.trim());
        if had_password {
            clone.redis_url = url;
        }
        clone
    }

    /// Merge a (possibly redacted) incoming document with the stored one.
    ///
    /// Secrets survive a read-modify-write cycle; everything else is taken from `self`.
    pub fn merge_with_existing(&mut self, existing: &Self) {
        if self.redis_password.is_empty() || self.redis_password == REDACTED_SECRET {
            self.redis_password = existing.redis_password.clone();
        }

        let incoming_url = self.redis_url.trim().to_string();
        if incoming_url.is_empty() {
            return;
        }
        let existing_url = existing.redis_url.trim();
        if existing_url.is_empty() {
            return;
        }
        if let Some(restored) = preserve_url_password(&incoming_url, existing_url) {
            self.redis_url = restored;
        } else if !self.has_password() {
            // Same-target restoration failed; surface the mask instead of persisting it.
            let (redacted, had_password) = redact_url_password(&incoming_url);
            if had_password {
                self.redis_url = redacted;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redis_url_resolution_prefers_explicit_then_rustok_then_generic_env() {
        let env = |name: &str| match name {
            "RUSTOK_REDIS_URL" => Some("redis://rustok:6379/0".to_string()),
            "REDIS_URL" => Some("redis://generic:6379/0".to_string()),
            _ => None,
        };

        assert_eq!(
            resolve_redis_url_from(Some("redis://explicit:6379/0"), env).as_deref(),
            Some("redis://explicit:6379/0")
        );
        assert_eq!(
            resolve_redis_url_from(None, env).as_deref(),
            Some("redis://rustok:6379/0")
        );
        assert_eq!(
            resolve_redis_url_from(None, |name| match name {
                "RUSTOK_REDIS_URL" => None,
                "REDIS_URL" => Some("redis://generic:6379/0".to_string()),
                _ => None,
            })
            .as_deref(),
            Some("redis://generic:6379/0")
        );
    }

    #[test]
    fn redis_url_resolution_treats_blank_values_as_absent() {
        // An exported-but-empty variable must not shadow a real target.
        assert_eq!(
            resolve_redis_url_from(None, |name| match name {
                "RUSTOK_REDIS_URL" => Some("   ".to_string()),
                "REDIS_URL" => Some("redis://generic:6379/0".to_string()),
                _ => None,
            })
            .as_deref(),
            Some("redis://generic:6379/0")
        );
        assert_eq!(
            resolve_redis_url_from(Some("  "), |name| match name {
                "RUSTOK_REDIS_URL" => Some("  redis://rustok:6379/0  ".to_string()),
                _ => None,
            })
            .as_deref(),
            Some("redis://rustok:6379/0"),
            "resolved URLs are trimmed"
        );
        assert_eq!(
            resolve_redis_url_from(None, |_| Some(String::new())),
            None
        );
    }

    fn redis_settings(url: &str) -> CacheSettings {
        CacheSettings {
            mode: DEFAULT_REDIS_MODE.to_string(),
            redis_url: url.to_string(),
            ..CacheSettings::default()
        }
    }

    #[test]
    fn default_settings_are_valid_memory_only() {
        let settings = CacheSettings::default();
        settings.validate().expect("defaults are valid");
        assert_eq!(settings.resolved_url(), None);
        assert!(!settings.has_password());
    }

    #[test]
    fn explicit_url_wins_over_structured_fields() {
        let settings = CacheSettings {
            mode: DEFAULT_REDIS_MODE.to_string(),
            redis_host: "10.0.0.1".to_string(),
            redis_port: 1234,
            redis_url: "redis://cache.internal:6380/3".to_string(),
            redis_db: 3,
            ..CacheSettings::default()
        };
        settings.validate().expect("explicit URL is consistent");
        assert_eq!(
            settings.resolved_url().as_deref(),
            Some("redis://cache.internal:6380/3")
        );
    }

    #[test]
    fn structured_fields_compose_a_url_in_redis_mode() {
        let settings = CacheSettings {
            mode: "redis".to_string(),
            redis_host: "cache.internal".to_string(),
            redis_port: 6380,
            redis_db: 2,
            redis_password: "s3cret".to_string(),
            redis_url: String::new(),
        };
        settings.validate().expect("composed target is valid");
        let url = settings.resolved_url().expect("composed url");
        assert_eq!(url, "redis://:s3cret@cache.internal:6380/2");
    }

    #[test]
    fn in_memory_mode_never_resolves_a_redis_target() {
        let settings = CacheSettings {
            mode: DEFAULT_CACHE_MODE.to_string(),
            redis_host: "cache.internal".to_string(),
            ..CacheSettings::default()
        };
        assert_eq!(settings.resolved_url(), None);
    }

    #[test]
    fn validate_rejects_in_memory_mode_with_redis_url() {
        let settings = CacheSettings {
            mode: DEFAULT_CACHE_MODE.to_string(),
            redis_url: "redis://cache.internal:6379".to_string(),
            ..CacheSettings::default()
        };
        let errors = settings.validate().expect_err("contradictory document");
        assert!(errors.iter().any(|error| error.contains("in-memory")));
    }

    #[test]
    fn validate_rejects_non_redis_scheme_and_bad_db() {
        let settings = redis_settings("http://cache.internal:6379");
        let errors = settings.validate().expect_err("wrong scheme");
        assert!(errors.iter().any(|error| error.contains("schemes")));

        let settings = redis_settings("redis://cache.internal:6379/999");
        let errors = settings.validate().expect_err("db out of range");
        assert!(errors.iter().any(|error| error.contains("between 0 and 255")));

        let settings = redis_settings("redis://cache.internal:6379/not-a-db");
        let errors = settings.validate().expect_err("non numeric db");
        assert!(errors.iter().any(|error| error.contains("not a number")));
    }

    #[test]
    fn validate_rejects_url_without_host() {
        let settings = redis_settings("redis://");
        let errors = settings.validate().expect_err("missing host");
        assert!(errors.iter().any(|error| error.contains("host")));
    }

    #[test]
    fn redaction_masks_both_password_forms() {
        let settings = CacheSettings {
            mode: DEFAULT_REDIS_MODE.to_string(),
            redis_password: "field-secret".to_string(),
            redis_url: "redis://:url-secret@cache.internal:6379/1".to_string(),
            redis_db: 1,
            ..CacheSettings::default()
        };
        let redacted = settings.redacted_for_client();
        assert_eq!(redacted.redis_password, REDACTED_SECRET);
        assert!(!redacted.redis_url.contains("url-secret"));
        assert!(redacted.redis_url.contains(REDACTED_SECRET));
        assert!(redacted.has_password());
    }

    #[test]
    fn round_trip_restores_the_url_password() {
        let stored = CacheSettings {
            mode: DEFAULT_REDIS_MODE.to_string(),
            redis_password: "field-secret".to_string(),
            redis_url: "redis://:url-secret@cache.internal:6379/1".to_string(),
            redis_db: 1,
            ..CacheSettings::default()
        };
        let mut incoming = stored.redacted_for_client();
        incoming.merge_with_existing(&stored);
        assert_eq!(incoming.redis_password, "field-secret");
        assert_eq!(incoming.redis_url, stored.redis_url);
    }

    #[test]
    fn round_trip_does_not_move_a_password_to_another_host() {
        let stored = redis_settings("redis://:url-secret@cache.internal:6379/0");
        let mut incoming = stored.redacted_for_client();
        incoming.redis_url = "redis://other.internal:6379/0".to_string();
        incoming.merge_with_existing(&stored);
        assert!(!incoming.redis_url.contains("url-secret"));
        assert!(
            preserve_url_password(&incoming.redis_url, &stored.redis_url).is_none(),
            "a different host must not inherit the old password"
        );
    }

    #[test]
    fn for_deployment_reports_memory_only_without_target() {
        let settings = CacheSettings::for_deployment(None);
        // Environment may legitimately configure Redis in CI; only assert consistency.
        settings
            .validate()
            .expect("deployment-derived settings are self-consistent");
        if settings.resolved_url().is_none() {
            assert_eq!(settings.mode, DEFAULT_CACHE_MODE);
        }
    }

    #[test]
    fn for_deployment_mirrors_the_explicit_url() {
        let settings = CacheSettings::for_deployment(Some("redis://:pw@cache.internal:6380/4"));
        assert_eq!(settings.mode, DEFAULT_REDIS_MODE);
        assert_eq!(settings.redis_host, "cache.internal");
        assert_eq!(settings.redis_port, 6380);
        assert_eq!(settings.redis_db, 4);
        assert_eq!(settings.redis_password, "pw");
        settings.validate().expect("deployment settings validate");
    }
}
