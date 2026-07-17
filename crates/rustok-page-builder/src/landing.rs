use crate::dto::PAGE_BUILDER_SUPPORTED_DOCUMENT_CONTRACTS;
use fly::{
    build_static_landing_artifact_v1, migrate_landing_document_v1, validate_project,
    ComponentRegistryManifest, FlyError, GrapesJsV1Codec, LandingDocumentV1,
    LandingReadinessPolicy, RegistryCompatibilityIssue, RegistryCompatibilityIssueKind,
    RegistryCompatibilityReport, RegistrySet, RenderPolicy, StaticLandingBuildResult,
    ValidationDiagnostic, ValidationLimits, ValidationReport, FLY_LANDING_DOCUMENT_V1,
    GRAPESJS_V1,
};
use serde_json::Value;

/// Framework-neutral landing inspection used by Leptos, Dioxus and static-export adapters.
///
/// Legacy GrapesJS payloads are migrated into the versioned Fly landing envelope at this boundary;
/// callers receive one typed document regardless of the transport that supplied it.
#[derive(Debug, Clone, PartialEq)]
pub struct LandingProjectInspection {
    landing: LandingDocumentV1,
    validation: ValidationReport,
    registry_compatibility: RegistryCompatibilityReport,
}

impl LandingProjectInspection {
    pub fn decode(schema_version: &str, project_data: &Value) -> LandingProjectResult<Self> {
        Self::decode_with(
            schema_version,
            project_data,
            &RegistrySet::with_builtins(),
            ValidationLimits::default(),
        )
    }

    pub fn decode_with(
        schema_version: &str,
        project_data: &Value,
        registries: &RegistrySet,
        limits: ValidationLimits,
    ) -> LandingProjectResult<Self> {
        let landing = match schema_version {
            GRAPESJS_V1 => {
                let document = GrapesJsV1Codec::decode_value(project_data.clone())
                    .map_err(LandingProjectError::Fly)?;
                LandingDocumentV1::new(document, registries)
            }
            FLY_LANDING_DOCUMENT_V1 => {
                let migrated = migrate_landing_document_v1(project_data.clone(), registries)
                    .map_err(LandingProjectError::Fly)?;
                if migrated.migrated {
                    return Err(LandingProjectError::ContractPayloadMismatch {
                        declared: FLY_LANDING_DOCUMENT_V1,
                        actual: GRAPESJS_V1,
                    });
                }
                migrated.document
            }
            actual => {
                return Err(LandingProjectError::UnsupportedSchema {
                    supported: &PAGE_BUILDER_SUPPORTED_DOCUMENT_CONTRACTS,
                    actual: actual.to_string(),
                });
            }
        };
        let validation = validate_project(&landing.document, registries, limits);
        let registry_compatibility = registry_compatibility(&landing, registries);
        Ok(Self {
            landing,
            validation,
            registry_compatibility,
        })
    }

    pub fn landing(&self) -> &LandingDocumentV1 {
        &self.landing
    }

    pub fn registry_manifest(&self) -> &ComponentRegistryManifest {
        &self.landing.registry
    }

    pub fn validation(&self) -> &ValidationReport {
        &self.validation
    }

    pub fn registry_compatibility(&self) -> &RegistryCompatibilityReport {
        &self.registry_compatibility
    }

    pub fn require_contract_valid(&self) -> LandingProjectResult<()> {
        if !self.validation.is_valid() {
            return Err(LandingProjectError::Validation {
                diagnostics: self.validation.errors().cloned().collect(),
            });
        }
        if !self.registry_compatibility.compatible {
            return Err(LandingProjectError::RegistryIncompatible {
                issue_count: self.registry_compatibility.issues.len(),
            });
        }
        Ok(())
    }

    pub fn build_static(
        &self,
        registries: &RegistrySet,
        readiness_policy: LandingReadinessPolicy,
        render_policy: &RenderPolicy,
    ) -> LandingProjectResult<StaticLandingBuildResult> {
        self.require_contract_valid()?;
        build_static_landing_artifact_v1(
            &self.landing.document,
            registries,
            readiness_policy,
            render_policy,
        )
        .map_err(LandingProjectError::Fly)
    }

    pub fn encode_landing_v1(&self) -> LandingProjectResult<Value> {
        self.landing.encode_value().map_err(LandingProjectError::Fly)
    }
}

