use crate::{
    PageSelection, ProjectDocument, ProjectHash, RenderPolicy, RuntimeContextScenario,
    RuntimeRenderResult, ValidationDiagnostic, ValidationSeverity,
    render_page_with_runtime_context,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeScenarioRenderCase {
    pub scenario_id: String,
    pub scenario_label: String,
    pub rendered: bool,
    pub page_id: Option<String>,
    pub html_hash: Option<String>,
    pub css_hash: Option<String>,
    pub document_hash: Option<String>,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub defaults_applied: usize,
    pub computed_applied: usize,
    pub computed_fallbacks: usize,
    pub unresolved_computed: usize,
    pub context_type_mismatches: usize,
    #[serde(default)]
    pub materialized_forms: usize,
    #[serde(default)]
    pub native_actions: usize,
    #[serde(default)]
    pub custom_actions: usize,
    #[serde(default)]
    pub fallback_actions: usize,
    #[serde(default)]
    pub unresolved_actions: usize,
    pub applied_bindings: usize,
    pub fallback_bindings: usize,
    pub unresolved_bindings: usize,
    pub evaluated_conditions: usize,
    pub hidden_components: usize,
    pub repeated_nodes: usize,
    pub error: Option<String>,
}

impl RuntimeScenarioRenderCase {
    pub fn has_blocking_diagnostics(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RuntimeScenarioRenderMatrix {
    pub cases: Vec<RuntimeScenarioRenderCase>,
    pub rendered_count: usize,
    pub failed_count: usize,
    pub unique_html_outputs: usize,
    pub duplicate_html_groups: Vec<Vec<String>>,
    pub diagnostics: Vec<ValidationDiagnostic>,
}

impl RuntimeScenarioRenderMatrix {
    pub fn is_renderable(&self) -> bool {
        self.failed_count == 0
            && !self
                .cases
                .iter()
                .any(RuntimeScenarioRenderCase::has_blocking_diagnostics)
    }

    pub fn case(&self, scenario_id: &str) -> Option<&RuntimeScenarioRenderCase> {
        self.cases
            .iter()
            .find(|case| case.scenario_id == scenario_id)
    }
}

pub fn render_runtime_scenario_matrix(
    document: &ProjectDocument,
    selection: &PageSelection,
    policy: &RenderPolicy,
    scenarios: &[RuntimeContextScenario],
) -> RuntimeScenarioRenderMatrix {
    let mut matrix = RuntimeScenarioRenderMatrix::default();
    let mut ids = BTreeSet::new();

    for scenario in scenarios {
        if !ids.insert(scenario.id.clone()) {
            matrix.diagnostics.push(matrix_diagnostic(
                ValidationSeverity::Error,
                "duplicate_runtime_scenario_id",
                format!("runtime_scenario:{}", scenario.id),
                format!("runtime scenario id `{}` is duplicated", scenario.id),
            ));
        }
        matrix
            .cases
            .push(render_case(document, selection, policy, scenario));
    }

    matrix.rendered_count = matrix.cases.iter().filter(|case| case.rendered).count();
    matrix.failed_count = matrix.cases.len().saturating_sub(matrix.rendered_count);
    let mut html_groups = BTreeMap::<String, Vec<String>>::new();
    for case in &matrix.cases {
        if let Some(hash) = case.html_hash.as_ref() {
            html_groups
                .entry(hash.clone())
                .or_default()
                .push(case.scenario_id.clone());
        }
    }
    matrix.unique_html_outputs = html_groups.len();
    matrix.duplicate_html_groups = html_groups
        .into_values()
        .filter(|scenario_ids| scenario_ids.len() > 1)
        .collect();
    matrix.duplicate_html_groups.sort();
    matrix
}

fn render_case(
    document: &ProjectDocument,
    selection: &PageSelection,
    policy: &RenderPolicy,
    scenario: &RuntimeContextScenario,
) -> RuntimeScenarioRenderCase {
    match render_page_with_runtime_context(document, selection, policy, &scenario.context) {
        Ok(result) => successful_case(scenario, result),
        Err(error) => RuntimeScenarioRenderCase {
            scenario_id: scenario.id.clone(),
            scenario_label: scenario.label.clone(),
            rendered: false,
            page_id: None,
            html_hash: None,
            css_hash: None,
            document_hash: None,
            diagnostics: Vec::new(),
            defaults_applied: 0,
            computed_applied: 0,
            computed_fallbacks: 0,
            unresolved_computed: 0,
            context_type_mismatches: 0,
            materialized_forms: 0,
            native_actions: 0,
            custom_actions: 0,
            fallback_actions: 0,
            unresolved_actions: 0,
            applied_bindings: 0,
            fallback_bindings: 0,
            unresolved_bindings: 0,
            evaluated_conditions: 0,
            hidden_components: 0,
            repeated_nodes: 0,
            error: Some(error.to_string()),
        },
    }
}

fn successful_case(
    scenario: &RuntimeContextScenario,
    result: RuntimeRenderResult,
) -> RuntimeScenarioRenderCase {
    let document_html = result.document_html();
    RuntimeScenarioRenderCase {
        scenario_id: scenario.id.clone(),
        scenario_label: scenario.label.clone(),
        rendered: true,
        page_id: result.page.page_id.clone(),
        html_hash: Some(ProjectHash::from_bytes(result.page.html.as_bytes()).hex()),
        css_hash: Some(ProjectHash::from_bytes(result.page.css.as_bytes()).hex()),
        document_hash: Some(ProjectHash::from_bytes(document_html.as_bytes()).hex()),
        diagnostics: result.diagnostics,
        defaults_applied: result.defaults_applied,
        computed_applied: result.computed_applied,
        computed_fallbacks: result.computed_fallbacks,
        unresolved_computed: result.unresolved_computed,
        context_type_mismatches: result.context_type_mismatches,
        materialized_forms: result.materialized_forms,
        native_actions: result.native_actions,
        custom_actions: result.custom_actions,
        fallback_actions: result.fallback_actions,
        unresolved_actions: result.unresolved_actions,
        applied_bindings: result.applied_bindings,
        fallback_bindings: result.fallback_bindings,
        unresolved_bindings: result.unresolved_bindings,
        evaluated_conditions: result.evaluated_conditions,
        hidden_components: result.hidden_components,
        repeated_nodes: result.repeated_nodes,
        error: None,
    }
}

fn matrix_diagnostic(
    severity: ValidationSeverity,
    code: impl Into<String>,
    path: impl Into<String>,
    message: impl Into<String>,
) -> ValidationDiagnostic {
    ValidationDiagnostic {
        severity,
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
                    "components": [{
                        "id": "title",
                        "type": "heading",
                        "content": "Static"
                    }]
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

    #[test]
    fn matrix_renders_distinct_scenario_outputs() {
        let scenarios = vec![
            RuntimeContextScenario::new("one", "One", json!({ "page": { "title": "First" } })),
            RuntimeContextScenario::new("two", "Two", json!({ "page": { "title": "Second" } })),
        ];
        let matrix = render_runtime_scenario_matrix(
            &document(),
            &PageSelection::Id("home".to_string()),
            &RenderPolicy::default(),
            &scenarios,
        );
        assert!(matrix.is_renderable());
        assert_eq!(matrix.rendered_count, 2);
        assert_eq!(matrix.unique_html_outputs, 2);
        assert_ne!(
            matrix.case("one").and_then(|case| case.html_hash.as_ref()),
            matrix.case("two").and_then(|case| case.html_hash.as_ref())
        );
    }

    #[test]
    fn matrix_carries_action_and_form_materialization_counters() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "form",
                        "type": "wrapper",
                        "flyForm": { "id": "contact", "action_url": "/contact" }
                    }, {
                        "id": "submit",
                        "type": "button",
                        "flyAction": { "kind": "submit_form", "form_id": "contact" }
                    }]
                }
            }]
        }))
        .expect("document");
        let matrix = render_runtime_scenario_matrix(
            &document,
            &PageSelection::Id("home".to_string()),
            &RenderPolicy::default(),
            &[RuntimeContextScenario::new("default", "Default", json!({}))],
        );
        let case = matrix.case("default").expect("scenario case");
        assert_eq!(case.materialized_forms, 1);
        assert_eq!(case.native_actions, 1);
        assert_eq!(case.unresolved_actions, 0);
    }

    #[test]
    fn matrix_groups_duplicate_outputs() {
        let scenarios = vec![
            RuntimeContextScenario::new("one", "One", json!({ "page": { "title": "Same" } })),
            RuntimeContextScenario::new("two", "Two", json!({ "page": { "title": "Same" } })),
        ];
        let matrix = render_runtime_scenario_matrix(
            &document(),
            &PageSelection::First,
            &RenderPolicy::default(),
            &scenarios,
        );
        assert_eq!(matrix.unique_html_outputs, 1);
        assert_eq!(matrix.duplicate_html_groups, vec![vec!["one", "two"]]);
    }

    #[test]
    fn matrix_captures_render_errors_per_case() {
        let matrix = render_runtime_scenario_matrix(
            &document(),
            &PageSelection::Id("missing".to_string()),
            &RenderPolicy::default(),
            &[RuntimeContextScenario::new("one", "One", json!({}))],
        );
        assert_eq!(matrix.failed_count, 1);
        assert!(
            matrix
                .case("one")
                .and_then(|case| case.error.as_ref())
                .is_some()
        );
    }
}
