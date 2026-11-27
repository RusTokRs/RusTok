use crate::context_schema::{
    computed_evaluation_order, context_path_is_prefix, context_paths_equal, context_paths_overlap,
};
use crate::{
    BindingCatalog, BindingTarget, ContextSchemaCatalog, DynamicCatalog, ProjectDocument,
    ValidationDiagnostic, ValidationSeverity, context_expression_dependencies,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeContextPathSource {
    DeclaredField,
    Computed,
    External,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "consumer", rename_all = "snake_case")]
pub enum RuntimeContextConsumer {
    Computed {
        id: String,
        target_path: String,
    },
    Binding {
        id: String,
        component_id: String,
        target: String,
    },
    Condition {
        id: String,
        component_id: String,
    },
    Repeater {
        id: String,
        component_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeContextDependencyNode {
    pub path: String,
    pub sources: Vec<RuntimeContextPathSource>,
    pub consumers: Vec<RuntimeContextConsumer>,
    pub required: bool,
    pub has_default: bool,
}

impl RuntimeContextDependencyNode {
    pub fn is_external(&self) -> bool {
        self.sources == [RuntimeContextPathSource::External]
    }

    pub fn is_unused(&self) -> bool {
        self.consumers.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeContextDependencyEdge {
    pub from_path: String,
    pub to_computed_path: String,
    pub computed_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RuntimeContextDependencyGraph {
    pub nodes: Vec<RuntimeContextDependencyNode>,
    pub edges: Vec<RuntimeContextDependencyEdge>,
    pub computed_evaluation_order: Vec<String>,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub declared_field_count: usize,
    pub computed_count: usize,
    pub external_path_count: usize,
    pub consumer_count: usize,
}

impl RuntimeContextDependencyGraph {
    pub fn node(&self, path: &str) -> Option<&RuntimeContextDependencyNode> {
        self.nodes.iter().find(|node| node.path == path)
    }

    pub fn blocking_diagnostics(&self) -> impl Iterator<Item = &ValidationDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
    }
}

#[derive(Debug, Clone, Default)]
struct NodeBuilder {
    sources: BTreeSet<RuntimeContextPathSource>,
    consumers: Vec<RuntimeContextConsumer>,
    required: bool,
    has_default: bool,
}

pub fn analyze_runtime_context_dependencies(
    document: &ProjectDocument,
) -> RuntimeContextDependencyGraph {
    let context = ContextSchemaCatalog::from_document(document);
    let bindings = BindingCatalog::from_document(document);
    let dynamic = DynamicCatalog::from_document(document);
    let mut nodes = BTreeMap::<String, NodeBuilder>::new();
    let mut edges = Vec::new();
    let mut diagnostics = Vec::new();

    for field in &context.fields {
        let node = nodes.entry(field.path.clone()).or_default();
        node.sources.insert(RuntimeContextPathSource::DeclaredField);
        node.required |= field.required;
        node.has_default |= field.default.is_some();
    }

    for computed in &context.computed {
        let node = nodes.entry(computed.path.clone()).or_default();
        node.sources.insert(RuntimeContextPathSource::Computed);
        for dependency in context_expression_dependencies(&computed.expression) {
            let dependency_node = nodes.entry(dependency.clone()).or_default();
            dependency_node
                .consumers
                .push(RuntimeContextConsumer::Computed {
                    id: computed.id.clone(),
                    target_path: computed.path.clone(),
                });
            edges.push(RuntimeContextDependencyEdge {
                from_path: dependency,
                to_computed_path: computed.path.clone(),
                computed_id: computed.id.clone(),
            });
        }
    }

    for binding in &bindings.bindings {
        let node = nodes.entry(binding.path.clone()).or_default();
        node.consumers.push(RuntimeContextConsumer::Binding {
            id: binding.id.clone(),
            component_id: binding.component_id.clone(),
            target: binding_target_label(&binding.target),
        });
    }

    for condition in &dynamic.conditions {
        let node = nodes.entry(condition.path.clone()).or_default();
        node.consumers.push(RuntimeContextConsumer::Condition {
            id: condition.id.clone(),
            component_id: condition.component_id.clone(),
        });
    }

    for repeater in &dynamic.repeaters {
        let node = nodes.entry(repeater.path.clone()).or_default();
        node.consumers.push(RuntimeContextConsumer::Repeater {
            id: repeater.id.clone(),
            component_id: repeater.component_id.clone(),
        });
    }

    for field in &context.fields {
        for computed in &context.computed {
            if context_paths_overlap(&field.path, &computed.path) {
                let (code, message) = if context_paths_equal(&field.path, &computed.path) {
                    (
                        "runtime_context_path_shadowed_by_computed",
                        format!(
                            "runtime path `{}` is declared as input and also written by a computed value",
                            field.path
                        ),
                    )
                } else {
                    (
                        "runtime_context_field_computed_target_overlap",
                        format!(
                            "computed value {} target {} overlaps declared input field {} path {}; input and computed paths must not overlap",
                            computed.id, computed.path, field.id, field.path
                        ),
                    )
                };
                diagnostics.push(dependency_diagnostic(
                    ValidationSeverity::Error,
                    code,
                    &computed.path,
                    message,
                ));
            }
        }
    }

    for (path, node) in &mut nodes {
        for field in &context.fields {
            if context_path_is_prefix(&field.path, path) {
                node.sources.insert(RuntimeContextPathSource::DeclaredField);
            }
        }
        for computed in &context.computed {
            if context_path_is_prefix(&computed.path, path) {
                node.sources.insert(RuntimeContextPathSource::Computed);
            }
        }
        if node.sources.is_empty() {
            node.sources.insert(RuntimeContextPathSource::External);
            diagnostics.push(dependency_diagnostic(
                ValidationSeverity::Info,
                "runtime_context_external_reference",
                path,
                format!(
                    "runtime path `{path}` is consumed but is not declared or computed; the host must provide it"
                ),
            ));
        }
        if node.consumers.is_empty() {
            if node
                .sources
                .contains(&RuntimeContextPathSource::DeclaredField)
            {
                diagnostics.push(dependency_diagnostic(
                    ValidationSeverity::Info,
                    "runtime_context_unused_field",
                    path,
                    format!("declared runtime field `{path}` is not consumed by the project"),
                ));
            }
            if node.sources.contains(&RuntimeContextPathSource::Computed) {
                diagnostics.push(dependency_diagnostic(
                    ValidationSeverity::Info,
                    "runtime_computed_unused",
                    path,
                    format!("computed runtime value `{path}` is not consumed by the project"),
                ));
            }
        }
    }

    let (computed_order, computed_diagnostics) = computed_evaluation_order(&context.computed);
    diagnostics.extend(computed_diagnostics);
    let computed_evaluation_order = computed_order
        .into_iter()
        .filter_map(|index| {
            context
                .computed
                .get(index)
                .map(|computed| computed.path.clone())
        })
        .collect::<Vec<_>>();

    let declared_field_count = context.fields.len();
    let computed_count = context.computed.len();
    let external_path_count = nodes
        .values()
        .filter(|node| node.sources == BTreeSet::from([RuntimeContextPathSource::External]))
        .count();
    let consumer_count = nodes.values().map(|node| node.consumers.len()).sum();
    let nodes = nodes
        .into_iter()
        .map(|(path, node)| RuntimeContextDependencyNode {
            path,
            sources: node.sources.into_iter().collect(),
            consumers: node.consumers,
            required: node.required,
            has_default: node.has_default,
        })
        .collect();
    deduplicate_diagnostics(&mut diagnostics);

    RuntimeContextDependencyGraph {
        nodes,
        edges,
        computed_evaluation_order,
        diagnostics,
        declared_field_count,
        computed_count,
        external_path_count,
        consumer_count,
    }
}

fn binding_target_label(target: &BindingTarget) -> String {
    match target {
        BindingTarget::Field { name } => format!("field:{name}"),
        BindingTarget::Attribute { name } => format!("attribute:{name}"),
        BindingTarget::Style { name } => format!("style:{name}"),
    }
}

fn deduplicate_diagnostics(diagnostics: &mut Vec<ValidationDiagnostic>) {
    let mut seen = BTreeSet::new();
    diagnostics.retain(|diagnostic| {
        seen.insert((
            diagnostic.severity as u8,
            diagnostic.code.clone(),
            diagnostic.path.clone(),
            diagnostic.message.clone(),
        ))
    });
}

fn dependency_diagnostic(
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

    #[test]
    fn graph_connects_computed_bindings_conditions_and_repeaters() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [
                        { "id": "title", "type": "text" },
                        { "id": "banner", "type": "section" },
                        { "id": "row", "type": "text" }
                    ]
                }
            }],
            "flyRuntimeContextSchema": [{
                "id": "first",
                "path": "user.first",
                "kind": "string"
            }, {
                "id": "last",
                "path": "user.last",
                "kind": "string"
            }],
            "flyRuntimeComputed": [{
                "id": "full-name",
                "path": "user.fullName",
                "expression": {
                    "op": "concat",
                    "values": [
                        { "op": "path", "path": "user.first" },
                        { "op": "path", "path": "user.last" }
                    ],
                    "separator": " "
                }
            }],
            "flyRuntimeBindings": [{
                "id": "title-binding",
                "component_id": "title",
                "path": "user.fullName",
                "target": "field",
                "name": "content"
            }],
            "flyRuntimeConditions": [{
                "id": "show-banner",
                "component_id": "banner",
                "path": "flags.banner",
                "operator": "truthy"
            }],
            "flyRuntimeRepeaters": [{
                "id": "rows",
                "component_id": "row",
                "path": "items"
            }]
        }))
        .expect("document");
        let graph = analyze_runtime_context_dependencies(&document);
        assert_eq!(graph.declared_field_count, 2);
        assert_eq!(graph.computed_count, 1);
        assert_eq!(graph.external_path_count, 2);
        assert_eq!(
            graph.computed_evaluation_order,
            vec!["user.fullName".to_string()]
        );
        assert_eq!(
            graph.node("user.fullName").map(|node| node.consumers.len()),
            Some(1)
        );
        assert!(
            graph
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_context_external_reference")
        );
    }

    #[test]
    fn graph_uses_structural_overlap_for_field_and_computed_ownership() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{"component": {"id": "root", "type": "wrapper"}}],
            "flyRuntimeContextSchema": [{
                "id": "user-name",
                "path": "$.user.name",
                "kind": "string"
            }],
            "flyRuntimeComputed": [{
                "id": "user",
                "path": "user",
                "expression": {"op": "literal", "value": {}}
            }]
        }))
        .expect("document");
        let graph = analyze_runtime_context_dependencies(&document);
        assert!(graph.blocking_diagnostics().any(|diagnostic| {
            diagnostic.code == "runtime_context_field_computed_target_overlap"
        }));
    }

    #[test]
    fn graph_rejects_input_computed_path_shadowing() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }],
            "flyRuntimeContextSchema": [{
                "id": "title-input",
                "path": "page.title",
                "kind": "string"
            }],
            "flyRuntimeComputed": [{
                "id": "title-computed",
                "path": "page.title",
                "expression": { "op": "literal", "value": "Computed" }
            }]
        }))
        .expect("document");
        let graph = analyze_runtime_context_dependencies(&document);
        assert!(
            graph
                .blocking_diagnostics()
                .any(|diagnostic| diagnostic.code == "runtime_context_path_shadowed_by_computed")
        );
    }

    #[test]
    fn graph_orders_computed_dependencies_using_structural_paths() {
        let document = GrapesJsCodec::decode_value(json!({
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }],
            "flyRuntimeComputed": [{
                "id": "label",
                "path": "label",
                "expression": { "op": "format", "template": "{{user.name}}" }
            }, {
                "id": "user",
                "path": "user",
                "expression": { "op": "literal", "value": {"name": "Ada"} }
            }]
        }))
        .expect("document");
        let graph = analyze_runtime_context_dependencies(&document);
        assert_eq!(graph.computed_evaluation_order, vec!["user", "label"]);
        let user_name = graph.node("user.name").expect("child dependency");
        assert!(
            user_name
                .sources
                .contains(&RuntimeContextPathSource::Computed)
        );
        assert!(!user_name.is_external());

        let reverse = GrapesJsCodec::decode_value(json!({
            "pages": [{ "component": { "id": "root", "type": "wrapper" } }],
            "flyRuntimeComputed": [{
                "id": "label",
                "path": "label",
                "expression": { "op": "path", "path": "user" }
            }, {
                "id": "user-name",
                "path": "user.name",
                "expression": { "op": "literal", "value": "Ada" }
            }]
        }))
        .expect("document");
        let graph = analyze_runtime_context_dependencies(&reverse);
        assert_eq!(graph.computed_evaluation_order, vec!["user.name", "label"]);
        assert!(graph.node("user").expect("parent dependency").is_external());
    }
}
