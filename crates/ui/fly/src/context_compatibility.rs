use crate::{
    ContextValueKind, ProjectDocument, ProjectHash, RuntimeComputedContract,
    RuntimeContextContract, RuntimeContextFieldContract, ValidationDiagnostic, ValidationSeverity,
    extract_runtime_context_contract,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Format marker for serialized compatibility reports.
pub const FLY_RUNTIME_CONTEXT_COMPATIBILITY: &str = "fly_runtime_context_compatibility";

/// One observable difference between a source runtime-context contract and a target one.
///
/// The kinds are deliberately fine-grained instead of a single "changed" value: authoring UIs and
/// release gates need to name *what* changed, and severity is decided per kind by
/// [`RuntimeContextCompatibilityPolicy`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeContextContractChangeKind {
    FieldAdded,
    FieldRemoved,
    FieldKindChanged,
    FieldItemKindChanged,
    FieldBecameRequired,
    FieldBecameOptional,
    FieldDefaultAdded,
    FieldDefaultRemoved,
    ComputedAdded,
    ComputedRemoved,
    ComputedDependenciesChanged,
    ComputedFallbackAdded,
    ComputedFallbackRemoved,
}

impl RuntimeContextContractChangeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FieldAdded => "field_added",
            Self::FieldRemoved => "field_removed",
            Self::FieldKindChanged => "field_kind_changed",
            Self::FieldItemKindChanged => "field_item_kind_changed",
            Self::FieldBecameRequired => "field_became_required",
            Self::FieldBecameOptional => "field_became_optional",
            Self::FieldDefaultAdded => "field_default_added",
            Self::FieldDefaultRemoved => "field_default_removed",
            Self::ComputedAdded => "computed_added",
            Self::ComputedRemoved => "computed_removed",
            Self::ComputedDependenciesChanged => "computed_dependencies_changed",
            Self::ComputedFallbackAdded => "computed_fallback_added",
            Self::ComputedFallbackRemoved => "computed_fallback_removed",
        }
    }

    /// Whether a change declared `breaking` is reported as an error by this kind's policy switch.
    const fn governed_by(self, policy: RuntimeContextCompatibilityPolicy) -> bool {
        match self {
            Self::FieldRemoved => policy.removed_path_is_error,
            Self::FieldAdded | Self::FieldBecameRequired | Self::FieldDefaultRemoved => {
                policy.newly_required_is_error
            }
            Self::FieldKindChanged | Self::FieldItemKindChanged => policy.kind_change_is_error,
            Self::ComputedAdded
            | Self::ComputedRemoved
            | Self::ComputedDependenciesChanged
            | Self::ComputedFallbackRemoved => policy.computed_change_is_error,
            // Relaxations never block a target contract.
            Self::FieldBecameOptional | Self::FieldDefaultAdded | Self::ComputedFallbackAdded => {
                false
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeContextContractChange {
    pub kind: RuntimeContextContractChangeKind,
    pub id: String,
    pub path: String,
    pub detail: String,
    /// Whether a runtime context payload authored against the source contract can still be
    /// materialized under the target contract.
    pub breaking: bool,
}

/// Which breaking changes are tolerated instead of failing the comparison.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeContextCompatibilityPolicy {
    pub removed_path_is_error: bool,
    pub kind_change_is_error: bool,
    pub newly_required_is_error: bool,
    pub computed_change_is_error: bool,
}

impl Default for RuntimeContextCompatibilityPolicy {
    fn default() -> Self {
        Self {
            removed_path_is_error: true,
            kind_change_is_error: true,
            newly_required_is_error: true,
            computed_change_is_error: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeContextCompatibilityReport {
    pub format: String,
    pub source_contract_hash: String,
    pub target_contract_hash: String,
    pub changes: Vec<RuntimeContextContractChange>,
    pub diagnostics: Vec<ValidationDiagnostic>,
    /// No error-severity diagnostic: the target contract accepts what the source contract declared.
    pub compatible: bool,
}

impl RuntimeContextCompatibilityReport {
    pub fn breaking_changes(&self) -> impl Iterator<Item = &RuntimeContextContractChange> {
        self.changes.iter().filter(|change| change.breaking)
    }
}

/// Compare the runtime context contracts of two documents.
///
/// The source is the contract an existing runtime context payload was authored against (for
/// example the published revision); the target is the candidate. The comparison is fail-closed:
/// differences that can make a previously valid payload invalid or change the effective context
/// are reported as errors under the default policy.
pub fn diff_runtime_context_contracts(
    source: &ProjectDocument,
    target: &ProjectDocument,
) -> RuntimeContextCompatibilityReport {
    diff_runtime_context_contracts_with_policy(
        source,
        target,
        RuntimeContextCompatibilityPolicy::default(),
    )
}

pub fn diff_runtime_context_contracts_with_policy(
    source: &ProjectDocument,
    target: &ProjectDocument,
    policy: RuntimeContextCompatibilityPolicy,
) -> RuntimeContextCompatibilityReport {
    let source_contract = extract_runtime_context_contract(source);
    let target_contract = extract_runtime_context_contract(target);
    let mut changes = Vec::new();

    let source_fields = fields_by_path(&source_contract);
    let target_fields = fields_by_path(&target_contract);
    for (path, target_field) in &target_fields {
        let Some(source_field) = source_fields.get(*path) else {
            let breaking = target_field.required && !target_field.has_default;
            changes.push(field_change(
                RuntimeContextContractChangeKind::FieldAdded,
                target_field,
                format!(
                    "field `{path}` is new{}",
                    if breaking {
                        " and required without a default"
                    } else {
                        ""
                    }
                ),
                breaking,
            ));
            continue;
        };
        diff_field(source_field, target_field, &mut changes);
    }
    for (path, source_field) in &source_fields {
        if !target_fields.contains_key(*path) {
            changes.push(field_change(
                RuntimeContextContractChangeKind::FieldRemoved,
                source_field,
                format!("field `{path}` is declared by the source contract only"),
                true,
            ));
        }
    }

    let source_computed = computed_by_path(&source_contract);
    let target_computed = computed_by_path(&target_contract);
    for (path, target_value) in &target_computed {
        let Some(source_value) = source_computed.get(*path) else {
            // A new computed value for a path that used to be caller input silently replaces that
            // input; any other new computed value only adds to the effective context.
            let shadows_input = source_fields.contains_key(*path);
            changes.push(computed_change(
                RuntimeContextContractChangeKind::ComputedAdded,
                target_value,
                format!(
                    "computed value `{path}` is new{}",
                    if shadows_input {
                        " and replaces a declared input path"
                    } else {
                        ""
                    }
                ),
                shadows_input,
            ));
            continue;
        };
        diff_computed(source_value, target_value, &mut changes);
    }
    for (path, source_value) in &source_computed {
        if !target_computed.contains_key(*path) {
            changes.push(computed_change(
                RuntimeContextContractChangeKind::ComputedRemoved,
                source_value,
                format!("computed value `{path}` is declared by the source contract only"),
                true,
            ));
        }
    }

    changes.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.kind.cmp(&right.kind))
    });

    let mut diagnostics = Vec::new();
    diagnostics.extend(target_contract.definition_diagnostics.iter().cloned());
    if source_contract
        .definition_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
    {
        diagnostics.push(compatibility_diagnostic(
            ValidationSeverity::Warning,
            "runtime_context_contract_source_invalid",
            "",
            "the source runtime context contract has definition errors; the comparison describes \
             a document that would not pass its own gates",
        ));
    }
    for change in &changes {
        if !change.breaking {
            continue;
        }
        let blocking = change.kind.governed_by(policy);
        diagnostics.push(compatibility_diagnostic(
            if blocking {
                ValidationSeverity::Error
            } else {
                ValidationSeverity::Warning
            },
            if blocking {
                "runtime_context_contract_breaking_change"
            } else {
                "runtime_context_contract_breaking_change_tolerated"
            },
            &change.path,
            format!("{} ({})", change.detail, change.kind.as_str()),
        ));
    }

    let compatible = !diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == ValidationSeverity::Error);

    RuntimeContextCompatibilityReport {
        format: FLY_RUNTIME_CONTEXT_COMPATIBILITY.to_string(),
        source_contract_hash: contract_fingerprint(&source_contract, &mut diagnostics),
        target_contract_hash: contract_fingerprint(&target_contract, &mut diagnostics),
        changes,
        diagnostics,
        compatible,
    }
}

fn diff_field(
    source: &RuntimeContextFieldContract,
    target: &RuntimeContextFieldContract,
    changes: &mut Vec<RuntimeContextContractChange>,
) {
    let path = &target.path;
    if source.kind != target.kind {
        changes.push(field_change(
            RuntimeContextContractChangeKind::FieldKindChanged,
            target,
            format!(
                "field `{path}` changes kind from `{}` to `{}`",
                source.kind.as_str(),
                target.kind.as_str()
            ),
            true,
        ));
    }
    if source.item_kind != target.item_kind {
        changes.push(field_change(
            RuntimeContextContractChangeKind::FieldItemKindChanged,
            target,
            format!(
                "field `{path}` changes item kind from `{}` to `{}`",
                item_kind_name(source.item_kind),
                item_kind_name(target.item_kind)
            ),
            true,
        ));
    }
    match (source.required, target.required) {
        (false, true) => changes.push(field_change(
            RuntimeContextContractChangeKind::FieldBecameRequired,
            target,
            format!(
                "field `{path}` becomes required{}",
                if target.has_default {
                    " (the target default keeps payloads that omit it valid)"
                } else {
                    " without a default"
                }
            ),
            !target.has_default,
        )),
        (true, false) => changes.push(field_change(
            RuntimeContextContractChangeKind::FieldBecameOptional,
            target,
            format!("field `{path}` is no longer required"),
            false,
        )),
        _ => {}
    }
    match (source.has_default, target.has_default) {
        (false, true) => changes.push(field_change(
            RuntimeContextContractChangeKind::FieldDefaultAdded,
            target,
            format!("field `{path}` gains a default"),
            false,
        )),
        (true, false) => changes.push(field_change(
            RuntimeContextContractChangeKind::FieldDefaultRemoved,
            target,
            format!(
                "field `{path}` loses its default{}",
                if target.required {
                    " while remaining required"
                } else {
                    " while remaining optional"
                }
            ),
            target.required,
        )),
        _ => {}
    }
}

fn diff_computed(
    source: &RuntimeComputedContract,
    target: &RuntimeComputedContract,
    changes: &mut Vec<RuntimeContextContractChange>,
) {
    let path = &target.path;
    let source_dependencies = source
        .dependencies
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let target_dependencies = target
        .dependencies
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if source_dependencies != target_dependencies {
        changes.push(computed_change(
            RuntimeContextContractChangeKind::ComputedDependenciesChanged,
            target,
            format!(
                "computed value `{path}` changes inputs from [{}] to [{}]",
                source_dependencies
                    .iter()
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", "),
                target_dependencies
                    .iter()
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            true,
        ));
    }
    match (source.has_fallback, target.has_fallback) {
        (false, true) => changes.push(computed_change(
            RuntimeContextContractChangeKind::ComputedFallbackAdded,
            target,
            format!("computed value `{path}` gains a fallback"),
            false,
        )),
        (true, false) => changes.push(computed_change(
            RuntimeContextContractChangeKind::ComputedFallbackRemoved,
            target,
            format!("computed value `{path}` loses its fallback"),
            true,
        )),
        _ => {}
    }
}

fn fields_by_path(
    contract: &RuntimeContextContract,
) -> BTreeMap<&str, &RuntimeContextFieldContract> {
    contract
        .fields
        .iter()
        .map(|field| (field.path.as_str(), field))
        .collect()
}

fn computed_by_path(contract: &RuntimeContextContract) -> BTreeMap<&str, &RuntimeComputedContract> {
    contract
        .computed
        .iter()
        .map(|computed| (computed.path.as_str(), computed))
        .collect()
}

fn field_change(
    kind: RuntimeContextContractChangeKind,
    field: &RuntimeContextFieldContract,
    detail: String,
    breaking: bool,
) -> RuntimeContextContractChange {
    RuntimeContextContractChange {
        kind,
        id: field.id.clone(),
        path: field.path.clone(),
        detail,
        breaking,
    }
}

fn computed_change(
    kind: RuntimeContextContractChangeKind,
    computed: &RuntimeComputedContract,
    detail: String,
    breaking: bool,
) -> RuntimeContextContractChange {
    RuntimeContextContractChange {
        kind,
        id: computed.id.clone(),
        path: computed.path.clone(),
        detail,
        breaking,
    }
}

fn item_kind_name(kind: Option<ContextValueKind>) -> &'static str {
    kind.map_or("none", ContextValueKind::as_str)
}

/// Cheap contract identity for change review, mirroring the JSON Schema export: never a constant,
/// because hashing an empty slice would make every unserializable contract share one fingerprint.
fn contract_fingerprint(
    contract: &RuntimeContextContract,
    diagnostics: &mut Vec<ValidationDiagnostic>,
) -> String {
    match serde_json::to_vec(contract) {
        Ok(bytes) => ProjectHash::from_bytes(&bytes).hex(),
        Err(error) => {
            diagnostics.push(compatibility_diagnostic(
                ValidationSeverity::Warning,
                "runtime_context_contract_hash_unavailable",
                "",
                format!("runtime context contract identity could not be computed: {error}"),
            ));
            String::new()
        }
    }
}

fn compatibility_diagnostic(
    severity: ValidationSeverity,
    code: &str,
    path: &str,
    message: impl Into<String>,
) -> ValidationDiagnostic {
    ValidationDiagnostic {
        severity,
        code: code.to_string(),
        path: path.to_string(),
        message: message.into(),
    }
}
