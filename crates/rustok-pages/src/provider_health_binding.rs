use std::{
    collections::BTreeMap,
    env, fs,
    io::ErrorKind,
    path::PathBuf,
    sync::Arc,
};

use chrono::{DateTime, Duration, SecondsFormat, Utc};
use rustok_page_builder::health::{ProviderHealthSnapshot, ProviderSloEvaluation};
use serde::Deserialize;
use thiserror::Error;

pub const PAGES_PROVIDER_HEALTH_ACCEPTANCE_PATH_ENV: &str =
    "RUSTOK_PAGES_PROVIDER_HEALTH_ACCEPTANCE_PATH";
pub const PAGES_PROVIDER_HEALTH_DEPLOYMENT_ID_ENV: &str =
    "RUSTOK_PAGES_PROVIDER_HEALTH_DEPLOYMENT_ID";
pub const PAGES_PROVIDER_HEALTH_DEPLOYMENT_IMAGE_DIGEST_ENV: &str =
    "RUSTOK_PAGES_PROVIDER_HEALTH_DEPLOYMENT_IMAGE_DIGEST";
pub const PAGE_BUILDER_PROVIDER_HEALTH_SOURCE_COMMIT_ENV: &str = "RUSTOK_SOURCE_COMMIT";

const ACCEPTANCE_FORMAT: &str = "pages_builder_provider_health_owner_acceptance_v1";
const ACCEPTED_STATUS: &str = "owner_accepted_server_binding_pending";
const ACCEPT_DECISION: &str = "accept_for_pages_binding";
const ROLLBACK_ACTION: &str = "restore_unobserved_provider_health";
const EVALUATION_FORMAT: &str = "page_builder_provider_health_deployment_evaluation_v1";
const EVALUATION_STATUS: &str = "deployment_health_evaluated_pages_binding_pending";
const MIN_QUERY_WINDOW_SECONDS: u64 = 300;
const MAX_QUERY_WINDOW_SECONDS: u64 = 86_400;
const MIN_FRESHNESS_SECONDS: u64 = 60;
const MAX_IDENTITY_AGE_SECONDS: f64 = 86_400.0;
const MINIMUM_SAMPLES_PER_OPERATION: f64 = 20.0;
const MAX_ACCEPTANCE_PACKET_BYTES: u64 = 2 * 1024 * 1024;
const MAX_CLOCK_SKEW_SECONDS: i64 = 5;

#[derive(Debug, Error)]
pub enum PagesProviderHealthBindingError {
    #[error("provider-health binding environment is incomplete")]
    IncompleteEnvironment,
    #[error("provider-health binding environment contains invalid Unicode")]
    InvalidEnvironmentUnicode,
    #[error("provider-health acceptance path must be absolute")]
    AcceptancePathNotAbsolute,
    #[error("provider-health acceptance packet must be a bounded regular non-symlink file")]
    InvalidAcceptanceFile,
    #[error("provider-health acceptance packet is invalid JSON")]
    InvalidJson,
    #[error("provider-health acceptance packet contract is invalid")]
    InvalidContract,
    #[error("provider-health acceptance packet does not match the live deployment identity")]
    IdentityMismatch,
    #[error("provider-health acceptance packet carries a non-canonical health snapshot")]
    HealthPolicyMismatch,
    #[error("provider-health acceptance packet is outside the admitted evidence bounds")]
    EvidenceBoundsInvalid,
}

impl PagesProviderHealthBindingError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::IncompleteEnvironment => "incomplete_environment",
            Self::InvalidEnvironmentUnicode => "invalid_environment_unicode",
            Self::AcceptancePathNotAbsolute => "acceptance_path_not_absolute",
            Self::InvalidAcceptanceFile => "invalid_acceptance_file",
            Self::InvalidJson => "invalid_json",
            Self::InvalidContract => "invalid_contract",
            Self::IdentityMismatch => "identity_mismatch",
            Self::HealthPolicyMismatch => "health_policy_mismatch",
            Self::EvidenceBoundsInvalid => "evidence_bounds_invalid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagesProviderHealthLiveIdentity {
    pub source_commit: String,
    pub deployment_id: String,
    pub deployment_image_digest: String,
}

