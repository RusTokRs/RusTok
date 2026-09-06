//! Transport-neutral contracts for RBAC-owned artifact permission registration.

use async_trait::async_trait;
use hex::ToHex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::PortError;

/// Scope under which an admitted artifact permission becomes available to RBAC.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ArtifactPermissionScope {
    Platform,
    Tenant { tenant_id: Uuid },
}

/// Localized, immutable operator-facing metadata for one artifact permission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactPermissionLocalization {
    pub locale: String,
    pub label: String,
    pub description: String,
}

/// One module-owned permission registered from an admitted immutable release.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactPermissionRegistration {
    pub key: String,
    pub localizations: Vec<ArtifactPermissionLocalization>,
}

/// Computes the canonical SHA-256 authorization fingerprint over sorted permission keys.
///
/// Localized display text (labels, descriptions) is explicitly excluded from this fingerprint,
/// guaranteeing that translation updates never invalidate existing authorization grants.
pub fn compute_canonical_authorization_fingerprint(
    permissions: &[ArtifactPermissionRegistration],
) -> String {
    let mut sorted_keys: Vec<&str> = permissions.iter().map(|p| p.key.as_str()).collect();
    sorted_keys.sort_unstable();
    let mut hasher = Sha256::new();
    for key in sorted_keys {
        hasher.update(key.as_bytes());
        hasher.update(b"\n");
    }
    format!("sha256:{}", hasher.finalize().encode_hex::<String>())
}

/// Request to admit immutable, inert release permission definitions.
///
/// Admission persists definitions keyed strictly by `(release_digest, module_slug, permission_key)`
/// without requiring an installation ID or scope binding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleasePermissionAdmissionRequest {
    pub module_slug: String,
    pub release_digest: String,
    pub permissions: Vec<ArtifactPermissionRegistration>,
}

/// Idempotent request to project admitted release definitions into a scoped installation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedPermissionProjectionRequest {
    pub scope: ArtifactPermissionScope,
    pub installation_id: Uuid,
    pub module_slug: String,
    pub release_digest: String,
}

/// Exact permission key diff between predecessor and candidate releases.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ArtifactPermissionDiff {
    pub unchanged_keys: Vec<String>,
    pub modified_keys: Vec<String>,
    pub added_keys: Vec<String>,
    pub removed_dormant_keys: Vec<String>,
}

/// Request to evaluate RBAC permission continuity across predecessor and candidate releases.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionContinuityEvaluationRequest {
    pub scope: ArtifactPermissionScope,
    pub predecessor_release_digest: String,
    pub candidate_release_digest: String,
    pub predecessor_permissions: Vec<ArtifactPermissionRegistration>,
    pub candidate_permissions: Vec<ArtifactPermissionRegistration>,
    pub expected_rbac_epoch: u64,
}

/// Bound continuity receipt certifying authorization fingerprint and epoch compatibility.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactPermissionContinuityReceipt {
    pub scope: ArtifactPermissionScope,
    pub predecessor_release_digest: String,
    pub candidate_release_digest: String,
    pub authorization_fingerprint: String,
    pub rbac_epoch: u64,
    pub diff: ArtifactPermissionDiff,
    pub approved: bool,
    pub receipt_digest: String,
}

/// RBAC-owned boundary for admitted artifact permissions and continuity.
#[async_trait]
pub trait ArtifactPermissionRegistrationPort: Send + Sync {
    /// Persists inert release permission definitions without an installation binding.
    async fn admit_release_permissions(
        &self,
        request: ReleasePermissionAdmissionRequest,
    ) -> Result<(), PortError>;

    /// Projects admitted release definitions idempotently under the designated scope and installation.
    async fn project_scoped_permissions(
        &self,
        request: ScopedPermissionProjectionRequest,
    ) -> Result<(), PortError>;

    /// Evaluates continuity across releases, computing permission diffs and authorization fingerprint.
    async fn evaluate_permission_continuity(
        &self,
        request: PermissionContinuityEvaluationRequest,
    ) -> Result<ArtifactPermissionContinuityReceipt, PortError>;
}
