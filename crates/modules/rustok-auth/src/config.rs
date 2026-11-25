use std::sync::{Arc, OnceLock};

use jsonwebtoken::{Algorithm, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::credentials::{
    DEFAULT_MIN_PASSWORD_LENGTH_CHARS, MAX_MIN_PASSWORD_LENGTH_CHARS,
    MIN_PASSWORD_LENGTH_FLOOR_CHARS,
};
use crate::error::{AuthError, Result};
use crate::jwt_keyring::{
    JwtKeyring, SpecialJwtKeyrings, ValidatedJwtKeyrings, rsa_signing_key, verification_key,
};

const DEFAULT_ACCESS_EXPIRATION_SECS: u64 = 900; // 15 minutes
const DEFAULT_REFRESH_EXPIRATION_SECS: u64 = 60 * 60 * 24 * 30; // 30 days
pub(crate) const MIN_HS256_SECRET_BYTES: usize = 32;

/// The single HS256 secret-strength rule, shared by startup validation and
/// by key materialization in `jwt.rs` so the two can never drift apart:
/// at least `MIN_HS256_SECRET_BYTES` non-whitespace bytes. (Counting only
/// `trim().len()` would credit interior whitespace toward the minimum.)
pub(crate) fn hs256_secret_has_min_length(secret: &str) -> bool {
    secret
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .count()
        >= MIN_HS256_SECRET_BYTES
}
const MIN_ACCESS_EXPIRATION_SECS: u64 = 60;
const MAX_ACCESS_EXPIRATION_SECS: u64 = 60 * 60 * 24 * 30;
const MIN_REFRESH_EXPIRATION_SECS: u64 = 60 * 5;
const MAX_REFRESH_EXPIRATION_SECS: u64 = 60 * 60 * 24 * 365;

#[derive(Debug, Deserialize, Serialize)]
struct Rs256KeyPairProbe {
    probe: String,
}

/// JWT signing algorithm selector.
///
/// - `HS256` (default): explicit symmetric signing material or a purpose-derived key.
/// - `RS256`: RSA-SHA256, asymmetric — set `rsa_private_key_pem` for signing,
///   `rsa_public_key_pem` for verification.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum JwtAlgorithm {
    #[default]
    HS256,
    RS256,
}

/// Auth configuration — framework-agnostic.
///
/// The server is responsible for constructing this from whatever config source
/// it uses (YAML, environment variables, etc.). `rustok-auth` never reads config files.
#[derive(Clone)]
pub struct AuthConfig {
    key_state: Arc<JwtKeyState>,
    pub access_expiration: u64,
    pub refresh_expiration: u64,
    pub issuer: String,
    pub audience: String,
    /// Minimum password length (in chars) enforced for new credentials.
    pub min_password_length: usize,
}

/// The immutable raw inputs and their once-only prepared result share the
/// same ownership boundary. Request-context clones do not copy secrets.
struct JwtKeyState {
    inputs: JwtKeyInputs,
    prepared: OnceLock<Result<ValidatedJwtKeyrings>>,
}

#[derive(Clone)]
struct JwtKeyInputs {
    secret: String,
    jwt_keyring: JwtKeyring,
    special_jwt_keyrings: SpecialJwtKeyrings,
    algorithm: JwtAlgorithm,
    rsa_private_key_pem: Option<String>,
    rsa_public_key_pem: Option<String>,
}

impl Clone for JwtKeyState {
    // Copy-on-write detaches a candidate without copying a prepared signer
    // or its cached failure. Ordinary AuthConfig clones only clone the Arc.
    fn clone(&self) -> Self {
        Self {
            inputs: self.inputs.clone(),
            prepared: OnceLock::new(),
        }
    }
}

impl std::fmt::Debug for AuthConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthConfig")
            .field("secret", &"<redacted>")
            .field("jwt_keyring", &self.key_state.inputs.jwt_keyring)
            .field("special_jwt_keyrings", &self.key_state.inputs.special_jwt_keyrings)
            .field("access_expiration", &self.access_expiration)
            .field("refresh_expiration", &self.refresh_expiration)
            .field("issuer", &self.issuer)
            .field("audience", &self.audience)
            .field("algorithm", &self.key_state.inputs.algorithm)
            .field(
                "rsa_private_key_pem",
                &self.key_state.inputs.rsa_private_key_pem.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "rsa_public_key_pem",
                &self.key_state.inputs.rsa_public_key_pem.as_ref().map(|_| "<redacted>"),
            )
            .field("min_password_length", &self.min_password_length)
            .finish()
    }
}

