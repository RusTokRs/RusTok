//! Governance constants, reason codes, and follow-up stages.

pub const REGISTRY_YANK_REASON_CODES: &[&str] = &[
    "security",
    "legal",
    "malware",
    "critical_regression",
    "rollback",
    "other",
];

pub const REGISTRY_OWNER_TRANSFER_REASON_CODES: &[&str] = &[
    "maintenance_handoff",
    "team_restructure",
    "publisher_rotation",
    "security_emergency",
    "governance_override",
    "other",
];

pub const REGISTRY_REJECT_REASON_CODES: &[&str] = &[
    "policy_mismatch",
    "quality_gate_failed",
    "ownership_mismatch",
    "security_risk",
    "legal",
    "other",
];

pub const REGISTRY_REQUEST_CHANGES_REASON_CODES: &[&str] = &[
    "artifact_mismatch",
    "quality_gap",
    "policy_gap",
    "docs_gap",
    "other",
];

pub const REGISTRY_HOLD_REASON_CODES: &[&str] = &[
    "release_window",
    "incident",
    "legal_hold",
    "security_review",
    "other",
];

pub const REGISTRY_RESUME_REASON_CODES: &[&str] = &[
    "review_complete",
    "incident_closed",
    "legal_cleared",
    "other",
];

pub const REGISTRY_APPROVE_OVERRIDE_REASON_CODES: &[&str] = &[
    "manual_review_complete",
    "trusted_first_party",
    "expedited_release",
    "governance_override",
    "other",
];

pub const REGISTRY_VALIDATION_STAGE_REASON_CODES: &[&str] = &[
    "local_runner_passed",
    "manual_review_complete",
    "build_failure",
    "test_failure",
    "policy_preflight_failed",
    "security_findings",
    "policy_exception",
    "license_issue",
    "manual_override",
    "other",
];

pub(crate) const REGISTRY_PUBLISH_ARTIFACT_NAMESPACE: &str = "registry-publish-artifact";

/// Explicit reasons why an external prebuilt artifact cannot provide a
/// reproducible source identity. Absence is a reviewable trust fact, never an
/// implicit downgrade to a platform build.
pub const REGISTRY_EXTERNAL_SOURCE_ABSENCE_REASON_CODES: &[&str] = &[
    "source_unavailable",
    "reproducibility_not_supported",
    "license_restriction",
    "other",
];

pub(crate) const REMOTE_VALIDATION_FOLLOW_UP_STAGES: &[&str] =
    &["compile_smoke", "targeted_tests", "security_policy_review"];

#[derive(Clone, Copy)]
pub(crate) struct PublicationFollowUpStage {
    key: &'static str,
    runner_kind: &'static str,
}

pub(crate) const PLATFORM_BUILT_FOLLOW_UP_STAGES: &[PublicationFollowUpStage] = &[
    PublicationFollowUpStage {
        key: "compile_smoke",
        runner_kind: "owner_evidence",
    },
    PublicationFollowUpStage {
        key: "targeted_tests",
        runner_kind: "owner_evidence",
    },
];
pub(crate) const EXTERNAL_PREBUILT_FOLLOW_UP_STAGES: &[PublicationFollowUpStage] =
    &[PublicationFollowUpStage {
        key: "security_policy_review",
        runner_kind: "owner_evidence",
    }];
pub(crate) const ALLOY_AUTHORED_FOLLOW_UP_STAGES: &[PublicationFollowUpStage] = &[PublicationFollowUpStage {
    key: "security_policy_review",
    runner_kind: "owner_evidence",
}];
pub const ALLOY_PUBLICATION_SMOKE_TEST_PATH: &str = "tests/publication_smoke.rhai";
pub(crate) const ALLOY_PUBLICATION_SMOKE_SCENARIO_DOMAIN: &[u8] = b"rustok.alloy.publication-smoke.scenario\0";
pub(crate) const MAX_REMOTE_VALIDATION_CLAIM_CANDIDATES: u64 = 128;
pub(crate) const MAX_PUBLICATION_REQUEST_ID_BYTES: usize = 128;
pub(crate) const MAX_PUBLICATION_EVIDENCE_REFERENCE_BYTES: usize = 512;
pub(crate) const MAX_PUBLICATION_EVIDENCE_IDENTITY_BYTES: usize = 256;
pub(crate) const MAX_PUBLICATION_EVIDENCE_POLICY_REVISION_BYTES: usize = 128;
pub(crate) const MAX_PLATFORM_ADMISSION_MEDIA_TYPE_BYTES: usize = 256;
pub(crate) const MAX_MARKETPLACE_TAXONOMY_ITEMS: usize = 32;
pub(crate) const MAX_MARKETPLACE_TAXONOMY_CHARS: usize = 64;
pub(crate) const MARKETPLACE_APPROVAL_POLICY_REVISION: &str = "registry-governance-v1";
/// A worker that has not materialized a terminal result within this interval
/// is considered lost. A later authorized enqueue marks that attempt failed
/// and creates a new durable attempt in the same owner transaction.
pub(crate) const VALIDATION_JOB_STALE_AFTER_SECONDS: u64 = 15 * 60;
pub(crate) const VALIDATION_WORK_ITEM_INVALID_ERROR: &str =
    "Validation job delivery facts are incomplete or malformed.";
pub(crate) const VALIDATION_JOB_RETRY_ERROR: &str =
    "Validation job delivery failed before artifact checks completed.";