fn registry_compatibility(
    landing: &LandingDocumentV1,
    registries: &RegistrySet,
) -> RegistryCompatibilityReport {
    let mut report = landing.registry.compatibility_with(registries);
    landing.document.project.visit_components(|component, _, _| {
        let component_type = component.component_type();
        let Some(available) = registries.components.get(component_type) else {
            return;
        };
        if let Some(provider) = component.provider.as_deref() {
            push_registry_issue(
                &mut report,
                (provider != available.provider).then(|| RegistryCompatibilityIssue {
                    component_type: component_type.to_string(),
                    kind: RegistryCompatibilityIssueKind::ProviderMismatch,
                    expected: Some(provider.to_string()),
                    actual: Some(available.provider.clone()),
                }),
            );
            push_registry_issue(
                &mut report,
                (component.schema_version.as_deref() != Some(available.schema_version.as_str()))
                    .then(|| RegistryCompatibilityIssue {
                        component_type: component_type.to_string(),
                        kind: RegistryCompatibilityIssueKind::SchemaVersionMismatch,
                        expected: component.schema_version.clone(),
                        actual: Some(available.schema_version.clone()),
                    }),
            );
        } else if let Some(schema_version) = component.schema_version.as_deref() {
            push_registry_issue(
                &mut report,
                (schema_version != available.schema_version).then(|| {
                    RegistryCompatibilityIssue {
                        component_type: component_type.to_string(),
                        kind: RegistryCompatibilityIssueKind::SchemaVersionMismatch,
                        expected: Some(schema_version.to_string()),
                        actual: Some(available.schema_version.clone()),
                    }
                }),
            );
        }
    });
    report.compatible = report.issues.is_empty();
    report
}

fn push_registry_issue(
    report: &mut RegistryCompatibilityReport,
    issue: Option<RegistryCompatibilityIssue>,
) {
    if let Some(issue) = issue {
        if !report.issues.contains(&issue) {
            report.issues.push(issue);
        }
    }
}

pub type LandingProjectResult<T> = Result<T, LandingProjectError>;

#[derive(Debug, thiserror::Error)]
pub enum LandingProjectError {
    #[error("unsupported page-builder document schema `{actual}`; supported: {supported:?}")]
    UnsupportedSchema {
        supported: &'static [&'static str],
        actual: String,
    },
    #[error(
        "page-builder payload does not match declared schema `{declared}`; received `{actual}`"
    )]
    ContractPayloadMismatch {
        declared: &'static str,
        actual: &'static str,
    },
    #[error("page-builder project validation failed")]
    Validation {
        diagnostics: Vec<ValidationDiagnostic>,
    },
    #[error("page-builder component registry is incompatible ({issue_count} issue(s))")]
    RegistryIncompatible { issue_count: usize },
    #[error(transparent)]
    Fly(#[from] FlyError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn project_value() -> Value {
        json!({
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
                    "tagName": "main",
                    "components": [{
                        "id": "hero-heading",
                        "type": "heading",
                        "tagName": "h1",
                        "content": "Welcome"
                    }]
                }
            }]
        })
    }

    #[test]
    fn legacy_and_landing_contracts_decode_to_the_same_typed_document() {
        let legacy = LandingProjectInspection::decode(GRAPESJS_V1, &project_value())
            .expect("legacy inspection");
        let landing_value = legacy.encode_landing_v1().expect("landing value");
        let typed = LandingProjectInspection::decode(FLY_LANDING_DOCUMENT_V1, &landing_value)
            .expect("typed inspection");
        assert_eq!(&legacy.landing().document, &typed.landing().document);
        assert_eq!(legacy.registry_manifest(), typed.registry_manifest());
    }

    #[test]
    fn declared_landing_contract_rejects_legacy_payload_shape() {
        let error = LandingProjectInspection::decode(FLY_LANDING_DOCUMENT_V1, &project_value())
            .expect_err("declared contract must be authoritative");
        assert!(matches!(
            error,
            LandingProjectError::ContractPayloadMismatch { .. }
        ));
    }

    #[test]
    fn declared_provider_schema_drift_blocks_contract_validity() {
        let mut project = project_value();
        project["pages"][0]["component"]["components"][0]["provider"] =
            json!("legacy.provider");
        project["pages"][0]["component"]["components"][0]["schemaVersion"] = json!("0");
        let inspection = LandingProjectInspection::decode(GRAPESJS_V1, &project)
            .expect("structural inspection");
        assert!(!inspection.registry_compatibility().compatible);
        assert!(inspection
            .registry_compatibility()
            .issues
            .iter()
            .any(|issue| issue.kind == RegistryCompatibilityIssueKind::ProviderMismatch));
        assert!(matches!(
            inspection.require_contract_valid(),
            Err(LandingProjectError::RegistryIncompatible { .. })
        ));
    }

    #[test]
    fn inspection_builds_static_publish_artifact() {
        let inspection = LandingProjectInspection::decode(GRAPESJS_V1, &project_value())
            .expect("inspection");
        inspection
            .require_contract_valid()
            .expect("contract-valid");
        let result = inspection
            .build_static(
                &RegistrySet::with_builtins(),
                LandingReadinessPolicy::default(),
                &RenderPolicy::default(),
            )
            .expect("static build");
        assert!(result.ready);
        let artifact = result.artifact.expect("artifact");
        assert_eq!(artifact.schema, fly::FLY_STATIC_LANDING_ARTIFACT_V1);
        assert_eq!(artifact.pages.len(), 1);
    }
}