impl PagesProviderHealthLiveIdentity {
    pub fn new(
        source_commit: impl Into<String>,
        deployment_id: impl Into<String>,
        deployment_image_digest: impl Into<String>,
    ) -> Result<Self, PagesProviderHealthBindingError> {
        let identity = Self {
            source_commit: source_commit.into(),
            deployment_id: deployment_id.into(),
            deployment_image_digest: deployment_image_digest.into(),
        };
        if !canonical_commit(&identity.source_commit)
            || !bounded_deployment_id(&identity.deployment_id)
            || !canonical_repo_digest(&identity.deployment_image_digest)
        {
            return Err(PagesProviderHealthBindingError::IdentityMismatch);
        }
        Ok(identity)
    }
}

#[derive(Debug, Clone)]
struct ValidatedAcceptance {
    snapshot: ProviderHealthSnapshot,
    evaluated_at: DateTime<Utc>,
    decided_at: DateTime<Utc>,
    health_valid_until: DateTime<Utc>,
}

impl ValidatedAcceptance {
    fn snapshot_at(&self, now: DateTime<Utc>) -> Option<ProviderHealthSnapshot> {
        let skew = Duration::seconds(MAX_CLOCK_SKEW_SECONDS);
        let latest_allowed = now + skew;
        if latest_allowed < self.evaluated_at || latest_allowed < self.decided_at {
            return None;
        }
        if now > self.health_valid_until + skew {
            return None;
        }
        Some(self.snapshot.clone())
    }
}

#[derive(Debug, Clone)]
enum PagesProviderHealthAuthoritySource {
    RetainedPacket(PathBuf),
    Static(ValidatedAcceptance),
}

#[derive(Debug, Clone)]
pub struct PagesProviderHealthAuthority {
    live_identity: PagesProviderHealthLiveIdentity,
    source: PagesProviderHealthAuthoritySource,
}

impl PagesProviderHealthAuthority {
    pub fn from_accepted_packet_bytes(
        bytes: &[u8],
        live_identity: &PagesProviderHealthLiveIdentity,
    ) -> Result<Self, PagesProviderHealthBindingError> {
        let accepted = validated_acceptance_from_bytes(bytes, live_identity)?;
        Ok(Self {
            live_identity: live_identity.clone(),
            source: PagesProviderHealthAuthoritySource::Static(accepted),
        })
    }

    pub fn from_retained_packet_path(
        path: PathBuf,
        live_identity: PagesProviderHealthLiveIdentity,
    ) -> Result<Self, PagesProviderHealthBindingError> {
        if !path.is_absolute() {
            return Err(PagesProviderHealthBindingError::AcceptancePathNotAbsolute);
        }
        match fs::symlink_metadata(&path) {
            Ok(metadata)
                if !metadata.file_type().is_file() || metadata.file_type().is_symlink() =>
            {
                return Err(PagesProviderHealthBindingError::InvalidAcceptanceFile);
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(_) => return Err(PagesProviderHealthBindingError::InvalidAcceptanceFile),
        }
        Ok(Self {
            live_identity,
            source: PagesProviderHealthAuthoritySource::RetainedPacket(path),
        })
    }

    pub fn current_snapshot(&self) -> Option<ProviderHealthSnapshot> {
        self.snapshot_at(Utc::now())
    }

    pub fn snapshot_at(&self, now: DateTime<Utc>) -> Option<ProviderHealthSnapshot> {
        let accepted = match &self.source {
            PagesProviderHealthAuthoritySource::RetainedPacket(path) => {
                let bytes = read_retained_packet(path).ok()?;
                validated_acceptance_from_bytes(&bytes, &self.live_identity).ok()?
            }
            PagesProviderHealthAuthoritySource::Static(accepted) => accepted.clone(),
        };
        accepted.snapshot_at(now)
    }

    pub fn source_commit(&self) -> &str {
        &self.live_identity.source_commit
    }

    pub fn deployment_id(&self) -> &str {
        &self.live_identity.deployment_id
    }

    pub fn deployment_image_digest(&self) -> &str {
        &self.live_identity.deployment_image_digest
    }
}

pub type SharedPagesProviderHealthAuthority = Arc<PagesProviderHealthAuthority>;

pub fn page_builder_provider_health_authority_from_environment(
) -> Result<Option<SharedPagesProviderHealthAuthority>, PagesProviderHealthBindingError> {
    let acceptance_path = environment_value(PAGES_PROVIDER_HEALTH_ACCEPTANCE_PATH_ENV)?;
    let deployment_id = environment_value(PAGES_PROVIDER_HEALTH_DEPLOYMENT_ID_ENV)?;
    let image_digest = environment_value(PAGES_PROVIDER_HEALTH_DEPLOYMENT_IMAGE_DIGEST_ENV)?;
    let source_commit = environment_value(PAGE_BUILDER_PROVIDER_HEALTH_SOURCE_COMMIT_ENV)?;

    let any_configured =
        acceptance_path.is_some() || deployment_id.is_some() || image_digest.is_some();
    if !any_configured {
        return Ok(None);
    }
    let (Some(acceptance_path), Some(deployment_id), Some(image_digest), Some(source_commit)) =
        (acceptance_path, deployment_id, image_digest, source_commit)
    else {
        return Err(PagesProviderHealthBindingError::IncompleteEnvironment);
    };

    let live_identity =
        PagesProviderHealthLiveIdentity::new(source_commit, deployment_id, image_digest)?;
    PagesProviderHealthAuthority::from_retained_packet_path(
        PathBuf::from(acceptance_path),
        live_identity,
    )
    .map(Arc::new)
    .map(Some)
}

fn environment_value(key: &'static str) -> Result<Option<String>, PagesProviderHealthBindingError> {
    match env::var(key) {
        Ok(value) if value.trim().is_empty() => Ok(None),
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => {
            Err(PagesProviderHealthBindingError::InvalidEnvironmentUnicode)
        }
    }
}

fn read_retained_packet(path: &PathBuf) -> Result<Vec<u8>, PagesProviderHealthBindingError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| PagesProviderHealthBindingError::InvalidAcceptanceFile)?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() == 0
        || metadata.len() > MAX_ACCEPTANCE_PACKET_BYTES
    {
        return Err(PagesProviderHealthBindingError::InvalidAcceptanceFile);
    }
    let bytes = fs::read(path).map_err(|_| PagesProviderHealthBindingError::InvalidAcceptanceFile)?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_ACCEPTANCE_PACKET_BYTES {
        return Err(PagesProviderHealthBindingError::InvalidAcceptanceFile);
    }
    Ok(bytes)
}

