use crate::dto::PageBuilderPreviewRuntime;
use crate::landing::LandingProjectError;
use crate::static_landing::StaticLandingCompiler;
use fly::{
    materialize_project_with_runtime_context, PageSelection, ProjectHash, RuntimeContextScenario,
    RuntimeScenarioRenderSnapshot, StaticLandingArtifact, ValidationSeverity,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
        self.artifact
            .verify_integrity()
            .map_err(|error| PageBuilderStaticLandingMaterializationError::Integrity(error.to_string()))?;
        if self.identity.format != PAGE_BUILDER_STATIC_MATERIALIZATION_FORMAT {
            return Err(PageBuilderStaticLandingMaterializationError::Integrity(
                "unsupported static runtime materialization format".to_string(),
            ));
        }
        if !is_sha256(&self.identity.runtime_context_hash)
            || !is_sha256(&self.identity.runtime_snapshot_hash)
            || !is_sha256(&self.identity.materialization_hash)
        {
            return Err(PageBuilderStaticLandingMaterializationError::Integrity(
                "static runtime materialization identity contains an invalid hash".to_string(),
            ));
        }
        if self.identity.static_build_hash != self.artifact.identity.build_hash
            || self.identity.static_artifact_hash != self.artifact.artifact_hash
        {
            return Err(PageBuilderStaticLandingMaterializationError::Integrity(
                "static runtime materialization identity does not match its Fly artifact".to_string(),
            ));
        }
        if self.runtime_snapshots.len() != self.artifact.pages.len() {
            return Err(PageBuilderStaticLandingMaterializationError::Integrity(
                "static runtime snapshot count does not match artifact pages".to_string(),
            ));
        }

        let expected_scenario_id = effective_scenario_id(self.identity.runtime_scenario_id.as_deref());
        for (page_index, (snapshot, page)) in self
            .runtime_snapshots
            .iter()
            .zip(self.artifact.pages.iter())
            .enumerate()
        {
            if !snapshot.is_valid_format() || !snapshot.is_renderable() {
                return Err(PageBuilderStaticLandingMaterializationError::Integrity(format!(
                    "runtime snapshot for static page {page_index} is not renderable"
                )));
            }
            if snapshot.selection != PageSelection::Index(page_index) || snapshot.cases.len() != 1 {
                return Err(PageBuilderStaticLandingMaterializationError::Integrity(format!(
                    "runtime snapshot for static page {page_index} has an invalid selection or case count"
                )));
            }
            let case = &snapshot.cases[0];
            if case.scenario_id != expected_scenario_id
                || case.page_id != page.page_id
                || case.document_hash.as_deref() != Some(page.content_hash.as_str())
            {
                return Err(PageBuilderStaticLandingMaterializationError::Integrity(format!(
                    "runtime snapshot for static page {page_index} does not match the materialized artifact"
                )));
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
) -> Result<PageBuilderMaterializedStaticLandingArtifact, PageBuilderStaticLandingMaterializationError>
{
    runtime
        .validate()
        .map_err(|error| PageBuilderStaticLandingMaterializationError::RuntimeContract(error.to_string()))?;

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
            RuntimeScenarioRenderSnapshot::capture(
                &document,
                &PageSelection::Index(page_index),
                compiler.render_policy(),
                std::slice::from_ref(&scenario),
            )
        })
        .collect::<Vec<_>>();
    let mut blocking_codes = runtime_snapshots
        .iter()
        .flat_map(|snapshot| snapshot.matrix_diagnostics.iter())
        .filter(|diagnostic| diagnostic.severity == ValidationSeverity::Error)
        .map(|diagnostic| diagnostic.code.clone())
        .collect::<Vec<_>>();

    let materialized = materialize_project_with_runtime_context(&document, &runtime.context);
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
        return Err(PageBuilderStaticLandingMaterializationError::RuntimeDiagnostics {
            codes: blocking_codes,
        });
    }

    let artifact = compiler.compile_prepared_document(&materialized.document)?;
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
    Ok(ProjectHash::from_bytes(&bytes).hex())
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

        assert_eq!(result.artifact.pages[0].document_html, preview_html);
        assert_eq!(
            result.runtime_snapshots[0].cases[0].document_hash.as_deref(),
            Some(result.artifact.pages[0].content_hash.as_str())
        );
        result.verify_integrity().expect("materialization integrity");
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

        assert_ne!(first.identity.runtime_context_hash, second.identity.runtime_context_hash);
        assert_ne!(first.identity.runtime_snapshot_hash, second.identity.runtime_snapshot_hash);
        assert_ne!(first.identity.materialization_hash, second.identity.materialization_hash);
        assert_ne!(first.artifact.artifact_hash, second.artifact.artifact_hash);
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
