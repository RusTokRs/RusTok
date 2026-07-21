use crate::landing::{LandingProjectError, LandingProjectInspection, LandingProjectResult};
use fly::{
    build_static_landing_artifact_with_renderer, FlyHtmlLandingRenderer, LandingReadinessPolicy,
    LandingRenderer, ProjectDocument, RegistrySet, RenderPolicy, SequentialIdGenerator,
    StaticLandingArtifact, ValidationDiagnostic, ValidationLimits, ValidationSeverity,
};
use serde_json::Value;

const STATIC_LANDING_ID_PREFIX: &str = "fly-static";

/// Pure compiler from an editor project into a deterministic static landing artifact.
///
/// The compiler owns validation and rendering policy only. Persistence, tenancy and publication
/// transitions remain the responsibility of the consuming module.
pub struct StaticLandingCompiler<R = FlyHtmlLandingRenderer> {
    registries: RegistrySet,
    limits: ValidationLimits,
    readiness_policy: LandingReadinessPolicy,
    render_policy: RenderPolicy,
    renderer: R,
}

impl Default for StaticLandingCompiler<FlyHtmlLandingRenderer> {
    fn default() -> Self {
        Self::new(FlyHtmlLandingRenderer)
    }
}

impl<R> StaticLandingCompiler<R>
where
    R: LandingRenderer,
{
    pub fn new(renderer: R) -> Self {
        // Public artifacts are served with an HTTPS-only resource CSP. Reject insecure
        // subresources before rendering instead of publishing markup the browser will block.
        let render_policy = RenderPolicy {
            allow_http: false,
            ..RenderPolicy::default()
        };
        Self {
            registries: RegistrySet::with_builtins(),
            limits: ValidationLimits::default(),
            readiness_policy: LandingReadinessPolicy::default(),
            render_policy,
            renderer,
        }
    }

    pub fn with_policy(
        renderer: R,
        registries: RegistrySet,
        limits: ValidationLimits,
        readiness_policy: LandingReadinessPolicy,
        render_policy: RenderPolicy,
    ) -> Self {
        Self {
            registries,
            limits,
            readiness_policy,
            render_policy,
            renderer,
        }
    }

    pub fn inspect(&self, project_data: &Value) -> LandingProjectResult<LandingProjectInspection> {
        LandingProjectInspection::decode_with(project_data, &self.registries, self.limits)
    }

    pub fn compile_publish(
        &self,
        project_data: &Value,
    ) -> LandingProjectResult<StaticLandingArtifact> {
        let document = self.prepare_document(project_data)?;
        self.compile_prepared_document(&document)
    }

    pub(crate) fn prepare_document(
        &self,
        project_data: &Value,
    ) -> LandingProjectResult<ProjectDocument> {
        let inspection = self.inspect(project_data)?;
        inspection.require_contract_valid()?;

        // GrapesJS permits components without explicit ids. Static rendering uses ids as stable CSS
        // hooks, so normalize a compiler-owned clone before readiness evaluation and rendering. The
        // sequential generator and document traversal make this transformation deterministic while
        // leaving the persisted editor source untouched.
        let mut document = inspection.document().clone();
        document.ensure_stable_ids(&mut SequentialIdGenerator::new(STATIC_LANDING_ID_PREFIX));
        if !self.render_policy.allow_http {
            require_secure_resource_urls(&document)?;
        }
        Ok(document)
    }

    pub(crate) fn compile_prepared_document(
        &self,
        document: &ProjectDocument,
    ) -> LandingProjectResult<StaticLandingArtifact> {
        // Runtime bindings can materialize new resource URLs after the authoring document was
        // prepared. Re-run the public artifact security policy on the exact document being built.
        if !self.render_policy.allow_http {
            require_secure_resource_urls(document)?;
        }
        let build = build_static_landing_artifact_with_renderer(
            document,
            &self.registries,
            self.readiness_policy,
            &self.render_policy,
            &self.renderer,
        )
        .map_err(LandingProjectError::Fly)?;
        match (build.ready, build.artifact) {
            (true, Some(artifact)) => Ok(artifact),
            _ => Err(LandingProjectError::PublishNotReady {
                blocking_codes: build
                    .readiness
                    .blocking_issues()
                    .map(|issue| issue.diagnostic.code.clone())
                    .collect(),
            }),
        }
    }

    pub(crate) fn render_policy(&self) -> &RenderPolicy {
        &self.render_policy
    }
}

