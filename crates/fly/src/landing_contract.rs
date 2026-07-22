use crate::{
    ComponentNode, ComponentObject, FlyError, FlyResult, LandingPropertyValidationReport,
    LandingReadinessPolicy, LandingReadinessReport, PageHead, PageSelection, ProjectDocument,
    RegistrySet, RenderPolicy, evaluate_landing_readiness, render_page,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
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
        content_hash: sha256_hex(&bytes),
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LandingRendererManifest {
    pub id: String,
    pub release: String,
}

impl LandingRendererManifest {
    pub fn new(id: impl Into<String>, release: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            release: release.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandingRenderedPage {
    pub page_index: usize,
    pub page_id: Option<String>,
    pub slug: Option<String>,
    pub head: PageHead,
    pub document_html: String,
    pub body_html: String,
    pub css: String,
}

/// Framework-neutral rendering boundary for static landing output.
///
/// Fly ships the canonical HTML renderer. Leptos and Dioxus adapters can implement this trait
/// without changing the document model, readiness checks or artifact persistence contract.
pub trait LandingRenderer: Send + Sync {
    fn manifest(&self) -> LandingRendererManifest;

    fn render_page(
        &self,
        document: &ProjectDocument,
        page_index: usize,
        policy: &RenderPolicy,
    ) -> FlyResult<LandingRenderedPage>;
}

pub const FLY_HTML_RENDERER_ID: &str = "fly_html";

#[derive(Debug, Clone, Copy, Default)]
pub struct FlyHtmlLandingRenderer;

impl LandingRenderer for FlyHtmlLandingRenderer {
    fn manifest(&self) -> LandingRendererManifest {
        LandingRendererManifest::new(FLY_HTML_RENDERER_ID, env!("CARGO_PKG_VERSION"))
    }

    fn render_page(
        &self,
        document: &ProjectDocument,
        page_index: usize,
        policy: &RenderPolicy,
    ) -> FlyResult<LandingRenderedPage> {
        let rendered = render_page(document, &PageSelection::Index(page_index), policy)?;
        let document_html = rendered.document_html();
        Ok(LandingRenderedPage {
            page_index,
            page_id: rendered.page_id,
            slug: rendered.metadata.slug,
            head: rendered.head,
            document_html,
            body_html: rendered.html,
            css: rendered.css,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StaticLandingBuildIdentity {
    pub source_hash: String,
    pub renderer: LandingRendererManifest,
    pub registry_hash: String,
    pub render_policy_hash: String,
    pub build_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StaticLandingPage {
    pub page_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    pub head: PageHead,
    pub document_html: String,
    pub body_html: String,
    pub css: String,
    pub content_hash: String,
    #[serde(default)]
    pub landing_sections: Vec<LandingSectionSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StaticLandingArtifact {
    pub identity: StaticLandingBuildIdentity,
    pub artifact_hash: String,
    pub registry: ComponentRegistryManifest,
    pub pages: Vec<StaticLandingPage>,
}

impl StaticLandingArtifact {
    pub fn verify_integrity(&self) -> FlyResult<()> {
        validate_renderer_manifest(&self.identity.renderer)?;
        if self.pages.is_empty() {
            return Err(FlyError::Encode(
                "static landing artifact must contain at least one page".to_string(),
            ));
        }
        let registry_hash = stable_hash(&self.registry)?;
        if registry_hash != self.identity.registry_hash {
            return Err(FlyError::Encode(
                "static landing registry hash does not match the artifact identity".to_string(),
            ));
        }
        let build_hash = stable_hash(&(
            &self.identity.source_hash,
            &self.identity.renderer,
            &self.identity.registry_hash,
            &self.identity.render_policy_hash,
        ))?;
        if build_hash != self.identity.build_hash {
            return Err(FlyError::Encode(
                "static landing build hash does not match the artifact identity".to_string(),
            ));
        }
        for page in &self.pages {
            let content_hash = sha256_hex(page.document_html.as_bytes());
            if content_hash != page.content_hash {
                return Err(FlyError::Encode(format!(
                    "static landing page {} content hash mismatch",
                    page.page_index
                )));
            }
        }
        let artifact_hash = stable_hash(&(&self.identity, &self.registry, &self.pages))?;
        if artifact_hash != self.artifact_hash {
            return Err(FlyError::Encode(
                "static landing artifact hash mismatch".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StaticLandingBuildResult {
    pub ready: bool,
    pub readiness: LandingReadinessReport,
    pub registry_compatibility: RegistryCompatibilityReport,
    pub landing_sections: LandingSectionValidationReport,
    pub landing_properties: LandingPropertyValidationReport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<StaticLandingArtifact>,
}

pub fn build_static_landing_artifact(
    document: &ProjectDocument,
    registries: &RegistrySet,
    readiness_policy: LandingReadinessPolicy,
    render_policy: &RenderPolicy,
) -> FlyResult<StaticLandingBuildResult> {
    build_static_landing_artifact_with_renderer(
        document,
        registries,
        readiness_policy,
        render_policy,
        &FlyHtmlLandingRenderer,
    )
}

pub fn build_static_landing_artifact_with_renderer<R>(
    document: &ProjectDocument,
    registries: &RegistrySet,
    readiness_policy: LandingReadinessPolicy,
    render_policy: &RenderPolicy,
    renderer: &R,
) -> FlyResult<StaticLandingBuildResult>
where
    R: LandingRenderer + ?Sized,
{
    let registry = ComponentRegistryManifest::for_document(document, registries);
    let registry_compatibility = registry.compatibility_with(registries);
    let landing_sections = LandingSectionValidationReport::for_document(document)?;
    let landing_properties = LandingPropertyValidationReport::for_document(document);
    let readiness = evaluate_landing_readiness(document, readiness_policy);
    let ready = readiness.ready
        && registry_compatibility.compatible
        && landing_sections.valid
        && landing_properties.valid;
    if !ready {
        return Ok(StaticLandingBuildResult {
            ready,
            readiness,
            registry_compatibility,
            landing_sections,
            landing_properties,
            artifact: None,
        });
    }

    let renderer_manifest = renderer.manifest();
    validate_renderer_manifest(&renderer_manifest)?;
    let source_hash = stable_hash(document)?;
    let registry_hash = stable_hash(&registry)?;
    let render_policy_hash = stable_hash(render_policy)?;
    let build_hash = stable_hash(&(
        &source_hash,
        &renderer_manifest,
        &registry_hash,
        &render_policy_hash,
    ))?;

    let mut pages = Vec::with_capacity(document.project.pages.len());
    for page_index in 0..document.project.pages.len() {
        let rendered = renderer.render_page(document, page_index, render_policy)?;
        if rendered.page_index != page_index {
            return Err(FlyError::Encode(format!(
                "landing renderer returned page index {} for requested page {page_index}",
                rendered.page_index
            )));
        }
        if rendered.document_html.trim().is_empty() || rendered.body_html.trim().is_empty() {
            return Err(FlyError::Encode(format!(
                "landing renderer returned empty output for page {page_index}"
            )));
        }
        let content_hash = sha256_hex(rendered.document_html.as_bytes());
        pages.push(StaticLandingPage {
            page_index: rendered.page_index,
            page_id: rendered.page_id,
            slug: rendered.slug,
            head: rendered.head,
            document_html: rendered.document_html,
            body_html: rendered.body_html,
            css: rendered.css,
            content_hash,
            landing_sections: landing_sections.pages[page_index].sections.clone(),
        });
    }

    let identity = StaticLandingBuildIdentity {
        source_hash,
        renderer: renderer_manifest,
        registry_hash,
        render_policy_hash,
        build_hash,
    };
    let artifact_hash = stable_hash(&(&identity, &registry, &pages))?;
    let artifact = StaticLandingArtifact {
        identity,
        artifact_hash,
        registry,
        pages,
    };
    artifact.verify_integrity()?;

    Ok(StaticLandingBuildResult {
        ready,
        readiness,
        registry_compatibility,
        landing_sections,
        landing_properties,
        artifact: Some(artifact),
    })
}

fn validate_renderer_manifest(manifest: &LandingRendererManifest) -> FlyResult<()> {
    if manifest.id.trim().is_empty() || manifest.release.trim().is_empty() {
        return Err(FlyError::Encode(
            "landing renderer id and release must not be empty".to_string(),
        ));
    }
    Ok(())
}

fn stable_hash(value: &impl Serialize) -> FlyResult<String> {
    let bytes = serde_json::to_vec(value).map_err(|error| FlyError::Encode(error.to_string()))?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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
        assert!(
            manifest
                .components
                .iter()
                .all(|component| component.provider.is_some())
        );
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
        assert_eq!(first.artifact_hash.len(), 64);
        assert_eq!(first.identity.source_hash.len(), 64);
        assert_eq!(first.identity.build_hash.len(), 64);
        assert_eq!(first.pages[0].content_hash, second.pages[0].content_hash);
        assert!(first.pages[0].landing_sections.is_empty());
        assert!(first.pages[0].document_html.starts_with("<!doctype html>"));
        assert!(first.pages[0].document_html.contains("<title>Home</title>"));
        assert!(first.pages[0].body_html.contains("Stable landing"));
        first.verify_integrity().expect("valid artifact");
    }

    #[derive(Debug, Clone, Copy)]
    struct ReleasedRenderer {
        release: &'static str,
    }

    impl LandingRenderer for ReleasedRenderer {
        fn manifest(&self) -> LandingRendererManifest {
            LandingRendererManifest::new("test_html", self.release)
        }

        fn render_page(
            &self,
            document: &ProjectDocument,
            page_index: usize,
            policy: &RenderPolicy,
        ) -> FlyResult<LandingRenderedPage> {
            FlyHtmlLandingRenderer.render_page(document, page_index, policy)
        }
    }

    #[test]
    fn renderer_release_is_part_of_build_identity() {
        let project = ready_project();
        let registries = RegistrySet::with_builtins();
        let first = build_static_landing_artifact_with_renderer(
            &project,
            &registries,
            LandingReadinessPolicy::default(),
            &RenderPolicy::default(),
            &ReleasedRenderer { release: "1" },
        )
        .expect("first renderer build")
        .artifact
        .expect("first artifact");
        let second = build_static_landing_artifact_with_renderer(
            &project,
            &registries,
            LandingReadinessPolicy::default(),
            &RenderPolicy::default(),
            &ReleasedRenderer { release: "2" },
        )
        .expect("second renderer build")
        .artifact
        .expect("second artifact");

        assert_eq!(first.identity.source_hash, second.identity.source_hash);
        assert_ne!(first.identity.build_hash, second.identity.build_hash);
        assert_ne!(first.artifact_hash, second.artifact_hash);
    }

    #[test]
    fn artifact_integrity_detects_document_tampering() {
        let project = ready_project();
        let mut artifact = build_static_landing_artifact(
            &project,
            &RegistrySet::with_builtins(),
            LandingReadinessPolicy::default(),
            &RenderPolicy::default(),
        )
        .expect("build")
        .artifact
        .expect("artifact");
        artifact.pages[0]
            .document_html
            .push_str("<!-- tampered -->");
        assert!(artifact.verify_integrity().is_err());
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
        assert!(
            !artifact.pages[0].landing_sections[0]
                .content_hash
                .is_empty()
        );
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
