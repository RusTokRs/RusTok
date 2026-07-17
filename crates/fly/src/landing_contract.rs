use crate::{
    evaluate_landing_readiness, render_page, FlyError, FlyResult, GrapesJsV1Codec,
    LandingReadinessPolicy, LandingReadinessReport, PageHead, PageSelection, ProjectDocument,
    ProjectHash, RegistrySet, RenderPolicy,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const FLY_LANDING_DOCUMENT_V1: &str = "fly_landing_v1";
pub const FLY_COMPONENT_REGISTRY_V1: &str = "fly_component_registry_v1";
pub const FLY_STATIC_LANDING_ARTIFACT_V1: &str = "fly_static_landing_v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LandingDocumentSchema {
    FlyLandingV1,
}

impl LandingDocumentSchema {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FlyLandingV1 => FLY_LANDING_DOCUMENT_V1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComponentRegistryContract {
    pub component_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComponentRegistryManifest {
    pub contract: String,
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
                    schema_version: Some(definition.schema_version.clone()),
                })
                .unwrap_or_else(|| ComponentRegistryContract {
                    component_type: component_type.clone(),
                    provider: component.provider.clone(),
                    schema_version: component.schema_version.clone(),
                });
            components.entry(component_type).or_insert(contract);
        });
        Self {
            contract: FLY_COMPONENT_REGISTRY_V1.to_string(),
            components: components.into_values().collect(),
        }
    }

    pub fn compatibility_with(&self, registries: &RegistrySet) -> RegistryCompatibilityReport {
        let mut issues = Vec::new();
        if self.contract != FLY_COMPONENT_REGISTRY_V1 {
            issues.push(RegistryCompatibilityIssue {
                component_type: "*".to_string(),
                kind: RegistryCompatibilityIssueKind::ContractMismatch,
                expected: Some(self.contract.clone()),
                actual: Some(FLY_COMPONENT_REGISTRY_V1.to_string()),
            });
        }
        for required in &self.components {
            let Some(available) = registries.components.get(&required.component_type) else {
                issues.push(RegistryCompatibilityIssue {
                    component_type: required.component_type.clone(),
                    kind: RegistryCompatibilityIssueKind::MissingComponent,
                    expected: required.schema_version.clone(),
                    actual: None,
                });
                continue;
            };
            if required
                .provider
                .as_deref()
                .is_some_and(|provider| provider != available.provider)
            {
                issues.push(RegistryCompatibilityIssue {
                    component_type: required.component_type.clone(),
                    kind: RegistryCompatibilityIssueKind::ProviderMismatch,
                    expected: required.provider.clone(),
                    actual: Some(available.provider.clone()),
                });
            }
            if required
                .schema_version
                .as_deref()
                .is_some_and(|version| version != available.schema_version)
            {
                issues.push(RegistryCompatibilityIssue {
                    component_type: required.component_type.clone(),
                    kind: RegistryCompatibilityIssueKind::SchemaVersionMismatch,
                    expected: required.schema_version.clone(),
                    actual: Some(available.schema_version.clone()),
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
    ContractMismatch,
    MissingComponent,
    ProviderMismatch,
    SchemaVersionMismatch,
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
pub struct LandingDocumentV1 {
    pub schema: LandingDocumentSchema,
    pub registry: ComponentRegistryManifest,
    pub document: ProjectDocument,
}

impl LandingDocumentV1 {
    pub fn new(document: ProjectDocument, registries: &RegistrySet) -> Self {
        let registry = ComponentRegistryManifest::for_document(&document, registries);
        Self {
            schema: LandingDocumentSchema::FlyLandingV1,
            registry,
            document,
        }
    }

    pub fn encode_value(&self) -> FlyResult<Value> {
        serde_json::to_value(self).map_err(|error| FlyError::Encode(error.to_string()))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LandingMigrationSource {
    GrapesJsV1,
    FlyLandingV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LandingMigrationResult {
    pub source: LandingMigrationSource,
    pub migrated: bool,
    pub document: LandingDocumentV1,
}

pub fn migrate_landing_document_v1(
    value: Value,
    registries: &RegistrySet,
) -> FlyResult<LandingMigrationResult> {
    match value.get("schema").and_then(Value::as_str) {
        Some(FLY_LANDING_DOCUMENT_V1) => {
            let document = serde_json::from_value::<LandingDocumentV1>(value)
                .map_err(|error| FlyError::Decode(error.to_string()))?;
            Ok(LandingMigrationResult {
                source: LandingMigrationSource::FlyLandingV1,
                migrated: false,
                document,
            })
        }
        Some(schema) => Err(FlyError::Decode(format!(
            "unsupported landing document schema `{schema}`"
        ))),
        None => {
            let document = GrapesJsV1Codec::decode_value(value)?;
            Ok(LandingMigrationResult {
                source: LandingMigrationSource::GrapesJsV1,
                migrated: true,
                document: LandingDocumentV1::new(document, registries),
            })
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StaticLandingPageV1 {
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
pub struct StaticLandingArtifactV1 {
    pub schema: String,
    pub source_hash: String,
    pub artifact_hash: String,
    pub registry: ComponentRegistryManifest,
    pub pages: Vec<StaticLandingPageV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StaticLandingBuildResult {
    pub ready: bool,
    pub readiness: LandingReadinessReport,
    pub registry_compatibility: RegistryCompatibilityReport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<StaticLandingArtifactV1>,
}

pub fn build_static_landing_artifact_v1(
    document: &ProjectDocument,
    registries: &RegistrySet,
    readiness_policy: LandingReadinessPolicy,
    render_policy: &RenderPolicy,
) -> FlyResult<StaticLandingBuildResult> {
    let landing = LandingDocumentV1::new(document.clone(), registries);
    let registry_compatibility = landing.registry.compatibility_with(registries);
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
        pages.push(StaticLandingPageV1 {
            page_index,
            page_id: rendered.page_id,
            slug: rendered.metadata.slug,
            head: rendered.head,
            content_hash: ProjectHash::from_bytes(html.as_bytes()).hex(),
            html,
        });
    }

    let source_hash = document.hash().hex();
    let artifact_bytes = serde_json::to_vec(&(
        FLY_STATIC_LANDING_ARTIFACT_V1,
        &source_hash,
        &landing.registry,
        &pages,
    ))
    .map_err(|error| FlyError::Encode(error.to_string()))?;
    let artifact = StaticLandingArtifactV1 {
        schema: FLY_STATIC_LANDING_ARTIFACT_V1.to_string(),
        source_hash,
        artifact_hash: ProjectHash::from_bytes(&artifact_bytes).hex(),
        registry: landing.registry,
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
    fn legacy_grapesjs_project_migrates_to_typed_landing_v1() {
        let registries = RegistrySet::with_builtins();
        let legacy = GrapesJsV1Codec::encode_value(&ready_project()).expect("legacy value");
        let migrated = migrate_landing_document_v1(legacy, &registries).expect("migration");
        assert!(migrated.migrated);
        assert_eq!(migrated.source, LandingMigrationSource::GrapesJsV1);
        assert_eq!(migrated.document.schema.as_str(), FLY_LANDING_DOCUMENT_V1);
        assert!(migrated
            .document
            .registry
            .compatibility_with(&registries)
            .compatible);
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
    }

    #[test]
    fn static_artifact_is_deterministic_and_contains_complete_html() {
        let project = ready_project();
        let registries = RegistrySet::with_builtins();
        let first = build_static_landing_artifact_v1(
            &project,
            &registries,
            LandingReadinessPolicy::default(),
            &RenderPolicy::default(),
        )
        .expect("first build");
        let second = build_static_landing_artifact_v1(
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
    fn readiness_failure_does_not_emit_publish_artifact() {
        let project = GrapesJsV1Codec::decode_value(json!({
            "pages": [{
                "id": "home",
                "component": { "id": "root", "type": "wrapper" }
            }]
        }))
        .expect("project");
        let result = build_static_landing_artifact_v1(
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