fn validated_acceptance_from_bytes(
    bytes: &[u8],
    live_identity: &PagesProviderHealthLiveIdentity,
) -> Result<ValidatedAcceptance, PagesProviderHealthBindingError> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_ACCEPTANCE_PACKET_BYTES {
        return Err(PagesProviderHealthBindingError::InvalidAcceptanceFile);
    }
    let packet: OwnerAcceptancePacket =
        serde_json::from_slice(bytes).map_err(|_| PagesProviderHealthBindingError::InvalidJson)?;
    validate_packet(&packet, live_identity)?;
    let evaluated_at = canonical_timestamp(&packet.evaluation.evaluated_at)?;
    let decided_at = canonical_timestamp(&packet.decided_at)?;
    let health_valid_until = canonical_timestamp(&packet.evaluation.health_valid_until)?;
    if decided_at < evaluated_at
        || decided_at > health_valid_until + Duration::seconds(MAX_CLOCK_SKEW_SECONDS)
    {
        return Err(PagesProviderHealthBindingError::EvidenceBoundsInvalid);
    }
    Ok(ValidatedAcceptance {
        snapshot: packet.evaluation.snapshot,
        evaluated_at,
        decided_at,
        health_valid_until,
    })
}

fn validate_packet(
    packet: &OwnerAcceptancePacket,
    live_identity: &PagesProviderHealthLiveIdentity,
) -> Result<(), PagesProviderHealthBindingError> {
    if packet.format != ACCEPTANCE_FORMAT
        || packet.status != ACCEPTED_STATUS
        || packet.decision.value != ACCEPT_DECISION
        || packet.decision.rollback_action.as_deref() != Some(ROLLBACK_ACTION)
        || !packet.decision.owner_identity_is_operator_assertion
        || packet.decision.cryptographic_signature_present
        || packet.decision.free_text_reason_retained
        || packet.evaluation.format != EVALUATION_FORMAT
        || packet.evaluation.status != EVALUATION_STATUS
        || !packet.evaluation.source_hashes_verified_against_checkout
        || packet.evaluation.raw_evaluation_path_persisted
        || !packet.binding.server_binding_authorized
        || packet.binding.server_binding_performed
        || packet.binding.failure_action != ROLLBACK_ACTION
        || packet.pages_provider_health_observed
        || packet.pages_ui_provider_health_bound
        || packet.pages_ssr_provider_health_bound
        || packet.standalone_browser_intent_provider_health_bound
        || packet.pages_reference_consumer_gate_accepted
        || packet.forum_wave_accepted
        || packet.ffa_promoted
        || packet.fba_promoted
        || !bounded_owner_id(&packet.decision.owner_id)
        || !canonical_sha256(&packet.evaluation.evaluation_sha256)
        || packet.source_files.is_empty()
        || packet
            .source_files
            .iter()
            .any(|(path, digest)| path.is_empty() || path.len() > 4096 || !canonical_sha256(digest))
    {
        return Err(PagesProviderHealthBindingError::InvalidContract);
    }

    if packet.deployment.source_commit != live_identity.source_commit
        || packet.deployment.deployment_id != live_identity.deployment_id
        || packet.deployment.deployment_image_digest != live_identity.deployment_image_digest
        || packet.binding.required_live_source_commit != live_identity.source_commit
        || packet.binding.required_deployment_image_digest != live_identity.deployment_image_digest
        || !canonical_commit(&packet.deployment.source_commit)
        || !bounded_deployment_id(&packet.deployment.deployment_id)
        || !canonical_repo_digest(&packet.deployment.deployment_image_digest)
    {
        return Err(PagesProviderHealthBindingError::IdentityMismatch);
    }

    if packet.deployment.expected_target_count == 0
        || packet.deployment.expected_target_count > 64
        || packet.deployment.expected_target_count != packet.deployment.verified_backend_target_count
        || packet.deployment.query_window_seconds < MIN_QUERY_WINDOW_SECONDS
        || packet.deployment.query_window_seconds > MAX_QUERY_WINDOW_SECONDS
        || packet.deployment.freshness_seconds < MIN_FRESHNESS_SECONDS
        || packet.deployment.freshness_seconds > packet.deployment.query_window_seconds
        || !packet.deployment.identity_age_seconds.is_finite()
        || packet.deployment.identity_age_seconds < packet.deployment.query_window_seconds as f64
        || packet.deployment.identity_age_seconds > MAX_IDENTITY_AGE_SECONDS
        || !packet.evaluation.max_target_operation_freshness_age_seconds.is_finite()
        || packet.evaluation.max_target_operation_freshness_age_seconds < 0.0
        || packet.evaluation.max_target_operation_freshness_age_seconds
            > packet.deployment.freshness_seconds as f64
        || !packet.evaluation.samples.preview.is_finite()
        || !packet.evaluation.samples.publish.is_finite()
        || packet.evaluation.samples.preview < MINIMUM_SAMPLES_PER_OPERATION
        || packet.evaluation.samples.publish < MINIMUM_SAMPLES_PER_OPERATION
    {
        return Err(PagesProviderHealthBindingError::EvidenceBoundsInvalid);
    }

    let evaluated_at = canonical_timestamp(&packet.evaluation.evaluated_at)?;
    let health_valid_until = canonical_timestamp(&packet.evaluation.health_valid_until)?;
    let remaining_millis = ((packet.deployment.freshness_seconds as f64
        - packet.evaluation.max_target_operation_freshness_age_seconds)
        * 1000.0)
        .floor();
    if !remaining_millis.is_finite() || remaining_millis < 0.0 || remaining_millis > i64::MAX as f64 {
        return Err(PagesProviderHealthBindingError::EvidenceBoundsInvalid);
    }
    let expected_valid_until = evaluated_at + Duration::milliseconds(remaining_millis as i64);
    if health_valid_until != expected_valid_until {
        return Err(PagesProviderHealthBindingError::EvidenceBoundsInvalid);
    }

    let canonical_snapshot = ProviderHealthSnapshot::evaluate(packet.evaluation.snapshot.observed);
    if packet.evaluation.snapshot != canonical_snapshot {
        return Err(PagesProviderHealthBindingError::HealthPolicyMismatch);
    }
    let canonical_slo = ProviderSloEvaluation::evaluate(
        canonical_snapshot.observed,
        canonical_snapshot.thresholds,
    );
    if packet.evaluation.slo_evaluation != canonical_slo {
        return Err(PagesProviderHealthBindingError::HealthPolicyMismatch);
    }

    canonical_timestamp(&packet.decided_at)?;
    Ok(())
}

