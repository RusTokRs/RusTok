use crate::{
    evaluate_landing_readiness, render_page, FlyError, FlyResult, LandingReadinessPolicy,
    LandingReadinessReport, PageHead, PageSelection, ProjectDocument, ProjectHash, RegistrySet,
    RenderPolicy,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    let readiness = evaluate_landing_readiness(document, readiness_policy);
    let ready = readiness.ready && registry_compatibility.compatible;
    if !ready {
        return Ok(StaticLandingBuildResult {
            ready,
            readiness,
            registry_compatibility,
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
        artifact: Some(artifact),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrapesJsV1Codec;
    use serde_json::json;

    fn ready_project() -> ProjectDocument {
        GrapesJsV1Codec::decode_value(json!({
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
        let first = first.artifact.expect("artifact");
        let second = second.artifact.expect("artifact");
        assert_eq!(first.artifact_hash, second.artifact_hash);
        assert_eq!(first.pages[0].content_hash, second.pages[0].content_hash);
        assert!(first.pages[0].html.starts_with("<!doctype html>"));
        assert!(first.pages[0].html.contains("<title>Home</title>"));
    }

    #[test]
    fn static_artifact_is_not_emitted_for_unready_project() {
        let project = GrapesJsV1Codec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": { "id": "root", "type": "wrapper" }
            }]
        }))
        .expect("project");
        let result = build_static_landing_artifact(
            &project,
            &RegistrySet::with_builtins(),
            LandingReadinessPolicy::default(),
            &RenderPolicy::default(),
        )
        .expect("build report");
        assert!(!result.ready);
        assert!(result.artifact.is_none());
        assert!(result.readiness.blocking_issues().next().is_some());
    }
}
