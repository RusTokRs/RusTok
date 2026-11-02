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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorization_fingerprint_is_order_independent() {
        let perm_a = ArtifactPermissionRegistration {
            key: "catalog:read".to_string(),
            localizations: vec![ArtifactPermissionLocalization {
                locale: "en".to_string(),
                label: "Read Catalog".to_string(),
                description: "Read catalog products".to_string(),
            }],
        };
        let perm_b = ArtifactPermissionRegistration {
            key: "orders:create".to_string(),
            localizations: vec![ArtifactPermissionLocalization {
                locale: "en".to_string(),
                label: "Create Orders".to_string(),
                description: "Create new customer orders".to_string(),
            }],
        };

        let fingerprint_ordered =
            compute_canonical_authorization_fingerprint(&[perm_a.clone(), perm_b.clone()]);
        let fingerprint_reversed =
            compute_canonical_authorization_fingerprint(&[perm_b.clone(), perm_a.clone()]);

        assert_eq!(fingerprint_ordered, fingerprint_reversed);
        assert!(fingerprint_ordered.starts_with("sha256:"));
    }

    #[test]
    fn authorization_fingerprint_excludes_localization_differences() {
        let perm_en = ArtifactPermissionRegistration {
            key: "catalog:read".to_string(),
            localizations: vec![ArtifactPermissionLocalization {
                locale: "en".to_string(),
                label: "Read Catalog".to_string(),
                description: "English description".to_string(),
            }],
        };
        let perm_ru = ArtifactPermissionRegistration {
            key: "catalog:read".to_string(),
            localizations: vec![ArtifactPermissionLocalization {
                locale: "ru".to_string(),
                label: "Чтение каталога".to_string(),
                description: "Русское описание".to_string(),
            }],
        };

        let fingerprint_en = compute_canonical_authorization_fingerprint(&[perm_en]);
        let fingerprint_ru = compute_canonical_authorization_fingerprint(&[perm_ru]);

        assert_eq!(
            fingerprint_en, fingerprint_ru,
            "localization changes must never alter authorization fingerprint"
        );
    }

    #[test]
    fn artifact_permission_scope_serde_roundtrips() {
        let platform = ArtifactPermissionScope::Platform;
        let encoded_platform = serde_json::to_string(&platform).expect("serialize platform scope");
        assert_eq!(encoded_platform, r#"{"kind":"platform"}"#);
        assert_eq!(
            serde_json::from_str::<ArtifactPermissionScope>(&encoded_platform).unwrap(),
            platform
        );

        let tenant_id = Uuid::new_v4();
        let tenant = ArtifactPermissionScope::Tenant { tenant_id };
        let encoded_tenant = serde_json::to_string(&tenant).expect("serialize tenant scope");
        assert_eq!(
            serde_json::from_str::<ArtifactPermissionScope>(&encoded_tenant).unwrap(),
            tenant
        );
    }

    #[test]
    fn artifact_permission_diff_default_and_serde() {
        let diff = ArtifactPermissionDiff::default();
        assert!(diff.unchanged_keys.is_empty());
        assert!(diff.modified_keys.is_empty());
        assert!(diff.added_keys.is_empty());
        assert!(diff.removed_dormant_keys.is_empty());

        let diff_with_keys = ArtifactPermissionDiff {
            unchanged_keys: vec!["a".to_string()],
            modified_keys: vec!["b".to_string()],
            added_keys: vec!["c".to_string()],
            removed_dormant_keys: vec!["d".to_string()],
        };
        let serialized = serde_json::to_string(&diff_with_keys).expect("serialize diff");
        assert_eq!(
            serde_json::from_str::<ArtifactPermissionDiff>(&serialized).unwrap(),
            diff_with_keys
        );
    }
}
