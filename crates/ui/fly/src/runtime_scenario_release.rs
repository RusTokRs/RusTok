use crate::{
    ContentDigest, FLY_RUNTIME_SCENARIO_RENDER_SNAPSHOT, PageSelection, ProjectDocument,
    ProjectHash, RenderPolicy, RuntimeContextScenario, RuntimeScenarioRegressionStatus,
    RuntimeScenarioRenderDiff, RuntimeScenarioRenderSnapshot, ValidationDiagnostic,
    ValidationSeverity, diff_runtime_scenario_render_snapshots,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const FLY_RUNTIME_SCENARIO_RELEASE_BASELINE: &str = "fly_runtime_scenario_release_baseline";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeScenarioReleaseBaseline {
    pub format: String,
    pub baseline_id: String,
    pub source_project_hash: String,
    pub scenarios: Vec<RuntimeContextScenario>,
    pub snapshot: RuntimeScenarioRenderSnapshot,
    pub baseline_hash: String,
}

impl RuntimeScenarioReleaseBaseline {
    pub fn capture(
        baseline_id: impl Into<String>,
        document: &ProjectDocument,
        selection: &PageSelection,
        policy: &RenderPolicy,
        scenarios: &[RuntimeContextScenario],
    ) -> Self {
        let mut baseline = Self {
            format: FLY_RUNTIME_SCENARIO_RELEASE_BASELINE.to_string(),
            baseline_id: baseline_id.into(),
            source_project_hash: document.hash().hex(),
            scenarios: scenarios.to_vec(),
            snapshot: RuntimeScenarioRenderSnapshot::capture(
                document, selection, policy, scenarios,
            ),
            baseline_hash: String::new(),
        };
        baseline.baseline_hash = baseline.computed_hash();
        baseline
    }

    /// Collision-resistant digest of the baseline's contents, rendered `sha256:<64 hex chars>`.
    ///
    /// An empty string means the baseline could not be encoded. Empty never parses as a
    /// [`ContentDigest`], so [`Self::has_valid_hash`] fails closed rather than comparing a
    /// constant.
    pub fn computed_hash(&self) -> String {
        match baseline_payload(self) {
            Some(bytes) => ContentDigest::from_bytes(&bytes).to_string(),
            None => String::new(),
        }
    }

    /// Whether `baseline_hash` matches these contents.
    ///
    /// Accepts the current `sha256:<hex>` digest and, for baselines captured before 2026-10-08,
    /// the retired FNV-1a 64 fingerprint — see [`Self::has_legacy_hash`]. This is the gate that
    /// `runtime_scenario_baseline_hash_invalid` reports on, and it is the value persisted as the
    /// compare-and-swap precondition when a baseline is replaced or deleted.
    pub fn has_valid_hash(&self) -> bool {
        match ContentDigest::parse(&self.baseline_hash) {
            Some(stored) => match baseline_payload(self) {
                Some(bytes) => stored.matches(&ContentDigest::from_bytes(&bytes)),
                None => false,
            },
            None => self.has_legacy_hash(),
        }
    }

    /// Whether `baseline_hash` is the retired FNV-1a 64 fingerprint of these contents.
    ///
    /// The baseline gate hashed with [`ProjectHash`] until 2026-10-08, and a 64-bit
    /// non-cryptographic fingerprint can be collided on demand. The form is still recognised so
    /// that already-persisted baselines keep loading; [`Self::is_legacy_hash_form`] reports it and
    /// [`Self::upgrade_legacy_hashes`] rewrites it. Nothing captured now produces it.
    pub fn has_legacy_hash(&self) -> bool {
        let Some(bytes) = baseline_payload(self) else {
            return false;
        };
        !self.baseline_hash.is_empty()
            && self.baseline_hash == ProjectHash::from_bytes(&bytes).hex()
    }

    /// Whether this baseline and its snapshot both still carry a retired FNV-1a 64 hash matching
    /// their contents.
    ///
    /// That is the shape of every baseline persisted before 2026-10-08, and the only shape
    /// [`Self::upgrade_legacy_hashes`] rewrites.
    pub fn is_legacy_hash_form(&self) -> bool {
        self.has_legacy_hash() && self.snapshot.has_legacy_hash()
    }

    /// Rewrite the retired FNV-1a 64 hashes of a fully legacy baseline as sha256 digests.
    ///
    /// Returns whether anything changed. This migrates the storage format of a row whose contents
    /// already matched *both* recorded fingerprints, so it preserves a verification result
    /// instead of manufacturing one: contents are never touched, and a row that fails either
    /// fingerprint — a mixed or altered envelope, which is exactly what the baseline hash exists
    /// to catch — is left as it is for [`Self::validate`] to reject. `baseline_hash` covers
    /// `snapshot.snapshot_hash`, so it is recomputed from the rewritten snapshot.
    ///
    /// Trust follows the fingerprint, not the algorithm: a row that verified under the retired
    /// FNV-1a form becomes verified under sha256, which is the same claim the old code made about
    /// it. The difference is that the claim can no longer be forged with a cheap 64-bit
    /// collision — rewriting a row already required the write access that this hash exists to
    /// hold to account.
    pub fn upgrade_legacy_hashes(&mut self) -> bool {
        if !self.is_legacy_hash_form() {
            return false;
        }
        self.snapshot.upgrade_legacy_hash();
        self.baseline_hash = self.computed_hash();
        true
    }

    pub fn validate(&self) -> Vec<ValidationDiagnostic> {
        let mut diagnostics = Vec::new();
        if self.format != FLY_RUNTIME_SCENARIO_RELEASE_BASELINE {
            diagnostics.push(release_diagnostic(
                "runtime_scenario_baseline_format_invalid",
                "baseline.format",
                format!(
                    "runtime scenario baseline format `{}` is unsupported; expected `{FLY_RUNTIME_SCENARIO_RELEASE_BASELINE}`",
                    self.format
                ),
            ));
        }
        if self.baseline_id.trim().is_empty() {
            diagnostics.push(release_diagnostic(
                "runtime_scenario_baseline_id_empty",
                "baseline.baseline_id",
                "runtime scenario baseline id must not be empty",
            ));
        }
        if self.source_project_hash.trim().is_empty() {
            diagnostics.push(release_diagnostic(
                "runtime_scenario_baseline_source_hash_empty",
                "baseline.source_project_hash",
                "runtime scenario baseline source project hash must not be empty",
            ));
        }
        if self.scenarios.is_empty() {
            diagnostics.push(release_diagnostic(
                "runtime_scenario_baseline_scenarios_empty",
                "baseline.scenarios",
                "runtime scenario release baseline must contain at least one scenario",
            ));
        }
        let mut ids = BTreeSet::new();
        for (index, scenario) in self.scenarios.iter().enumerate() {
            if scenario.id.trim().is_empty() {
                diagnostics.push(release_diagnostic(
                    "runtime_scenario_baseline_scenario_id_empty",
                    format!("baseline.scenarios[{index}].id"),
                    "runtime scenario id must not be empty",
                ));
            } else if !ids.insert(scenario.id.as_str()) {
                diagnostics.push(release_diagnostic(
                    "runtime_scenario_baseline_scenario_id_duplicate",
                    format!("baseline.scenarios[{index}].id"),
                    format!("runtime scenario id `{}` is duplicated", scenario.id),
                ));
            }
        }
        if self.snapshot.format != FLY_RUNTIME_SCENARIO_RENDER_SNAPSHOT {
            diagnostics.push(release_diagnostic(
                "runtime_scenario_baseline_snapshot_format_invalid",
                "baseline.snapshot.format",
                format!(
                    "runtime scenario snapshot format `{}` is unsupported",
                    self.snapshot.format
                ),
            ));
        }
        if !self.snapshot.has_valid_hash() {
            diagnostics.push(release_diagnostic(
                "runtime_scenario_baseline_snapshot_hash_invalid",
                "baseline.snapshot.snapshot_hash",
                "runtime scenario snapshot integrity hash does not match its contents",
            ));
        }
        if !self.snapshot.is_renderable() {
            diagnostics.push(release_diagnostic(
                "runtime_scenario_baseline_snapshot_not_renderable",
                "baseline.snapshot",
                "runtime scenario release baseline must be captured from a renderable matrix",
            ));
        }
        if self.snapshot.cases.len() != self.scenarios.len() {
            diagnostics.push(release_diagnostic(
                "runtime_scenario_baseline_case_count_mismatch",
                "baseline.snapshot.cases",
                format!(
                    "runtime scenario baseline contains {} scenarios but {} snapshot cases",
                    self.scenarios.len(),
                    self.snapshot.cases.len()
                ),
            ));
        }
        if !self.has_valid_hash() {
            diagnostics.push(release_diagnostic(
                "runtime_scenario_baseline_hash_invalid",
                "baseline.baseline_hash",
                "runtime scenario release baseline integrity hash does not match its contents",
            ));
        }
        diagnostics
    }

    pub fn is_valid(&self) -> bool {
        self.validate().is_empty()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeScenarioReleaseMode {
    Disabled,
    BlockBroken,
    RequireStable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeScenarioReleasePolicy {
    pub mode: RuntimeScenarioReleaseMode,
    pub require_baseline: bool,
}

impl RuntimeScenarioReleasePolicy {
    pub const fn disabled() -> Self {
        Self {
            mode: RuntimeScenarioReleaseMode::Disabled,
            require_baseline: false,
        }
    }

    pub const fn block_broken() -> Self {
        Self {
            mode: RuntimeScenarioReleaseMode::BlockBroken,
            require_baseline: true,
        }
    }

    pub const fn require_stable() -> Self {
        Self {
            mode: RuntimeScenarioReleaseMode::RequireStable,
            require_baseline: true,
        }
    }
}

impl Default for RuntimeScenarioReleasePolicy {
    fn default() -> Self {
        Self::disabled()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeScenarioReleaseStatus {
    Disabled,
    BaselineMissing,
    BaselineInvalid,
    Stable,
    RequiresReview,
    Broken,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeScenarioReleaseEvaluation {
    pub allowed: bool,
    pub status: RuntimeScenarioReleaseStatus,
    pub baseline_id: Option<String>,
    pub baseline_hash: Option<String>,
    pub candidate_snapshot: Option<RuntimeScenarioRenderSnapshot>,
    pub diff: Option<RuntimeScenarioRenderDiff>,
    pub diagnostics: Vec<ValidationDiagnostic>,
}

impl RuntimeScenarioReleaseEvaluation {
    pub fn blocking_diagnostics(&self) -> impl Iterator<Item = &ValidationDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
    }
}

pub fn evaluate_runtime_scenario_release(
    document: &ProjectDocument,
    baseline: Option<&RuntimeScenarioReleaseBaseline>,
    policy: RuntimeScenarioReleasePolicy,
) -> RuntimeScenarioReleaseEvaluation {
    if policy.mode == RuntimeScenarioReleaseMode::Disabled {
        return RuntimeScenarioReleaseEvaluation {
            allowed: true,
            status: RuntimeScenarioReleaseStatus::Disabled,
            baseline_id: baseline.map(|baseline| baseline.baseline_id.clone()),
            baseline_hash: baseline.map(|baseline| baseline.baseline_hash.clone()),
            candidate_snapshot: None,
            diff: None,
            diagnostics: Vec::new(),
        };
    }

    let Some(baseline) = baseline else {
        let diagnostics = policy.require_baseline.then(|| {
            vec![release_diagnostic(
                "runtime_scenario_release_baseline_missing",
                "baseline",
                "runtime scenario release policy requires a persisted baseline",
            )]
        });
        return RuntimeScenarioReleaseEvaluation {
            allowed: !policy.require_baseline,
            status: RuntimeScenarioReleaseStatus::BaselineMissing,
            baseline_id: None,
            baseline_hash: None,
            candidate_snapshot: None,
            diff: None,
            diagnostics: diagnostics.unwrap_or_default(),
        };
    };

    let diagnostics = baseline.validate();
    if !diagnostics.is_empty() {
        return RuntimeScenarioReleaseEvaluation {
            allowed: false,
            status: RuntimeScenarioReleaseStatus::BaselineInvalid,
            baseline_id: Some(baseline.baseline_id.clone()),
            baseline_hash: Some(baseline.baseline_hash.clone()),
            candidate_snapshot: None,
            diff: None,
            diagnostics,
        };
    }

    let candidate = RuntimeScenarioRenderSnapshot::capture(
        document,
        &baseline.snapshot.selection,
        &baseline.snapshot.policy,
        &baseline.scenarios,
    );
    let diff = diff_runtime_scenario_render_snapshots(&baseline.snapshot, &candidate);
    let status = match diff.status {
        RuntimeScenarioRegressionStatus::Stable => RuntimeScenarioReleaseStatus::Stable,
        RuntimeScenarioRegressionStatus::RequiresReview => {
            RuntimeScenarioReleaseStatus::RequiresReview
        }
        RuntimeScenarioRegressionStatus::Broken => RuntimeScenarioReleaseStatus::Broken,
    };
    let allowed = match policy.mode {
        RuntimeScenarioReleaseMode::Disabled => true,
        RuntimeScenarioReleaseMode::BlockBroken => {
            diff.status != RuntimeScenarioRegressionStatus::Broken
        }
        RuntimeScenarioReleaseMode::RequireStable => {
            diff.status == RuntimeScenarioRegressionStatus::Stable
        }
    };
    let diagnostics = if allowed {
        Vec::new()
    } else {
        vec![release_diagnostic(
            "runtime_scenario_release_regression_blocked",
            "baseline",
            format!(
                "runtime scenario regression status `{:?}` is rejected by release policy `{:?}`",
                diff.status, policy.mode
            ),
        )]
    };

    RuntimeScenarioReleaseEvaluation {
        allowed,
        status,
        baseline_id: Some(baseline.baseline_id.clone()),
        baseline_hash: Some(baseline.baseline_hash.clone()),
        candidate_snapshot: Some(candidate),
        diff: Some(diff),
        diagnostics,
    }
}

/// Serialize the fields `baseline_hash` covers, or `None` when they cannot be encoded.
fn baseline_payload(baseline: &RuntimeScenarioReleaseBaseline) -> Option<Vec<u8>> {
    serde_json::to_vec(&(
        &baseline.format,
        &baseline.baseline_id,
        &baseline.source_project_hash,
        &baseline.scenarios,
        &baseline.snapshot,
    ))
    .ok()
}

fn release_diagnostic(
    code: impl Into<String>,
    path: impl Into<String>,
    message: impl Into<String>,
) -> ValidationDiagnostic {
    ValidationDiagnostic {
        severity: ValidationSeverity::Error,
        code: code.into(),
        path: path.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsCodec;
    use serde_json::json;

    fn document() -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{ "id": "title", "type": "text" }]
                }
            }],
            "flyRuntimeBindings": [{
                "id": "title-content",
                "component_id": "title",
                "path": "page.title",
                "target": "field",
                "name": "content"
            }]
        }))
        .expect("document")
    }

    fn scenarios(title: &str) -> Vec<RuntimeContextScenario> {
        vec![RuntimeContextScenario::new(
            "default",
            "Default",
            json!({ "page": { "title": title } }),
        )]
    }

    #[test]
    fn stable_candidate_passes_strict_gate() {
        let document = document();
        let baseline = RuntimeScenarioReleaseBaseline::capture(
            "release-1",
            &document,
            &PageSelection::First,
            &RenderPolicy::default(),
            &scenarios("Welcome"),
        );
        assert!(baseline.is_valid());
        let evaluation = evaluate_runtime_scenario_release(
            &document,
            Some(&baseline),
            RuntimeScenarioReleasePolicy::require_stable(),
        );
        assert!(evaluation.allowed);
        assert_eq!(evaluation.status, RuntimeScenarioReleaseStatus::Stable);
    }

    #[test]
    fn visual_drift_passes_block_broken_but_not_strict_gate() {
        let document = document();
        let baseline = RuntimeScenarioReleaseBaseline::capture(
            "release-1",
            &document,
            &PageSelection::First,
            &RenderPolicy::default(),
            &scenarios("Welcome"),
        );
        let mut changed = baseline.clone();
        changed.scenarios = scenarios("Changed");
        changed.snapshot = baseline.snapshot.clone();
        changed.baseline_hash = changed.computed_hash();

        let review = evaluate_runtime_scenario_release(
            &document,
            Some(&changed),
            RuntimeScenarioReleasePolicy::block_broken(),
        );
        assert!(review.allowed);
        assert_eq!(review.status, RuntimeScenarioReleaseStatus::RequiresReview);

        let strict = evaluate_runtime_scenario_release(
            &document,
            Some(&changed),
            RuntimeScenarioReleasePolicy::require_stable(),
        );
        assert!(!strict.allowed);
    }

    #[test]
    fn captured_baseline_records_sha256_digests() {
        let baseline = RuntimeScenarioReleaseBaseline::capture(
            "release-1",
            &document(),
            &PageSelection::First,
            &RenderPolicy::default(),
            &scenarios("Welcome"),
        );
        assert!(baseline.baseline_hash.starts_with("sha256:"));
        assert_eq!(baseline.baseline_hash.len(), "sha256:".len() + 64);
        assert!(baseline.snapshot.snapshot_hash.starts_with("sha256:"));
        assert!(!baseline.is_legacy_hash_form());
        assert!(baseline.is_valid());
    }

    /// Rebuild the storage form Fly wrote before 2026-10-08: bare FNV-1a 64 fingerprints on both
    /// the snapshot and the baseline that covers it.
    fn legacy_form(baseline: &mut RuntimeScenarioReleaseBaseline) {
        baseline.snapshot.snapshot_hash = baseline.snapshot.legacy_hash_for_tests();
        let payload = super::baseline_payload(baseline).expect("baseline payload");
        baseline.baseline_hash = ProjectHash::from_bytes(&payload).hex();
    }

    #[test]
    fn legacy_fnv_baseline_is_accepted_then_upgraded_in_place() {
        let document = document();
        let mut baseline = RuntimeScenarioReleaseBaseline::capture(
            "release-1",
            &document,
            &PageSelection::First,
            &RenderPolicy::default(),
            &scenarios("Welcome"),
        );
        legacy_form(&mut baseline);
        assert!(baseline.has_legacy_hash());
        assert!(baseline.snapshot.has_legacy_hash());
        assert!(baseline.is_legacy_hash_form());
        assert!(baseline.is_valid());

        // A baseline stored before the switch still releases; it is not retroactively rejected.
        let evaluation = evaluate_runtime_scenario_release(
            &document,
            Some(&baseline),
            RuntimeScenarioReleasePolicy::require_stable(),
        );
        assert!(evaluation.allowed);
        assert_eq!(evaluation.status, RuntimeScenarioReleaseStatus::Stable);

        assert!(baseline.upgrade_legacy_hashes());
        assert!(!baseline.has_legacy_hash());
        assert!(!baseline.snapshot.has_legacy_hash());
        assert!(baseline.is_valid());
        assert!(baseline.baseline_hash.starts_with("sha256:"));
        assert!(!baseline.upgrade_legacy_hashes(), "upgrade is idempotent");
    }

    #[test]
    fn upgrade_does_not_repair_a_value_that_matches_no_recorded_content() {
        let mut baseline = RuntimeScenarioReleaseBaseline::capture(
            "release-1",
            &document(),
            &PageSelection::First,
            &RenderPolicy::default(),
            &scenarios("Welcome"),
        );
        baseline.baseline_hash = "0000000000000000".to_string();
        assert!(!baseline.has_legacy_hash());
        assert!(!baseline.is_legacy_hash_form());
        assert!(!baseline.upgrade_legacy_hashes());
        assert_eq!(baseline.baseline_hash, "0000000000000000");
        assert!(!baseline.is_valid());
    }

    #[test]
    fn upgrade_does_not_launder_an_altered_envelope() {
        // Snapshot left in the legacy form, envelope no longer covered by any recorded hash: the
        // row is not fully legacy, so migration must leave it broken rather than re-derive the
        // envelope hash from contents that were never verified against it.
        let mut baseline = RuntimeScenarioReleaseBaseline::capture(
            "release-1",
            &document(),
            &PageSelection::First,
            &RenderPolicy::default(),
            &scenarios("Welcome"),
        );
        baseline.snapshot.snapshot_hash = baseline.snapshot.legacy_hash_for_tests();
        assert!(baseline.snapshot.has_legacy_hash());
        assert!(!baseline.has_legacy_hash());
        assert!(!baseline.is_legacy_hash_form());
        assert!(!baseline.upgrade_legacy_hashes());
        assert!(!baseline.has_valid_hash());
        assert!(!baseline.is_valid());
    }

    #[test]
    fn invalid_integrity_hash_blocks_release() {
        let document = document();
        let mut baseline = RuntimeScenarioReleaseBaseline::capture(
            "release-1",
            &document,
            &PageSelection::First,
            &RenderPolicy::default(),
            &scenarios("Welcome"),
        );
        baseline.baseline_hash = "tampered".to_string();
        let evaluation = evaluate_runtime_scenario_release(
            &document,
            Some(&baseline),
            RuntimeScenarioReleasePolicy::block_broken(),
        );
        assert!(!evaluation.allowed);
        assert_eq!(
            evaluation.status,
            RuntimeScenarioReleaseStatus::BaselineInvalid
        );
    }

    #[test]
    fn required_baseline_blocks_when_missing() {
        let evaluation = evaluate_runtime_scenario_release(
            &document(),
            None,
            RuntimeScenarioReleasePolicy::block_broken(),
        );
        assert!(!evaluation.allowed);
        assert_eq!(
            evaluation.status,
            RuntimeScenarioReleaseStatus::BaselineMissing
        );
    }
}