impl AuthConfig {
    /// Stable host MAC root, distinct from the active JWT signing material.
    pub fn secret(&self) -> &str {
        &self.key_state.inputs.secret
    }

    pub fn algorithm(&self) -> &JwtAlgorithm {
        &self.key_state.inputs.algorithm
    }

    pub fn rsa_private_key_pem(&self) -> Option<&str> {
        self.key_state.inputs.rsa_private_key_pem.as_deref()
    }

    pub fn rsa_public_key_pem(&self) -> Option<&str> {
        self.key_state.inputs.rsa_public_key_pem.as_deref()
    }

    pub(crate) fn jwt_keyring(&self) -> &JwtKeyring {
        &self.key_state.inputs.jwt_keyring
    }

    pub(crate) fn special_jwt_keyrings(&self) -> &SpecialJwtKeyrings {
        &self.key_state.inputs.special_jwt_keyrings
    }

    /// Once per immutable key configuration, including concurrent first use.
    /// Failure is cached too; an invalid configuration cannot sign or verify.
    pub(crate) fn prepared_jwt(&self) -> Result<&ValidatedJwtKeyrings> {
        self.key_state.prepared.get_or_init(|| ValidatedJwtKeyrings::prepare(self))
            .as_ref().map_err(Clone::clone)
    }

    fn edit_key_inputs(&mut self) -> &mut JwtKeyInputs {
        let state = Arc::make_mut(&mut self.key_state);
        state.prepared = OnceLock::new();
        &mut state.inputs
    }

    #[cfg(test)]
    pub(crate) fn set_test_jwt_keyring(&mut self, ring: JwtKeyring) {
        self.edit_key_inputs().jwt_keyring = ring;
    }

    #[cfg(test)]
    pub(crate) fn set_test_special_jwt_keyrings(&mut self, rings: SpecialJwtKeyrings) {
        self.edit_key_inputs().special_jwt_keyrings = rings;
    }

    /// Host production policy checks these secrets without exposing them in diagnostics.
    pub fn explicit_hs256_secrets(&self) -> impl Iterator<Item = &str> {
        crate::jwt_keyring::explicit_hs256_secrets(self)
    }

    pub fn new(secret: String) -> Self {
        Self {
            key_state: Arc::new(JwtKeyState {
                inputs: JwtKeyInputs {
                    secret,
                    jwt_keyring: JwtKeyring::default(),
                    special_jwt_keyrings: SpecialJwtKeyrings::default(),
                    algorithm: JwtAlgorithm::HS256,
                    rsa_private_key_pem: None,
                    rsa_public_key_pem: None,
                },
                prepared: OnceLock::new(),
            }),
            access_expiration: DEFAULT_ACCESS_EXPIRATION_SECS,
            refresh_expiration: DEFAULT_REFRESH_EXPIRATION_SECS,
            issuer: "rustok".to_string(),
            audience: "rustok-admin".to_string(),
            min_password_length: DEFAULT_MIN_PASSWORD_LENGTH_CHARS,
        }
    }

    pub fn with_expiration(mut self, access: u64, refresh: u64) -> Self {
        self.access_expiration = access;
        self.refresh_expiration = refresh;
        self
    }

    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.issuer = issuer.into();
        self
    }

    pub fn with_audience(mut self, audience: impl Into<String>) -> Self {
        self.audience = audience.into();
        self
    }

    pub fn with_rs256(
        mut self,
        private_key_pem: impl Into<String>,
        public_key_pem: impl Into<String>,
    ) -> Self {
        let inputs = self.edit_key_inputs();
        inputs.algorithm = JwtAlgorithm::RS256;
        inputs.rsa_private_key_pem = Some(private_key_pem.into());
        inputs.rsa_public_key_pem = Some(public_key_pem.into());
        self
    }

    pub fn with_min_password_length(mut self, min_password_length: usize) -> Self {
        self.min_password_length = min_password_length;
        self
    }
}

