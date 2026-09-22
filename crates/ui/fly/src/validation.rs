use crate::{
    AssetCatalog, AssetPolicy, PageMetadata, ProjectDocument, RegistrySet, StyleRuleCatalog,
    StyleRuleScope, normalize_slug, validate_runtime_extensions,
};
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
    pub page_count: usize,
    pub asset_count: usize,
    pub style_rule_count: usize,
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

    pub fn warnings(&self) -> impl Iterator<Item = &ValidationDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == ValidationSeverity::Warning)
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
    validate_pages(document, &mut report);
    validate_components(document, registries, limits, &mut report);
    validate_assets(document, &mut report);
    validate_style_rules(document, &mut report);

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
        .diagnostics
        .extend(validate_runtime_extensions(document));
    deduplicate_diagnostics(&mut report.diagnostics);
    report
}

fn validate_pages(document: &ProjectDocument, report: &mut ValidationReport) {
    report.page_count = document.project.pages.len();
    if document.project.pages.is_empty() {
        report.diagnostics.push(ValidationDiagnostic {
            severity: ValidationSeverity::Error,
            code: "missing_pages".to_string(),
            path: "pages".to_string(),
            message: "project must contain at least one page".to_string(),
        });
        return;
    }

    let mut page_ids = BTreeSet::new();
    for (index, page) in document.project.pages.iter().enumerate() {
        let path = format!("pages[{index}]");
        match page.id.as_deref() {
            Some(id) if id.trim().is_empty() => report.diagnostics.push(diagnostic(
                ValidationSeverity::Warning,
                "empty_page_id",
                format!("{path}.id"),
                "page id is empty; stable navigation should use a non-empty id",
            )),
            Some(id) if !page_ids.insert(id.to_string()) => report.diagnostics.push(diagnostic(
                ValidationSeverity::Error,
                "duplicate_page_id",
                format!("{path}.id"),
                format!("page id `{id}` is duplicated"),
            )),
            Some(_) => {}
            None => report.diagnostics.push(diagnostic(
                ValidationSeverity::Warning,
                "missing_page_id",
                format!("{path}.id"),
                "page has no stable id",
            )),
        }

        if page.component.is_none() {
            report.diagnostics.push(diagnostic(
                ValidationSeverity::Error,
                "missing_page_root",
                format!("{path}.component"),
                "page does not contain an editable root component",
            ));
        }

        let metadata = PageMetadata::from_page(page);
        validate_page_metadata(&metadata, &path, report);
    }
}

fn validate_page_metadata(metadata: &PageMetadata, page_path: &str, report: &mut ValidationReport) {
    if metadata
        .title
        .as_deref()
        .is_some_and(|title| title.chars().count() > 70)
    {
        report.diagnostics.push(diagnostic(
            ValidationSeverity::Warning,
            "seo_title_too_long",
            format!("{page_path}.flyPageMeta.title"),
            "SEO title is longer than 70 characters",
        ));
    }
    if metadata
        .description
        .as_deref()
        .is_some_and(|description| description.chars().count() > 180)
    {
        report.diagnostics.push(diagnostic(
            ValidationSeverity::Warning,
            "seo_description_too_long",
            format!("{page_path}.flyPageMeta.description"),
            "SEO description is longer than 180 characters",
        ));
    }
    if let Some(slug) = metadata.slug.as_deref() {
        let normalized = normalize_slug(slug.to_string());
        if normalized != slug {
            report.diagnostics.push(diagnostic(
                ValidationSeverity::Warning,
                "non_normalized_page_slug",
                format!("{page_path}.flyPageMeta.slug"),
                format!("page slug should be normalized as `{normalized}`"),
            ));
        }
    }
    for (field, value) in [
        ("canonicalUrl", metadata.canonical_url.as_deref()),
        ("openGraphImage", metadata.open_graph_image.as_deref()),
    ] {
        if value.is_some_and(|value| !metadata_url_allowed(value)) {
            report.diagnostics.push(diagnostic(
                ValidationSeverity::Warning,
                "invalid_page_metadata_url",
                format!("{page_path}.flyPageMeta.{field}"),
                format!("metadata field `{field}` contains an unsupported URL"),
            ));
        }
    }
}

fn validate_components(
    document: &ProjectDocument,
    registries: &RegistrySet,
    limits: ValidationLimits,
    report: &mut ValidationReport,
) {
    let mut ids = BTreeSet::new();
    document.project.visit_components(|component, depth, path| {
        report.node_count += 1;
        report.maximum_depth = report.maximum_depth.max(depth);

        match component.id() {
            Some(id) if !ids.insert(id.to_string()) => report.diagnostics.push(diagnostic(
                ValidationSeverity::Error,
                "duplicate_component_id",
                path,
                format!("component id `{id}` is duplicated"),
            )),
            Some(_) => {}
            None => report.diagnostics.push(diagnostic(
                ValidationSeverity::Warning,
                "missing_component_id",
                path,
                "component has no stable id; Fly will assign one before mutation",
            )),
        }

        if depth > limits.maximum_depth {
            report.diagnostics.push(diagnostic(
                ValidationSeverity::Error,
                "maximum_depth_exceeded",
                path,
                format!(
                    "component depth {depth} exceeds configured maximum {}",
                    limits.maximum_depth
                ),
            ));
        }

        let component_type = component.component_type();
        if !registries.components.contains(component_type) {
            report.diagnostics.push(diagnostic(
                ValidationSeverity::Warning,
                "missing_component_provider",
                path,
                format!(
                    "component type `{component_type}` has no registered provider; node is preserved"
                ),
            ));
        }
    });
}

