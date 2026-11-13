use crate::landing::{LandingProjectError, LandingProjectInspection, LandingProjectResult};
use crate::static_publish_policy::{
    PageBuilderStaticPublishPolicyError, validate_static_publish_document,
};
use crate::{
    FlyHtmlLandingRenderer, LandingReadinessPolicy, LandingRenderer, StaticLandingArtifact,
    build_static_landing_artifact_with_renderer,
};
use fly::{
    ProjectDocument, RegistrySet, RenderPolicy, SequentialIdGenerator, ValidationDiagnostic,
    ValidationLimits, ValidationSeverity,
};
use serde_json::Value;

#[path = "static_publish_resource_limits.rs"]
pub mod static_publish_resource_limits;

use static_publish_resource_limits::{
    PageBuilderStaticPublishResourceLimitError, validate_static_publish_resource_limits,
};

const STATIC_LANDING_ID_PREFIX: &str = "fly-static";

/// Render policy of a public static landing artifact.
///
/// Single source for the publish compiler and for every preview that claims to show what
/// will be published: a preview rendered with a looser policy shows resources the publish
/// step then rejects, turning an authoring mistake into a late, confusing publish failure.
///
/// Public artifacts are served with an HTTPS-only resource CSP, so `http://` subresources
/// are not rendered; everything else follows the Fly default.
pub fn static_publish_render_policy() -> RenderPolicy {
    RenderPolicy {
        allow_http: false,
        ..RenderPolicy::default()
    }
}

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
        // Insecure subresources are rejected before rendering instead of publishing markup
        // the browser will block; see `static_publish_render_policy`.
        Self {
            registries: RegistrySet::with_builtins(),
            limits: ValidationLimits::default(),
            readiness_policy: LandingReadinessPolicy::default(),
            render_policy: static_publish_render_policy(),
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

        // Reject excessive current-tree shape before the recursive public-resource and static
        // publish policy passes. Recheck after stable-id normalization because generated ids are
        // part of the exact prepared project that will be hashed and materialized.
        let mut document = inspection.document().clone();
        require_static_publish_resource_limits(&document)?;

        // GrapesJS permits components without explicit ids. Static rendering uses ids as stable CSS
        // hooks, so normalize a compiler-owned clone before readiness evaluation and rendering. The
        // sequential generator and document traversal make this transformation deterministic while
        // leaving the persisted editor source untouched.
        document.ensure_stable_ids(&mut SequentialIdGenerator::new(STATIC_LANDING_ID_PREFIX));
        require_static_publish_resource_limits(&document)?;
        if !self.render_policy.allow_http {
            require_secure_resource_urls(&document)?;
        }
        require_static_publish_policy(&document)?;
        Ok(document)
    }

    pub(crate) fn compile_prepared_document(
        &self,
        document: &ProjectDocument,
    ) -> LandingProjectResult<StaticLandingArtifact> {
        self.compile_document(document, false)
    }

    pub(crate) fn compile_materialized_document(
        &self,
        document: &ProjectDocument,
    ) -> LandingProjectResult<StaticLandingArtifact> {
        self.compile_document(document, true)
    }

    fn compile_document(
        &self,
        document: &ProjectDocument,
        already_materialized: bool,
    ) -> LandingProjectResult<StaticLandingArtifact> {
        // Runtime bindings can materialize new components, attributes, CSS and URLs after the
        // authoring document was prepared. Re-run every public-artifact policy on the exact
        // document being built, with bounded shape checks before recursive policy traversal.
        require_static_publish_resource_limits(document)?;
        if !self.render_policy.allow_http {
            require_secure_resource_urls(document)?;
        }
        require_static_publish_policy(document)?;
        let build = if already_materialized {
            crate::domain::build_materialized_static_landing_artifact_with_renderer(
                document,
                &self.registries,
                self.readiness_policy,
                &self.render_policy,
                &self.renderer,
            )
        } else {
            build_static_landing_artifact_with_renderer(
                document,
                &self.registries,
                self.readiness_policy,
                &self.render_policy,
                &self.renderer,
            )
        }
        .map_err(LandingProjectError::Fly)?;
        match (build.ready, build.artifact) {
            (true, Some(artifact)) => Ok(artifact),
            _ => {
                let mut blocking_codes: Vec<String> = build
                    .readiness
                    .blocking_issues()
                    .map(|issue| issue.diagnostic.code.clone())
                    .collect();
                if !build.registry_compatibility.compatible {
                    for issue in &build.registry_compatibility.issues {
                        blocking_codes.push(format!("registry_incompatible: {:?}", issue));
                    }
                }
                if !build.landing_sections.valid {
                    for issue in &build.landing_sections.issues {
                        blocking_codes.push(format!("landing_sections_invalid: {:?}", issue));
                    }
                }
                if !build.landing_properties.valid {
                    for issue in &build.landing_properties.issues {
                        blocking_codes.push(format!("landing_properties_invalid: {:?}", issue));
                    }
                }
                Err(LandingProjectError::PublishNotReady { blocking_codes })
            }
        }
    }

    pub(crate) fn render_policy(&self) -> &RenderPolicy {
        &self.render_policy
    }
}

