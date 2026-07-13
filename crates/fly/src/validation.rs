use crate::{ProjectDocument, RegistrySet};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationDiagnostic {
    pub severity: ValidationSeverity,
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ValidationReport {
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub node_count: usize,
    pub maximum_depth: usize,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
    }

    pub fn errors(&self) -> impl Iterator<Item = &ValidationDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationLimits {
    pub maximum_nodes: usize,
    pub maximum_depth: usize,
}

impl Default for ValidationLimits {
    fn default() -> Self {
        Self {
            maximum_nodes: 10_000,
            maximum_depth: 64,
        }
    }
}

pub fn validate_project(
    document: &ProjectDocument,
    registries: &RegistrySet,
    limits: ValidationLimits,
) -> ValidationReport {
    let mut report = ValidationReport::default();
    let mut ids = BTreeSet::new();

    document.project.visit_components(|component, depth, path| {
        report.node_count += 1;
        report.maximum_depth = report.maximum_depth.max(depth);

        match component.id() {
            Some(id) if !ids.insert(id.to_string()) => report.diagnostics.push(
                ValidationDiagnostic {
                    severity: ValidationSeverity::Error,
                    code: "duplicate_component_id".to_string(),
                    path: path.to_string(),
                    message: format!("component id `{id}` is duplicated"),
                },
            ),
            Some(_) => {}
            None => report.diagnostics.push(ValidationDiagnostic {
                severity: ValidationSeverity::Warning,
                code: "missing_component_id".to_string(),
                path: path.to_string(),
                message: "component has no stable id; Fly will assign one before mutation"
                    .to_string(),
            }),
        }

        if depth > limits.maximum_depth {
            report.diagnostics.push(ValidationDiagnostic {
                severity: ValidationSeverity::Error,
                code: "maximum_depth_exceeded".to_string(),
                path: path.to_string(),
                message: format!(
                    "component depth {depth} exceeds configured maximum {}",
                    limits.maximum_depth
                ),
            });
        }

        let component_type = component.component_type();
        if !registries.components.contains(component_type) {
            report.diagnostics.push(ValidationDiagnostic {
                severity: ValidationSeverity::Warning,
                code: "missing_component_provider".to_string(),
                path: path.to_string(),
                message: format!(
                    "component type `{component_type}` has no registered provider; node is preserved"
                ),
            });
        }

        if component.provider.is_some() && component.schema_version.is_none() {
            report.diagnostics.push(ValidationDiagnostic {
                severity: ValidationSeverity::Warning,
                code: "missing_provider_schema_version".to_string(),
                path: path.to_string(),
                message: "provider-owned component should carry schemaVersion".to_string(),
            });
        }
    });

    if report.node_count > limits.maximum_nodes {
        report.diagnostics.push(ValidationDiagnostic {
            severity: ValidationSeverity::Error,
            code: "maximum_nodes_exceeded".to_string(),
            path: "project".to_string(),
            message: format!(
                "project contains {} components, exceeding configured maximum {}",
                report.node_count, limits.maximum_nodes
            ),
        });
    }

    report
}
