use crate::{
    evaluate_landing_readiness, render_page, ComponentNode, ComponentObject, FlyError, FlyResult,
    LandingReadinessPolicy, LandingReadinessReport, PageHead, PageSelection, ProjectDocument,
    ProjectHash, RegistrySet, RenderPolicy,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const FLY_LANDING_SECTION_FIELD: &str = "flyLandingSection";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum LandingSectionKind {
    Hero,
    TwoColumns,
    FeatureGrid,
    CallToAction,
    ContactForm,
}

impl LandingSectionKind {
    pub const ALL: [Self; 5] = [
        Self::Hero,
        Self::TwoColumns,
        Self::FeatureGrid,
        Self::CallToAction,
        Self::ContactForm,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hero => "hero",
            Self::TwoColumns => "two_columns",
            Self::FeatureGrid => "feature_grid",
            Self::CallToAction => "call_to_action",
            Self::ContactForm => "contact_form",
        }
    }

    pub const fn block_id(self) -> &'static str {
        match self {
            Self::Hero => "fly.hero",
            Self::TwoColumns => "fly.two_columns",
            Self::FeatureGrid => "fly.feature_grid",
            Self::CallToAction => "fly.cta",
            Self::ContactForm => "fly.contact_form",
        }
    }

    pub const fn required_component_types(self) -> &'static [&'static str] {
        match self {
            Self::Hero => &["heading", "text", "button"],
            Self::TwoColumns => &["row", "column", "heading", "text", "button", "image"],
            Self::FeatureGrid => &["heading", "grid", "column", "text"],
            Self::CallToAction => &["heading", "text", "button"],
            Self::ContactForm => &["heading", "text", "form", "input", "textarea", "submit"],
        }
    }

    pub fn from_marker(marker: &str) -> Option<Self> {
        match marker.trim() {
            "hero" => Some(Self::Hero),
            "two_columns" => Some(Self::TwoColumns),
            "feature_grid" => Some(Self::FeatureGrid),
            "call_to_action" => Some(Self::CallToAction),
            "contact_form" => Some(Self::ContactForm),
            _ => None,
        }
    }
}

