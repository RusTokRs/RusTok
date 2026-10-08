use crate::{
    ContentDigest, PageSelection, ProjectDocument, ProjectHash, RenderPolicy,
    RuntimeContextScenario, RuntimeScenarioRenderMatrix, ValidationDiagnostic, ValidationSeverity,
    render_runtime_scenario_matrix,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const FLY_RUNTIME_SCENARIO_RENDER_SNAPSHOT: &str = "fly_runtime_scenario_render_snapshot";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeScenarioRenderSnapshotCase {
    pub scenario_id: String,
    pub scenario_label: String,
    pub rendered: bool,
    pub page_id: Option<String>,
    pub html_hash: Option<String>,
    pub css_hash: Option<String>,
    pub document_hash: Option<String>,
    pub blocking_diagnostics: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeScenarioRenderSnapshot {
    pub format: String,
    pub selection: PageSelection,
    pub policy: RenderPolicy,
    pub cases: Vec<RuntimeScenarioRenderSnapshotCase>,
    pub matrix_diagnostics: Vec<ValidationDiagnostic>,
    pub snapshot_hash: String,
}

impl RuntimeScenarioRenderSnapshot {
    pub fn capture(
        document: &ProjectDocument,
        selection: &PageSelection,
        policy: &RenderPolicy,
        scenarios: &[RuntimeContextScenario],
    ) -> Self {
        Self::from_matrix(
            selection.clone(),
            policy.clone(),
            render_runtime_scenario_matrix(document, selection, policy, scenarios),
        )
    }

    pub fn from_matrix(
        selection: PageSelection,
        policy: RenderPolicy,
        matrix: RuntimeScenarioRenderMatrix,
    ) -> Self {
        let cases = matrix
            .cases
            .into_iter()
            .map(|case| RuntimeScenarioRenderSnapshotCase {
                scenario_id: case.scenario_id,
                scenario_label: case.scenario_label,
                rendered: case.rendered,
                page_id: case.page_id,
                html_hash: case.html_hash,
                css_hash: case.css_hash,
                document_hash: case.document_hash,
                blocking_diagnostics: case
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
                    .count(),
                error: case.error,
            })
            .collect::<Vec<_>>();
        let format = FLY_RUNTIME_SCENARIO_RENDER_SNAPSHOT.to_string();
        let snapshot_hash =
            snapshot_hash(&format, &selection, &policy, &cases, &matrix.diagnostics);
        Self {
            format,
            selection,
            policy,
            cases,
            matrix_diagnostics: matrix.diagnostics,
            snapshot_hash,
        }
    }

    pub fn is_valid_format(&self) -> bool {
        self.format == FLY_RUNTIME_SCENARIO_RENDER_SNAPSHOT
    }

    /// Whether `snapshot_hash` matches these contents.
    ///
    /// Accepts the current `sha256:<hex>` digest and, for snapshots captured before 2026-10-08,
    /// the retired FNV-1a 64 fingerprint — see [`Self::has_legacy_hash`]. Everything newly
    /// captured is sha256 only, and [`Self::upgrade_legacy_hash`] rewrites the older form.
    pub fn has_valid_hash(&self) -> bool {
        match ContentDigest::parse(&self.snapshot_hash) {
            Some(stored) => match self.payload_bytes() {
                Some(bytes) => stored.matches(&ContentDigest::from_bytes(&bytes)),
                None => false,
            },
            None => self.has_legacy_hash(),
        }
    }

    /// Whether `snapshot_hash` is the retired FNV-1a 64 fingerprint of these contents.
    ///
    /// Fly hashed replay snapshots with [`ProjectHash`] until 2026-10-08. The value is still
    /// recognised so that persisted snapshots keep loading, but it is never produced again: the
    /// digest of an approved artefact must not be forgeable with a cheap 64-bit collision.
    pub fn has_legacy_hash(&self) -> bool {
        let Some(bytes) = self.payload_bytes() else {
            return false;
        };
        !self.snapshot_hash.is_empty()
            && self.snapshot_hash == ProjectHash::from_bytes(&bytes).hex()
    }

    /// Replace a retired FNV-1a 64 fingerprint with the sha256 digest of the same contents.
    ///
    /// Returns whether the hash changed. Contents are never touched, and a hash that matches
    /// neither form (a tampered or truncated value) is left exactly as it is, so this cannot make
    /// an invalid snapshot validate.
    pub fn upgrade_legacy_hash(&mut self) -> bool {
        if !self.has_legacy_hash() {
            return false;
        }
        self.snapshot_hash = snapshot_hash(
            &self.format,
            &self.selection,
            &self.policy,
            &self.cases,
            &self.matrix_diagnostics,
        );
        true
    }

    /// The retired FNV-1a 64 fingerprint of these contents.
    ///
    /// Exists so migration fixtures can rebuild the pre-2026-10-08 storage form. Production code
    /// never produces it, and nothing reads it except tests.
    #[cfg(test)]
    pub(crate) fn legacy_hash_for_tests(&self) -> String {
        self.payload_bytes()
            .map_or_else(String::new, |bytes| ProjectHash::from_bytes(&bytes).hex())
    }

    /// The bytes `snapshot_hash` covers, or `None` when they cannot be encoded.
    fn payload_bytes(&self) -> Option<Vec<u8>> {
        snapshot_payload(
            &self.format,
            &self.selection,
            &self.policy,
            &self.cases,
            &self.matrix_diagnostics,
        )
    }

    pub fn is_renderable(&self) -> bool {
        self.matrix_diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != ValidationSeverity::Error)
            && self
                .cases
                .iter()
                .all(|case| case.rendered && case.blocking_diagnostics == 0)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeScenarioRenderChangeImpact {
    Informational,
    Visual,
    Breaking,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuntimeScenarioRenderChange {
    SnapshotFormatChanged {
        previous: String,
        next: String,
    },
    SelectionChanged {
        previous: PageSelection,
        next: PageSelection,
    },
    PolicyChanged,
    ScenarioAdded {
        scenario_id: String,
    },
    ScenarioRemoved {
        scenario_id: String,
    },
    RenderStateChanged {
        scenario_id: String,
        previous: bool,
        next: bool,
    },
    PageChanged {
        scenario_id: String,
        previous: Option<String>,
        next: Option<String>,
    },
    HtmlChanged {
        scenario_id: String,
    },
    CssChanged {
        scenario_id: String,
    },
    DocumentChanged {
        scenario_id: String,
    },
    BlockingDiagnosticsChanged {
        scenario_id: String,
        previous: usize,
        next: usize,
    },
    RenderErrorChanged {
        scenario_id: String,
        previous: Option<String>,
        next: Option<String>,
    },
}

impl RuntimeScenarioRenderChange {
    pub fn impact(&self) -> RuntimeScenarioRenderChangeImpact {
        match self {
            Self::ScenarioAdded { .. } => RuntimeScenarioRenderChangeImpact::Informational,
            Self::PolicyChanged
            | Self::HtmlChanged { .. }
            | Self::CssChanged { .. }
            | Self::DocumentChanged { .. } => RuntimeScenarioRenderChangeImpact::Visual,
            Self::SnapshotFormatChanged { .. }
            | Self::SelectionChanged { .. }
            | Self::ScenarioRemoved { .. }
            | Self::RenderStateChanged { .. }
            | Self::PageChanged { .. }
            | Self::BlockingDiagnosticsChanged { .. }
            | Self::RenderErrorChanged { .. } => RuntimeScenarioRenderChangeImpact::Breaking,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeScenarioRegressionStatus {
    Stable,
    RequiresReview,
    Broken,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeScenarioRenderDiff {
    pub previous_hash: String,
    pub next_hash: String,
    pub status: RuntimeScenarioRegressionStatus,
    pub changes: Vec<RuntimeScenarioRenderChange>,
}

impl RuntimeScenarioRenderDiff {
    pub fn is_stable(&self) -> bool {
        self.status == RuntimeScenarioRegressionStatus::Stable
    }
}

pub fn diff_runtime_scenario_render_snapshots(
    previous: &RuntimeScenarioRenderSnapshot,
    next: &RuntimeScenarioRenderSnapshot,
) -> RuntimeScenarioRenderDiff {
    let mut changes = Vec::new();
    if previous.format != next.format {
        changes.push(RuntimeScenarioRenderChange::SnapshotFormatChanged {
            previous: previous.format.clone(),
            next: next.format.clone(),
        });
    }
    if previous.selection != next.selection {
        changes.push(RuntimeScenarioRenderChange::SelectionChanged {
            previous: previous.selection.clone(),
            next: next.selection.clone(),
        });
    }
    if previous.policy != next.policy {
        changes.push(RuntimeScenarioRenderChange::PolicyChanged);
    }

    let previous_cases = previous
        .cases
        .iter()
        .map(|case| (case.scenario_id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    let next_cases = next
        .cases
        .iter()
        .map(|case| (case.scenario_id.as_str(), case))
        .collect::<BTreeMap<_, _>>();

    for scenario_id in previous_cases.keys() {
        if !next_cases.contains_key(scenario_id) {
            changes.push(RuntimeScenarioRenderChange::ScenarioRemoved {
                scenario_id: (*scenario_id).to_string(),
            });
        }
    }
    for scenario_id in next_cases.keys() {
        if !previous_cases.contains_key(scenario_id) {
            changes.push(RuntimeScenarioRenderChange::ScenarioAdded {
                scenario_id: (*scenario_id).to_string(),
            });
        }
    }
    for (scenario_id, previous_case) in previous_cases {
        let Some(next_case) = next_cases.get(scenario_id) else {
            continue;
        };
        compare_case(scenario_id, previous_case, next_case, &mut changes);
    }

    let status = if changes
        .iter()
        .any(|change| change.impact() == RuntimeScenarioRenderChangeImpact::Breaking)
    {
        RuntimeScenarioRegressionStatus::Broken
    } else if changes.is_empty() {
        RuntimeScenarioRegressionStatus::Stable
    } else {
        RuntimeScenarioRegressionStatus::RequiresReview
    };
    RuntimeScenarioRenderDiff {
        previous_hash: previous.snapshot_hash.clone(),
        next_hash: next.snapshot_hash.clone(),
        status,
        changes,
    }
}

fn compare_case(
    scenario_id: &str,
    previous: &RuntimeScenarioRenderSnapshotCase,
    next: &RuntimeScenarioRenderSnapshotCase,
    changes: &mut Vec<RuntimeScenarioRenderChange>,
) {
    if previous.rendered != next.rendered {
        changes.push(RuntimeScenarioRenderChange::RenderStateChanged {
            scenario_id: scenario_id.to_string(),
            previous: previous.rendered,
            next: next.rendered,
        });
    }
    if previous.page_id != next.page_id {
        changes.push(RuntimeScenarioRenderChange::PageChanged {
            scenario_id: scenario_id.to_string(),
            previous: previous.page_id.clone(),
            next: next.page_id.clone(),
        });
    }
    if previous.html_hash != next.html_hash {
        changes.push(RuntimeScenarioRenderChange::HtmlChanged {
            scenario_id: scenario_id.to_string(),
        });
    }
    if previous.css_hash != next.css_hash {
        changes.push(RuntimeScenarioRenderChange::CssChanged {
            scenario_id: scenario_id.to_string(),
        });
    }
    if previous.document_hash != next.document_hash {
        changes.push(RuntimeScenarioRenderChange::DocumentChanged {
            scenario_id: scenario_id.to_string(),
        });
    }
    if previous.blocking_diagnostics != next.blocking_diagnostics {
        changes.push(RuntimeScenarioRenderChange::BlockingDiagnosticsChanged {
            scenario_id: scenario_id.to_string(),
            previous: previous.blocking_diagnostics,
            next: next.blocking_diagnostics,
        });
    }
    if previous.error != next.error {
        changes.push(RuntimeScenarioRenderChange::RenderErrorChanged {
            scenario_id: scenario_id.to_string(),
            previous: previous.error.clone(),
            next: next.error.clone(),
        });
    }
}

/// Serialize the fields `snapshot_hash` covers, or `None` when they cannot be encoded.
fn snapshot_payload(
    format: &str,
    selection: &PageSelection,
    policy: &RenderPolicy,
    cases: &[RuntimeScenarioRenderSnapshotCase],
    diagnostics: &[ValidationDiagnostic],
) -> Option<Vec<u8>> {
    serde_json::to_vec(&(format, selection, policy, cases, diagnostics)).ok()
}

/// Collision-resistant digest of the snapshot's contents, rendered `sha256:<64 hex chars>`.
///
/// An empty string means the snapshot could not be encoded. Empty never parses as a
/// [`ContentDigest`], so every verification path fails closed instead of comparing a constant.
fn snapshot_hash(
    format: &str,
    selection: &PageSelection,
    policy: &RenderPolicy,
    cases: &[RuntimeScenarioRenderSnapshotCase],
    diagnostics: &[ValidationDiagnostic],
) -> String {
    match snapshot_payload(format, selection, policy, cases, diagnostics) {
        Some(bytes) => ContentDigest::from_bytes(&bytes).to_string(),
        None => String::new(),
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

    fn scenario(title: &str) -> Vec<RuntimeContextScenario> {
        vec![RuntimeContextScenario::new(
            "default",
            "Default",
            json!({ "page": { "title": title } }),
        )]
    }

    #[test]
    fn captured_snapshot_records_a_sha256_digest() {
        let snapshot = RuntimeScenarioRenderSnapshot::capture(
            &document(),
            &PageSelection::First,
            &RenderPolicy::default(),
            &scenario("Welcome"),
        );
        assert!(snapshot.snapshot_hash.starts_with("sha256:"));
        assert_eq!(snapshot.snapshot_hash.len(), "sha256:".len() + 64);
        assert!(snapshot.has_valid_hash());
        assert!(!snapshot.has_legacy_hash());
    }

    #[test]
    fn legacy_fnv_snapshot_hash_is_recognised_then_upgraded() {
        let mut snapshot = RuntimeScenarioRenderSnapshot::capture(
            &document(),
            &PageSelection::First,
            &RenderPolicy::default(),
            &scenario("Welcome"),
        );
        let payload = snapshot.payload_bytes().expect("snapshot payload");
        // The form Fly persisted until 2026-10-08: a bare FNV-1a 64 fingerprint, no algorithm label.
        snapshot.snapshot_hash = ProjectHash::from_bytes(&payload).hex();
        assert!(snapshot.has_legacy_hash());
        assert!(snapshot.has_valid_hash());

        assert!(snapshot.upgrade_legacy_hash());
        assert!(!snapshot.has_legacy_hash());
        assert!(snapshot.has_valid_hash());
        assert!(snapshot.snapshot_hash.starts_with("sha256:"));
        assert!(!snapshot.upgrade_legacy_hash(), "upgrade is idempotent");
    }

    #[test]
    fn upgrade_leaves_a_hash_that_matches_neither_form_alone() {
        let mut snapshot = RuntimeScenarioRenderSnapshot::capture(
            &document(),
            &PageSelection::First,
            &RenderPolicy::default(),
            &scenario("Welcome"),
        );
        snapshot.snapshot_hash = "0000000000000000".to_string();
        assert!(!snapshot.has_legacy_hash());
        assert!(!snapshot.has_valid_hash());
        assert!(!snapshot.upgrade_legacy_hash());
        assert_eq!(snapshot.snapshot_hash, "0000000000000000");
    }

    #[test]
    fn identical_snapshots_are_stable() {
        let snapshot = RuntimeScenarioRenderSnapshot::capture(
            &document(),
            &PageSelection::First,
            &RenderPolicy::default(),
            &scenario("Welcome"),
        );
        let diff = diff_runtime_scenario_render_snapshots(&snapshot, &snapshot);
        assert_eq!(diff.status, RuntimeScenarioRegressionStatus::Stable);
        assert!(diff.changes.is_empty());
    }

    #[test]
    fn html_drift_requires_review() {
        let previous = RuntimeScenarioRenderSnapshot::capture(
            &document(),
            &PageSelection::First,
            &RenderPolicy::default(),
            &scenario("Welcome"),
        );
        let next = RuntimeScenarioRenderSnapshot::capture(
            &document(),
            &PageSelection::First,
            &RenderPolicy::default(),
            &scenario("Changed"),
        );
        let diff = diff_runtime_scenario_render_snapshots(&previous, &next);
        assert_eq!(diff.status, RuntimeScenarioRegressionStatus::RequiresReview);
        assert!(
            diff.changes
                .iter()
                .any(|change| matches!(change, RuntimeScenarioRenderChange::HtmlChanged { .. }))
        );
    }

    #[test]
    fn removing_a_scenario_is_breaking() {
        let previous = RuntimeScenarioRenderSnapshot::capture(
            &document(),
            &PageSelection::First,
            &RenderPolicy::default(),
            &scenario("Welcome"),
        );
        let next = RuntimeScenarioRenderSnapshot::capture(
            &document(),
            &PageSelection::First,
            &RenderPolicy::default(),
            &[],
        );
        let diff = diff_runtime_scenario_render_snapshots(&previous, &next);
        assert_eq!(diff.status, RuntimeScenarioRegressionStatus::Broken);
    }
}
