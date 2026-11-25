use std::sync::{Arc, OnceLock, RwLock};

use jsonwebtoken::{Algorithm, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::credentials::{
    DEFAULT_MIN_PASSWORD_LENGTH_CHARS, MAX_MIN_PASSWORD_LENGTH_CHARS,
    MIN_PASSWORD_LENGTH_FLOOR_CHARS,
};
use crate::error::{AuthError, Result};
use crate::jwt_keyring::{
    JwtKeyring, SpecialJwtKeyring, SpecialJwtKeyrings, ValidatedJwtKeyrings, rsa_signing_key, verification_key,
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
    root: Arc<str>,
    key_state: JwtKeySource,
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
pub(crate) struct JwtKeyInputs {
    pub(crate) jwt_keyring: JwtKeyring,
    pub(crate) special_jwt_keyrings: SpecialJwtKeyrings,
    pub(crate) algorithm: JwtAlgorithm,
    pub(crate) rsa_private_key_pem: Option<String>,
    pub(crate) rsa_public_key_pem: Option<String>,
}

#[derive(Clone)]
enum JwtKeySource {
    Fixed(Arc<JwtKeyState>),
    Live(Arc<RwLock<VersionedJwtKeys>>),
}

struct VersionedJwtKeys {
    generation: u64,
    keys: Arc<JwtKeyState>,
}

/// Host-only publication capability. Request consumers receive AuthConfig,
/// not this handle. The four prepared rings and generation move together.
pub struct JwtKeyringPublisher {
    config: AuthConfig,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JwtKeyringDocument {
    schema_version: u16,
    generation: u64,
    access: SpecialJwtKeyring,
    password_reset: SpecialJwtKeyring,
    email_verification: SpecialJwtKeyring,
    invite: SpecialJwtKeyring,
}

pub const MAX_JWT_KEYRING_DOCUMENT_BYTES: usize = 256 * 1024;

impl JwtKeyState {
    fn prepared(&self, root: &str) -> Result<&ValidatedJwtKeyrings> {
        self.prepared
            .get_or_init(|| ValidatedJwtKeyrings::prepare(&self.inputs, root))
            .as_ref()
            .map_err(Clone::clone)
    }
}

fn keyring_runtime_error() -> AuthError {
    AuthError::Internal("JWT keyring runtime state is unavailable".to_string())
}

fn invalid_rotation() -> AuthError {
    AuthError::Internal("JWT keyring rotation candidate is incompatible".to_string())
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
            .field("key_material", &"<redacted>")
            .field("access_expiration", &self.access_expiration)
            .field("refresh_expiration", &self.refresh_expiration)
            .field("issuer", &self.issuer)
            .field("audience", &self.audience)
            .field("min_password_length", &self.min_password_length)
            .finish_non_exhaustive()
    }
}

impl AuthConfig {
    /// Stable host MAC root, distinct from the active JWT signing material.
    pub fn secret(&self) -> &str {
        &self.root
    }

    fn key_snapshot(&self) -> Result<Arc<JwtKeyState>> {
        match &self.key_state {
            JwtKeySource::Fixed(keys) => Ok(keys.clone()),
            JwtKeySource::Live(current) => current.read()
                .map(|current| current.keys.clone())
                .map_err(|_| keyring_runtime_error()),
        }
    }

    /// Inspect current access algorithm without retaining a stale raw configuration.
    pub fn algorithm(&self) -> Result<JwtAlgorithm> {
        Ok(self.key_snapshot()?.inputs.algorithm.clone())
    }

    pub fn rsa_private_key_pem(&self) -> Result<Option<String>> {
        Ok(self.key_snapshot()?.inputs.rsa_private_key_pem.clone())
    }

    pub fn rsa_public_key_pem(&self) -> Result<Option<String>> {
        Ok(self.key_snapshot()?.inputs.rsa_public_key_pem.clone())
    }

    /// Run a synchronous cryptographic operation under a read fence.
    /// Publication waits for in-flight crypto, but never for async DB/network IO.
    pub(crate) fn with_prepared_jwt<T>(
        &self, operation: impl FnOnce(&ValidatedJwtKeyrings) -> Result<T>,
    ) -> Result<T> {
        match &self.key_state {
            JwtKeySource::Fixed(keys) => operation(keys.prepared(self.secret())?),
            JwtKeySource::Live(current) => {
                let guard = current.read().map_err(|_| keyring_runtime_error())?;
                operation(guard.keys.prepared(self.secret())?)
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn prepared_jwt(&self) -> Result<&ValidatedJwtKeyrings> {
        match &self.key_state {
            JwtKeySource::Fixed(keys) => keys.prepared(self.secret()),
            JwtKeySource::Live(_) => Err(invalid_rotation()),
        }
    }

    #[cfg(test)]
    pub(crate) fn fixed_inputs(&self) -> Result<&JwtKeyInputs> {
        match &self.key_state {
            JwtKeySource::Fixed(keys) => Ok(&keys.inputs),
            JwtKeySource::Live(_) => Err(invalid_rotation()),
        }
    }

    fn edit_key_inputs(&mut self, edit: impl FnOnce(&mut JwtKeyInputs)) -> Result<()> {
        let mut keys = self.key_snapshot()?;
        let state = Arc::make_mut(&mut keys);
        state.prepared = OnceLock::new();
        edit(&mut state.inputs);
        self.key_state = JwtKeySource::Fixed(keys);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn set_test_jwt_keyring(&mut self, ring: JwtKeyring) {
        self.edit_key_inputs(|inputs| inputs.jwt_keyring = ring).unwrap();
    }

    #[cfg(test)]
    pub(crate) fn set_test_special_jwt_keyrings(&mut self, rings: SpecialJwtKeyrings) {
        self.edit_key_inputs(|inputs| inputs.special_jwt_keyrings = rings).unwrap();
    }

    /// Inspect all current explicit HS256 material without exporting a secret copy.
    /// The visitor must not retain or log the material.
    pub fn inspect_hs256_secrets(
        &self, mut inspect: impl FnMut(&str) -> Result<()>,
    ) -> Result<()> {
        let keys = self.key_snapshot()?;
        for secret in crate::jwt_keyring::explicit_hs256_secrets(&keys.inputs, self.secret()) {
            inspect(secret)?;
        }
        Ok(())
    }

    pub fn new(secret: String) -> Self {
        Self {
            root: Arc::from(secret),
            key_state: JwtKeySource::Fixed(Arc::new(JwtKeyState {
                inputs: JwtKeyInputs {
                    jwt_keyring: JwtKeyring::default(),
                    special_jwt_keyrings: SpecialJwtKeyrings::default(),
                    algorithm: JwtAlgorithm::HS256,
                    rsa_private_key_pem: None,
                    rsa_public_key_pem: None,
                },
                prepared: OnceLock::new(),
            })),
            access_expiration: DEFAULT_ACCESS_EXPIRATION_SECS,
            refresh_expiration: DEFAULT_REFRESH_EXPIRATION_SECS,
            issuer: "rustok".to_string(),
            audience: "rustok-admin".to_string(),
            min_password_length: DEFAULT_MIN_PASSWORD_LENGTH_CHARS,
        }
    }

    /// Parse a complete versioned key document against this immutable policy.
    /// Never mutate the live runtime while preparing a candidate.
    pub fn prepare_jwt_document(&self, bytes: &[u8]) -> Result<(u64, Self)> {
        if bytes.len() > MAX_JWT_KEYRING_DOCUMENT_BYTES {
            return Err(invalid_rotation());
        }
        let doc: JwtKeyringDocument = serde_json::from_slice(bytes)
            .map_err(|_| invalid_rotation())?;
        if doc.schema_version != 1 || doc.generation == 0 {
            return Err(invalid_rotation());
        }
        let mut candidate = self.clone();
        candidate.key_state = JwtKeySource::Fixed(Arc::new(JwtKeyState {
            inputs: JwtKeyInputs {
                algorithm: doc.access.algorithm,
                jwt_keyring: doc.access.keyring,
                rsa_private_key_pem: doc.access.rsa_private_key_pem,
                rsa_public_key_pem: doc.access.rsa_public_key_pem,
                special_jwt_keyrings: SpecialJwtKeyrings {
                    password_reset: doc.password_reset,
                    email_verification: doc.email_verification,
                    invite: doc.invite,
                },
            },
            prepared: OnceLock::new(),
        }));
        validate_auth_config(&candidate)?;
        Ok((doc.generation, candidate))
    }

    pub(crate) fn fence_snapshot(
        &self,
        fingerprint: impl Fn(&[u8; 32]) -> Result<String>,
    ) -> Result<crate::jwt_fence::Snapshot> {
        if !matches!(&self.key_state, JwtKeySource::Fixed(_)) {
            return Err(invalid_rotation());
        }
        self.with_prepared_jwt(|prepared| prepared.fence_snapshot(fingerprint))
    }

    /// Convert a validated fixed candidate into a shared process-local runtime.
    /// Call before cloning into the host's transports and providers.
    pub fn into_live_jwt(mut self, generation: u64) -> Result<(Self, JwtKeyringPublisher)> {
        if generation == 0 || !matches!(&self.key_state, JwtKeySource::Fixed(_)) {
            return Err(invalid_rotation());
        }
        validate_auth_config(&self)?;
        let keys = self.key_snapshot()?;
        self.key_state = JwtKeySource::Live(Arc::new(RwLock::new(VersionedJwtKeys {
            generation,
            keys,
        })));
        let publisher = JwtKeyringPublisher { config: self.clone() };
        Ok((self, publisher))
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
        // A builder creates a detached fixed configuration, never a publication.
        // On an unavailable live source keep the failing source rather than
        // constructing a default signer.
        if let Err(error) = self.edit_key_inputs(|inputs| {
            inputs.algorithm = JwtAlgorithm::RS256;
            inputs.rsa_private_key_pem = Some(private_key_pem.into());
            inputs.rsa_public_key_pem = Some(public_key_pem.into());
        }) {
            tracing::error!(%error, "Cannot detach JWT configuration");
        }
        self
    }

    pub fn with_min_password_length(mut self, min_password_length: usize) -> Self {
        self.min_password_length = min_password_length;
        self
    }
}

impl JwtKeyringPublisher {
    pub fn generation(&self) -> Result<u64> {
        match &self.config.key_state {
            JwtKeySource::Live(current) => current.read()
                .map(|current| current.generation)
                .map_err(|_| keyring_runtime_error()),
            JwtKeySource::Fixed(_) => Err(invalid_rotation()),
        }
    }

    pub fn prepare_document(&self, bytes: &[u8]) -> Result<(u64, AuthConfig)> {
        self.config.prepare_jwt_document(bytes)
    }

    /// Candidate preparation and host policy checks happen before this call.
    /// Recheck the process generation and key continuity under the write fence.
    pub fn publish(&self, generation: u64, candidate: AuthConfig) -> Result<()> {
        if !matches!(&candidate.key_state, JwtKeySource::Fixed(_))
            || self.config.secret() != candidate.secret()
            || self.config.issuer != candidate.issuer
            || self.config.audience != candidate.audience
            || self.config.access_expiration != candidate.access_expiration
            || self.config.refresh_expiration != candidate.refresh_expiration
            || self.config.min_password_length != candidate.min_password_length
        {
            return Err(invalid_rotation());
        }
        validate_auth_config(&candidate)?;
        let keys = candidate.key_snapshot()?;
        let next = keys.prepared(candidate.secret())?;
        let JwtKeySource::Live(current) = &self.config.key_state else {
            return Err(invalid_rotation());
        };
        let mut guard = current.write().map_err(|_| keyring_runtime_error())?;
        if generation <= guard.generation {
            return Err(invalid_rotation());
        }
        guard.keys.prepared(self.config.secret())?.validate_successor(next)?;
        *guard = VersionedJwtKeys { generation, keys };
        Ok(())
    }
}

/// Helper for loading auth settings from nested YAML `settings.auth`.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
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
        if matches!(&config.key_state, JwtKeySource::Live(_)) {
            return Err(invalid_rotation());
        }
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
            let ring = JwtKeyring::from_json(&env_resolver(&name)?)?;
            config.edit_key_inputs(|inputs| inputs.jwt_keyring = ring)?;
        }
        if let Some(name) = self.jwt_special_keyrings_env {
            if name.trim().is_empty() {
                return Err(AuthError::Internal("Special JWT keyring environment name is empty".to_string()));
            }
            let rings = SpecialJwtKeyrings::from_json(&env_resolver(&name)?)?;
            config.edit_key_inputs(|inputs| inputs.special_jwt_keyrings = rings)?;
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
            config.edit_key_inputs(|inputs| inputs.algorithm = v)?;
        }
        if self.rsa_private_key_pem.is_some() || self.rsa_private_key_env.is_some() {
            let pem = resolve_key_material(
                self.rsa_private_key_pem, self.rsa_private_key_env, env_resolver,
            )?;
            config.edit_key_inputs(|inputs| inputs.rsa_private_key_pem = pem)?;
        }
        if self.rsa_public_key_pem.is_some() || self.rsa_public_key_env.is_some() {
            let pem = resolve_key_material(
                self.rsa_public_key_pem, self.rsa_public_key_env, env_resolver,
            )?;
            config.edit_key_inputs(|inputs| inputs.rsa_public_key_pem = pem)?;
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
        algorithm = ?config.algorithm()?,
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

    config.with_prepared_jwt(|_| Ok(()))
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
    fn auth_overrides_reject_unknown_security_settings() {
        for value in [
            serde_json::json!({"algoritm": "RS256"}),
            serde_json::json!({"jwt_keyring_en": "JWT_KEYS"}),
            serde_json::json!({"jwt_special_keyring_env": "SPECIAL_KEYS"}),
            serde_json::json!({"rsa_public_key_en": "RSA_PUBLIC"}),
        ] {
            assert!(serde_json::from_value::<AuthSettingsOverrides>(value).is_err());
        }
        let valid = serde_json::from_value::<AuthSettingsOverrides>(serde_json::json!({
            "algorithm": "RS256",
            "jwt_keyring_env": "JWT_KEYS",
            "jwt_special_keyrings_env": "SPECIAL_KEYS",
            "rsa_public_key_env": "RSA_PUBLIC",
            "rsa_private_key_env": "RSA_PRIVATE"
        })).unwrap();
        assert_eq!(valid.jwt_keyring_env.as_deref(), Some("JWT_KEYS"));
        assert!(serde_json::from_value::<AuthSettingsOverrides>(serde_json::json!({})).is_ok());
    }

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

    fn document(generation: u64, kid: &str, secret: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "generation": generation,
            "access": {"keyring": {"active_kid": kid, "active_hs256_secret": secret}},
            "password_reset": {"keyring": {"active_kid": "reset-default"}},
            "email_verification": {"keyring": {"active_kid": "verify-default"}},
            "invite": {"keyring": {"active_kid": "invite-default"}}
        })).unwrap()
    }

    #[test]
    fn live_rotation_updates_existing_clones_and_rejects_rollback() {
        use crate::{encode_access_token, decode_access_token};
        use rustok_core::UserRole;
        use uuid::Uuid;
        let base = AuthConfig::new(secret());
        let (generation, first) = base.prepare_jwt_document(&document(
            1, "one", "first-independent-signing-key-at-least-32-bytes",
        )).unwrap();
        let (live, publisher) = first.into_live_jwt(generation).unwrap();
        let clone = live.clone();
        let issue = |config: &AuthConfig| encode_access_token(
            config, Uuid::new_v4(), Uuid::new_v4(), UserRole::Customer, Uuid::new_v4(),
        ).unwrap();
        let old = issue(&clone);
        let (g, next) = publisher.prepare_document(&document(
            2, "two", "second-independent-signing-key-at-least-32-bytes",
        )).unwrap();
        publisher.publish(g, next.clone()).unwrap();
        assert_eq!(publisher.generation().unwrap(), 2);
        assert!(decode_access_token(&clone, &old).is_err());
        let fresh = issue(&clone);
        assert!(decode_access_token(&live, &fresh).is_ok());
        assert!(decode_access_token(&next, &fresh).is_ok());
        assert!(publisher.publish(1, next.clone()).is_err());
        assert!(publisher.publish(2, next).is_err());
        assert!(publisher.prepare_document(b"{}").is_err());
        assert_eq!(publisher.generation().unwrap(), 2);
        assert!(decode_access_token(&clone, &fresh).is_ok());

        let (g, reused) = publisher.prepare_document(&document(
            3, "two", "changed-independent-signing-key-at-least-32-bytes",
        )).unwrap();
        assert!(publisher.publish(g, reused).is_err());
        assert_eq!(publisher.generation().unwrap(), 2);
    }

    #[test]
    fn live_rotation_supports_stage_promote_and_retire() {
        use crate::{decode_access_token, encode_access_token};
        use rustok_core::UserRole;
        use uuid::Uuid;
        let old_secret = "first-independent-signing-key-at-least-32-bytes";
        let new_secret = "second-independent-signing-key-at-least-32-bytes";
        let base = AuthConfig::new(secret());
        let (generation, first) = base.prepare_jwt_document(&document(1, "one", old_secret)).unwrap();
        let (live, publisher) = first.into_live_jwt(generation).unwrap();
        let existing_clone = live.clone();
        let issue = || encode_access_token(
            &existing_clone, Uuid::new_v4(), Uuid::new_v4(), UserRole::Customer, Uuid::new_v4(),
        ).unwrap();
        let old = issue();
        let until = chrono::Utc::now().timestamp() + 3600;
        let with_history = |generation, active_kid, active_secret, history_kid, history_secret| {
            let mut value: serde_json::Value = serde_json::from_slice(
                &document(generation, active_kid, active_secret)
            ).unwrap();
            value["access"]["keyring"]["verification_keys"] = serde_json::json!([{
                "kid": history_kid, "material": history_secret, "verify_until": until
            }]);
            serde_json::to_vec(&value).unwrap()
        };
        let (g, staged) = publisher.prepare_document(
            &with_history(2, "one", old_secret, "two", new_secret)
        ).unwrap();
        publisher.publish(g, staged).unwrap();
        assert_eq!(jsonwebtoken::decode_header(&issue()).unwrap().kid.as_deref(), Some("one"));
        let (g, promoted) = publisher.prepare_document(
            &with_history(3, "two", new_secret, "one", old_secret)
        ).unwrap();
        publisher.publish(g, promoted).unwrap();
        let fresh = issue();
        assert_eq!(jsonwebtoken::decode_header(&fresh).unwrap().kid.as_deref(), Some("two"));
        assert!(decode_access_token(&live, &old).is_ok());
        assert!(decode_access_token(&live, &fresh).is_ok());
        let (g, retired) = publisher.prepare_document(&document(4, "two", new_secret)).unwrap();
        publisher.publish(g, retired).unwrap();
        assert!(decode_access_token(&existing_clone, &old).is_err());
        assert!(decode_access_token(&existing_clone, &fresh).is_ok());
        assert_eq!(publisher.generation().unwrap(), 4);
    }

    #[test]
    fn live_rotation_serializes_concurrent_publishers() {
        let base = AuthConfig::new(secret());
        let (g, first) = base.prepare_jwt_document(&document(
            1, "one", "first-independent-signing-key-at-least-32-bytes",
        )).unwrap();
        let (_, publisher) = first.into_live_jwt(g).unwrap();
        let (g2, second) = publisher.prepare_document(&document(
            2, "two", "second-independent-signing-key-at-least-32-bytes",
        )).unwrap();
        let (g3, third) = publisher.prepare_document(&document(
            3, "three", "third-independent-signing-key-at-least-32-bytes",
        )).unwrap();
        std::thread::scope(|scope| {
            let a = scope.spawn(|| publisher.publish(g2, second));
            let b = scope.spawn(|| publisher.publish(g3, third));
            let _second_result = a.join().unwrap();
            b.join().unwrap().unwrap();
        });
        assert_eq!(publisher.generation().unwrap(), 3);
    }

    #[test]
    fn prepared_snapshot_is_shared_and_crypto_mutation_detaches() {
        let original = AuthConfig::new(secret());
        let before_first_use = original.clone();
        assert!(Arc::ptr_eq(&original.key_snapshot().unwrap(), &before_first_use.key_snapshot().unwrap()));
        let prepared = original.prepared_jwt().unwrap();
        assert!(std::ptr::eq(prepared, before_first_use.prepared_jwt().unwrap()));
        let policy_only = original.clone().with_issuer("other-issuer");
        assert!(std::ptr::eq(prepared, policy_only.prepared_jwt().unwrap()));
        let rsa = original.clone().with_rs256(
            crate::jwt::TEST_RSA_PRIVATE_KEY, crate::jwt::TEST_RSA_PUBLIC_KEY,
        );
        assert!(!Arc::ptr_eq(&original.key_snapshot().unwrap(), &rsa.key_snapshot().unwrap()));
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

        assert_eq!(config.algorithm().unwrap(), JwtAlgorithm::HS256);
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
            config.rsa_private_key_pem().unwrap().as_deref(),
            Some(crate::jwt::TEST_RSA_PRIVATE_KEY)
        );
        assert_eq!(
            config.rsa_public_key_pem().unwrap().as_deref(),
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
