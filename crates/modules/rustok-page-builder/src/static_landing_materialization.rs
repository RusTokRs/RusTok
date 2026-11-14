use crate::dto::PageBuilderPreviewRuntime;
use crate::interaction_providers::HostFormProviderEndpoints;
use crate::landing::LandingProjectError;
use crate::static_landing::StaticLandingCompiler;
use crate::{RuntimeContextScenario, RuntimeScenarioRenderSnapshot, StaticLandingArtifact};
use fly::{
    PageSelection, ProjectHash, ValidationSeverity,
    materialize_project_with_runtime_context_with_providers,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const PAGE_BUILDER_STATIC_MATERIALIZATION_FORMAT: &str =
    "page_builder_static_runtime_materialization_v1";
const DEFAULT_STATIC_RUNTIME_SCENARIO_ID: &str = "page_builder_static_default";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageBuilderStaticLandingMaterializationIdentity {
    pub format: String,
    pub runtime_context_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_scenario_id: Option<String>,
    pub runtime_snapshot_hash: String,
    pub static_build_hash: String,
    pub static_artifact_hash: String,
    pub materialization_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderMaterializedStaticLandingArtifact {
    pub identity: PageBuilderStaticLandingMaterializationIdentity,
    pub runtime_snapshots: Vec<RuntimeScenarioRenderSnapshot>,
    pub artifact: StaticLandingArtifact,
}

impl PageBuilderMaterializedStaticLandingArtifact {
    pub fn verify_integrity(&self) -> Result<(), PageBuilderStaticLandingMaterializationError> {
        self.artifact.verify_integrity().map_err(|error| {
            PageBuilderStaticLandingMaterializationError::Integrity(error.to_string())
        })?;
        if self.identity.format != PAGE_BUILDER_STATIC_MATERIALIZATION_FORMAT {
            return Err(PageBuilderStaticLandingMaterializationError::Integrity(
                "unsupported static runtime materialization format".to_string(),
            ));
        }
        if !is_sha256(&self.identity.runtime_context_hash)
            || !is_sha256(&self.identity.runtime_snapshot_hash)
            || !is_sha256(&self.identity.static_build_hash)
            || !is_sha256(&self.identity.static_artifact_hash)
            || !is_sha256(&self.identity.materialization_hash)
        {
            return Err(PageBuilderStaticLandingMaterializationError::Integrity(
                "static runtime materialization identity contains an invalid SHA-256 hash"
                    .to_string(),
            ));
        }
        if self.identity.static_build_hash != self.artifact.identity.build_hash
            || self.identity.static_artifact_hash != self.artifact.artifact_hash
        {
            return Err(PageBuilderStaticLandingMaterializationError::Integrity(
                "static runtime materialization identity does not match its Fly artifact"
                    .to_string(),
            ));
        }
        if self.runtime_snapshots.len() != self.artifact.pages.len() {
            return Err(PageBuilderStaticLandingMaterializationError::Integrity(
                "static runtime snapshot count does not match artifact pages".to_string(),
            ));
        }

        let expected_scenario_id =
            effective_scenario_id(self.identity.runtime_scenario_id.as_deref());
        for (page_index, (snapshot, page)) in self
            .runtime_snapshots
            .iter()
            .zip(self.artifact.pages.iter())
            .enumerate()
        {
            if !snapshot.is_valid_format() || !snapshot.is_renderable() {
                return Err(PageBuilderStaticLandingMaterializationError::Integrity(
                    format!("runtime snapshot for static page {page_index} is not renderable"),
                ));
            }
            if snapshot.selection != PageSelection::Index(page_index) || snapshot.cases.len() != 1 {
                return Err(PageBuilderStaticLandingMaterializationError::Integrity(
                    format!(
                        "runtime snapshot for static page {page_index} has an invalid selection or case count"
                    ),
                ));
            }
            let case = &snapshot.cases[0];
            let static_document_hash = ProjectHash::from_bytes(page.document_html.as_bytes()).hex();
            if case.scenario_id != expected_scenario_id
                || case.page_id != page.page_id
                || case.document_hash.as_deref() != Some(static_document_hash.as_str())
            {
                return Err(PageBuilderStaticLandingMaterializationError::Integrity(
                    format!(
                        "runtime snapshot for static page {page_index} does not match the materialized artifact"
                    ),
                ));
            }
        }

        let runtime_snapshot_hash = stable_hash(&self.runtime_snapshots)?;
        if runtime_snapshot_hash != self.identity.runtime_snapshot_hash {
            return Err(PageBuilderStaticLandingMaterializationError::Integrity(
                "static runtime snapshot hash mismatch".to_string(),
            ));
        }
        let materialization_hash = materialization_hash(
            &self.identity.runtime_context_hash,
            self.identity.runtime_scenario_id.as_deref(),
            &self.identity.runtime_snapshot_hash,
            &self.identity.static_build_hash,
            &self.identity.static_artifact_hash,
        )?;
        if materialization_hash != self.identity.materialization_hash {
            return Err(PageBuilderStaticLandingMaterializationError::Integrity(
                "static runtime materialization hash mismatch".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PageBuilderStaticLandingMaterializationError {
    #[error("invalid canonical preview runtime: {0}")]
    RuntimeContract(String),
    #[error("runtime materialization has blocking diagnostics: {codes:?}")]
    RuntimeDiagnostics { codes: Vec<String> },
    #[error(transparent)]
    Landing(#[from] LandingProjectError),
    #[error("static runtime materialization encoding failed: {0}")]
    Encode(String),
    #[error("static runtime materialization integrity failed: {0}")]
    Integrity(String),
}

pub fn compile_materialized_static_landing(
    project_data: &Value,
    runtime: PageBuilderPreviewRuntime,
) -> Result<
    PageBuilderMaterializedStaticLandingArtifact,
    PageBuilderStaticLandingMaterializationError,
> {
    runtime.validate().map_err(|error| {
        PageBuilderStaticLandingMaterializationError::RuntimeContract(error.to_string())
    })?;

    let compiler = StaticLandingCompiler::default();
    let document = compiler.prepare_document(project_data)?;
    let scenario_id = effective_scenario_id(runtime.scenario_id.as_deref());
    let scenario = RuntimeContextScenario::new(
        scenario_id.clone(),
        runtime
            .scenario_id
            .clone()
            .unwrap_or_else(|| "Static default".to_string()),
        runtime.context.clone(),
    );
    let runtime_snapshots = (0..document.project.pages.len())
        .map(|page_index| {
            RuntimeScenarioRenderSnapshot::capture_with_providers(
                &document,
                &PageSelection::Index(page_index),
                compiler.render_policy(),
                std::slice::from_ref(&scenario),
                &HostFormProviderEndpoints,
            )
        })
        .collect::<Vec<_>>();
    let mut blocking_codes = runtime_snapshots
        .iter()
        .flat_map(|snapshot| snapshot.matrix_diagnostics.iter())
        .filter(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
        .map(|diagnostic| diagnostic.code.clone())
        .collect::<Vec<_>>();

    let materialized = materialize_project_with_runtime_context_with_providers(
        &document,
        &runtime.context,
        &HostFormProviderEndpoints,
    );
    blocking_codes.extend(
        materialized
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
            .map(|diagnostic| diagnostic.code.clone()),
    );
    blocking_codes.sort();
    blocking_codes.dedup();
    if !blocking_codes.is_empty() {
        return Err(
            PageBuilderStaticLandingMaterializationError::RuntimeDiagnostics {
                codes: blocking_codes,
            },
        );
    }

    // These authoring definitions have already run against the reviewed context. Their source
    // targets may intentionally no longer exist after a repeater or condition. Keep them in the
    // build's source identity, but validate and render the lowered tree without running bindings
    // or provider routing a second time.
    let artifact = compiler.compile_materialized_document(&materialized.document)?;
    let runtime_context_hash = stable_hash(&runtime.context)?;
    let runtime_snapshot_hash = stable_hash(&runtime_snapshots)?;
    let static_build_hash = artifact.identity.build_hash.clone();
    let static_artifact_hash = artifact.artifact_hash.clone();
    let materialization_hash = materialization_hash(
        &runtime_context_hash,
        runtime.scenario_id.as_deref(),
        &runtime_snapshot_hash,
        &static_build_hash,
        &static_artifact_hash,
    )?;
    let result = PageBuilderMaterializedStaticLandingArtifact {
        identity: PageBuilderStaticLandingMaterializationIdentity {
            format: PAGE_BUILDER_STATIC_MATERIALIZATION_FORMAT.to_string(),
            runtime_context_hash,
            runtime_scenario_id: runtime.scenario_id,
            runtime_snapshot_hash,
            static_build_hash,
            static_artifact_hash,
            materialization_hash,
        },
        runtime_snapshots,
        artifact,
    };
    result.verify_integrity()?;
    Ok(result)
}

fn effective_scenario_id(scenario_id: Option<&str>) -> String {
    scenario_id
        .unwrap_or(DEFAULT_STATIC_RUNTIME_SCENARIO_ID)
        .to_string()
}

fn materialization_hash(
    runtime_context_hash: &str,
    runtime_scenario_id: Option<&str>,
    runtime_snapshot_hash: &str,
    static_build_hash: &str,
    static_artifact_hash: &str,
) -> Result<String, PageBuilderStaticLandingMaterializationError> {
    stable_hash(&(
        PAGE_BUILDER_STATIC_MATERIALIZATION_FORMAT,
        runtime_context_hash,
        runtime_scenario_id,
        runtime_snapshot_hash,
        static_build_hash,
        static_artifact_hash,
    ))
}

fn stable_hash(
    value: &impl Serialize,
) -> Result<String, PageBuilderStaticLandingMaterializationError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| PageBuilderStaticLandingMaterializationError::Encode(error.to_string()))?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::PageBuilderRenderer;
    use fly::{PageSelection, RenderPolicy};
    use serde_json::json;

    fn project() -> Value {
        json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": {
                    "title": "Home",
                    "description": "Runtime-bound landing",
                    "slug": "home"
                },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "title",
                        "type": "heading",
                        "tagName": "h1",
                        "content": "Static"
                    }]
                }
            }],
            "flyRuntimeBindings": [{
                "id": "title-content",
                "component_id": "title",
                "path": "page.title",
                "target": "field",
                "name": "content"
            }]
        })
    }

    fn project_with_runtime_bound_image() -> Value {
        json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": {
                    "title": "Home",
                    "description": "Runtime-bound landing",
                    "slug": "home"
                },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "title",
                        "type": "heading",
                        "tagName": "h1",
                        "content": "Welcome"
                    }, {
                        "id": "hero",
                        "type": "image",
                        "attributes": { "src": "https://cdn.example.com/default.webp" }
                    }]
                }
            }],
            "flyRuntimeBindings": [{
                "id": "hero-src",
                "component_id": "hero",
                "path": "asset.url",
                "target": "attribute",
                "name": "src"
            }]
        })
    }

    #[test]
    fn preview_and_static_artifact_share_fly_materialization_output() {
        let runtime = PageBuilderPreviewRuntime::new(
            json!({ "page": { "title": "Scenario title" } }),
            Some("landing-primary".to_string()),
        );
        let preview_html = PageBuilderRenderer
            .render_runtime_document_html(
                project(),
                PageSelection::First,
                RenderPolicy {
                    allow_http: false,
                    ..RenderPolicy::default()
                },
                runtime.context.clone(),
            )
            .expect("preview render");
        let result = compile_materialized_static_landing(&project(), runtime)
            .expect("materialized static artifact");
        let preview_document_hash = ProjectHash::from_bytes(preview_html.as_bytes()).hex();

        assert_eq!(result.artifact.pages[0].document_html, preview_html);
        assert_eq!(
            result.runtime_snapshots[0].cases[0]
                .document_hash
                .as_deref(),
            Some(preview_document_hash.as_str())
        );
        // Retain the previously materialized source in the build identity, including its
        // authoring definitions, so compatible retained artifacts remain reproducible.
        let prepared = StaticLandingCompiler::default()
            .prepare_document(&project())
            .unwrap();
        let lowered = materialize_project_with_runtime_context_with_providers(
            &prepared,
            &json!({ "page": { "title": "Scenario title" } }),
            &HostFormProviderEndpoints,
        );
        assert_eq!(
            result.artifact.identity.source_hash,
            stable_hash(&lowered.document).unwrap()
        );
        assert_eq!(result.identity.runtime_context_hash.len(), 64);
        assert_eq!(result.identity.runtime_snapshot_hash.len(), 64);
        assert_eq!(result.identity.materialization_hash.len(), 64);
        result
            .verify_integrity()
            .expect("materialization integrity");
    }

    #[test]
    fn context_and_scenario_are_part_of_materialization_identity() {
        let first = compile_materialized_static_landing(
            &project(),
            PageBuilderPreviewRuntime::new(
                json!({ "page": { "title": "First" } }),
                Some("scenario-a".to_string()),
            ),
        )
        .expect("first artifact");
        let second = compile_materialized_static_landing(
            &project(),
            PageBuilderPreviewRuntime::new(
                json!({ "page": { "title": "Second" } }),
                Some("scenario-b".to_string()),
            ),
        )
        .expect("second artifact");

        assert_ne!(
            first.identity.runtime_context_hash,
            second.identity.runtime_context_hash
        );
        assert_ne!(
            first.identity.runtime_snapshot_hash,
            second.identity.runtime_snapshot_hash
        );
        assert_ne!(
            first.identity.materialization_hash,
            second.identity.materialization_hash
        );
        assert_ne!(first.artifact.artifact_hash, second.artifact.artifact_hash);
    }

    #[test]
    fn runtime_bound_http_resource_is_rejected_before_static_artifact_creation() {
        let error = compile_materialized_static_landing(
            &project_with_runtime_bound_image(),
            PageBuilderPreviewRuntime::new(
                json!({ "asset": { "url": "http://cdn.example.com/runtime.webp" } }),
                Some("insecure-resource".to_string()),
            ),
        )
        .expect_err("materialized HTTP resource must be rejected");
        let PageBuilderStaticLandingMaterializationError::Landing(
            LandingProjectError::Validation { diagnostics },
        ) = error
        else {
            panic!("expected typed landing validation error");
        };
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "landing_insecure_resource_url"
                && diagnostic.path.ends_with("attributes.src")
        }));
    }

    #[test]
    fn static_publish_rejects_an_unexpanded_repeater_containing_a_form() {
        let project = json!({
            "pages": [{
                "id": "home", "flyPageMeta": {"title": "Home", "description": "Landing", "slug": "home"},
                "component": {"id": "root", "type": "wrapper", "components": [
                    {"id": "title", "type": "heading", "tagName": "h1", "content": "Home"},
                    {"id": "card", "type": "section", "components": [
                        {"id": "name", "type": "text", "content": "{{item.name}}"},
                        {"id": "form", "type": "form", "flyForm": {"id": "contact", "action_url": "/contact"}}
                    ]}
                ]}
            }],
            "flyRuntimeRepeaters": [{"id": "items", "component_id": "card", "path": "items"}]
        });
        let context = json!({"items": [{"name": "Apple"}, {"name": "Pear"}]});
        let document = fly::GrapesJsCodec::decode_value(project.clone()).unwrap();
        let readiness = crate::evaluate_landing_readiness_with_context(
            &document,
            Some(&context),
            crate::LandingReadinessPolicy::default(),
        );
        assert!(!readiness.ready);
        assert!(readiness.issues.iter().any(|issue| issue.diagnostic.code
            == "runtime_repeater_form_identity_ambiguous"
            && issue.diagnostic.severity == ValidationSeverity::Error));
        let gate = crate::evaluate_runtime_publish_gate(
            &document,
            Some(&context),
            &[],
            &crate::RuntimePublishGatePolicy {
                readiness: Some(crate::LandingReadinessPolicy::default()),
                ..Default::default()
            },
        );
        assert!(!gate.allowed);
        let error = compile_materialized_static_landing(
            &project,
            PageBuilderPreviewRuntime::new(context, Some("reviewed".to_string())),
        )
        .expect_err("unexpanded runtime templates must not become static artifacts");
        assert!(
            matches!(error, PageBuilderStaticLandingMaterializationError::RuntimeDiagnostics { codes }
            if codes.iter().any(|code| code == "runtime_repeater_form_identity_ambiguous"))
        );
    }

    #[test]
    fn static_publish_keeps_one_conditionally_visible_repeater_form() {
        let project = json!({
            "pages": [{
                "id": "home", "flyPageMeta": {"title": "Home", "description": "Landing", "slug": "home"},
                "component": {"id": "root", "type": "wrapper", "components": [
                    {"id": "title", "type": "heading", "tagName": "h1", "content": "Home"},
                    {"id": "card", "type": "section", "components": [
                        {"id": "name", "type": "text", "content": "{{item.name}}"},
                        {"id": "form", "type": "form", "flyForm": {"id": "contact", "provider": "forms", "action": "submit"}}
                    ]}
                ]}
            }],
            "flyRuntimeConditions": [{"id": "show-form", "component_id": "form", "path": "item.showForm", "operator": "truthy"}],
            "flyRuntimeRepeaters": [{"id": "items", "component_id": "card", "path": "items"}]
        });
        let context = json!({"items": [
            {"name": "Apple", "showForm": true}, {"name": "Pear", "showForm": false}
        ]});
        let result = compile_materialized_static_landing(
            &project,
            PageBuilderPreviewRuntime::new(context, Some("reviewed".to_string())),
        )
        .expect("the reviewed runtime emits exactly one provider form");
        let html = &result.artifact.pages[0].document_html;
        assert!(html.contains("Apple") && html.contains("Pear"), "{html}");
        assert!(!html.contains("{{item.name}}"), "{html}");
        assert_eq!(html.matches(" id=\"contact\"").count(), 1, "{html}");
        assert!(
            html.contains("action=\"/api/forms/contact/submit/form\""),
            "{html}"
        );
        assert_eq!(result.runtime_snapshots[0].cases[0].blocking_diagnostics, 0);
        result
            .verify_integrity()
            .expect("runtime snapshot matches published HTML");
    }

    #[test]
    fn static_publish_preserves_dynamic_ids_and_external_literal_links() {
        let project = json!({
            "pages": [{
                "id": "home", "flyPageMeta": {"title": "Home", "description": "Landing", "slug": "home"},
                "component": {"id": "root", "type": "wrapper", "components": [
                    {"id": "title", "type": "heading", "tagName": "h1", "content": "Home"},
                    {"id": "outside", "type": "section", "attributes": {"id": "item-apple"}},
                    {"id": "item", "type": "section", "components": [
                        {"id": "external-link", "type": "link", "attributes": {"href": "#item-apple"}, "content": "External"},
                        {"id": "local-link", "type": "link", "attributes": {"href": "#item-{{item.slug}}"}, "content": "Local"},
                        {"id": "target", "type": "section", "attributes": {"id": "item-{{item.slug}}"}}
                    ]}
                ]}
            }],
            "flyRuntimeRepeaters": [{"id": "items", "component_id": "item", "path": "items"}]
        });
        let result = compile_materialized_static_landing(
            &project,
            PageBuilderPreviewRuntime::new(
                json!({"items": [{"slug": "apple"}, {"slug": "apple"}, {"slug": "яблоко"}]}),
                None,
            ),
        )
        .expect("dynamic ids and references should survive publication");
        let html = &result.artifact.pages[0].document_html;
        assert_eq!(html.matches("href=\"#item-apple\"").count(), 3, "{html}");
        for suffix in ["0", "1"] {
            let id = format!("item-apple--items-{suffix}");
            assert!(html.contains(&format!("id=\"{id}\"")), "{html}");
            assert!(html.contains(&format!("href=\"#{id}\"")), "{html}");
        }
        assert!(html.contains("id=\"item-яблоко\""), "{html}");
        assert!(html.contains("href=\"#item-яблоко\""), "{html}");
        result
            .verify_integrity()
            .expect("runtime snapshot matches published HTML");
    }

    #[test]
    fn runtime_bindings_are_not_applied_again_to_the_static_artifact() {
        let project = json!({
            "pages": [{
                "id": "home", "flyPageMeta": {"title": "Home", "description": "Landing", "slug": "home"},
                "component": {"id": "root", "type": "wrapper", "components": [
                    {"id": "title", "type": "heading", "tagName": "h1", "content": "Draft"}
                ]}
            }],
            "flyRuntimeBindings": [{"id": "title-binding", "component_id": "title", "path": "title", "target": "field", "name": "content", "fallback": "Fallback"}]
        });
        let result = compile_materialized_static_landing(
            &project,
            PageBuilderPreviewRuntime::new(json!({"title": "Reviewed"}), None),
        )
        .expect("the reviewed context is the only binding input");
        let html = &result.artifact.pages[0].document_html;
        assert!(html.contains("Reviewed"), "{html}");
        assert!(!html.contains("Fallback"), "{html}");
        result
            .verify_integrity()
            .expect("preview and static output match");
    }

    #[test]
    fn static_publish_accepts_forms_with_distinct_materialized_ids() {
        let project = json!({
            "pages": [{
                "id": "home", "flyPageMeta": {"title": "Home", "description": "Landing", "slug": "home"},
                "component": {"id": "root", "type": "wrapper", "components": [
                    {"id": "title", "type": "heading", "tagName": "h1", "content": "Home"},
                    {"id": "first", "type": "form", "attributes": {"id": "lead"}, "flyForm": {"id": "signup", "action_url": "/signup"}},
                    {"id": "second", "type": "form", "flyForm": {"id": "lead", "action_url": "/lead"}}
                ]}
            }]
        });
        let result = compile_materialized_static_landing(
            &project,
            PageBuilderPreviewRuntime::new(json!({}), None),
        )
        .expect("the overwritten draft id is not an HTML conflict");
        let html = &result.artifact.pages[0].document_html;
        assert_eq!(html.matches(" id=\"signup\"").count(), 1);
        assert_eq!(html.matches(" id=\"lead\"").count(), 1);
        result.verify_integrity().expect("artifact integrity");
    }

    #[test]
    fn invalid_canonical_runtime_is_rejected_before_materialization() {
        let error = compile_materialized_static_landing(
            &project(),
            PageBuilderPreviewRuntime::new(json!(["invalid"]), None),
        )
        .expect_err("invalid runtime");
        assert!(matches!(
            error,
            PageBuilderStaticLandingMaterializationError::RuntimeContract(_)
        ));
    }
}
