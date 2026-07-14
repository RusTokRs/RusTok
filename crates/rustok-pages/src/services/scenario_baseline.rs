use chrono::Utc;
use rustok_api::{Action, Resource};
use rustok_core::{SecurityContext, CONTENT_FORMAT_GRAPESJS_V1};
use rustok_page_builder::runtime_scenario_release::{
    evaluate_page_builder_runtime_scenario_release, PageBuilderRuntimeScenarioReleaseRequest,
    RuntimeScenarioReleaseBaseline, RuntimeScenarioReleaseEvaluation,
    RuntimeScenarioReleasePolicy, PAGE_BUILDER_SCENARIO_REGRESSION_BLOCKED_ERROR_CODE,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder,
};
use serde_json::Value;
use uuid::Uuid;

use crate::entities::{page, page_body, page_builder_scenario_baseline};
use crate::error::{PagesError, PagesResult};
use crate::services::rbac::enforce_owned_scope;

pub struct PageBuilderScenarioBaselineService {
    db: DatabaseConnection,
}

impl PageBuilderScenarioBaselineService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<Option<RuntimeScenarioReleaseBaseline>> {
        let page = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Read,
            page.author_id,
        )?;
        self.load_unchecked(tenant_id, page_id).await
    }

    pub async fn save(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        baseline: RuntimeScenarioReleaseBaseline,
    ) -> PagesResult<RuntimeScenarioReleaseBaseline> {
        let page = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Update,
            page.author_id,
        )?;
        let diagnostics = baseline.validate();
        if !diagnostics.is_empty() {
            return Err(PagesError::validation(format!(
                "Invalid Page Builder scenario release baseline: {}",
                diagnostics
                    .into_iter()
                    .take(4)
                    .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
                    .collect::<Vec<_>>()
                    .join("; ")
            )));
        }

        let now = Utc::now();
        let baseline_json = serde_json::to_value(&baseline).map_err(|error| {
            PagesError::validation(format!("Unable to encode scenario baseline: {error}"))
        })?;
        match page_builder_scenario_baseline::Entity::find()
            .filter(page_builder_scenario_baseline::Column::TenantId.eq(tenant_id))
            .filter(page_builder_scenario_baseline::Column::PageId.eq(page_id))
            .one(&self.db)
            .await?
        {
            Some(existing) => {
                let mut active: page_builder_scenario_baseline::ActiveModel = existing.into();
                active.baseline_id = Set(baseline.baseline_id.clone());
                active.baseline_hash = Set(baseline.baseline_hash.clone());
                active.source_project_hash = Set(baseline.source_project_hash.clone());
                active.baseline = Set(baseline_json);
                active.updated_at = Set(now.into());
                active.update(&self.db).await?;
            }
            None => {
                page_builder_scenario_baseline::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(tenant_id),
                    page_id: Set(page_id),
                    baseline_id: Set(baseline.baseline_id.clone()),
                    baseline_hash: Set(baseline.baseline_hash.clone()),
                    source_project_hash: Set(baseline.source_project_hash.clone()),
                    baseline: Set(baseline_json),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&self.db)
                .await?;
            }
        }
        Ok(baseline)
    }

    pub async fn delete(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<bool> {
        let page = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Update,
            page.author_id,
        )?;
        let result = page_builder_scenario_baseline::Entity::delete_many()
            .filter(page_builder_scenario_baseline::Column::TenantId.eq(tenant_id))
            .filter(page_builder_scenario_baseline::Column::PageId.eq(page_id))
            .exec(&self.db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    pub async fn evaluate_publish(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> PagesResult<Option<RuntimeScenarioReleaseEvaluation>> {
        let Some(baseline) = self.load_unchecked(tenant_id, page_id).await? else {
            return Ok(None);
        };
        let body = page_body::Entity::find()
            .filter(page_body::Column::PageId.eq(page_id))
            .filter(page_body::Column::Format.eq(CONTENT_FORMAT_GRAPESJS_V1))
            .order_by_desc(page_body::Column::UpdatedAt)
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                PagesError::validation(
                    "Page Builder scenario baseline exists but no grapesjs_v1 body is available",
                )
            })?;
        let project_data = serde_json::from_str(&body.content).map_err(|error| {
            PagesError::validation(format!(
                "Stored Page Builder project is not valid JSON: {error}"
            ))
        })?;
        self.evaluate_candidate_with_baseline(project_data, baseline)
            .map(Some)
    }

    pub async fn evaluate_candidate(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
        project_data: Value,
    ) -> PagesResult<Option<RuntimeScenarioReleaseEvaluation>> {
        let Some(baseline) = self.load_unchecked(tenant_id, page_id).await? else {
            return Ok(None);
        };
        self.evaluate_candidate_with_baseline(project_data, baseline)
            .map(Some)
    }

    pub async fn ensure_publish_allowed(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> PagesResult<()> {
        match self.evaluate_publish(tenant_id, page_id).await? {
            Some(evaluation) => ensure_evaluation_allowed(evaluation),
            None => Ok(()),
        }
    }

    pub async fn ensure_published_candidate_allowed(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
        project_data: Value,
    ) -> PagesResult<()> {
        let page = self.find_page(tenant_id, page_id).await?;
        if page.status != "published" {
            return Ok(());
        }
        match self
            .evaluate_candidate(tenant_id, page_id, project_data)
            .await?
        {
            Some(evaluation) => ensure_evaluation_allowed(evaluation),
            None => Ok(()),
        }
    }

    fn evaluate_candidate_with_baseline(
        &self,
        project_data: Value,
        baseline: RuntimeScenarioReleaseBaseline,
    ) -> PagesResult<RuntimeScenarioReleaseEvaluation> {
        evaluate_page_builder_runtime_scenario_release(
            PageBuilderRuntimeScenarioReleaseRequest {
                project_data,
                baseline: Some(baseline),
                policy: RuntimeScenarioReleasePolicy::block_broken(),
            },
        )
        .map(|response| response.evaluation)
        .map_err(|error| PagesError::validation(error.to_string()))
    }

    async fn load_unchecked(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> PagesResult<Option<RuntimeScenarioReleaseBaseline>> {
        let Some(model) = page_builder_scenario_baseline::Entity::find()
            .filter(page_builder_scenario_baseline::Column::TenantId.eq(tenant_id))
            .filter(page_builder_scenario_baseline::Column::PageId.eq(page_id))
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };
        let baseline: RuntimeScenarioReleaseBaseline = serde_json::from_value(model.baseline)
            .map_err(|error| {
                PagesError::validation(format!(
                    "Stored Page Builder scenario baseline is invalid: {error}"
                ))
            })?;
        let diagnostics = baseline.validate();
        if !diagnostics.is_empty()
            || baseline.baseline_hash != model.baseline_hash
            || baseline.source_project_hash != model.source_project_hash
            || baseline.baseline_id != model.baseline_id
        {
            return Err(PagesError::validation(
                "Stored Page Builder scenario baseline failed integrity validation",
            ));
        }
        Ok(Some(baseline))
    }

    async fn find_page(&self, tenant_id: Uuid, page_id: Uuid) -> PagesResult<page::Model> {
        page::Entity::find()
            .filter(page::Column::Id.eq(page_id))
            .filter(page::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| PagesError::page_not_found(page_id))
    }
}

fn ensure_evaluation_allowed(evaluation: RuntimeScenarioReleaseEvaluation) -> PagesResult<()> {
    if evaluation.allowed {
        return Ok(());
    }
    let details = evaluation
        .blocking_diagnostics()
        .take(4)
        .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
        .collect::<Vec<_>>()
        .join("; ");
    Err(PagesError::validation(format!(
        "{PAGE_BUILDER_SCENARIO_REGRESSION_BLOCKED_ERROR_CODE}: {}",
        if details.is_empty() {
            format!("release status {:?} is not allowed", evaluation.status)
        } else {
            details
        }
    )))
}
