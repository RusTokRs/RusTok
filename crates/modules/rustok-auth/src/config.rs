use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

use crate::jwt_clock::{ClockGuard, ClockSample, LeaseDeadline};

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
pub(crate) const MIN_SYMMETRIC_SECRET_BYTES: usize = 32;
const MIN_PRODUCTION_MAC_ROOT_BYTES: usize = 64;

/// The shared minimum for symmetric cryptographic material, independent of
/// JWT signing policy. Interior whitespace does not count toward the floor.
pub(crate) fn symmetric_secret_has_min_length(secret: &str) -> bool {
    secret
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .count()
        >= MIN_SYMMETRIC_SECRET_BYTES
}

/// The MAC root must be independently provisioned symmetric material, not
/// a PEM-encoded public or private key (or certificate). This checks the
/// representation and length floor, not the entropy of supplied material.
pub(crate) fn mac_root_is_valid(root: &str) -> bool {
    symmetric_secret_has_min_length(root) && !root.contains("-----BEGIN ")
}

fn mac_root_looks_like_placeholder(root: &str) -> bool {
    let normalized = root.trim().to_ascii_lowercase();
    const PLACEHOLDER_MARKERS: &[&str] = &[
        "change_me", "changeme", "replace_me", "replace-this", "do-not-use",
        "development", "example-secret", "test-secret",
    ];
    PLACEHOLDER_MARKERS.iter().any(|marker| normalized.contains(marker))
        || normalized.chars().collect::<std::collections::HashSet<_>>().len() < 16
}

const MIN_ACCESS_EXPIRATION_SECS: u64 = 60;
const MAX_ACCESS_EXPIRATION_SECS: u64 = 60 * 60 * 24 * 30;
const MIN_REFRESH_EXPIRATION_SECS: u64 = 60 * 5;
const MAX_REFRESH_EXPIRATION_SECS: u64 = 60 * 60 * 24 * 365;

#[derive(Debug, Deserialize, Serialize)]
struct Rs256KeyPairProbe {
    probe: String,
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
    pub(crate) rsa_private_key_pem: String,
    pub(crate) rsa_public_key_pem: String,
}

#[derive(Clone)]
enum JwtKeySource {
    Fixed(Arc<JwtKeyState>),
    Live(Arc<RwLock<VersionedJwtKeys>>),
}

struct VersionedJwtKeys {
    generation: u64,
    keys: Arc<JwtKeyState>,
    lease_required: bool,
    lease_until: Option<LeaseDeadline>,
    clock: Arc<ClockGuard>,
    authority_clock: Option<Arc<ClockGuard>>,
    emergency_fenced: bool,
}

impl VersionedJwtKeys {
    fn clock_healthy_at(&self, now: ClockSample) -> bool {
        if !self.clock.healthy_at(now) {
            return false;
        }
        if self.authority_clock.as_ref().is_some_and(|clock| !clock.healthy_at(now)) {
            self.clock.trip();
        }
        !self.clock.faulted()
    }

    fn permits_crypto_at(&self, now: ClockSample) -> bool {
        !self.emergency_fenced && (!self.lease_required || (
            self.clock_healthy_at(now)
                && self.lease_until.as_ref().is_some_and(|lease| lease.remaining(now) > Duration::ZERO)
        ))
    }
}

/// Read-only readiness view of the exact lease gate used by JWT operations.
/// It does not expose key material, generation mutation, or renewal authority.
#[derive(Clone)]
pub struct JwtLeaseReadiness {
    current: Arc<RwLock<VersionedJwtKeys>>,
}

impl JwtLeaseReadiness {
    /// Sample the real lease gate under a single read fence.
    pub(crate) fn operational_state(&self) -> Option<(u64, bool, bool, f64, bool)> {
        let guard = self.current.read().ok()?;
        let now = ClockSample::now();
        let clock_healthy = guard.clock_healthy_at(now);
        let ready = guard.lease_required && guard.permits_crypto_at(now);
        let remaining = if ready {
            guard.lease_until.as_ref()?.remaining(now).as_secs_f64()
        } else {
            0.0
        };
        Some((guard.generation, ready, guard.emergency_fenced, remaining, !clock_healthy || guard.clock.faulted()))
    }

    pub fn is_ready(&self) -> bool {
        self.current.read().is_ok_and(|guard| {
            guard.lease_required && guard.permits_crypto_at(ClockSample::now())
        })
    }
}

/// Host-only publication capability. Request consumers receive AuthConfig,
/// not this handle. The four prepared rings and generation move together.
pub struct JwtKeyringPublisher {
    config: AuthConfig,
    incarnation: uuid::Uuid,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JwtKeyringDocument {
    generation: u64,
    access: SpecialJwtKeyring,
    password_reset: SpecialJwtKeyring,
    email_verification: SpecialJwtKeyring,
    invite: SpecialJwtKeyring,
}

pub const MAX_JWT_KEYRING_DOCUMENT_BYTES: usize = 256 * 1024;

impl JwtKeyringDocument {
    fn parse(bytes: &[u8]) -> Result<(u64, JwtKeyInputs)> {
        if bytes.len() > MAX_JWT_KEYRING_DOCUMENT_BYTES {
            return Err(invalid_rotation());
        }
        let doc: Self = serde_json::from_slice(bytes).map_err(|_| invalid_rotation())?;
        if doc.generation == 0 || i64::try_from(doc.generation).is_err() {
            return Err(invalid_rotation());
        }
        Ok((doc.generation, JwtKeyInputs {
            jwt_keyring: doc.access.keyring,
            rsa_private_key_pem: doc.access.rsa_private_key_pem,
            rsa_public_key_pem: doc.access.rsa_public_key_pem,
            special_jwt_keyrings: SpecialJwtKeyrings {
                password_reset: doc.password_reset,
                email_verification: doc.email_verification,
                invite: doc.invite,
            },
        }))
    }
}

impl JwtKeyInputs {
    fn into_fixed_source(self) -> JwtKeySource {
        JwtKeySource::Fixed(Arc::new(JwtKeyState {
            inputs: self,
            prepared: OnceLock::new(),
        }))
    }
}

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
            .field("mac_root", &"<redacted>")
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
    /// Keep extraction inside Auth; downstream consumers use purpose-scoped APIs.
    pub(crate) fn secret(&self) -> &str {
        &self.root
    }