fn require_static_publish_resource_limits(document: &ProjectDocument) -> LandingProjectResult<()> {
    match validate_static_publish_resource_limits(document) {
        Ok(_) => Ok(()),
        Err(PageBuilderStaticPublishResourceLimitError::Rejected { diagnostics }) => {
            Err(LandingProjectError::Validation {
                diagnostics: diagnostics
                    .into_iter()
                    .map(|diagnostic| ValidationDiagnostic {
                        severity: ValidationSeverity::Error,
                        code: diagnostic.code,
                        path: diagnostic.path,
                        message: diagnostic.message,
                    })
                    .collect(),
            })
        }
        Err(error) => Err(LandingProjectError::Validation {
            diagnostics: vec![ValidationDiagnostic {
                severity: ValidationSeverity::Error,
                code: "landing_static_publish_resource_limits_integrity".to_string(),
                path: "project".to_string(),
                message: error.to_string(),
            }],
        }),
    }
}

fn require_static_publish_policy(document: &ProjectDocument) -> LandingProjectResult<()> {
    match validate_static_publish_document(document) {
        Ok(_) => Ok(()),
        Err(PageBuilderStaticPublishPolicyError::Rejected { diagnostics }) => {
            Err(LandingProjectError::Validation {
                diagnostics: diagnostics
                    .into_iter()
                    .map(|diagnostic| ValidationDiagnostic {
                        severity: ValidationSeverity::Error,
                        code: diagnostic.code,
                        path: diagnostic.path,
                        message: diagnostic.message,
                    })
                    .collect(),
            })
        }
        Err(error) => Err(LandingProjectError::Validation {
            diagnostics: vec![ValidationDiagnostic {
                severity: ValidationSeverity::Error,
                code: "landing_static_publish_policy_integrity".to_string(),
                path: "project".to_string(),
                message: error.to_string(),
            }],
        }),
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
    use super::static_publish_resource_limits::PageBuilderStaticPublishResourceLimits;
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
        assert!(
            page.body_html
                .contains("data-fly-style-id=\"fly-static-heading-1\"")
        );
        assert!(
            page.css
                .contains("[data-fly-style-id=\"fly-static-heading-1\"]{margin-top:12px}")
        );
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

    #[test]
    fn compiler_rechecks_materialized_css_policy() {
        let compiler = StaticLandingCompiler::default();
        let mut document = compiler.prepare_document(&project()).expect("prepared");
        document.component_mut("heading").expect("heading").style =
            Some(json!({ "background-image": "url(javascript:alert(1))" }));

        let error = compiler
            .compile_prepared_document(&document)
            .expect_err("materialized CSS must be rejected");
        let LandingProjectError::Validation { diagnostics } = error else {
            panic!("expected typed validation diagnostics");
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "landing_css_value_rejected")
        );
    }

    #[test]
    fn compiler_rechecks_materialized_resource_limits_before_recursive_policy() {
        let compiler = StaticLandingCompiler::default();
        let mut document = compiler.prepare_document(&project()).expect("prepared");
        let page = document.project.pages[0].clone();
        document.project.pages =
            vec![page; PageBuilderStaticPublishResourceLimits::default().max_pages + 1];

        let error = compiler
            .compile_prepared_document(&document)
            .expect_err("materialized resource limit must be rejected");
        let LandingProjectError::Validation { diagnostics } = error else {
            panic!("expected typed validation diagnostics");
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "landing_page_count_exceeded")
        );
    }

    #[test]
    fn preview_policy_omits_exactly_the_http_resources_publish_rejects() {
        // Audit §6.2: a preview rendered with the shared policy must not show an image that
        // the publish compiler refuses, and must keep showing the HTTPS one it accepts.
        let project_with_image = |src: &str| {
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
                            "attributes": { "src": src, "alt": "Product hero" }
                        }]
                    }
                }]
            })
        };
        let preview = |src: &str, policy: RenderPolicy| {
            crate::render::PageBuilderRenderer
                .render_runtime_document_html(
                    project_with_image(src),
                    fly::PageSelection::First,
                    policy,
                    json!({}),
                )
                .expect("preview")
        };
        let insecure = "http://cdn.example.com/hero.webp";
        let secure = "https://cdn.example.com/hero.webp";

        assert!(preview(insecure, RenderPolicy::default()).contains(insecure));
        assert!(!preview(insecure, static_publish_render_policy()).contains(insecure));
        assert!(
            StaticLandingCompiler::default()
                .compile_publish(&project_with_image(insecure))
                .is_err()
        );

        assert!(preview(secure, static_publish_render_policy()).contains(secure));
        StaticLandingCompiler::default()
            .compile_publish(&project_with_image(secure))
            .expect("HTTPS image publishes");
    }

    #[test]
    fn default_compiler_uses_the_shared_publish_render_policy() {
        let compiler = StaticLandingCompiler::default();
        assert_eq!(compiler.render_policy, static_publish_render_policy());
        assert!(!static_publish_render_policy().allow_http);
        assert_eq!(
            RenderPolicy {
                allow_http: true,
                ..static_publish_render_policy()
            },
            RenderPolicy::default()
        );
    }

    fn project_with_srcset(srcset: &str) -> Value {
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
                        "attributes": {
                            "src": "https://cdn.example.com/hero.webp",
                            "alt": "Product hero",
                            "srcset": srcset,
                            "sizes": "(max-width: 800px) 100vw, 800px"
                        }
                    }]
                }
            }]
        })
    }

    #[test]
    fn compiler_renders_validated_srcset_into_artifact() {
        // End-to-end contract from audit §3.1: a srcset that passes the
        // publish policy must reach the published artifact — together with
        // its `sizes`, never orphaned again.
        let artifact = StaticLandingCompiler::default()
            .compile_publish(&project_with_srcset(
                "https://cdn.example.com/hero.webp 800w, https://cdn.example.com/hero@2x.webp 1600w",
            ))
            .expect("artifact");
        let page = &artifact.pages[0];
        assert!(
            page.body_html.contains(
                "srcset=\"https://cdn.example.com/hero.webp 800w, \
                 https://cdn.example.com/hero@2x.webp 1600w\""
            ),
            "body: {}",
            page.body_html
        );
        assert!(
            page.body_html
                .contains("sizes=\"(max-width: 800px) 100vw, 800px\""),
            "body: {}",
            page.body_html
        );
        artifact.verify_integrity().expect("artifact integrity");
    }

    #[test]
    fn compiler_rejects_insecure_srcset_candidate_before_artifact_creation() {
        let error = StaticLandingCompiler::default()
            .compile_publish(&project_with_srcset(
                "https://cdn.example.com/hero.webp 800w, http://cdn.example.com/hero@2x.webp 1600w",
            ))
            .expect_err("HTTP srcset candidate must be rejected");
        let LandingProjectError::Validation { diagnostics } = error else {
            panic!("expected typed validation diagnostics");
        };
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "landing_srcset_rejected"
                && diagnostic.path.ends_with("attributes.srcset")
        }));
    }

    #[test]
    fn compiler_rejects_data_url_srcset_candidate_before_artifact_creation() {
        let error = StaticLandingCompiler::default()
            .compile_publish(&project_with_srcset(
                "https://cdn.example.com/hero.webp 800w, data:image/png;base64,AAAA 2x",
            ))
            .expect_err("data: srcset candidate must be rejected");
        let LandingProjectError::Validation { diagnostics } = error else {
            panic!("expected typed validation diagnostics");
        };
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "landing_srcset_rejected"
                && diagnostic.path.ends_with("attributes.srcset")
        }));
    }
}