impl std::fmt::Display for LandingSectionKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LandingSectionIssueKind {
    InvalidMarker,
    WrongRootType,
    MissingRequiredComponent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LandingSectionIssue {
    pub page_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_id: Option<String>,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_id: Option<String>,
    pub kind: LandingSectionIssueKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_component_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LandingSectionSnapshot {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_id: Option<String>,
    pub kind: LandingSectionKind,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LandingSectionPageManifest {
    pub page_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_id: Option<String>,
    #[serde(default)]
    pub sections: Vec<LandingSectionSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LandingSectionValidationReport {
    pub valid: bool,
    #[serde(default)]
    pub pages: Vec<LandingSectionPageManifest>,
    #[serde(default)]
    pub issues: Vec<LandingSectionIssue>,
}

impl LandingSectionValidationReport {
    pub fn for_document(document: &ProjectDocument) -> FlyResult<Self> {
        let mut pages = Vec::with_capacity(document.project.pages.len());
        let mut issues = Vec::new();

        for (page_index, page) in document.project.pages.iter().enumerate() {
            let mut sections = Vec::new();
            if let Some(root) = page.component.as_ref() {
                collect_landing_sections(
                    root,
                    page_index,
                    page.id.as_deref(),
                    &format!("project.pages[{page_index}].component"),
                    &mut sections,
                    &mut issues,
                )?;
            }
            pages.push(LandingSectionPageManifest {
                page_index,
                page_id: page.id.clone(),
                sections,
            });
        }

        Ok(Self {
            valid: issues.is_empty(),
            pages,
            issues,
        })
    }
}

fn collect_landing_sections(
    node: &ComponentNode,
    page_index: usize,
    page_id: Option<&str>,
    path: &str,
    sections: &mut Vec<LandingSectionSnapshot>,
    issues: &mut Vec<LandingSectionIssue>,
) -> FlyResult<()> {
    let Some(component) = node.as_object() else {
        return Ok(());
    };

    if let Some(marker) = component.extensions.get(FLY_LANDING_SECTION_FIELD) {
        validate_landing_section(
            component, marker, page_index, page_id, path, sections, issues,
        )?;
    }

    for (index, child) in component.children().iter().enumerate() {
        collect_landing_sections(
            child,
            page_index,
            page_id,
            &format!("{path}.components[{index}]"),
            sections,
            issues,
        )?;
    }
    Ok(())
}

fn validate_landing_section(
    component: &ComponentObject,
    marker: &Value,
    page_index: usize,
    page_id: Option<&str>,
    path: &str,
    sections: &mut Vec<LandingSectionSnapshot>,
    issues: &mut Vec<LandingSectionIssue>,
) -> FlyResult<()> {
    let component_id = component.id.clone();
    let Some(marker) = marker.as_str() else {
        issues.push(LandingSectionIssue {
            page_index,
            page_id: page_id.map(ToString::to_string),
            path: path.to_string(),
            component_id,
            kind: LandingSectionIssueKind::InvalidMarker,
            marker: Some(marker.to_string()),
            required_component_type: None,
        });
        return Ok(());
    };
    let Some(kind) = LandingSectionKind::from_marker(marker) else {
        issues.push(LandingSectionIssue {
            page_index,
            page_id: page_id.map(ToString::to_string),
            path: path.to_string(),
            component_id,
            kind: LandingSectionIssueKind::InvalidMarker,
            marker: Some(marker.to_string()),
            required_component_type: None,
        });
        return Ok(());
    };

    if component.component_type() != "section" {
        issues.push(LandingSectionIssue {
            page_index,
            page_id: page_id.map(ToString::to_string),
            path: path.to_string(),
            component_id: component.id.clone(),
            kind: LandingSectionIssueKind::WrongRootType,
            marker: Some(kind.as_str().to_string()),
            required_component_type: Some("section".to_string()),
        });
    }

    let mut component_types = BTreeMap::<String, usize>::new();
    collect_component_type_counts(component, &mut component_types);
    for required in kind.required_component_types() {
        if component_types.get(*required).copied().unwrap_or_default() == 0 {
            issues.push(LandingSectionIssue {
                page_index,
                page_id: page_id.map(ToString::to_string),
                path: path.to_string(),
                component_id: component.id.clone(),
                kind: LandingSectionIssueKind::MissingRequiredComponent,
                marker: Some(kind.as_str().to_string()),
                required_component_type: Some((*required).to_string()),
            });
        }
    }

    let bytes =
        serde_json::to_vec(component).map_err(|error| FlyError::Encode(error.to_string()))?;
    sections.push(LandingSectionSnapshot {
        path: path.to_string(),
        component_id: component.id.clone(),
        kind,
        content_hash: ProjectHash::from_bytes(&bytes).hex(),
    });
    Ok(())
}

fn collect_component_type_counts(
    component: &ComponentObject,
    counts: &mut BTreeMap<String, usize>,
) {
    *counts
        .entry(component.component_type().to_string())
        .or_default() += 1;
    for child in component.children() {
        if let Some(child) = child.as_object() {
            collect_component_type_counts(child, counts);
        }
    }
}

/// Components required by a page document.
///
/// Compatibility is owned by the providing module. The landing payload deliberately carries no
/// independent schema version: module semver is the only version boundary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComponentRegistryContract {
    pub component_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComponentRegistryManifest {
    #[serde(default)]
    pub components: Vec<ComponentRegistryContract>,
}

impl ComponentRegistryManifest {
    pub fn for_document(document: &ProjectDocument, registries: &RegistrySet) -> Self {
        let mut components = BTreeMap::<String, ComponentRegistryContract>::new();
        document.project.visit_components(|component, _, _| {
            let component_type = component.component_type().to_string();
            let contract = registries
                .components
                .get(&component_type)
                .map(|definition| ComponentRegistryContract {
                    component_type: component_type.clone(),
                    provider: Some(definition.provider.clone()),
                })
                .unwrap_or_else(|| ComponentRegistryContract {
                    component_type: component_type.clone(),
                    provider: component.provider.clone(),
                });
            components.entry(component_type).or_insert(contract);
        });
        Self {
            components: components.into_values().collect(),
        }
    }

    pub fn compatibility_with(&self, registries: &RegistrySet) -> RegistryCompatibilityReport {
        let mut issues = Vec::new();
        for required in &self.components {
            let Some(available) = registries.components.get(&required.component_type) else {
                issues.push(RegistryCompatibilityIssue {
                    component_type: required.component_type.clone(),
                    kind: RegistryCompatibilityIssueKind::MissingComponent,
                    expected: required.provider.clone(),
                    actual: None,
                });
                continue;
            };
            if required
                .provider
                .as_deref()
                .is_some_and(|provider| provider != available.provider.as_str())
            {
                issues.push(RegistryCompatibilityIssue {
                    component_type: required.component_type.clone(),
                    kind: RegistryCompatibilityIssueKind::ProviderMismatch,
                    expected: required.provider.clone(),
                    actual: Some(available.provider.clone()),
                });
            }
        }
        RegistryCompatibilityReport {
            compatible: issues.is_empty(),
            issues,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RegistryCompatibilityIssueKind {
    MissingComponent,
    ProviderMismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryCompatibilityIssue {
    pub component_type: String,
    pub kind: RegistryCompatibilityIssueKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryCompatibilityReport {
    pub compatible: bool,
    #[serde(default)]
    pub issues: Vec<RegistryCompatibilityIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StaticLandingPage {
    pub page_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    pub head: PageHead,
    pub html: String,
    pub content_hash: String,
    #[serde(default)]
    pub landing_sections: Vec<LandingSectionSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StaticLandingArtifact {
    pub source_hash: String,
    pub artifact_hash: String,
    pub registry: ComponentRegistryManifest,
    pub pages: Vec<StaticLandingPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StaticLandingBuildResult {
    pub ready: bool,
    pub readiness: LandingReadinessReport,
    pub registry_compatibility: RegistryCompatibilityReport,
    pub landing_sections: LandingSectionValidationReport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<StaticLandingArtifact>,
}

pub fn build_static_landing_artifact(
    document: &ProjectDocument,
    registries: &RegistrySet,
    readiness_policy: LandingReadinessPolicy,
    render_policy: &RenderPolicy,
) -> FlyResult<StaticLandingBuildResult> {
    let registry = ComponentRegistryManifest::for_document(document, registries);
    let registry_compatibility = registry.compatibility_with(registries);
    let landing_sections = LandingSectionValidationReport::for_document(document)?;
    let readiness = evaluate_landing_readiness(document, readiness_policy);
    let ready = readiness.ready && registry_compatibility.compatible && landing_sections.valid;
    if !ready {
        return Ok(StaticLandingBuildResult {
            ready,
            readiness,
            registry_compatibility,
            landing_sections,
            artifact: None,
        });
    }

    let mut pages = Vec::with_capacity(document.project.pages.len());
    for page_index in 0..document.project.pages.len() {
        let rendered = render_page(document, &PageSelection::Index(page_index), render_policy)?;
        let html = rendered.document_html();
        pages.push(StaticLandingPage {
            page_index,
            page_id: rendered.page_id,
            slug: rendered.metadata.slug,
            head: rendered.head,
            content_hash: ProjectHash::from_bytes(html.as_bytes()).hex(),
            html,
            landing_sections: landing_sections.pages[page_index].sections.clone(),
        });
    }

    let source_hash = document.hash().hex();
    let artifact_bytes = serde_json::to_vec(&(&source_hash, &registry, &pages))
        .map_err(|error| FlyError::Encode(error.to_string()))?;
    let artifact = StaticLandingArtifact {
        source_hash,
        artifact_hash: ProjectHash::from_bytes(&artifact_bytes).hex(),
        registry,
        pages,
    };

    Ok(StaticLandingBuildResult {
        ready,
        readiness,
        registry_compatibility,
        landing_sections,
        artifact: Some(artifact),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsCodec;
    use serde_json::json;

    fn ready_project() -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": {
                    "title": "Home",
                    "description": "A stable landing page",
                    "slug": "home"
                },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "hero-title",
                        "type": "heading",
                        "tagName": "h1",
                        "content": "Stable landing"
                    }]
                }
            }]
        }))
        .expect("ready project")
    }

    fn project_with_section(section: Value) -> ProjectDocument {
        GrapesJsCodec::decode_value(json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": {
                    "title": "Home",
                    "description": "A stable landing page",
                    "slug": "home"
                },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [section]
                }
            }]
        }))
        .expect("landing project")
    }

    fn builtin_section(kind: LandingSectionKind) -> Value {
        let registries = RegistrySet::with_builtins();
        let block = registries
            .blocks
            .get(kind.block_id())
            .expect("built-in landing block");
        let mut value = serde_json::to_value(&block.component).expect("component JSON");
        value
            .as_object_mut()
            .expect("landing root object")
            .insert("id".to_string(), Value::String(kind.as_str().to_string()));
        value
    }

    #[test]
    fn registry_manifest_contains_only_used_components_in_stable_order() {
        let manifest = ComponentRegistryManifest::for_document(
            &ready_project(),
            &RegistrySet::with_builtins(),
        );
        assert_eq!(
            manifest
                .components
                .iter()
                .map(|component| component.component_type.as_str())
                .collect::<Vec<_>>(),
            vec!["heading", "wrapper"]
        );
        assert!(manifest
            .components
            .iter()
            .all(|component| component.provider.is_some()));
    }

    #[test]
    fn static_artifact_is_deterministic_and_contains_complete_html() {
        let project = ready_project();
        let registries = RegistrySet::with_builtins();
        let first = build_static_landing_artifact(
            &project,
            &registries,
            LandingReadinessPolicy::default(),
            &RenderPolicy::default(),
        )
        .expect("first build");
        let second = build_static_landing_artifact(
            &project,
            &registries,
            LandingReadinessPolicy::default(),
            &RenderPolicy::default(),
        )
        .expect("second build");
        assert!(first.ready);
        assert!(first.landing_sections.valid);
        let first = first.artifact.expect("artifact");
        let second = second.artifact.expect("artifact");
        assert_eq!(first.artifact_hash, second.artifact_hash);
        assert_eq!(first.pages[0].content_hash, second.pages[0].content_hash);
        assert!(first.pages[0].landing_sections.is_empty());
        assert!(first.pages[0].html.starts_with("<!doctype html>"));
        assert!(first.pages[0].html.contains("<title>Home</title>"));
    }

    #[test]
    fn typed_landing_section_is_validated_and_snapshotted() {
        let project = project_with_section(builtin_section(LandingSectionKind::Hero));
        let result = build_static_landing_artifact(
            &project,
            &RegistrySet::with_builtins(),
            LandingReadinessPolicy::default(),
            &RenderPolicy::default(),
        )
        .expect("landing build");
        assert!(result.ready);
        assert!(result.landing_sections.valid);
        assert!(result.landing_sections.issues.is_empty());
        let artifact = result.artifact.expect("artifact");
        assert_eq!(artifact.pages[0].landing_sections.len(), 1);
        assert_eq!(
            artifact.pages[0].landing_sections[0].kind,
            LandingSectionKind::Hero
        );
        assert!(!artifact.pages[0].landing_sections[0]
            .content_hash
            .is_empty());
    }

    #[test]
    fn malformed_typed_section_blocks_static_artifact() {
        let project = project_with_section(json!({
            "id": "broken-hero",
            "type": "section",
            "flyLandingSection": "hero",
            "components": [{ "type": "text", "content": "Only copy" }]
        }));
        let result = build_static_landing_artifact(
            &project,
            &RegistrySet::with_builtins(),
            LandingReadinessPolicy::default(),
            &RenderPolicy::default(),
        )
        .expect("landing build");
        assert!(!result.ready);
        assert!(!result.landing_sections.valid);
        assert!(result.artifact.is_none());
        assert!(result.landing_sections.issues.iter().any(|issue| {
            issue.kind == LandingSectionIssueKind::MissingRequiredComponent
                && issue.required_component_type.as_deref() == Some("heading")
        }));
        assert!(result.landing_sections.issues.iter().any(|issue| {
            issue.kind == LandingSectionIssueKind::MissingRequiredComponent
                && issue.required_component_type.as_deref() == Some("button")
        }));
    }

    #[test]
    fn unknown_section_marker_is_rejected() {
        let project = project_with_section(json!({
            "id": "unknown-section",
            "type": "section",
            "flyLandingSection": "unknown",
            "components": []
        }));
        let report = LandingSectionValidationReport::for_document(&project).expect("report");
        assert!(!report.valid);
        assert_eq!(report.issues.len(), 1);
        assert_eq!(
            report.issues[0].kind,
            LandingSectionIssueKind::InvalidMarker
        );
    }
}