/// Helper for loading auth settings from nested YAML `settings.rustok.auth`.
#[derive(Deserialize, Default)]
pub struct AuthSettingsOverrides {
    pub refresh_expiration: Option<u64>,
    pub issuer: Option<String>,
    pub audience: Option<String>,
    pub algorithm: Option<JwtAlgorithm>,
    /// Name of an environment variable containing the JWT keyring JSON.
    pub jwt_keyring_env: Option<String>,
    /// Environment variable containing the independent internal-link rings.
    pub jwt_special_keyrings_env: Option<String>,
    pub rsa_private_key_pem: Option<String>,
    pub rsa_public_key_pem: Option<String>,
    pub rsa_private_key_env: Option<String>,
    pub rsa_public_key_env: Option<String>,
    pub min_password_length: Option<usize>,
}

impl std::fmt::Debug for AuthSettingsOverrides {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthSettingsOverrides")
            .field("jwt_keyring_env", &self.jwt_keyring_env)
            .field("jwt_special_keyrings_env", &self.jwt_special_keyrings_env)
            .field("refresh_expiration", &self.refresh_expiration)
            .field("issuer", &self.issuer)
            .field("audience", &self.audience)
            .field("algorithm", &self.algorithm)
            .field(
                "rsa_private_key_pem",
                &self.rsa_private_key_pem.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "rsa_public_key_pem",
                &self.rsa_public_key_pem.as_ref().map(|_| "<redacted>"),
            )
            .field("rsa_private_key_env", &self.rsa_private_key_env)
            .field("rsa_public_key_env", &self.rsa_public_key_env)
            .field("min_password_length", &self.min_password_length)
            .finish()
    }
}

impl AuthSettingsOverrides {
    /// Apply overrides on top of a base `AuthConfig`.
    pub fn apply(self, config: &mut AuthConfig) -> Result<()> {
        let mut candidate = config.clone();
        self.apply_with_env(&mut candidate, &mut read_auth_env)?;
        validate_auth_config(&candidate)?;
        *config = candidate;
        Ok(())
    }

    fn apply_with_env<F>(self, config: &mut AuthConfig, env_resolver: &mut F) -> Result<()>
    where
        F: FnMut(&str) -> Result<String>,
    {
        // Each cryptographic edit goes through the single copy-on-write boundary.
        if let Some(name) = self.jwt_keyring_env {
            if name.trim().is_empty() {
                return Err(AuthError::Internal("JWT keyring environment name is empty".to_string()));
            }
            config.edit_key_inputs().jwt_keyring = JwtKeyring::from_json(&env_resolver(&name)?)?;
        }
        if let Some(name) = self.jwt_special_keyrings_env {
            if name.trim().is_empty() {
                return Err(AuthError::Internal("Special JWT keyring environment name is empty".to_string()));
            }
            config.edit_key_inputs().special_jwt_keyrings = SpecialJwtKeyrings::from_json(&env_resolver(&name)?)?;
        }
        if let Some(v) = self.refresh_expiration {
            config.refresh_expiration = v;
        }
        if let Some(v) = self.issuer {
            config.issuer = v;
        }
        if let Some(v) = self.audience {
            config.audience = v;
        }
        if let Some(v) = self.algorithm {
            config.edit_key_inputs().algorithm = v;
        }
        if self.rsa_private_key_pem.is_some() || self.rsa_private_key_env.is_some() {
            config.edit_key_inputs().rsa_private_key_pem = resolve_key_material(
                self.rsa_private_key_pem, self.rsa_private_key_env, env_resolver,
            )?;
        }
        if self.rsa_public_key_pem.is_some() || self.rsa_public_key_env.is_some() {
            config.edit_key_inputs().rsa_public_key_pem = resolve_key_material(
                self.rsa_public_key_pem, self.rsa_public_key_env, env_resolver,
            )?;
        }
        if let Some(v) = self.min_password_length {
            config.min_password_length = v;
        }
        Ok(())
    }
}

/// Build and validate `AuthConfig` from host-provided settings.
///
/// The host remains responsible for reading its framework config, but the
/// semantics of auth overrides, RS256 key resolution, defaults, and validation
/// live in `rustok-auth` so transport adapters do not duplicate auth lifecycle
/// rules.
pub fn build_auth_config(
    secret: String,
    access_expiration: u64,
    auth_settings: AuthSettingsOverrides,
) -> Result<AuthConfig> {
    build_auth_config_with_env(secret, access_expiration, auth_settings, read_auth_env)
}

fn read_auth_env(name: &str) -> Result<String> {
    std::env::var(name)
        .map_err(|_| AuthError::Internal(format!("Missing auth env var: {name}")))
}