    /// Enforce the stronger deployment-only MAC root policy without exporting
    /// the raw root to host composition or other dependent crates.
    pub fn validate_production_mac_root(&self) -> Result<()> {
        let root = self.secret();
        if root.bytes().filter(|byte| !byte.is_ascii_whitespace()).count()
            < MIN_PRODUCTION_MAC_ROOT_BYTES
        {
            return Err(AuthError::Validation(format!(
                "Production MAC root must contain at least {MIN_PRODUCTION_MAC_ROOT_BYTES} non-whitespace bytes"
            )));
        }
        if mac_root_looks_like_placeholder(root) {
            return Err(AuthError::Validation(
                "Production MAC root looks like a placeholder or low-diversity value".to_string(),
            ));
        }
        Ok(())
    }

    fn key_snapshot(&self) -> Result<Arc<JwtKeyState>> {
        match &self.key_state {
            JwtKeySource::Fixed(keys) => Ok(keys.clone()),
            JwtKeySource::Live(current) => current.read()
                .map(|current| current.keys.clone())
                .map_err(|_| keyring_runtime_error()),
        }
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

    /// Only JWT operations use the lease gate; candidate validation and
    /// metadata inspection never require a request-serving lease.
    pub(crate) fn with_jwt_operation<T>(
        &self, operation: impl FnOnce(&ValidatedJwtKeyrings) -> Result<T>,
    ) -> Result<T> {
        self.with_jwt_operation_at(ClockSample::now, operation)
    }

    fn with_jwt_operation_at<T>(
        &self, now: impl Fn() -> ClockSample,
        operation: impl FnOnce(&ValidatedJwtKeyrings) -> Result<T>,
    ) -> Result<T> {
        match &self.key_state {
            JwtKeySource::Fixed(keys) => operation(keys.prepared(self.secret())?),
            JwtKeySource::Live(current) => {
                let guard = current.read().map_err(|_| keyring_runtime_error())?;
                let authorized = || guard.permits_crypto_at(now());
                if !authorized() {
                    return Err(keyring_runtime_error());
                }
                let result = operation(guard.keys.prepared(self.secret())?);
                if !authorized() {
                    return Err(keyring_runtime_error());
                }
                result
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

    /// Exercise lazy materialization and direct runtime rejection in tests.
    /// Production construction must validate the complete document.
    #[cfg(test)]
    pub(crate) fn from_test_document_unvalidated(root: String, bytes: &[u8]) -> Self {
        let (_, inputs) = JwtKeyringDocument::parse(bytes).unwrap();
        Self::from_key_inputs(root, inputs)
    }

    fn from_key_inputs(secret: String, inputs: JwtKeyInputs) -> Self {
        Self {
            root: Arc::from(secret),
            key_state: inputs.into_fixed_source(),
            access_expiration: DEFAULT_ACCESS_EXPIRATION_SECS,
            refresh_expiration: DEFAULT_REFRESH_EXPIRATION_SECS,
            issuer: "rustok".to_string(),
            audience: "rustok-admin".to_string(),
            min_password_length: DEFAULT_MIN_PASSWORD_LENGTH_CHARS,
        }
    }

    /// Parse a complete generation-bearing key document against this immutable policy.
    /// Never mutate the live runtime while preparing a candidate.
    pub fn prepare_jwt_document(&self, bytes: &[u8]) -> Result<(u64, Self)> {
        let (generation, inputs) = JwtKeyringDocument::parse(bytes)?;
        let mut candidate = self.clone();
        candidate.key_state = inputs.into_fixed_source();
        validate_auth_config(&candidate)?;
        Ok((generation, candidate))
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
    pub fn into_live_jwt(self, generation: u64) -> Result<(Self, JwtKeyringPublisher)> {
        self.into_jwt_runtime(generation, false)
    }

    /// HTTP host composition: a positive enrolled generation is mandatory.
    /// JWT operations fail closed until a matching durable lease is installed.
    pub fn into_cluster_jwt(self, generation: u64) -> Result<(Self, JwtKeyringPublisher)> {
        self.into_jwt_runtime(generation, true)
    }

    fn into_jwt_runtime(mut self, generation: u64, lease_required: bool) -> Result<(Self, JwtKeyringPublisher)> {
        if generation == 0
            || i64::try_from(generation).is_err()
            || !matches!(&self.key_state, JwtKeySource::Fixed(_))
        {
            return Err(invalid_rotation());
        }
        validate_auth_config(&self)?;
        let keys = self.key_snapshot()?;
        self.key_state = JwtKeySource::Live(Arc::new(RwLock::new(VersionedJwtKeys {
            generation,
            keys,
            lease_required,
            lease_until: None,
            clock: Arc::new(ClockGuard::new()),
            authority_clock: None,
            emergency_fenced: false,
        })));
        let publisher = JwtKeyringPublisher { config: self.clone(), incarnation: uuid::Uuid::new_v4() };
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

    /// Obtain a read-only view for the host readiness probe.
    pub fn lease_readiness(&self) -> Result<JwtLeaseReadiness> {
        let JwtKeySource::Live(current) = &self.config.key_state else {
            return Err(invalid_rotation());
        };
        if !current.read().map_err(|_| keyring_runtime_error())?.lease_required {
            return Err(invalid_rotation());
        }
        Ok(JwtLeaseReadiness { current: current.clone() })
    }

    pub(crate) fn incarnation(&self) -> uuid::Uuid {
        self.incarnation
    }

    pub(crate) fn lease_root(&self) -> &str {
        self.config.secret()
    }

    pub(crate) fn lease_metadata(
        &self, fingerprint: impl Fn(&[u8; 32]) -> Result<String>,
    ) -> Result<(u64, crate::jwt_fence::Snapshot)> {
        let JwtKeySource::Live(current) = &self.config.key_state else {
            return Err(invalid_rotation());
        };
        let guard = current.read().map_err(|_| keyring_runtime_error())?;
        if !guard.lease_required {
            return Err(invalid_rotation());
        }
        let snapshot = guard.keys.prepared(self.config.secret())?.fence_snapshot(fingerprint)?;
        Ok((guard.generation, snapshot))
    }

    pub(crate) fn clock_healthy(&self) -> Result<bool> {
        let JwtKeySource::Live(current) = &self.config.key_state else {
            return Err(invalid_rotation());
        };
        let guard = current.read().map_err(|_| keyring_runtime_error())?;
        Ok(guard.clock_healthy_at(ClockSample::now()))
    }

    pub(crate) fn fence_clock(&self) -> Result<()> {
        let JwtKeySource::Live(current) = &self.config.key_state else {
            return Err(invalid_rotation());
        };
        current.read().map_err(|_| keyring_runtime_error())?.clock.trip();
        Ok(())
    }

    pub(crate) fn install_lease(&self, generation: u64, deadline: LeaseDeadline) -> Result<bool> {
        let JwtKeySource::Live(current) = &self.config.key_state else {
            return Err(invalid_rotation());
        };
        let mut guard = current.write().map_err(|_| keyring_runtime_error())?;
        let now = ClockSample::now();
        if !guard.clock_healthy_at(now) || !deadline.authority.healthy_at(now) {
            guard.clock.trip();
            return Ok(false);
        }
        if !guard.lease_required || guard.emergency_fenced || guard.generation != generation
            || deadline.remaining(now) == Duration::ZERO
        {
            return Ok(false);
        }
        if guard.lease_until.as_ref().is_none_or(|previous| deadline.later_than(previous)) {
            guard.authority_clock = Some(deadline.authority.clone());
            guard.lease_until = Some(deadline);
        }
        // A delayed older grant must not report a successful renewal if
        // the retained newer deadline has already been observed expired.
        Ok(guard.permits_crypto_at(now))
    }

    /// Permanently close this local generation after observing a durable
    /// emergency revocation. A delayed grant cannot reopen the same gate.
    pub(crate) fn fence_revoked_generation(&self, generation: u64) -> Result<()> {
        let JwtKeySource::Live(current) = &self.config.key_state else {
            return Err(invalid_rotation());
        };
        let mut guard = current.write().map_err(|_| keyring_runtime_error())?;
        if guard.generation == generation {
            guard.lease_until = None;
            guard.emergency_fenced = true;
        }
        Ok(())
    }

    /// Clear local authority on host shutdown or supervisor cancellation.
    pub fn revoke_lease(&self) {
        if let JwtKeySource::Live(current) = &self.config.key_state {
            if let Ok(mut guard) = current.write() {
                guard.lease_until = None;
            }
        }
    }

    pub fn prepare_document(&self, bytes: &[u8]) -> Result<(u64, AuthConfig)> {
        self.config.prepare_jwt_document(bytes)
    }

    /// Candidate preparation and host policy checks happen before this call.
    /// Recheck the process generation and key continuity under the write fence.
    pub fn publish(&self, generation: u64, candidate: AuthConfig) -> Result<()> {
        self.publish_inner(generation, candidate, None).map(|_| ())
    }

    /// Only the Auth fence may install a deadline obtained for this candidate.
    pub(crate) fn publish_granted(
        &self, generation: u64, candidate: AuthConfig, deadline: LeaseDeadline,
    ) -> Result<bool> {
        self.publish_inner(generation, candidate, Some(deadline))
    }

    fn matches_candidate_policy(&self, candidate: &AuthConfig) -> bool {
        matches!(&candidate.key_state, JwtKeySource::Fixed(_))
            && self.config.secret() == candidate.secret()
            && self.config.issuer == candidate.issuer
            && self.config.audience == candidate.audience
            && self.config.access_expiration == candidate.access_expiration
            && self.config.refresh_expiration == candidate.refresh_expiration
            && self.config.min_password_length == candidate.min_password_length
    }

    /// Reject a locally incompatible candidate before reserving a durable
    /// replica grant. Publication still rechecks under the write fence.
    pub(crate) fn accepts_grant_candidate(
        &self, generation: u64, candidate: &AuthConfig,
    ) -> Result<bool> {
        if !self.matches_candidate_policy(candidate) || i64::try_from(generation).is_err() {
            return Ok(false);
        }
        let JwtKeySource::Live(current) = &self.config.key_state else {
            return Err(invalid_rotation());
        };
        let guard = current.read().map_err(|_| keyring_runtime_error())?;
        Ok(guard.lease_required && generation > guard.generation)
    }

    fn publish_inner(
        &self, generation: u64, candidate: AuthConfig, deadline: Option<LeaseDeadline>,
    ) -> Result<bool> {
        if !self.matches_candidate_policy(&candidate) {
            return Err(invalid_rotation());
        }
        validate_auth_config(&candidate)?;
        let keys = candidate.key_snapshot()?;
        let next = keys.prepared(candidate.secret())?;
        let JwtKeySource::Live(current) = &self.config.key_state else {
            return Err(invalid_rotation());
        };
        let mut guard = current.write().map_err(|_| keyring_runtime_error())?;
        if generation <= guard.generation || i64::try_from(generation).is_err() {
            return Err(invalid_rotation());
        }
        // A cluster runtime cannot use this candidate until the database
        // grants a lease for its exact metadata. It may legitimately skip
        // intervening generations already validated by the durable ledger.
        if !guard.lease_required {
            guard.keys.prepared(self.config.secret())?.validate_successor(next)?;
        }
        let lease_required = guard.lease_required;
        if lease_required && deadline.is_none() {
            return Err(invalid_rotation());
        }
        if lease_required {
            let now = ClockSample::now();
            if !guard.clock_healthy_at(now) {
                return Err(keyring_runtime_error());
            }
            if let Some(deadline) = deadline.as_ref() {
                if !deadline.authority.healthy_at(now) {
                    guard.clock.trip();
                    return Err(keyring_runtime_error());
                }
                if deadline.remaining(now) == Duration::ZERO {
                    return Ok(false);
                }
            }
        } else if deadline.is_some() {
            return Ok(false);
        }
        let clock = guard.clock.clone();
        let authority_clock = deadline.as_ref().map(|lease| lease.authority.clone())
            .or_else(|| guard.authority_clock.clone());
        *guard = VersionedJwtKeys {
            generation, keys, lease_required, lease_until: deadline,
            clock, authority_clock, emergency_fenced: false,
        };
        Ok(true)
    }
}

/// Policy-only settings loaded from nested YAML `settings.auth`.
///
/// JWT key material has exactly one source: the complete generation-bearing
/// keyring document. Unknown algorithm and key-source fields are rejected.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct AuthSettingsOverrides {
    pub refresh_expiration: Option<u64>,
    pub issuer: Option<String>,
    pub audience: Option<String>,
    pub min_password_length: Option<usize>,
}

impl std::fmt::Debug for AuthSettingsOverrides {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthSettingsOverrides")
            .field("refresh_expiration", &self.refresh_expiration)
            .field("issuer", &self.issuer)
            .field("audience", &self.audience)
            .field("min_password_length", &self.min_password_length)
            .finish()
    }
}

impl AuthSettingsOverrides {
    fn apply_policy(self, config: &mut AuthConfig) {
        if let Some(value) = self.refresh_expiration {
            config.refresh_expiration = value;
        }
        if let Some(value) = self.issuer {
            config.issuer = value;
        }
        if let Some(value) = self.audience {
            config.audience = value;
        }
        if let Some(value) = self.min_password_length {
            config.min_password_length = value;
        }
    }
}

/// Construct and validate the complete immutable JWT candidate from the
/// mandatory generation-bearing document. No default or secondary key source
/// is prepared alongside the selected file.
pub fn build_auth_config_from_document(
    secret: String,
    access_expiration: u64,
    auth_settings: AuthSettingsOverrides,
    bytes: &[u8],
) -> Result<(u64, AuthConfig)> {
    let (generation, inputs) = JwtKeyringDocument::parse(bytes)?;
    let mut candidate = AuthConfig::from_key_inputs(secret, inputs);
    candidate.access_expiration = access_expiration;
    auth_settings.apply_policy(&mut candidate);
    validate_auth_config(&candidate)?;
    Ok((generation, candidate))
}

/// Validate auth config invariants owned by `rustok-auth`.
pub fn validate_auth_config(config: &AuthConfig) -> Result<()> {
    tracing::debug!(
        issuer = %config.issuer,
        audience = %config.audience,
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

    // Host proofs and durable authority use an independent symmetric root,
    // even when JWT signing uses RSA.
    if !mac_root_is_valid(config.secret()) {
        return Err(AuthError::Internal(format!(
            "Auth MAC root must contain at least {MIN_SYMMETRIC_SECRET_BYTES} non-whitespace bytes and must not contain PEM key material"
        )));
    }

    config.with_prepared_jwt(|_| Ok(()))
}

pub(crate) fn validate_rs256_key_pair(private_key_pem: &str, public_key_pem: &str) -> Result<()> {
    let encoding_key = rsa_signing_key(private_key_pem, public_key_pem)?;
    let decoding_key = verification_key(public_key_pem)?;
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
    use rustok_test_utils::jwt::{JwtKeyringFixture, RsaKeyPairFixture};

    fn rsa_config(root: String, fixture: &JwtKeyringFixture) -> AuthConfig {
        build_auth_config_from_document(
            root, 900, Default::default(), fixture.document(1).to_string().as_bytes(),
        ).unwrap().1
    }

    fn document(
        fixture: &JwtKeyringFixture, generation: u64, kid: &str, pair: &RsaKeyPairFixture,
    ) -> Vec<u8> {
        serde_json::to_vec(&fixture.document_with_access(generation, kid, pair, None)).unwrap()
    }


    #[test]
    fn clock_fault_is_sticky_across_grants_and_publication() {
        let root = secret();
        let fixture = JwtKeyringFixture::generate().unwrap();
        let candidate = rsa_config(root, &fixture);
        let (_, publisher) = candidate.clone().into_cluster_jwt(1).unwrap();
        let authority = Arc::new(ClockGuard::new());
        let deadline = LeaseDeadline::new(ClockSample::now(), Duration::from_secs(10), authority.clone()).unwrap();
        assert!(publisher.install_lease(1, deadline).unwrap());
        let readiness = publisher.lease_readiness().unwrap();
        assert!(readiness.is_ready());
        authority.trip();
        assert!(!readiness.is_ready());
        let fresh = Arc::new(ClockGuard::new());
        let deadline = LeaseDeadline::new(ClockSample::now(), Duration::from_secs(10), fresh).unwrap();
        assert!(!publisher.install_lease(1, deadline.clone()).unwrap());
        assert!(publisher.publish_granted(2, candidate, deadline).is_err());
        assert!(!readiness.is_ready());
    }

    #[test]
    fn cluster_lease_gate_rejects_crypto_at_or_after_deadline() {
        use std::cell::Cell;
        use std::time::Duration;
        let fixture = JwtKeyringFixture::generate().unwrap();
        let (config, publisher) = rsa_config(secret(), &fixture).into_cluster_jwt(1).unwrap();
        let readiness = publisher.lease_readiness().unwrap();
        assert!(!readiness.is_ready());
        let authority = Arc::new(ClockGuard::new());
        let start = ClockSample::now();
        let deadline = start.checked_add(Duration::from_secs(60)).unwrap();
        let lease = LeaseDeadline::new(start, Duration::from_secs(60), authority.clone()).unwrap();
        assert!(!publisher.install_lease(2, lease.clone()).unwrap());
        assert!(!publisher.install_lease(1, LeaseDeadline::new(start, Duration::ZERO, authority).unwrap()).unwrap());
        assert!(publisher.install_lease(1, lease).unwrap());
        assert!(readiness.is_ready());

        // The post-operation fence must discard a result even if crypto
        // started before expiry. Simulate the clock without sleeping.
        let clock = Cell::new(start);
        let calls = Cell::new(0);
        let result = config.with_jwt_operation_at(|| clock.get(), |_| {
            calls.set(calls.get() + 1);
            clock.set(deadline);
            Ok(123_u32)
        });
        assert!(result.is_err());
        assert_eq!(calls.get(), 1);
        assert!(config.with_jwt_operation_at(|| deadline, |_| {
            panic!("expired lease must not enter crypto");
            #[allow(unreachable_code)]
            Ok(())
        }).is_err());
        publisher.revoke_lease();
        assert!(!readiness.is_ready());
        assert!(config.with_jwt_operation(|_| Ok(())).is_err());
    }

    #[test]
    fn cluster_runtime_rejects_publication_without_a_durable_grant() {
        let fixture = JwtKeyringFixture::generate().unwrap();
        let (config, publisher) = rsa_config(secret(), &fixture).into_cluster_jwt(1).unwrap();
        let next = RsaKeyPairFixture::generate().unwrap();
        let document = document(&fixture, 2, "access-next", &next);
        let (_, candidate) = config.prepare_jwt_document(&document).unwrap();

        assert!(publisher.publish(2, candidate).is_err());
        assert_eq!(publisher.generation().unwrap(), 1);
        assert!(!publisher.lease_readiness().unwrap().is_ready());
    }

    #[test]
    fn auth_overrides_are_policy_only_and_reject_key_sources() {
        for value in [
            serde_json::json!({"algorithm": "RS256"}),
            serde_json::json!({"jwt_keyring_env": "JWT_KEYS"}),
            serde_json::json!({"jwt_special_keyrings_env": "SPECIAL_KEYS"}),
            serde_json::json!({"rsa_private_key_pem": "private"}),
            serde_json::json!({"rsa_public_key_pem": "public"}),
            serde_json::json!({"rsa_private_key_env": "PRIVATE"}),
            serde_json::json!({"rsa_public_key_env": "PUBLIC"}),
        ] {
            assert!(serde_json::from_value::<AuthSettingsOverrides>(value).is_err());
        }
        let valid = serde_json::from_value::<AuthSettingsOverrides>(serde_json::json!({
            "refresh_expiration": 172800,
            "issuer": "https://issuer.example",
            "audience": "rustok-admin",
            "min_password_length": 16
        })).unwrap();
        assert_eq!(valid.refresh_expiration, Some(172800));
        assert_eq!(valid.issuer.as_deref(), Some("https://issuer.example"));
        assert_eq!(valid.audience.as_deref(), Some("rustok-admin"));
        assert_eq!(valid.min_password_length, Some(16));
        assert!(serde_json::from_value::<AuthSettingsOverrides>(serde_json::json!({})).is_ok());
    }

    #[test]
    fn legacy_hs256_document_fields_are_rejected() {
        let fixture = JwtKeyringFixture::generate().unwrap();

        let mut active_secret = fixture.document(1);
        active_secret["access"]["keyring"]["active_hs256_secret"] =
            serde_json::json!("legacy-secret-material-that-must-not-be-used");
        assert!(build_auth_config_from_document(
            secret(), 900, Default::default(), active_secret.to_string().as_bytes(),
        ).is_err());

        let mut active_algorithm = fixture.document(1);
        active_algorithm["access"]["algorithm"] = serde_json::json!("HS256");
        assert!(build_auth_config_from_document(
            secret(), 900, Default::default(), active_algorithm.to_string().as_bytes(),
        ).is_err());

        let mut history_algorithm = fixture.document(1);
        history_algorithm["access"]["keyring"]["verification_keys"] = serde_json::json!([{
            "kid": "legacy", "algorithm": "HS256",
            "material": fixture.access.public_pem(), "verify_until": 4_000_000_000_i64
        }]);
        assert!(build_auth_config_from_document(
            secret(), 900, Default::default(), history_algorithm.to_string().as_bytes(),
        ).is_err());
    }

    #[test]
    fn rsa_keyring_rejects_ambiguous_or_invalid_history() {
        let fixture = JwtKeyringFixture::generate().unwrap();
        let mut invalid_documents = Vec::new();

        let mut empty_kid = fixture.document(1);
        empty_kid["access"]["keyring"]["active_kid"] = serde_json::json!("");
        invalid_documents.push(empty_kid);

        let mut active_kid_history = fixture.document(1);
        active_kid_history["access"]["keyring"]["verification_keys"] = serde_json::json!([{
            "kid": "access", "material": fixture.access.public_pem(), "verify_until": 4_000_000_000_i64
        }]);
        invalid_documents.push(active_kid_history);

        let mut short_history = fixture.document(1);
        short_history["access"]["keyring"]["verification_keys"] = serde_json::json!([{
            "kid": "old", "material": "short", "verify_until": 4_000_000_000_i64
        }]);
        invalid_documents.push(short_history);

        let mut duplicate_history = fixture.document(1);
        duplicate_history["access"]["keyring"]["verification_keys"] = serde_json::json!([
            {"kid": "old", "material": fixture.access.public_pem(), "verify_until": 4_000_000_000_i64},
            {"kid": "old", "material": fixture.access.public_pem(), "verify_until": 4_000_000_001_i64}
        ]);
        invalid_documents.push(duplicate_history);

        let mut obsolete_field = fixture.document(1);
        obsolete_field["access"]["keyring"]["legacy_without_kid"] = serde_json::json!({
            "kid": "old", "verify_until": 4_000_000_000_i64
        });
        invalid_documents.push(obsolete_field);

        for document in invalid_documents {
            assert!(build_auth_config_from_document(
                secret(), 900, Default::default(), document.to_string().as_bytes(),
            ).is_err());
        }
    }

    #[test]
    fn document_bootstrap_uses_only_file_keys_and_host_policy() {
        let fixture = JwtKeyringFixture::generate().unwrap();
        let mut doc = fixture.document(7);
        doc["access"]["keyring"]["active_kid"] = serde_json::json!("file-key");
        let bytes = serde_json::to_vec(&doc).unwrap();
        let (generation, config) = build_auth_config_from_document(
            secret(), 900,
            AuthSettingsOverrides {
                issuer: Some("https://issuer.example".to_string()),
                min_password_length: Some(15),
                ..Default::default()
            },
            &bytes,
        ).unwrap();
        assert_eq!(generation, 7);
        assert_eq!(config.issuer, "https://issuer.example");
        assert_eq!(config.min_password_length, 15);
        assert_eq!(config.access_expiration, 900);
        let token = crate::encode_access_token(&config, uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(), rustok_core::UserRole::Customer, uuid::Uuid::new_v4()).unwrap();
        assert_eq!(jsonwebtoken::decode_header(&token).unwrap().kid.as_deref(), Some("file-key"));
        assert!(build_auth_config_from_document(secret(), 0, Default::default(), &bytes).is_err());
        assert!(build_auth_config_from_document(secret(), 900, Default::default(), b"{}").is_err());
    }

    #[test]
    fn complete_rsa_document_round_trips_every_purpose_and_rejects_key_reuse() {
        use crate::jwt::*;
        use rustok_core::UserRole;
        use uuid::Uuid;

        let fixture = rustok_test_utils::jwt::JwtKeyringFixture::generate().unwrap();
        let document = fixture.document(1);
        let (generation, config) = build_auth_config_from_document(
            secret(), 900, Default::default(), document.to_string().as_bytes(),
        ).unwrap();
        assert_eq!(generation, 1);
        let user = Uuid::new_v4();
        let tenant = Uuid::new_v4();
        let access = encode_access_token(&config, user, tenant, UserRole::Customer, Uuid::new_v4()).unwrap();
        let oauth = encode_oauth_access_token(&config, OauthAccessTokenInput {
            app_id: user,
            tenant_id: tenant,
            role: UserRole::Customer,
            client_id: Uuid::new_v4(),
            scopes: &["catalog:read".to_string()],
            grant_type: "client_credentials",
            expires_in_secs: 900,
            token_id: Uuid::new_v4(),
        }).unwrap();
        let reset = encode_password_reset_token(&config, tenant, "a@example.com", 900).unwrap();
        let email = encode_email_verification_token(&config, tenant, "a@example.com", 900).unwrap();
        let invite = encode_invite_token(&config, tenant, "a@example.com", UserRole::Customer, None, 900).unwrap();
        for (token, kid) in [
            (&access, "access"), (&oauth, "access"), (&reset, "password-reset"),
            (&email, "email-verification"), (&invite, "invite"),
        ] {
            let header = jsonwebtoken::decode_header(token).unwrap();
            assert_eq!(header.alg, Algorithm::RS256);
            assert_eq!(header.kid.as_deref(), Some(kid));
        }
        assert!(decode_access_token(&config, &access).is_ok());
        assert!(decode_access_token(&config, &oauth).is_ok());
        assert!(decode_password_reset_token(&config, &reset).is_ok());
        assert!(decode_email_verification_token(&config, &email).is_ok());
        assert!(decode_invite_token(&config, &invite).is_ok());
        assert!(decode_access_token(&config, &reset).is_err());
        assert!(decode_password_reset_token(&config, &email).is_err());

        let mut reused = document.clone();
        reused["invite"] = document["access"].clone();
        reused["invite"]["keyring"]["active_kid"] = serde_json::json!("different-kid");
        assert!(build_auth_config_from_document(
            secret(), 900, Default::default(), reused.to_string().as_bytes(),
        ).is_err());
        for purpose in ["access", "password_reset", "email_verification", "invite"] {
            let mut incomplete = document.clone();
            incomplete[purpose].as_object_mut().unwrap().remove("rsa_private_key_pem");
            assert!(build_auth_config_from_document(
                secret(), 900, Default::default(), incomplete.to_string().as_bytes(),
            ).is_err());
        }
    }

    #[test]
    fn generation_bearing_document_requires_four_complete_rsa_purpose_rings() {
        let fixture = JwtKeyringFixture::generate().unwrap();
        let document = fixture.document(1);
        assert!(build_auth_config_from_document(
            secret(), 900, Default::default(), document.to_string().as_bytes(),
        ).is_ok());

        for purpose in ["access", "password_reset", "email_verification", "invite"] {
            let mut incomplete = document.clone();
            incomplete.as_object_mut().unwrap().remove(purpose);
            assert!(
                build_auth_config_from_document(
                    secret(), 900, Default::default(), incomplete.to_string().as_bytes(),
                ).is_err(),
                "{purpose} must be required",
            );

            let mut marked = document.clone();
            marked[purpose]["algorithm"] = serde_json::json!("RS256");
            assert!(
                build_auth_config_from_document(
                    secret(), 900, Default::default(), marked.to_string().as_bytes(),
                ).is_err(),
                "{purpose} must not carry an algorithm selector or marker",
            );
        }
    }

    #[test]
    fn keyring_document_is_unversioned_and_generation_is_bounded() {
        let fixture = JwtKeyringFixture::generate().unwrap();
        let base = rsa_config(secret(), &fixture);
        let material = &fixture.access;
        let max = i64::MAX as u64;
        for generation in [1, max] {
            assert_eq!(base.prepare_jwt_document(&document(
                &fixture, generation, "one", material,
            )).unwrap().0, generation);
        }
        for generation in [0, max + 1, u64::MAX] {
            assert!(base.prepare_jwt_document(&document(
                &fixture, generation, "one", material,
            )).is_err());
        }
        let mut obsolete: serde_json::Value =
            serde_json::from_slice(&document(&fixture, 1, "one", material)).unwrap();
        obsolete["schema_version"] = serde_json::json!(1);
        assert!(base.prepare_jwt_document(&serde_json::to_vec(&obsolete).unwrap()).is_err());
        assert!(base.clone().into_live_jwt(max + 1).is_err());
        assert!(base.clone().into_cluster_jwt(max + 1).is_err());
        assert!(base.clone().into_cluster_jwt(0).is_err());
        let (generation, first) = base.prepare_jwt_document(
            &document(&fixture, 1, "one", material),
        ).unwrap();
        let (_, publisher) = first.clone().into_live_jwt(generation).unwrap();
        assert!(publisher.publish(max + 1, first).is_err());
        assert_eq!(publisher.generation().unwrap(), generation);
    }

    #[test]
    fn live_rotation_updates_existing_clones_and_rejects_rollback() {
        use crate::{encode_access_token, decode_access_token};
        use rustok_core::UserRole;
        use uuid::Uuid;
        let fixture = JwtKeyringFixture::generate().unwrap();
        let next_key = RsaKeyPairFixture::generate().unwrap();
        let reused_key = RsaKeyPairFixture::generate().unwrap();
        let (generation, first) = build_auth_config_from_document(
            secret(), 900, Default::default(),
            &document(&fixture, 1, "one", &fixture.access),
        ).unwrap();
        let (live, publisher) = first.into_live_jwt(generation).unwrap();
        let clone = live.clone();
        let issue = |config: &AuthConfig| encode_access_token(
            config, Uuid::new_v4(), Uuid::new_v4(), UserRole::Customer, Uuid::new_v4(),
        ).unwrap();
        let old = issue(&clone);
        let (g, next) = publisher.prepare_document(&document(
            &fixture, 2, "two", &next_key,
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
            &fixture, 3, "two", &reused_key,
        )).unwrap();
        assert!(publisher.publish(g, reused).is_err());
        assert_eq!(publisher.generation().unwrap(), 2);
    }

    #[test]
    fn live_rotation_supports_stage_promote_and_retire() {
        use crate::{decode_access_token, encode_access_token};
        use rustok_core::UserRole;
        use uuid::Uuid;
        let fixture = JwtKeyringFixture::generate().unwrap();
        let next_key = RsaKeyPairFixture::generate().unwrap();
        let (generation, first) = build_auth_config_from_document(
            secret(), 900, Default::default(),
            &document(&fixture, 1, "one", &fixture.access),
        ).unwrap();
        let (live, publisher) = first.into_live_jwt(generation).unwrap();
        let existing_clone = live.clone();
        let issue = || encode_access_token(
            &existing_clone, Uuid::new_v4(), Uuid::new_v4(), UserRole::Customer, Uuid::new_v4(),
        ).unwrap();
        let old = issue();
        let until = chrono::Utc::now().timestamp() + 3600;
        let with_history = |generation, active_kid, active_key, history_kid, history_key| {
            serde_json::to_vec(&fixture.document_with_access(
                generation, active_kid, active_key, Some((history_kid, history_key, until)),
            )).unwrap()
        };
        let (g, staged) = publisher.prepare_document(
            &with_history(2, "one", &fixture.access, "two", &next_key)
        ).unwrap();
        publisher.publish(g, staged).unwrap();
        assert_eq!(jsonwebtoken::decode_header(&issue()).unwrap().kid.as_deref(), Some("one"));
        let (g, promoted) = publisher.prepare_document(
            &with_history(3, "two", &next_key, "one", &fixture.access)
        ).unwrap();
        publisher.publish(g, promoted).unwrap();
        let fresh = issue();
        assert_eq!(jsonwebtoken::decode_header(&fresh).unwrap().kid.as_deref(), Some("two"));
        assert!(decode_access_token(&live, &old).is_ok());
        assert!(decode_access_token(&live, &fresh).is_ok());
        let (g, retired) = publisher.prepare_document(&document(&fixture, 4, "two", &next_key)).unwrap();
        publisher.publish(g, retired).unwrap();
        assert!(decode_access_token(&existing_clone, &old).is_err());
        assert!(decode_access_token(&existing_clone, &fresh).is_ok());
        assert_eq!(publisher.generation().unwrap(), 4);
    }

    #[test]
    fn live_rotation_serializes_concurrent_publishers() {
        let fixture = JwtKeyringFixture::generate().unwrap();
        let second_key = RsaKeyPairFixture::generate().unwrap();
        let third_key = RsaKeyPairFixture::generate().unwrap();
        let (g, first) = build_auth_config_from_document(
            secret(), 900, Default::default(),
            &document(&fixture, 1, "one", &fixture.access),
        ).unwrap();
        let (_, publisher) = first.into_live_jwt(g).unwrap();
        let (g2, second) = publisher.prepare_document(&document(
            &fixture, 2, "two", &second_key,
        )).unwrap();
        let (g3, third) = publisher.prepare_document(&document(
            &fixture, 3, "three", &third_key,
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
        let fixture = JwtKeyringFixture::generate().unwrap();
        let (_, inputs) = JwtKeyringDocument::parse(&document(&fixture, 1, "one", &fixture.access)).unwrap();
        let original = AuthConfig::from_key_inputs(secret(), inputs);
        let before_first_use = original.clone();
        assert!(Arc::ptr_eq(&original.key_snapshot().unwrap(), &before_first_use.key_snapshot().unwrap()));
        let prepared = original.prepared_jwt().unwrap();
        assert!(std::ptr::eq(prepared, before_first_use.prepared_jwt().unwrap()));
        let policy_only = original.clone().with_issuer("other-issuer");
        assert!(std::ptr::eq(prepared, policy_only.prepared_jwt().unwrap()));
        let next_key = RsaKeyPairFixture::generate().unwrap();
        let (_, inputs) = JwtKeyringDocument::parse(&document(&fixture, 2, "two", &next_key)).unwrap();
        let mut candidate = original.clone();
        candidate.edit_key_inputs(|current| *current = inputs).unwrap();
        assert!(!Arc::ptr_eq(&original.key_snapshot().unwrap(), &candidate.key_snapshot().unwrap()));
        let candidate_prepared = candidate.prepared_jwt().unwrap();
        assert!(!std::ptr::eq(prepared, candidate_prepared));
        assert!(std::ptr::eq(prepared, original.prepared_jwt().unwrap()));
    }

    #[test]
    fn concurrent_first_use_and_failed_reconfiguration_are_isolated() {
        let fixture = JwtKeyringFixture::generate().unwrap();
        let (_, inputs) = JwtKeyringDocument::parse(&document(&fixture, 1, "one", &fixture.access)).unwrap();
        let mut config = AuthConfig::from_key_inputs(secret(), inputs.clone());
        let clone = config.clone();
        std::thread::scope(|scope| {
            let thread = scope.spawn(|| clone.prepared_jwt().unwrap() as *const _ as usize);
            let first = config.prepared_jwt().unwrap() as *const _ as usize;
            assert_eq!(thread.join().unwrap(), first);
        });
        let invalid = AuthSettingsOverrides {
            rsa_private_key_pem: Some("invalid-private-key".into()),
            ..Default::default()
        };
        assert!(invalid.apply(&mut config).is_err());
        assert!(std::ptr::eq(config.prepared_jwt().unwrap(), clone.prepared_jwt().unwrap()));
        let invalid = AuthConfig::from_key_inputs("short".into(), inputs);
        assert!(invalid.prepared_jwt().is_err());
        assert!(invalid.prepared_jwt().is_err());
    }

    fn secret() -> String {
        "test-secret-key-for-unit-tests-only-32bytes!".to_string()
    }

    #[test]
    fn auth_config_debug_redacts_secret_and_private_keys() {
        let fixture = JwtKeyringFixture::generate().unwrap();
        let root = "super-secret-auth-key-0123456789-with-32-bytes".to_string();
        let document = fixture.document(1).to_string();
        let config = build_auth_config_from_document(
            root.clone(), 900, Default::default(), document.as_bytes(),
        ).unwrap().1;
        let rendered = format!("{config:?}");

        assert!(!rendered.contains(&root));
        assert!(!rendered.contains(fixture.access.private_pem()));
        assert!(!rendered.contains(fixture.access.public_pem()));
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn auth_settings_debug_exposes_only_policy() {
        let settings = AuthSettingsOverrides {
            issuer: Some("https://issuer.example".to_string()),
            audience: Some("rustok-admin".to_string()),
            min_password_length: Some(16),
            ..Default::default()
        };
        let rendered = format!("{settings:?}");
        assert!(rendered.contains("https://issuer.example"));
        assert!(rendered.contains("rustok-admin"));
        assert!(!rendered.contains("private"));
        assert!(!rendered.contains("public"));
    }

    #[test]
    fn rs256_configuration_requires_independent_mac_root() {
        let fixture = JwtKeyringFixture::generate().unwrap();
        let doc = fixture.document(1);
        for weak in [
            "", "short-secret", "abc                              ",
            fixture.access.public_pem(), fixture.access.private_pem(),
        ] {
            let config = AuthConfig::from_test_document_unvalidated(
                weak.to_string(), doc.to_string().as_bytes(),
            );
            assert!(validate_auth_config(&config).is_err());
        }
    }

    #[test]
    fn production_mac_root_policy_rejects_short_placeholders_and_low_diversity() {
        let fixture = JwtKeyringFixture::generate().unwrap();
        let document = fixture.document(1).to_string();
        for weak in [
            "short".to_string(),
            "development".repeat(8),
            "aB3!".repeat(20),
        ] {
            let config = AuthConfig::from_test_document_unvalidated(
                weak, document.as_bytes(),
            );
            assert!(config.validate_production_mac_root().is_err());
        }

        let strong = AuthConfig::from_test_document_unvalidated(
            "aB3!zY7@qW8#eR2$".repeat(5), document.as_bytes(),
        );
        assert!(strong.validate_production_mac_root().is_ok());
    }

    #[test]
    fn document_bootstrap_rejects_invalid_policy_and_rsa_material() {
        let fixture = JwtKeyringFixture::generate().unwrap();
        let bytes = fixture.document(1).to_string();

        for settings in [
            AuthSettingsOverrides { issuer: Some("  ".to_string()), ..Default::default() },
            AuthSettingsOverrides { audience: Some(String::new()), ..Default::default() },
            AuthSettingsOverrides {
                min_password_length: Some(MIN_PASSWORD_LENGTH_FLOOR_CHARS - 1),
                ..Default::default()
            },
            AuthSettingsOverrides {
                min_password_length: Some(MAX_MIN_PASSWORD_LENGTH_CHARS + 1),
                ..Default::default()
            },
        ] {
            assert!(build_auth_config_from_document(
                secret(), 900, settings, bytes.as_bytes(),
            ).is_err());
        }

        assert!(build_auth_config_from_document(
            secret(), MIN_ACCESS_EXPIRATION_SECS - 1, Default::default(), bytes.as_bytes(),
        ).is_err());
        assert!(build_auth_config_from_document(
            secret(), 900,
            AuthSettingsOverrides { refresh_expiration: Some(300), ..Default::default() },
            bytes.as_bytes(),
        ).is_err());

        let other = JwtKeyringFixture::generate().unwrap();
        let mut malformed = fixture.document(1);
        malformed["access"]["rsa_private_key_pem"] = serde_json::json!("not-a-private-key");
        assert!(build_auth_config_from_document(
            secret(), 900, Default::default(), malformed.to_string().as_bytes(),
        ).is_err());

        let mut mismatched = fixture.document(1);
        mismatched["access"]["rsa_public_key_pem"] = serde_json::json!(other.access.public_pem());
        assert!(build_auth_config_from_document(
            secret(), 900, Default::default(), mismatched.to_string().as_bytes(),
        ).is_err());

        for weak_root in ["", "short-secret", fixture.access.public_pem(), fixture.access.private_pem()] {
            assert!(build_auth_config_from_document(
                weak_root.to_string(), 900, Default::default(), bytes.as_bytes(),
            ).is_err());
        }
    }

}
