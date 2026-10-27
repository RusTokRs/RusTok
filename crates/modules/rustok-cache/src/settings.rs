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

/// Whether two Redis URLs describe the same target (scheme, host, port, database, username).
///
/// The password is excluded on purpose: the target identity is exactly what decides whether a
/// stored secret may be restored into a document. Two documents that point at different targets
/// must never share a password.
pub fn same_redis_target(left: &str, right: &str) -> bool {
    match (identity_of_url(left), identity_of_url(right)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

/// Remove a redaction mask from a URL password.
///
/// [`REDACTED_SECRET`] is a placeholder, not a secret: a document that was never merged back must
/// not persist it, because the next reader would treat it as a real credential.
fn strip_masked_url_password(url: &str) -> String {
    match parse_redis_url(url) {
        Ok(parsed) if parsed.password() == Some(REDACTED_SECRET) => {
            redis_url_without_password(url)
        }
        _ => url.to_string(),
    }
}

/// Whether a Redis URL carries a password.
///
/// The predicate that decides whether redaction has to rewrite the URL at all, so a URL without one
/// is returned byte-for-byte instead of being canonicalised. An empty password is not a secret and
/// reports `false`; a [`REDACTED_SECRET`] mask reports `true`, because the placeholder still has to
/// be removed from the document.
pub fn redis_url_has_password(url: &str) -> bool {
    parse_redis_url(url)
        .ok()
        .and_then(|parsed| parsed.password().map(|password| !password.is_empty()))
        .unwrap_or(false)
}

/// Remove the password from a Redis URL, keeping any ACL username.
///
/// This is the redaction policy for documents a client may see: a username is not a secret, the
/// password always is. A URL that does not parse is returned unchanged (validation reports it), and
/// a URL whose password cannot be removed yields an empty string — a document that is presented as
/// credential-free must never carry a credential.
pub fn redis_url_without_password(url: &str) -> String {
    let Ok(mut parsed) = parse_redis_url(url) else {
        return url.to_string();
    };
    if parsed.password().is_none() {
        return parsed.to_string();
    }
    if parsed.set_password(None).is_err() {
        // `Url::set_password` only fails for a URL that cannot carry credentials at all; the
        // message is fixed because its error type has no detail.
        tracing::error!("could not remove the password from a Redis URL");
        return String::new();
    }
    parsed.to_string()
}

/// Remove every credential (`user:password@`) from a Redis URL.
///
/// Used where a document or an identity value must contain no credential at all: the stored settings
/// mirror and the Redis target signature. Same fail-closed rule as
/// [`redis_url_without_password`].
pub fn redis_url_without_credentials(url: &str) -> String {
    let Ok(mut parsed) = parse_redis_url(url) else {
        return url.to_string();
    };
    if parsed.password().is_none() && parsed.username().is_empty() {
        return parsed.to_string();
    }
    if parsed.set_password(None).is_err() || parsed.set_username("").is_err() {
        tracing::error!("could not remove the credentials from a Redis URL");
        return String::new();
    }
    parsed.to_string()
}

/// Strip the password from a Redis URL and report whether one was present.
fn redact_url_password(url: &str) -> (String, bool) {
    match parse_redis_url(url) {
        Ok(mut parsed) => {
            let had_password = redis_url_has_password(url);
            if !had_password {
                return (parsed.to_string(), false);
            }
            if parsed.set_password(Some(REDACTED_SECRET)).is_err() {
                // Fail closed: a redacted document must never carry the credential. Returning the
                // unmodified URL here would hand the real password to the client while claiming it
                // was masked, so the whole password channel is dropped instead.
                tracing::error!("could not mask the redis_url password; dropping the URL");
                return (String::new(), true);
            }
            (parsed.to_string(), true)
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
        match self.url_from_fields() {
            Ok(url) => url,
            Err(error) => {
                // Fail closed instead of emitting a URL that points somewhere else: the composed
                // host could not be written, and the base URL is `localhost`, which is a different
                // Redis than the document names. `validate` rejects such documents, so this only
                // guards stored or hand-edited rows.
                tracing::error!(
                    %error,
                    "cache settings name an unusable Redis target; no explicit target is used"
                );
                None
            }
        }
    }

    /// Compose the Redis URL described by `redis_host`/`redis_port`/`redis_db`/`redis_password`.
    ///
    /// `Ok(None)` means the document names no structured target at all (no host or no port).
    /// `Err` means the document names one that cannot be expressed, which must never be replaced by
    /// the composition base URL.
    fn url_from_fields(&self) -> Result<Option<String>, String> {
        let host = self.redis_host.trim();
        if host.is_empty() || self.redis_port == 0 {
            return Ok(None);
        }
        let mut url = url::Url::parse("redis://localhost").expect("static Redis URL parses");
        url.set_host(Some(host)).map_err(|error| {
            format!("redis_host '{host}' cannot be used in a Redis URL: {error}")
        })?;
        url.set_port(Some(self.redis_port)).map_err(|()| {
            format!("redis_port {} cannot be used in a Redis URL", self.redis_port)
        })?;
        url.set_path(&format!("/{}", self.redis_db));
        if !self.redis_password.is_empty() {
            url.set_password(Some(self.redis_password.as_str()))
                .map_err(|()| "redis_password cannot be used in a Redis URL".to_string())?;
        }
        Ok(Some(url.to_string()))
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

    /// Whether this document carries a real Redis password (field or URL).
    ///
    /// [`REDACTED_SECRET`] is a mask, so a redacted document carries no password: reporting one
    /// would make a read-modify-write cycle persist the mask as if it were the credential.
    pub fn has_password(&self) -> bool {
        if !self.redis_password.is_empty() && self.redis_password != REDACTED_SECRET {
            return true;
        }
        parse_redis_url(self.redis_url.trim())
            .ok()
            .and_then(|parsed| {
                parsed
                    .password()
                    .map(|password| !password.is_empty() && password != REDACTED_SECRET)
            })
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
        } else if explicit_url.is_empty() && self.mode != DEFAULT_CACHE_MODE {
            // The structured fields *are* the target here, so they must compose into a URL the
            // runtime can actually connect to. Accepting an unusable host would leave the runtime
            // to fall back to `localhost` or to deployment resolution while the document claims a
            // different Redis.
            match self.url_from_fields() {
                Ok(Some(url)) => {
                    if let Err(error) = parse_redis_url(&url) {
                        errors.push(error);
                    }
                }
                Ok(None) => {}
                Err(error) => errors.push(error),
            }
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
    /// Both password channels (the `redis_password` field and the password inside `redis_url`) are
    /// restored only while the incoming document still points at the same target as the stored one
    /// ([`same_redis_target`]). A document that moved to another target is merged without any
    /// secret, and a [`REDACTED_SECRET`] mask is never left behind for storage.
    pub fn merge_with_existing(&mut self, existing: &Self) {
        let same_target = match (self.resolved_url(), existing.resolved_url()) {
            (Some(incoming), Some(previous)) => same_redis_target(&incoming, &previous),
            _ => false,
        };

        if !same_target {
            // A password must never follow a document to a different target, and a mask must never
            // be persisted: dropping both channels is the only truthful result.
            if self.redis_password == REDACTED_SECRET {
                self.redis_password.clear();
            }
            self.redis_url = strip_masked_url_password(self.redis_url.trim());
            return;
        }

        if self.redis_password.is_empty() || self.redis_password == REDACTED_SECRET {
            self.redis_password = if existing.redis_password == REDACTED_SECRET {
                String::new()
            } else {
                existing.redis_password.clone()
            };
        }

        let incoming_url = self.redis_url.trim().to_string();
        let existing_url = existing.redis_url.trim();
        if incoming_url.is_empty() || existing_url.is_empty() {
            return;
        }
        if let Some(restored) = preserve_url_password(&incoming_url, existing_url) {
            self.redis_url = restored;
        } else {
            // Same target but nothing to restore (for example the stored document had no
            // password): never keep the mask in place of a credential.
            self.redis_url = strip_masked_url_password(&incoming_url);
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
    fn validate_rejects_a_structured_host_that_cannot_be_used_in_a_url() {
        let settings = CacheSettings {
            mode: DEFAULT_REDIS_MODE.to_string(),
            redis_host: "not a host".to_string(),
            redis_port: 6379,
            ..CacheSettings::default()
        };
        let errors = settings.validate().expect_err("unusable structured host");
        assert!(
            errors
                .iter()
                .any(|error| error.contains("redis_host") || error.contains("redis_url")),
            "the error must name the unusable field, got {errors:?}"
        );
    }

    #[test]
    fn unusable_structured_host_never_falls_back_to_localhost() {
        let settings = CacheSettings {
            mode: DEFAULT_REDIS_MODE.to_string(),
            redis_host: "not a host".to_string(),
            redis_port: 6379,
            redis_db: 0,
            ..CacheSettings::default()
        };

        // Composing through the `redis://localhost` base used to ignore the failed host write and
        // hand back `redis://localhost:6379/0` — a different Redis than the document named.
        assert_eq!(
            settings.resolved_url(),
            None,
            "an unusable host must resolve to no target, never to the composition base"
        );
    }

    #[test]
    fn validate_rejects_url_without_host() {
        let settings = redis_settings("redis://");
        let errors = settings.validate().expect_err("missing host");
        assert!(errors.iter().any(|error| error.contains("host")));
    }

    #[test]
    fn url_credential_helpers_remove_only_what_they_name() {
        let url = "redis://app:s3cret@cache.internal:6380/2";

        let without_password = redis_url_without_password(url);
        assert!(
            !without_password.contains("s3cret"),
            "the password must never survive redaction: {without_password}"
        );
        assert!(
            without_password.contains("app"),
            "an ACL username is not a secret and must be kept: {without_password}"
        );

        assert_eq!(
            redis_url_without_credentials(url),
            "redis://cache.internal:6380/2",
            "stored documents and target signatures must contain no credential at all"
        );
    }

    #[test]
    fn password_presence_is_the_redaction_trigger() {
        assert!(redis_url_has_password("redis://app:s3cret@cache.internal:6379/0"));
        assert!(redis_url_has_password("redis://:s3cret@cache.internal:6379/0"));
        assert!(
            !redis_url_has_password("redis://cache.internal:6379/0"),
            "a credential-free URL needs no rewriting"
        );
        assert!(
            !redis_url_has_password("redis://app:@cache.internal:6379/0"),
            "an empty password is not a secret"
        );
        assert!(!redis_url_has_password("not a url"));
    }

    #[test]
    fn url_credential_helpers_leave_credential_free_urls_alone() {
        let url = "redis://cache.internal:6380/2";

        assert_eq!(redis_url_without_password(url), url);
        assert_eq!(redis_url_without_credentials(url), url);
        // Unparsable input is returned unchanged: `validate` is the owner of parse errors, and
        // silently rewriting an unknown document would hide them.
        assert_eq!(redis_url_without_password("not a url"), "not a url");
        assert_eq!(redis_url_without_credentials("not a url"), "not a url");
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
        assert!(
            !redacted.has_password(),
            "a mask is not a credential; reporting one would persist the mask"
        );
        assert!(settings.has_password());
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
            !incoming.redis_url.contains(REDACTED_SECRET),
            "a mask must not be persisted when the target changed: {}",
            incoming.redis_url
        );
        assert!(
            preserve_url_password(&incoming.redis_url, &stored.redis_url).is_none(),
            "a different host must not inherit the old password"
        );
    }

    #[test]
    fn round_trip_does_not_move_a_field_password_to_another_host() {
        let stored = CacheSettings {
            mode: DEFAULT_REDIS_MODE.to_string(),
            redis_password: "field-secret".to_string(),
            redis_url: "redis://cache.internal:6379/0".to_string(),
            ..CacheSettings::default()
        };
        let mut incoming = stored.redacted_for_client();
        incoming.redis_url = "redis://other.internal:6379/0".to_string();
        incoming.merge_with_existing(&stored);

        assert!(
            !incoming.has_password(),
            "the stored field password must not follow the document to another target"
        );
        assert!(incoming.redis_password != REDACTED_SECRET);
    }

    #[test]
    fn round_trip_restores_the_field_password_for_the_same_target() {
        let stored = CacheSettings {
            mode: DEFAULT_REDIS_MODE.to_string(),
            redis_password: "field-secret".to_string(),
            redis_url: "redis://cache.internal:6379/0".to_string(),
            ..CacheSettings::default()
        };
        let mut incoming = stored.redacted_for_client();
        // The client touched only an unrelated field.
        incoming.redis_host = "cache.internal".to_string();
        incoming.merge_with_existing(&stored);

        assert_eq!(incoming.redis_password, "field-secret");
        assert!(incoming.has_password());
    }

    #[test]
    fn target_identity_ignores_the_password_but_not_the_scheme_or_database() {
        // Same target, different passwords: a stored secret may be restored.
        assert!(same_redis_target(
            "redis://:one@cache.internal:6379/1",
            "redis://:two@CACHE.internal:6379/1"
        ));
        assert!(!same_redis_target(
            "redis://:one@cache.internal:6379/1",
            "rediss://:two@cache.internal:6379/1"
        ));
        assert!(!same_redis_target(
            "redis://:one@cache.internal:6379/1",
            "redis://:one@cache.internal:6379/2"
        ));
        assert!(!same_redis_target("redis://:one@cache.internal:6379/1", ""));
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
