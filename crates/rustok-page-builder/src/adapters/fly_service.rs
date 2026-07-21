use super::FlyProjectInspection;
use crate::preview_port::PageBuilderPreviewRenderingPort;
use crate::runtime_scenario_release::{
    release_gate_error, NoopPageBuilderScenarioBaselineStore, PageBuilderScenarioBaselineStore,
};
use crate::runtime_telemetry::{
    NoopPageBuilderRuntimeTelemetry, PageBuilderRuntimeCallEvidence, PageBuilderRuntimeTelemetry,
};
use crate::service::{
    PageBuilderCapabilityService, PageBuilderProjectStore, PageBuilderServiceError,
    PageBuilderServiceResult,
};
use async_trait::async_trait;
use fly::{
    evaluate_runtime_scenario_release, RegistrySet, RuntimeScenarioReleaseMode,
    RuntimeScenarioReleasePolicy, ValidationLimits,
};
use rustok_api::PortContext;
use serde_json::Value;

/// Fly-backed current provider that keeps the existing storage/rendering ports while making Fly
/// authoritative for project decode, structural validation, layers traversal, component lookup,
/// and optional runtime-scenario release gating.
pub struct FlyAdapterBackedPageBuilderService<
    S,
    R,
    T = NoopPageBuilderRuntimeTelemetry,
    B = NoopPageBuilderScenarioBaselineStore,
> {
    store: S,
    renderer: R,
    telemetry: T,
    baseline_store: B,
    release_policy: RuntimeScenarioReleasePolicy,
    registries: RegistrySet,
    limits: ValidationLimits,
}

impl<S, R>
    FlyAdapterBackedPageBuilderService<
        S,
        R,
        NoopPageBuilderRuntimeTelemetry,
        NoopPageBuilderScenarioBaselineStore,
    >
{
    pub fn new(store: S, renderer: R) -> Self {
        Self {
            store,
            renderer,
            telemetry: NoopPageBuilderRuntimeTelemetry,
            baseline_store: NoopPageBuilderScenarioBaselineStore,
            release_policy: RuntimeScenarioReleasePolicy::disabled(),
            registries: RegistrySet::with_builtins(),
            limits: ValidationLimits::default(),
        }
    }
}

impl<S, R, T> FlyAdapterBackedPageBuilderService<S, R, T, NoopPageBuilderScenarioBaselineStore> {
    pub fn with_telemetry(store: S, renderer: R, telemetry: T) -> Self {
        Self {
            store,
            renderer,
            telemetry,
            baseline_store: NoopPageBuilderScenarioBaselineStore,
            release_policy: RuntimeScenarioReleasePolicy::disabled(),
            registries: RegistrySet::with_builtins(),
            limits: ValidationLimits::default(),
        }
    }
}

impl<S, R, T, B> FlyAdapterBackedPageBuilderService<S, R, T, B> {
    pub fn with_policy(mut self, registries: RegistrySet, limits: ValidationLimits) -> Self {
        self.registries = registries;
        self.limits = limits;
        self
    }

    pub fn with_scenario_release_gate<B2>(
        self,
        baseline_store: B2,
        release_policy: RuntimeScenarioReleasePolicy,
    ) -> FlyAdapterBackedPageBuilderService<S, R, T, B2> {
        FlyAdapterBackedPageBuilderService {
            store: self.store,
            renderer: self.renderer,
            telemetry: self.telemetry,
            baseline_store,
            release_policy,
            registries: self.registries,
            limits: self.limits,
        }
    }

    fn inspect(&self, project_data: &Value) -> PageBuilderServiceResult<FlyProjectInspection> {
        let inspection =
            FlyProjectInspection::decode_with(project_data, &self.registries, self.limits)?;
        inspection.require_valid()?;
        Ok(inspection)
    }
}

