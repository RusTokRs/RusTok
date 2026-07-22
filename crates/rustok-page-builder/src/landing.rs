use fly::{
    ComponentRegistryManifest, FlyError, GrapesJsCodec, LandingPropertyValidationReport,
    LandingReadinessPolicy, LandingRenderer, ProjectDocument, RegistryCompatibilityIssue,
    RegistryCompatibilityIssueKind, RegistryCompatibilityReport, RegistrySet, RenderPolicy,
    StaticLandingBuildResult, ValidationDiagnostic, ValidationLimits, ValidationReport,
    build_static_landing_artifact, build_static_landing_artifact_with_renderer, validate_project,
};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct LandingProjectInspection {
    document: ProjectDocument,
    registry: ComponentRegistryManifest,
    validation: ValidationReport,
    landing_properties: LandingPropertyValidationReport,
    registry_compatibility: RegistryCompatibilityReport,
}

impl LandingProjectInspection {
    pub fn decode(project_data: &Value) -> LandingProjectResult<Self> {
        Self::decode_with(
            project_data,
            &RegistrySet::with_builtins(),
            ValidationLimits::default(),
        )
    }

    pub fn decode_with(
        project_data: &Value,
        registries: &RegistrySet,
        limits: ValidationLimits,
    ) -> LandingProjectResult<Self> {
        let document =
            GrapesJsCodec::decode_value(project_data.clone()).map_err(LandingProjectError::Fly)?;
        let registry = ComponentRegistryManifest::for_document(&document, registries);
        let validation = validate_project(&document, registries, limits);
        let landing_properties = LandingPropertyValidationReport::for_document(&document);
        let registry_compatibility = registry_compatibility(&document, &registry, registries);
        Ok(Self {
            document,
            registry,
            validation,
            landing_properties,
            registry_compatibility,
        })
    }

    pub fn document(&self) -> &ProjectDocument {
        &self.document
    }

    pub fn registry_manifest(&self) -> &ComponentRegistryManifest {
        &self.registry
    }

    pub fn validation(&self) -> &ValidationReport {
        &self.validation
    }

    pub fn landing_properties(&self) -> &LandingPropertyValidationReport {
        &self.landing_properties
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
        if !self.landing_properties.valid {
            return Err(LandingProjectError::PropertyContractInvalid {
                issue_count: self.landing_properties.issues.len(),
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
        build_static_landing_artifact(&self.document, registries, readiness_policy, render_policy)
            .map_err(LandingProjectError::Fly)
    }

    pub fn build_static_with_renderer<R>(
        &self,
        registries: &RegistrySet,
        readiness_policy: LandingReadinessPolicy,
        render_policy: &RenderPolicy,
        renderer: &R,
    ) -> LandingProjectResult<StaticLandingBuildResult>
    where
        R: LandingRenderer + ?Sized,
    {
        self.require_contract_valid()?;
        build_static_landing_artifact_with_renderer(
            &self.document,
            registries,
            readiness_policy,
            render_policy,
            renderer,
        )
        .map_err(LandingProjectError::Fly)
    }
}

fn registry_compatibility(
    document: &ProjectDocument,
    registry: &ComponentRegistryManifest,
    registries: &RegistrySet,
) -> RegistryCompatibilityReport {
    let mut report = registry.compatibility_with(registries);
    document.project.visit_components(|component, _, _| {
        let component_type = component.component_type();
        let Some(available) = registries.components.get(component_type) else {
            return;
        };
        if let Some(provider) = component.provider.as_deref() {
            push_registry_issue(
                &mut report,
                (provider != available.provider.as_str()).then(|| RegistryCompatibilityIssue {
                    component_type: component_type.to_string(),
                    kind: RegistryCompatibilityIssueKind::ProviderMismatch,
                    expected: Some(provider.to_string()),
                    actual: Some(available.provider.clone()),
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
    #[error("page-builder project validation failed")]
    Validation {
        diagnostics: Vec<ValidationDiagnostic>,
    },
    #[error("page-builder landing property contract is invalid ({issue_count} issue(s))")]
    PropertyContractInvalid { issue_count: usize },
    #[error("page-builder component registry is incompatible ({issue_count} issue(s))")]
    RegistryIncompatible { issue_count: usize },
    #[error("page-builder static publish is not ready")]
    PublishNotReady { blocking_codes: Vec<String> },
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
    fn api_decodes_to_the_domain_document() {
        let inspection = LandingProjectInspection::decode(&project_value()).expect("inspection");
        assert_eq!(inspection.document().project.pages.len(), 1);
        assert!(inspection.registry_manifest().components.len() >= 2);
    }

    #[test]
    fn declared_provider_drift_blocks_contract_validity() {
        let mut project = project_value();
        project["pages"][0]["component"]["components"][0]["provider"] = json!("other.provider");
        let inspection = LandingProjectInspection::decode(&project).expect("structural inspection");
        assert!(!inspection.registry_compatibility().compatible);
        assert!(
            inspection
                .registry_compatibility()
                .issues
                .iter()
                .any(|issue| issue.kind == RegistryCompatibilityIssueKind::ProviderMismatch)
        );
        assert!(matches!(
            inspection.require_contract_valid(),
            Err(LandingProjectError::RegistryIncompatible { .. })
        ));
    }

    #[test]
    fn inspection_builds_static_publish_artifact() {
        let inspection = LandingProjectInspection::decode(&project_value()).expect("inspection");
        inspection.require_contract_valid().expect("contract-valid");
        let result = inspection
            .build_static(
                &RegistrySet::with_builtins(),
                LandingReadinessPolicy::default(),
                &RenderPolicy::default(),
            )
            .expect("static build");
        assert!(result.ready);
        let artifact = result.artifact.expect("artifact");
        assert_eq!(artifact.pages.len(), 1);
    }
}