/// Build and validate `AuthConfig` with an injectable env resolver for tests.
pub fn build_auth_config_with_env<F>(
    secret: String,
    access_expiration: u64,
    auth_settings: AuthSettingsOverrides,
    mut env_resolver: F,
) -> Result<AuthConfig>
where
    F: FnMut(&str) -> Result<String>,
{
    let mut config = AuthConfig::new(secret);
    config.access_expiration = access_expiration;
    auth_settings.apply_with_env(&mut config, &mut env_resolver)?;

    validate_auth_config(&config)?;
    Ok(config)
}

fn resolve_key_material<F>(
    inline_pem: Option<String>,
    env_name: Option<String>,
    env_resolver: &mut F,
) -> Result<Option<String>>
where
    F: FnMut(&str) -> Result<String>,
{
    if let Some(env_name) = env_name {
        if env_name.trim().is_empty() {
            return Err(AuthError::Internal("RSA key environment name is empty".to_string()));
        }
        let value = env_resolver(&env_name)?;
        if value.trim().is_empty() {
            return Err(AuthError::Internal(format!(
                "Auth env var {env_name} must not be empty"
            )));
        }
        return Ok(Some(value));
    }

    Ok(inline_pem.filter(|pem| !pem.trim().is_empty()))
}

/// Validate auth config invariants owned by `rustok-auth`.
pub fn validate_auth_config(config: &AuthConfig) -> Result<()> {
    tracing::debug!(
        issuer = %config.issuer,
        audience = %config.audience,
        algorithm = ?config.algorithm(),
        "Validating auth configuration"
    );
    if config.issuer.trim().is_empty() {
        return Err(AuthError::Internal(
            "JWT issuer must not be empty".to_string(),
        ));
    }
    if config.audience.trim().is_empty() {
        return Err(AuthError::Internal(
            "JWT audience must not be empty".to_string(),
        ));
    }
    if !(MIN_ACCESS_EXPIRATION_SECS..=MAX_ACCESS_EXPIRATION_SECS)
        .contains(&config.access_expiration)
    {
        return Err(AuthError::Internal(format!(
            "JWT access_expiration must be between {MIN_ACCESS_EXPIRATION_SECS} and {MAX_ACCESS_EXPIRATION_SECS} seconds"
        )));
    }
    if !(MIN_REFRESH_EXPIRATION_SECS..=MAX_REFRESH_EXPIRATION_SECS)
        .contains(&config.refresh_expiration)
    {
        return Err(AuthError::Internal(format!(
            "JWT refresh_expiration must be between {MIN_REFRESH_EXPIRATION_SECS} and {MAX_REFRESH_EXPIRATION_SECS} seconds"
        )));
    }
    if config.refresh_expiration < config.access_expiration {
        return Err(AuthError::Internal(
            "JWT refresh_expiration must be greater than or equal to access_expiration".to_string(),
        ));
    }
    if !(MIN_PASSWORD_LENGTH_FLOOR_CHARS..=MAX_MIN_PASSWORD_LENGTH_CHARS)
        .contains(&config.min_password_length)
    {
        return Err(AuthError::Internal(format!(
            "min_password_length must be between {MIN_PASSWORD_LENGTH_FLOOR_CHARS} and {MAX_MIN_PASSWORD_LENGTH_CHARS} chars"
        )));
    }

    // The host also uses this secret for CSRF and credential binding even
    // when JWT signatures use RSA. Never allow weak symmetric key material.
    if !hs256_secret_has_min_length(config.secret()) {
        return Err(AuthError::Internal(format!(
            "Auth secret must contain at least {MIN_HS256_SECRET_BYTES} non-whitespace bytes"
        )));
    }

    config.prepared_jwt()?;
    Ok(())
}