#[async_trait]
impl<S, R, T, B> PageBuilderCapabilityService for FlyAdapterBackedPageBuilderService<S, R, T, B>
where
    S: PageBuilderProjectStore,
    R: PageBuilderPreviewRenderingPort,
    T: PageBuilderRuntimeTelemetry,
    B: PageBuilderScenarioBaselineStore,
{
    async fn preview(
        &self,
        context: &PortContext,
        input: crate::dto::PreviewPageBuilderInput,
    ) -> PageBuilderServiceResult<crate::dto::PreviewPageBuilderResult> {
        self.inspect(&input.project_data)?;
        input
            .runtime
            .validate()
            .map_err(|error| PageBuilderServiceError::Validation(error.to_string()))?;
        let evidence = PageBuilderRuntimeCallEvidence::render_preview(context, &input.page_id);
        self.telemetry.record_runtime_call(&evidence);
        let html = match self.renderer.render_preview(context, &input).await {
            Ok(html) => {
                self.telemetry.record_runtime_call(&evidence.succeeded());
                html
            }
            Err(error) => {
                self.telemetry.record_runtime_call(&evidence.failed(&error));
                return Err(error);
            }
        };

        Ok(crate::dto::PreviewPageBuilderResult {
            page_id: input.page_id,
            html,
            runtime_scenario_id: input.runtime.scenario_id,
        })
    }

    async fn tree(
        &self,
        context: &PortContext,
        input: crate::dto::BuilderTreeInput,
    ) -> PageBuilderServiceResult<crate::dto::BuilderTreeResult> {
        if input.page_id.trim().is_empty() {
            return Err(PageBuilderServiceError::Validation(
                "page_id must not be empty".to_string(),
            ));
        }
        let evidence = PageBuilderRuntimeCallEvidence::load_project(context, &input.page_id);
        self.telemetry.record_runtime_call(&evidence);
        let project_data = match self.store.load_project(context, &input.page_id).await {
            Ok(project_data) => {
                self.telemetry.record_runtime_call(&evidence.succeeded());
                project_data
            }
            Err(error) => {
                self.telemetry.record_runtime_call(&evidence.failed(&error));
                return Err(error);
            }
        };
        let nodes = match project_data {
            Some(project_data) => self.inspect(&project_data)?.tree_nodes(),
            None => Vec::new(),
        };

        Ok(crate::dto::BuilderTreeResult {
            page_id: input.page_id,
            nodes,
        })
    }

    async fn properties(
        &self,
        context: &PortContext,
        input: crate::dto::BuilderNodePropertiesInput,
    ) -> PageBuilderServiceResult<crate::dto::BuilderNodePropertiesResult> {
        if input.page_id.trim().is_empty() || input.node_id.trim().is_empty() {
            return Err(PageBuilderServiceError::Validation(
                "page_id and node_id must not be empty".to_string(),
            ));
        }
        if !input.properties.is_object() {
            return Err(PageBuilderServiceError::Validation(
                "properties must be a JSON object".to_string(),
            ));
        }

        let evidence = PageBuilderRuntimeCallEvidence::load_project(context, &input.page_id);
        self.telemetry.record_runtime_call(&evidence);
        if let Some(project_data) = match self.store.load_project(context, &input.page_id).await {
            Ok(project_data) => {
                self.telemetry.record_runtime_call(&evidence.succeeded());
                project_data
            }
            Err(error) => {
                self.telemetry.record_runtime_call(&evidence.failed(&error));
                return Err(error);
            }
        } {
            self.inspect(&project_data)?
                .component_properties(&input.node_id)?;
        }

        Ok(crate::dto::BuilderNodePropertiesResult {
            page_id: input.page_id,
            node_id: input.node_id,
            properties: input.properties,
        })
    }

    async fn publish(
        &self,
        context: &PortContext,
        input: crate::dto::PublishPageBuilderInput,
    ) -> PageBuilderServiceResult<crate::dto::PublishPageBuilderResult> {
        if input.revision_id.trim().is_empty() {
            return Err(PageBuilderServiceError::Validation(
                "revision_id must not be empty".to_string(),
            ));
        }
        let inspection = self.inspect(&input.project_data)?;
        if self.release_policy.mode != RuntimeScenarioReleaseMode::Disabled {
            let baseline = self
                .baseline_store
                .load_scenario_baseline(context, &input.page_id)
                .await?;
            let evaluation = evaluate_runtime_scenario_release(
                inspection.document(),
                baseline.as_ref(),
                self.release_policy,
            );
            if !evaluation.allowed {
                return Err(release_gate_error(&evaluation));
            }
        }

        let evidence = PageBuilderRuntimeCallEvidence::save_project(
            context,
            &input.page_id,
            &input.revision_id,
        );
        self.telemetry.record_runtime_call(&evidence);
        let saved = match self
            .store
            .save_project(
                context,
                &input.page_id,
                &input.revision_id,
                input.project_data,
            )
            .await
        {
            Ok(saved) => saved,
            Err(error) => {
                self.telemetry.record_runtime_call(&evidence.failed(&error));
                return Err(error);
            }
        };

        if saved.page_id != input.page_id {
            let error = PageBuilderServiceError::Runtime(format!(
                "project store persisted page `{}`, expected `{}`",
                saved.page_id, input.page_id
            ));
            self.telemetry.record_runtime_call(&evidence.failed(&error));
            return Err(error);
        }
        if saved.revision_id.trim().is_empty() {
            let error = PageBuilderServiceError::Runtime(
                "project store returned an empty persisted revision".to_string(),
            );
            self.telemetry.record_runtime_call(&evidence.failed(&error));
            return Err(error);
        }
        self.telemetry.record_runtime_call(&evidence.succeeded());

        Ok(crate::dto::PublishPageBuilderResult {
            page_id: saved.page_id,
            revision_id: saved.revision_id,
            published: saved.published,
        })
    }
}