fn canonical_timestamp(raw: &str) -> Result<DateTime<Utc>, PagesProviderHealthBindingError> {
    let parsed = DateTime::parse_from_rfc3339(raw)
        .map_err(|_| PagesProviderHealthBindingError::EvidenceBoundsInvalid)?
        .with_timezone(&Utc);
    if parsed.to_rfc3339_opts(SecondsFormat::Millis, true) != raw {
        return Err(PagesProviderHealthBindingError::EvidenceBoundsInvalid);
    }
    Ok(parsed)
}

fn canonical_commit(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn canonical_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn bounded_owner_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn bounded_deployment_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'/' | b'-')
        })
}

fn canonical_repo_digest(value: &str) -> bool {
    let Some((repository, digest)) = value.rsplit_once("@sha256:") else {
        return false;
    };
    !repository.is_empty()
        && repository.len() <= 1024
        && !repository
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte == b'@')
        && canonical_sha256(digest)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerAcceptancePacket {
    format: String,
    status: String,
    decided_at: String,
    decision: OwnerDecision,
    deployment: AcceptedDeployment,
    evaluation: AcceptedEvaluation,
    binding: BindingAuthority,
    source_files: BTreeMap<String, String>,
    pages_provider_health_observed: bool,
    pages_ui_provider_health_bound: bool,
    pages_ssr_provider_health_bound: bool,
    standalone_browser_intent_provider_health_bound: bool,
    pages_reference_consumer_gate_accepted: bool,
    forum_wave_accepted: bool,
    ffa_promoted: bool,
    fba_promoted: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerDecision {
    value: String,
    owner_id: String,
    owner_identity_is_operator_assertion: bool,
    cryptographic_signature_present: bool,
    rollback_action: Option<String>,
    free_text_reason_retained: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptedDeployment {
    deployment_id: String,
    deployment_image_digest: String,
    source_commit: String,
    expected_target_count: u64,
    verified_backend_target_count: u64,
    query_window_seconds: u64,
    freshness_seconds: u64,
    identity_age_seconds: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptedEvaluation {
    format: String,
    status: String,
    evaluated_at: String,
    max_target_operation_freshness_age_seconds: f64,
    health_valid_until: String,
    evaluation_sha256: String,
    raw_evaluation_path_persisted: bool,
    source_hashes_verified_against_checkout: bool,
    samples: AcceptedSamples,
    snapshot: ProviderHealthSnapshot,
    slo_evaluation: ProviderSloEvaluation,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptedSamples {
    preview: f64,
    publish: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingAuthority {
    server_binding_authorized: bool,
    server_binding_performed: bool,
    required_live_source_commit: String,
    required_deployment_image_digest: String,
    failure_action: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_page_builder::health::ProviderSloObservations;

    #[test]
    fn binding_environment_names_are_pages_owned_and_source_identity_is_shared() {
        assert_eq!(
            PAGES_PROVIDER_HEALTH_ACCEPTANCE_PATH_ENV,
            "RUSTOK_PAGES_PROVIDER_HEALTH_ACCEPTANCE_PATH"
        );
        assert_eq!(
            PAGES_PROVIDER_HEALTH_DEPLOYMENT_ID_ENV,
            "RUSTOK_PAGES_PROVIDER_HEALTH_DEPLOYMENT_ID"
        );
        assert_eq!(
            PAGES_PROVIDER_HEALTH_DEPLOYMENT_IMAGE_DIGEST_ENV,
            "RUSTOK_PAGES_PROVIDER_HEALTH_DEPLOYMENT_IMAGE_DIGEST"
        );
        assert_eq!(
            PAGE_BUILDER_PROVIDER_HEALTH_SOURCE_COMMIT_ENV,
            "RUSTOK_SOURCE_COMMIT"
        );
    }

    #[test]
    fn identity_helpers_reject_noncanonical_values() {
        assert!(canonical_commit(&"a".repeat(40)));
        assert!(!canonical_commit(&"A".repeat(40)));
        assert!(bounded_deployment_id("prod-eu/blue:1"));
        assert!(!bounded_deployment_id("prod eu"));
        assert!(canonical_repo_digest(&format!(
            "ghcr.io/rustok/server@sha256:{}",
            "b".repeat(64)
        )));
    }

    #[test]
    fn authority_freshness_expires_to_unobserved() {
        let evaluated_at = DateTime::parse_from_rfc3339("2026-08-09T18:00:00.000Z")
            .unwrap()
            .with_timezone(&Utc);
        let accepted = ValidatedAcceptance {
            snapshot: ProviderHealthSnapshot::evaluate(ProviderSloObservations {
                preview_p95_ms: 100,
                publish_p95_ms: 200,
                sanitize_failure_rate: 0.0,
                runtime_error_rate: 0.0,
            }),
            evaluated_at,
            decided_at: evaluated_at,
            health_valid_until: evaluated_at + Duration::seconds(60),
        };
        assert!(accepted
            .snapshot_at(evaluated_at + Duration::seconds(65))
            .is_some());
        assert!(accepted
            .snapshot_at(evaluated_at + Duration::seconds(66))
            .is_none());
    }
}