fn validate_assets(document: &ProjectDocument, report: &mut ValidationReport) {
    let catalog = AssetCatalog::from_document(document);
    report.asset_count = catalog.assets.len() + catalog.unknown_entries.len();
    for duplicate in &catalog.duplicate_ids {
        report.diagnostics.push(diagnostic(
            ValidationSeverity::Error,
            "duplicate_asset_id",
            "assets",
            format!("asset id `{duplicate}` is duplicated"),
        ));
    }
    if !catalog.unknown_entries.is_empty() {
        report.diagnostics.push(diagnostic(
            ValidationSeverity::Info,
            "opaque_asset_entries",
            "assets",
            format!(
                "{} asset entries are opaque and preserved without normalization",
                catalog.unknown_entries.len()
            ),
        ));
    }
    for message in catalog.validate(&AssetPolicy::default()) {
        report.diagnostics.push(diagnostic(
            ValidationSeverity::Warning,
            "asset_policy_warning",
            "assets",
            message,
        ));
    }
}

fn validate_style_rules(document: &ProjectDocument, report: &mut ValidationReport) {
    let catalog = StyleRuleCatalog::from_document(document);
    report.style_rule_count = catalog.rules.len() + catalog.unknown_entries.len();
    let mut identities = BTreeSet::new();
    for (index, rule) in catalog.rules.iter().enumerate() {
        let path = format!("styles[{index}]");
        if let Some(component_id) = rule.component_id.as_deref() {
            if !document.contains_component(component_id) {
                report.diagnostics.push(diagnostic(
                    ValidationSeverity::Warning,
                    "orphan_component_style_rule",
                    &path,
                    format!(
                        "style rule references missing component `{component_id}` and is preserved"
                    ),
                ));
            }
            let identity = format!("{}|{}", component_id, rule.scope.stable_key());
            if !identities.insert(identity) {
                report.diagnostics.push(diagnostic(
                    ValidationSeverity::Warning,
                    "duplicate_component_style_rule",
                    &path,
                    format!(
                        "multiple style rules target component `{component_id}` in the same scope"
                    ),
                ));
            }
        }
        if matches!(&rule.scope, StyleRuleScope::Media { query } if query.trim().is_empty()) {
            report.diagnostics.push(diagnostic(
                ValidationSeverity::Error,
                "empty_media_query",
                &path,
                "responsive style rule has an empty media query",
            ));
        }
        if rule.declarations.is_empty() {
            report.diagnostics.push(diagnostic(
                ValidationSeverity::Info,
                "empty_style_rule",
                &path,
                "style rule has no declarations",
            ));
        }
    }
    if !catalog.unknown_entries.is_empty() {
        report.diagnostics.push(diagnostic(
            ValidationSeverity::Info,
            "opaque_style_rules",
            "styles",
            format!(
                "{} style rules are opaque and preserved without normalization",
                catalog.unknown_entries.len()
            ),
        ));
    }
}

fn metadata_url_allowed(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    value.is_empty()
        || value.starts_with('/')
        || value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("data:image/")
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

fn diagnostic(
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
    use crate::{GrapesJsCodec, RegistrySet};
    use serde_json::json;

    #[test]
    fn validates_pages_assets_orphan_rules_and_runtime_extensions() {
        let document = GrapesJsCodec::decode_value(json!({
            "assets": [
                { "id": "asset", "src": "/one.png" },
                { "id": "asset", "src": "/two.png" }
            ],
            "styles": [{
                "selectors": [{ "name": "missing", "type": 2 }],
                "style": { "color": "red" },
                "flyComponentId": "missing"
            }],
            "pages": [{
                "id": "home",
                "flyPageMeta": {
                    "slug": "Not Normalized!",
                    "title": "This title is deliberately much longer than seventy characters so validation reports it"
                },
                "component": { "id": "root", "type": "wrapper" }
            }],
            "flyRuntimeContextSchema": [{
                "id": "invalid-root",
                "path": "",
                "kind": "object"
            }]
        }))
        .expect("document");
        let report = validate_project(
            &document,
            &RegistrySet::with_builtins(),
            ValidationLimits::default(),
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "duplicate_asset_id")
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "orphan_component_style_rule")
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "non_normalized_page_slug")
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime_context_field_path_invalid")
        );
        assert_eq!(report.page_count, 1);
        assert_eq!(report.asset_count, 2);
        assert_eq!(report.style_rule_count, 1);
    }

    #[test]
    fn empty_project_is_invalid() {
        let document = GrapesJsCodec::decode_value(json!({ "pages": [] })).expect("document");
        let report = validate_project(
            &document,
            &RegistrySet::with_builtins(),
            ValidationLimits::default(),
        );
        assert!(!report.is_valid());
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "missing_pages")
        );
    }
}