pub(crate) fn validate_rs256_key_pair(private_key_pem: &str, public_key_pem: &str) -> Result<()> {
    let encoding_key = rsa_signing_key(private_key_pem, public_key_pem)?;
    let decoding_key = verification_key(&JwtAlgorithm::RS256, public_key_pem)?;
    let token = encode(
        &Header::new(Algorithm::RS256),
        &Rs256KeyPairProbe {
            probe: "rustok-auth-config".to_string(),
        },
        &encoding_key,
    )
    .map_err(|_| AuthError::Internal("RS256 private key cannot sign tokens".to_string()))?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.required_spec_claims.clear();
    validation.validate_exp = false;
    decode::<Rs256KeyPairProbe>(&token, &decoding_key, &validation).map_err(|_| {
        AuthError::Internal("RS256 private/public keys do not form a pair".to_string())
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn keyring_configuration_rejects_ambiguous_or_weak_keys() {
        use serde_json::json;
        let active = secret();
        let valid_material = "different-test-verification-key-at-least-32-bytes";
        for value in [
            json!({"active_kid": ""}),
            json!({"active_kid": "a", "verification_keys": [
                {"kid":"a","material":valid_material,"verify_until":1}
            ]}),
            json!({"active_kid": "a", "verification_keys": [
                {"kid":"b","material":"short","verify_until":1}
            ]}),
            json!({"active_kid": "a", "verification_keys": [
                {"kid":"b","material":valid_material,"verify_until":1},
                {"kid":"b","material":"different-other-test-key-at-least-32-bytes","verify_until":2}
            ]}),
            json!({"active_kid": "a", "legacy_without_kid":{"kid":"missing","verify_until":1}}),
            json!({"active_kid": "a", "active_hs256_secret":"short"}),
            json!({"active_kid": "a", "active_hs256_secret":active}),
        ] {
            let result = build_auth_config_with_env(
                active.clone(), 900,
                AuthSettingsOverrides { jwt_keyring_env: Some("JWT_KEYS".into()), ..Default::default() },
                |_| Ok(value.to_string()),
            );
            assert!(result.is_err());
        }
    }

    #[test]
    fn purpose_config_rejects_cross_ring_key_reuse_and_legacy_special_ids() {
        use serde_json::json;
        let shared = "independent-but-reused-signing-material-test-123456";
        let root = secret();
        for value in [
            json!({
                "invite":{"keyring":{"active_kid":"i","active_hs256_secret":shared}},
                "password_reset":{"keyring":{"active_kid":"r","active_hs256_secret":shared}}
            }),
            json!({
                "invite":{"keyring":{"active_kid":"i","active_hs256_secret":root}}
            }),
            json!({
                "invite":{"keyring":{"active_kid":"i","legacy_without_kid":{"kid":"i","verify_until":123}}}
            }),
            json!({
                "invite":{"keyring":{"active_kid":"i","verification_keys":[
                    {"kid":"old","material":shared,"verify_until":123}
                ]}},
                "password_reset":{"keyring":{"active_kid":"r","active_hs256_secret":shared}}
            }),
            json!({
                "invite":{"algorithm":"RS256","keyring":{"active_kid":"i"}}
            }),
        ] {
            let result = build_auth_config_with_env(
                root.clone(), 900,
                AuthSettingsOverrides { jwt_special_keyrings_env: Some("SPECIAL_KEYS".into()), ..Default::default() },
                |_| Ok(value.to_string()),
            );
            assert!(result.is_err());
        }
    }

    #[test]
    fn prepared_snapshot_is_shared_and_crypto_mutation_detaches() {
        let original = AuthConfig::new(secret());
        let before_first_use = original.clone();
        assert!(Arc::ptr_eq(&original.key_state, &before_first_use.key_state));
        let prepared = original.prepared_jwt().unwrap();
        assert!(std::ptr::eq(prepared, before_first_use.prepared_jwt().unwrap()));
        let policy_only = original.clone().with_issuer("other-issuer");
        assert!(std::ptr::eq(prepared, policy_only.prepared_jwt().unwrap()));
        let rsa = original.clone().with_rs256(
            crate::jwt::TEST_RSA_PRIVATE_KEY, crate::jwt::TEST_RSA_PUBLIC_KEY,
        );
        assert!(!Arc::ptr_eq(&original.key_state, &rsa.key_state));
        let rsa_prepared = rsa.prepared_jwt().unwrap();
        assert!(!std::ptr::eq(prepared, rsa_prepared));
        assert!(std::ptr::eq(prepared, original.prepared_jwt().unwrap()));
    }

    #[test]
    fn concurrent_first_use_and_failed_reconfiguration_are_isolated() {
        let mut config = AuthConfig::new(secret());
        let clone = config.clone();
        std::thread::scope(|scope| {
            let thread = scope.spawn(|| clone.prepared_jwt().unwrap() as *const _ as usize);
            let first = config.prepared_jwt().unwrap() as *const _ as usize;
            assert_eq!(thread.join().unwrap(), first);
        });
        let invalid = AuthSettingsOverrides {
            algorithm: Some(JwtAlgorithm::RS256),
            ..Default::default()
        };
        assert!(invalid.apply(&mut config).is_err());
        assert!(std::ptr::eq(config.prepared_jwt().unwrap(), clone.prepared_jwt().unwrap()));
        let invalid = AuthConfig::new("short".into());
        assert!(invalid.prepared_jwt().is_err());
        assert!(invalid.prepared_jwt().is_err());
    }

    fn secret() -> String {
        "test-secret-key-for-unit-tests-only-32bytes!".to_string()
    }

    #[test]
    fn auth_config_debug_redacts_secret_and_private_keys() {
        let config = AuthConfig::new("super-secret-auth-key-0123456789".to_string())
            .with_rs256("PRIVATE-KEY-MATERIAL", "PUBLIC-KEY-MATERIAL");
        let rendered = format!("{config:?}");

        assert!(!rendered.contains("super-secret-auth-key-0123456789"));
        assert!(!rendered.contains("PRIVATE-KEY-MATERIAL"));
        assert!(!rendered.contains("PUBLIC-KEY-MATERIAL"));
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn auth_settings_debug_redacts_inline_key_material() {
        let settings = AuthSettingsOverrides {
            rsa_private_key_pem: Some("PRIVATE-KEY-MATERIAL".to_string()),
            rsa_public_key_pem: Some("PUBLIC-KEY-MATERIAL".to_string()),
            rsa_private_key_env: Some("AUTH_PRIVATE".to_string()),
            rsa_public_key_env: Some("AUTH_PUBLIC".to_string()),
            ..AuthSettingsOverrides::default()
        };
        let rendered = format!("{settings:?}");

        assert!(!rendered.contains("PRIVATE-KEY-MATERIAL"));
        assert!(!rendered.contains("PUBLIC-KEY-MATERIAL"));
        assert!(rendered.contains("AUTH_PRIVATE"));
        assert!(rendered.contains("AUTH_PUBLIC"));
    }

    #[test]
    fn build_auth_config_defaults_to_hs256() {
        let config =
            build_auth_config_with_env(secret(), 900, AuthSettingsOverrides::default(), |_| {
                panic!("env resolver should not be called")
            })
            .expect("auth config");

        assert_eq!(config.algorithm(), &JwtAlgorithm::HS256);
        assert_eq!(config.access_expiration, 900);
        assert_eq!(config.refresh_expiration, DEFAULT_REFRESH_EXPIRATION_SECS);
        assert_eq!(
            config.min_password_length,
            DEFAULT_MIN_PASSWORD_LENGTH_CHARS
        );
    }

    #[test]
    fn build_auth_config_applies_min_password_length_override() {
        let config = build_auth_config_with_env(
            secret(),
            900,
            AuthSettingsOverrides {
                min_password_length: Some(16),
                ..AuthSettingsOverrides::default()
            },
            |_| panic!("env resolver should not be called"),
        )
        .expect("auth config");

        assert_eq!(config.min_password_length, 16);
    }

    #[test]
    fn build_auth_config_rejects_min_password_length_outside_bounds() {
        for invalid in [
            MIN_PASSWORD_LENGTH_FLOOR_CHARS - 1,
            MAX_MIN_PASSWORD_LENGTH_CHARS + 1,
        ] {
            assert!(
                build_auth_config_with_env(
                    secret(),
                    900,
                    AuthSettingsOverrides {
                        min_password_length: Some(invalid),
                        ..AuthSettingsOverrides::default()
                    },
                    |_| panic!("env resolver should not be called"),
                )
                .is_err(),
                "min_password_length {invalid} must be rejected"
            );
        }
    }

    #[test]
    fn build_auth_config_resolves_rs256_keys_from_env() {
        let config = build_auth_config_with_env(
            secret(),
            900,
            AuthSettingsOverrides {
                algorithm: Some(JwtAlgorithm::RS256),
                rsa_private_key_env: Some("PRIVATE".to_string()),
                rsa_public_key_env: Some("PUBLIC".to_string()),
                ..AuthSettingsOverrides::default()
            },
            |name| match name {
                "PRIVATE" => Ok(crate::jwt::TEST_RSA_PRIVATE_KEY.to_string()),
                "PUBLIC" => Ok(crate::jwt::TEST_RSA_PUBLIC_KEY.to_string()),
                _ => panic!("unexpected env key"),
            },
        )
        .expect("auth config");

        assert_eq!(
            config.rsa_private_key_pem(),
            Some(crate::jwt::TEST_RSA_PRIVATE_KEY)
        );
        assert_eq!(
            config.rsa_public_key_pem(),
            Some(crate::jwt::TEST_RSA_PUBLIC_KEY)
        );
    }

    #[test]
    fn rs256_configuration_still_requires_strong_symmetric_secret() {
        for weak in ["", "short-secret", "abc                              "] {
            let config = AuthConfig::new(weak.to_string()).with_rs256(
                crate::jwt::TEST_RSA_PRIVATE_KEY,
                crate::jwt::TEST_RSA_PUBLIC_KEY,
            );
            assert!(validate_auth_config(&config).is_err());
        }
    }

    #[test]
    fn build_auth_config_rejects_rs256_without_keys() {
        let result = build_auth_config_with_env(
            secret(),
            900,
            AuthSettingsOverrides {
                algorithm: Some(JwtAlgorithm::RS256),
                ..AuthSettingsOverrides::default()
            },
            |_| panic!("env resolver should not be called"),
        );

        assert!(result.is_err());
    }

    #[test]
    fn build_auth_config_rejects_malformed_rs256_keys() {
        let result = build_auth_config_with_env(
            secret(),
            900,
            AuthSettingsOverrides {
                algorithm: Some(JwtAlgorithm::RS256),
                rsa_private_key_pem: Some("not-a-private-key".to_string()),
                rsa_public_key_pem: Some("not-a-public-key".to_string()),
                ..AuthSettingsOverrides::default()
            },
            |_| panic!("env resolver should not be called"),
        );

        assert!(result.is_err());
    }

    #[test]
    fn build_auth_config_rejects_mismatched_rs256_key_pair() {
        let different_public_key = crate::jwt::TEST_RSA_PUBLIC_KEY.replacen("3bmu", "3cmu", 1);
        let result = build_auth_config_with_env(
            secret(),
            900,
            AuthSettingsOverrides {
                algorithm: Some(JwtAlgorithm::RS256),
                rsa_private_key_pem: Some(crate::jwt::TEST_RSA_PRIVATE_KEY.to_string()),
                rsa_public_key_pem: Some(different_public_key),
                ..AuthSettingsOverrides::default()
            },
            |_| panic!("env resolver should not be called"),
        );

        assert!(result.is_err());
    }

    #[test]
    fn build_auth_config_rejects_weak_hs256_secret() {
        let result = build_auth_config_with_env(
            "short-secret".to_string(),
            900,
            AuthSettingsOverrides::default(),
            |_| panic!("env resolver should not be called"),
        );

        assert!(result.is_err());
    }

    #[test]
    fn build_auth_config_rejects_blank_issuer_and_audience() {
        for overrides in [
            AuthSettingsOverrides {
                issuer: Some("  ".to_string()),
                ..AuthSettingsOverrides::default()
            },
            AuthSettingsOverrides {
                audience: Some(String::new()),
                ..AuthSettingsOverrides::default()
            },
        ] {
            assert!(
                build_auth_config_with_env(secret(), 900, overrides, |_| {
                    panic!("env resolver should not be called")
                })
                .is_err()
            );
        }
    }

    #[test]
    fn build_auth_config_rejects_invalid_ttl_bounds() {
        assert!(
            build_auth_config_with_env(
                secret(),
                MIN_ACCESS_EXPIRATION_SECS - 1,
                AuthSettingsOverrides::default(),
                |_| panic!("env resolver should not be called"),
            )
            .is_err()
        );

        assert!(
            build_auth_config_with_env(
                secret(),
                900,
                AuthSettingsOverrides {
                    refresh_expiration: Some(300),
                    ..AuthSettingsOverrides::default()
                },
                |_| panic!("env resolver should not be called"),
            )
            .is_err()
        );
    }

    #[test]
    fn build_auth_config_rejects_rsa_material_with_hs256() {
        let result = build_auth_config_with_env(
            secret(),
            900,
            AuthSettingsOverrides {
                rsa_private_key_pem: Some("private".to_string()),
                rsa_public_key_pem: Some("public".to_string()),
                ..AuthSettingsOverrides::default()
            },
            |_| panic!("env resolver should not be called"),
        );

        assert!(result.is_err());
    }
}