fn require_secure_resource_urls(document: &ProjectDocument) -> LandingProjectResult<()> {
    let mut diagnostics = Vec::new();
    document.project.visit_components(|component, _, path| {
        for (attribute, value) in &component.attributes {
            let attribute = attribute.to_ascii_lowercase();
            if !matches!(attribute.as_str(), "src" | "poster") {
                continue;
            }
            let Some(value) = value.as_str() else {
                continue;
            };
            if value
                .trim()
                .to_ascii_lowercase()
                .starts_with("http://")
            {
                diagnostics.push(ValidationDiagnostic {
                    severity: ValidationSeverity::Error,
                    code: "landing_insecure_resource_url".to_string(),
                    path: format!("{path}.attributes.{attribute}"),
                    message: format!(
                        "static landing resource `{attribute}` must use HTTPS, a relative URL, or an allowed data image"
                    ),
                });
            }
        }
    });
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(LandingProjectError::Validation { diagnostics })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn project() -> Value {
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
                    "components": [{
                        "id": "heading",
                        "type": "heading",
                        "tagName": "h1",
                        "content": "Welcome"
                    }]
                }
            }]
        })
    }

    fn project_with_idless_styled_component() -> Value {
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
                    "components": [{
                        "type": "heading",
                        "tagName": "h1",
                        "content": "Welcome",
                        "style": { "margin-top": "12px" }
                    }]
                }
            }]
        })
    }

    fn project_with_insecure_resource() -> Value {
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
                    "components": [{
                        "id": "heading",
                        "type": "heading",
                        "tagName": "h1",
                        "content": "Welcome"
                    }, {
                        "id": "hero",
                        "type": "image",
                        "attributes": { "SRC": "http://cdn.example.com/hero.webp" }
                    }]
                }
            }]
        })
    }

    #[test]
    fn compiler_is_deterministic_and_persistence_free() {
        let compiler = StaticLandingCompiler::default();
        let first = compiler
            .compile_publish(&project())
            .expect("first artifact");
        let second = compiler
            .compile_publish(&project())
            .expect("second artifact");

        assert_eq!(first.identity, second.identity);
        assert_eq!(first.artifact_hash, second.artifact_hash);
        assert_eq!(first.pages.len(), 1);
        first.verify_integrity().expect("artifact integrity");
    }

    #[test]
    fn compiler_assigns_deterministic_style_hooks_to_idless_components() {
        let compiler = StaticLandingCompiler::default();
        let first = compiler
            .compile_publish(&project_with_idless_styled_component())
            .expect("first artifact");
        let second = compiler
            .compile_publish(&project_with_idless_styled_component())
            .expect("second artifact");

        assert_eq!(first.identity, second.identity);
        assert_eq!(first.artifact_hash, second.artifact_hash);
        let page = &first.pages[0];
        assert!(page
            .body_html
            .contains("data-fly-style-id=\"fly-static-heading-1\""));
        assert!(page
            .css
            .contains("[data-fly-style-id=\"fly-static-heading-1\"]{margin-top:12px}"));
        assert!(!page.body_html.contains(" style=\""));
        first.verify_integrity().expect("artifact integrity");
    }

    #[test]
    fn compiler_rejects_http_subresources_before_artifact_creation() {
        let error = StaticLandingCompiler::default()
            .compile_publish(&project_with_insecure_resource())
            .expect_err("HTTP subresource must be rejected");
        let LandingProjectError::Validation { diagnostics } = error else {
            panic!("expected typed validation diagnostics");
        };
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "landing_insecure_resource_url"
                && diagnostic.path.ends_with("attributes.src")
        }));
    }
}
