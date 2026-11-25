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

#[derive(Clone, Default)]
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
            algorithm: doc.access.algorithm,
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

    #[cfg(test)]
    pub(crate) fn fixed_inputs(&self) -> Result<&JwtKeyInputs> {
        match &self.key_state {
            JwtKeySource::Fixed(keys) => Ok(&keys.inputs),
            JwtKeySource::Live(_) => Err(invalid_rotation()),
        }
    }

    fn edit_key_inputs(&mut self, edit: impl FnOnce(&mut JwtKeyInputs)) -> Result<()> {
        if let JwtKeySource::Live(current) = &self.key_state {
            if current.read().map_err(|_| keyring_runtime_error())?.lease_required {
                return Err(invalid_rotation());
            }
        }
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
        Self::from_key_inputs(secret, JwtKeyInputs::default())
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

    /// HTTP host composition: generation zero is static mode. JWT operations
    /// fail closed until a matching durable lease is installed.
    pub fn into_cluster_jwt(self, generation: u64) -> Result<(Self, JwtKeyringPublisher)> {
        self.into_jwt_runtime(generation, true)
    }

    fn into_jwt_runtime(mut self, generation: u64, lease_required: bool) -> Result<(Self, JwtKeyringPublisher)> {
        if (!lease_required && generation == 0)
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
    fn has_key_overrides(&self) -> bool {
        self.algorithm.is_some()
            || self.jwt_keyring_env.is_some()
            || self.jwt_special_keyrings_env.is_some()
            || self.rsa_private_key_pem.is_some()
            || self.rsa_public_key_pem.is_some()
            || self.rsa_private_key_env.is_some()
            || self.rsa_public_key_env.is_some()
    }

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

    /// Apply only non-key settings; no key source is constructed or resolved.
    fn apply_policy(&mut self, config: &mut AuthConfig) {
        if let Some(v) = self.refresh_expiration.take() {
            config.refresh_expiration = v;
        }
        if let Some(v) = self.issuer.take() {
            config.issuer = v;
        }
        if let Some(v) = self.audience.take() {
            config.audience = v;
        }
        if let Some(v) = self.min_password_length.take() {
            config.min_password_length = v;
        }
    }

    fn apply_with_env<F>(mut self, config: &mut AuthConfig, env_resolver: &mut F) -> Result<()>
    where
        F: FnMut(&str) -> Result<String>,
    {
        self.apply_policy(config);
        // Each cryptographic edit goes through the single copy-on-write boundary.
        if let Some(name) = self.jwt_keyring_env {
            let ring = JwtKeyring::from_json(&resolve_auth_env(&name, env_resolver)?)?;
            config.edit_key_inputs(|inputs| inputs.jwt_keyring = ring)?;
        }
        if let Some(name) = self.jwt_special_keyrings_env {
            let rings = SpecialJwtKeyrings::from_json(&resolve_auth_env(&name, env_resolver)?)?;
            config.edit_key_inputs(|inputs| inputs.special_jwt_keyrings = rings)?;
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

/// Build from the complete file source without preparing an unused startup
/// keyring. The file is the exclusive source of JWT signing and verification
/// material; host settings supply only immutable policy and the MAC root.
pub fn build_auth_config_from_document(
    secret: String,
    access_expiration: u64,
    mut auth_settings: AuthSettingsOverrides,
    bytes: &[u8],
) -> Result<(u64, AuthConfig)> {
    if auth_settings.has_key_overrides() {
        return Err(AuthError::Validation(
            "JWT keyring file cannot be combined with startup key overrides".to_string(),
        ));
    }
    // Construct the configuration directly from the selected document.
    // No default keyring or unused startup source is even instantiated.
    let (generation, inputs) = JwtKeyringDocument::parse(bytes)?;
    let mut candidate = AuthConfig::from_key_inputs(secret, inputs);
    candidate.access_expiration = access_expiration;
    auth_settings.apply_policy(&mut candidate);
    validate_auth_config(&candidate)?;
    Ok((generation, candidate))
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

fn resolve_auth_env<F>(name: &str, env_resolver: &mut F) -> Result<String>
where
    F: FnMut(&str) -> Result<String>,
{
    if name.trim().is_empty() || name.trim() != name || name.contains('=') || name.contains('\0') {
        return Err(AuthError::Internal("Auth key environment name is invalid".to_string()));
    }
    let value = env_resolver(name)?;
    if value.trim().is_empty() {
        return Err(AuthError::Internal(format!("Auth env var {name} must not be empty")));
    }
    Ok(value)
}

fn resolve_key_material<F>(
    inline_pem: Option<String>,
    env_name: Option<String>,
    env_resolver: &mut F,
) -> Result<Option<String>>
where
    F: FnMut(&str) -> Result<String>,
{
    if inline_pem.is_some() && env_name.is_some() {
        return Err(AuthError::Internal(
            "RSA key must have exactly one configured source".to_string(),
        ));
    }
    if let Some(env_name) = env_name {
        return resolve_auth_env(&env_name, env_resolver).map(Some);
    }

    if inline_pem.as_ref().is_some_and(|pem| pem.trim().is_empty()) {
        return Err(AuthError::Internal("Inline RSA key must not be empty".to_string()));
    }
    Ok(inline_pem)
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
    fn clock_fault_is_sticky_across_grants_and_publication() {
        let root = secret();
        let (_, publisher) = AuthConfig::new(root.clone()).into_cluster_jwt(0).unwrap();
        let authority = Arc::new(ClockGuard::new());
        let deadline = LeaseDeadline::new(ClockSample::now(), Duration::from_secs(10), authority.clone()).unwrap();
        assert!(publisher.install_lease(0, deadline).unwrap());
        let readiness = publisher.lease_readiness().unwrap();
        assert!(readiness.is_ready());
        authority.trip();
        assert!(!readiness.is_ready());
        let fresh = Arc::new(ClockGuard::new());
        let deadline = LeaseDeadline::new(ClockSample::now(), Duration::from_secs(10), fresh).unwrap();
        assert!(!publisher.install_lease(0, deadline.clone()).unwrap());
        assert!(publisher.publish_granted(1, AuthConfig::new(root), deadline).is_err());
        assert!(!readiness.is_ready());
    }

    #[test]
    fn cluster_lease_gate_rejects_crypto_at_or_after_deadline() {
        use std::cell::Cell;
        use std::time::Duration;
        let (config, publisher) = AuthConfig::new(secret()).into_cluster_jwt(0).unwrap();
        let readiness = publisher.lease_readiness().unwrap();
        assert!(!readiness.is_ready());
        let authority = Arc::new(ClockGuard::new());
        let start = ClockSample::now();
        let deadline = start.checked_add(Duration::from_secs(60)).unwrap();
        let lease = LeaseDeadline::new(start, Duration::from_secs(60), authority.clone()).unwrap();
        assert!(!publisher.install_lease(1, lease.clone()).unwrap());
        assert!(!publisher.install_lease(0, LeaseDeadline::new(start, Duration::ZERO, authority).unwrap()).unwrap());
        assert!(publisher.install_lease(0, lease).unwrap());
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
            json!({"active_kid": "a", "legacy_without_kid":{"kid":"a","verify_until":1}}),
            json!({"active_kid": "a", "verification_keys": [{"kid":"root","material":active,"verify_until":1}]}),
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
    fn purpose_config_rejects_cross_ring_key_reuse_and_obsolete_fields() {
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
            "generation": generation,
            "access": {"keyring": {"active_kid": kid, "active_hs256_secret": secret}},
            "password_reset": {"keyring": {"active_kid": "reset-default"}},
            "email_verification": {"keyring": {"active_kid": "verify-default"}},
            "invite": {"keyring": {"active_kid": "invite-default"}}
        })).unwrap()
    }

    #[test]
    fn document_bootstrap_uses_only_file_keys_and_host_policy() {
        let fixture = rustok_test_utils::jwt::JwtKeyringFixture::generate().unwrap();
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
        for settings in [
            AuthSettingsOverrides { algorithm: Some(JwtAlgorithm::RS256), ..Default::default() },
            AuthSettingsOverrides { jwt_keyring_env: Some("UNUSED".into()), ..Default::default() },
            AuthSettingsOverrides { jwt_special_keyrings_env: Some("UNUSED".into()), ..Default::default() },
            AuthSettingsOverrides { rsa_private_key_pem: Some("unused".into()), ..Default::default() },
            AuthSettingsOverrides { rsa_public_key_pem: Some("unused".into()), ..Default::default() },
            AuthSettingsOverrides { rsa_private_key_env: Some("UNUSED".into()), ..Default::default() },
            AuthSettingsOverrides { rsa_public_key_env: Some("UNUSED".into()), ..Default::default() },
        ] {
            assert!(build_auth_config_from_document(secret(), 900, settings, &bytes).is_err());
        }
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
    fn keyring_document_is_unversioned_and_generation_is_bounded() {
        let base = AuthConfig::new(secret());
        let material = "first-independent-signing-key-at-least-32-bytes";
        let max = i64::MAX as u64;
        for generation in [1, max] {
            assert_eq!(base.prepare_jwt_document(&document(
                generation, "one", material,
            )).unwrap().0, generation);
        }
        for generation in [0, max + 1, u64::MAX] {
            assert!(base.prepare_jwt_document(&document(
                generation, "one", material,
            )).is_err());
        }
        let mut obsolete: serde_json::Value =
            serde_json::from_slice(&document(1, "one", material)).unwrap();
        obsolete["schema_version"] = serde_json::json!(1);
        assert!(base.prepare_jwt_document(&serde_json::to_vec(&obsolete).unwrap()).is_err());
        assert!(base.clone().into_live_jwt(max + 1).is_err());
        assert!(base.clone().into_cluster_jwt(max + 1).is_err());
        assert!(base.clone().into_cluster_jwt(0).is_ok());
        let (generation, first) = base.prepare_jwt_document(
            &document(1, "one", material),
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
    fn all_key_environment_sources_reject_invalid_names_before_lookup() {
        for invalid in ["", " ", " JWT_KEYS", "JWT_KEYS ", "JWT=KEYS", "JWT\0KEYS"] {
            for overrides in [
                AuthSettingsOverrides { jwt_keyring_env: Some(invalid.into()), ..Default::default() },
                AuthSettingsOverrides { jwt_special_keyrings_env: Some(invalid.into()), ..Default::default() },
                AuthSettingsOverrides { rsa_private_key_env: Some(invalid.into()), ..Default::default() },
                AuthSettingsOverrides { rsa_public_key_env: Some(invalid.into()), ..Default::default() },
            ] {
                assert!(build_auth_config_with_env(secret(), 900, overrides, |_| {
                    panic!("invalid key source must not resolve an environment variable")
                }).is_err());
            }
        }
    }

    #[test]
    fn rsa_key_source_is_exclusive_and_explicit() {
        let mut no_lookup = |_: &str| -> Result<String> {
            panic!("invalid or inline key configuration must not read the environment")
        };
        assert!(resolve_key_material(
            Some("inline".to_string()), Some("RSA_KEY".to_string()), &mut no_lookup,
        ).is_err());
        for invalid in ["", " ", " RSA_KEY", "RSA_KEY ", "RSA=KEY", "RSA\0KEY"] {
            assert!(resolve_key_material(None, Some(invalid.to_string()), &mut no_lookup).is_err());
        }
        for blank in ["", " \n "] {
            assert!(resolve_key_material(Some(blank.to_string()), None, &mut no_lookup).is_err());
        }
        assert_eq!(
            resolve_key_material(Some("inline".to_string()), None, &mut no_lookup).unwrap(),
            Some("inline".to_string()),
        );
        assert_eq!(resolve_key_material(None, None, &mut no_lookup).unwrap(), None);
        assert!(resolve_key_material(
            None, Some("RSA_KEY".to_string()), &mut |_| Ok(" ".to_string()),
        ).is_err());
        assert!(build_auth_config_with_env(
            secret(), 900,
            AuthSettingsOverrides {
                rsa_private_key_pem: Some(" ".to_string()),
                ..Default::default()
            },
            |_| panic!("unexpected lookup"),
        ).is_err());
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
    fn rs256_configuration_requires_independent_mac_root() {
        for weak in [
            "", "short-secret", "abc                              ",
            crate::jwt::TEST_RSA_PUBLIC_KEY, crate::jwt::TEST_RSA_PRIVATE_KEY,
        ] {
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
