use chrono::Utc;
use rustok_api::{Action, Resource};
use rustok_core::{CONTENT_FORMAT_GRAPESJS, SecurityContext};
use rustok_page_builder::runtime_scenario_release::{
    PAGE_BUILDER_SCENARIO_REGRESSION_BLOCKED_ERROR_CODE, PageBuilderRuntimeScenarioReleaseRequest,
    RuntimeScenarioReleaseBaseline, RuntimeScenarioReleaseEvaluation, RuntimeScenarioReleasePolicy,
    evaluate_page_builder_runtime_scenario_release,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, sea_query::Expr,
};
use serde_json::Value;
use uuid::Uuid;

use crate::entities::{page, page_body, page_builder_scenario_baseline};
use crate::error::{PagesError, PagesResult};
use crate::services::rbac::enforce_owned_scope;

pub const PAGE_BUILDER_SCENARIO_BASELINE_CONFLICT_ERROR_CODE: &str = "SCENARIO_BASELINE_CONFLICT";
pub const PAGE_BUILDER_SCENARIO_BASELINE_PROMOTION_NOTE_REQUIRED_ERROR_CODE: &str =
    "SCENARIO_BASELINE_PROMOTION_NOTE_REQUIRED";

#[derive(Clone, Debug)]
pub struct PageBuilderScenarioBaselineRecord {
    pub baseline: RuntimeScenarioReleaseBaseline,
    pub previous_baseline_hash: Option<String>,
    pub promoted_by: Option<Uuid>,
    pub promotion_note: Option<String>,
    pub promoted_at: Option<sea_orm::prelude::DateTimeWithTimeZone>,
}

pub struct PageBuilderScenarioBaselineService {
    db: DatabaseConnection,
}

pub struct SaveIfCurrentScenarioBaselineRequest {
    pub tenant_id: Uuid,
    pub security: SecurityContext,
    pub page_id: Uuid,
    pub baseline: RuntimeScenarioReleaseBaseline,
    pub expected_baseline_hash: Option<String>,
    pub promoted_by: Uuid,
    pub promotion_note: Option<String>,
}

struct SaveScenarioBaselineRequest {
    tenant_id: Uuid,
    security: SecurityContext,
    page_id: Uuid,
    baseline: RuntimeScenarioReleaseBaseline,
    expected_baseline_hash: Option<String>,
    enforce_expected_state: bool,
    promoted_by: Option<Uuid>,
    promotion_note: Option<String>,
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
        Ok(self
            .get_record(tenant_id, security, page_id)
            .await?
            .map(|record| record.baseline))
    }

    pub async fn get_record(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<Option<PageBuilderScenarioBaselineRecord>> {
        let page = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(&security, Resource::Pages, Action::Read, page.author_id)?;
        self.load_record_unchecked(tenant_id, page_id).await
    }

    pub async fn save(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        baseline: RuntimeScenarioReleaseBaseline,
    ) -> PagesResult<RuntimeScenarioReleaseBaseline> {
        self.save_internal(SaveScenarioBaselineRequest {
            tenant_id,
            security,
            page_id,
            baseline,
            expected_baseline_hash: None,
            enforce_expected_state: false,
            promoted_by: None,
            promotion_note: None,
        })
        .await
    }

    pub async fn save_if_current(
        &self,
        request: SaveIfCurrentScenarioBaselineRequest,
    ) -> PagesResult<RuntimeScenarioReleaseBaseline> {
        self.save_internal(SaveScenarioBaselineRequest {
            tenant_id: request.tenant_id,
            security: request.security,
            page_id: request.page_id,
            baseline: request.baseline,
            expected_baseline_hash: request.expected_baseline_hash,
            enforce_expected_state: true,
            promoted_by: Some(request.promoted_by),
            promotion_note: request.promotion_note,
        })
        .await
    }

    async fn save_internal(
        &self,
        request: SaveScenarioBaselineRequest,
    ) -> PagesResult<RuntimeScenarioReleaseBaseline> {
        let SaveScenarioBaselineRequest {
            tenant_id,
            security,
            page_id,
            baseline,
            expected_baseline_hash,
            enforce_expected_state,
            promoted_by,
            promotion_note,
        } = request;
        let page = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(&security, Resource::Pages, Action::Update, page.author_id)?;
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

        let now: sea_orm::prelude::DateTimeWithTimeZone = Utc::now().into();
        let baseline_json = serde_json::to_value(&baseline).map_err(|error| {
            PagesError::validation(format!("Unable to encode scenario baseline: {error}"))
        })?;
        let existing = page_builder_scenario_baseline::Entity::find()
            .filter(page_builder_scenario_baseline::Column::TenantId.eq(tenant_id))
            .filter(page_builder_scenario_baseline::Column::PageId.eq(page_id))
            .one(&self.db)
            .await?;
        let promotion_note = normalized_promotion_note(promotion_note.as_deref());
        if enforce_expected_state && existing.is_some() && promotion_note.is_none() {
            return Err(PagesError::validation(format!(
                "{PAGE_BUILDER_SCENARIO_BASELINE_PROMOTION_NOTE_REQUIRED_ERROR_CODE}: replacing an existing scenario baseline requires a review note"
            )));
        }

        match (existing, expected_baseline_hash.as_deref()) {
            (Some(existing), Some(expected_hash)) => {
                let previous_hash = existing.baseline_hash.clone();
                let result = page_builder_scenario_baseline::Entity::update_many()
                    .col_expr(
                        page_builder_scenario_baseline::Column::BaselineId,
                        Expr::value(baseline.baseline_id.clone()),
                    )
                    .col_expr(
                        page_builder_scenario_baseline::Column::BaselineHash,
                        Expr::value(baseline.baseline_hash.clone()),
                    )
                    .col_expr(
                        page_builder_scenario_baseline::Column::SourceProjectHash,
                        Expr::value(baseline.source_project_hash.clone()),
                    )
                    .col_expr(
                        page_builder_scenario_baseline::Column::Baseline,
                        Expr::value(baseline_json),
                    )
                    .col_expr(
                        page_builder_scenario_baseline::Column::PreviousBaselineHash,
                        Expr::value(previous_hash),
                    )
                    .col_expr(
                        page_builder_scenario_baseline::Column::PromotedBy,
                        Expr::value(promoted_by),
                    )
                    .col_expr(
                        page_builder_scenario_baseline::Column::PromotionNote,
                        Expr::value(promotion_note.clone()),
                    )
                    .col_expr(
                        page_builder_scenario_baseline::Column::PromotedAt,
                        Expr::value(now),
                    )
                    .col_expr(
                        page_builder_scenario_baseline::Column::UpdatedAt,
                        Expr::value(now),
                    )
                    .filter(page_builder_scenario_baseline::Column::TenantId.eq(tenant_id))
                    .filter(page_builder_scenario_baseline::Column::PageId.eq(page_id))
                    .filter(page_builder_scenario_baseline::Column::BaselineHash.eq(expected_hash))
                    .exec(&self.db)
                    .await?;
                if result.rows_affected != 1 {
                    return Err(baseline_conflict(page_id));
                }
            }
            (Some(_), None) if enforce_expected_state => {
                return Err(baseline_conflict(page_id));
            }
            (Some(existing), None) => {
                let previous_hash = existing.baseline_hash.clone();
                let mut active: page_builder_scenario_baseline::ActiveModel = existing.into();
                active.baseline_id = Set(baseline.baseline_id.clone());
                active.baseline_hash = Set(baseline.baseline_hash.clone());
                active.source_project_hash = Set(baseline.source_project_hash.clone());
                active.baseline = Set(baseline_json);
                active.previous_baseline_hash = Set(Some(previous_hash));
                active.promoted_by = Set(promoted_by);
                active.promotion_note = Set(promotion_note);
                active.promoted_at = Set(Some(now));
                active.updated_at = Set(now);
                active.update(&self.db).await?;
            }
            (None, Some(_)) => return Err(baseline_conflict(page_id)),
            (None, None) => {
                let insert = page_builder_scenario_baseline::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(tenant_id),
                    page_id: Set(page_id),
                    baseline_id: Set(baseline.baseline_id.clone()),
                    baseline_hash: Set(baseline.baseline_hash.clone()),
                    source_project_hash: Set(baseline.source_project_hash.clone()),
                    baseline: Set(baseline_json),
                    previous_baseline_hash: Set(None),
                    promoted_by: Set(promoted_by),
                    promotion_note: Set(promotion_note),
                    promoted_at: Set(Some(now)),
                    created_at: Set(now),
                    updated_at: Set(now),
                }
                .insert(&self.db)
                .await;
                if let Err(error) = insert {
                    if enforce_expected_state {
                        return Err(baseline_conflict(page_id));
                    }
                    return Err(error.into());
                }
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
        self.delete_internal(tenant_id, security, page_id, None, false)
            .await
    }

    pub async fn delete_if_current(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        expected_baseline_hash: Option<&str>,
    ) -> PagesResult<bool> {
        self.delete_internal(tenant_id, security, page_id, expected_baseline_hash, true)
            .await
    }

    async fn delete_internal(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        expected_baseline_hash: Option<&str>,
        enforce_expected_state: bool,
    ) -> PagesResult<bool> {
        let page = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(&security, Resource::Pages, Action::Update, page.author_id)?;

        if enforce_expected_state && expected_baseline_hash.is_none() {
            let exists = page_builder_scenario_baseline::Entity::find()
                .filter(page_builder_scenario_baseline::Column::TenantId.eq(tenant_id))
                .filter(page_builder_scenario_baseline::Column::PageId.eq(page_id))
                .one(&self.db)
                .await?
                .is_some();
            return if exists {
                Err(baseline_conflict(page_id))
            } else {
                Ok(false)
            };
        }

        let mut delete = page_builder_scenario_baseline::Entity::delete_many()
            .filter(page_builder_scenario_baseline::Column::TenantId.eq(tenant_id))
            .filter(page_builder_scenario_baseline::Column::PageId.eq(page_id));
        if let Some(expected_hash) = expected_baseline_hash {
            delete = delete
                .filter(page_builder_scenario_baseline::Column::BaselineHash.eq(expected_hash));
        }
        let result = delete.exec(&self.db).await?;
        if enforce_expected_state && result.rows_affected != 1 {
            return Err(baseline_conflict(page_id));
        }
        Ok(result.rows_affected > 0)
    }

    pub async fn evaluate_publish(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> PagesResult<Option<RuntimeScenarioReleaseEvaluation>> {
        let Some(record) = self.load_record_unchecked(tenant_id, page_id).await? else {
            return Ok(None);
        };
        let body = page_body::Entity::find()
            .filter(page_body::Column::PageId.eq(page_id))
            .filter(page_body::Column::Format.eq(CONTENT_FORMAT_GRAPESJS))
            .order_by_desc(page_body::Column::UpdatedAt)
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                PagesError::validation(
                    "Page Builder scenario baseline exists but no grapesjs body is available",
                )
            })?;
        let project_data = serde_json::from_str(&body.content).map_err(|error| {
            PagesError::validation(format!(
                "Stored Page Builder project is not valid JSON: {error}"
            ))
        })?;
        self.evaluate_candidate_with_baseline(project_data, record.baseline)
            .map(Some)
    }

    pub async fn evaluate_candidate(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
        project_data: Value,
    ) -> PagesResult<Option<RuntimeScenarioReleaseEvaluation>> {
        let Some(record) = self.load_record_unchecked(tenant_id, page_id).await? else {
            return Ok(None);
        };
        self.evaluate_candidate_with_baseline(project_data, record.baseline)
            .map(Some)
    }

    pub async fn ensure_publish_allowed(&self, tenant_id: Uuid, page_id: Uuid) -> PagesResult<()> {
        let Some(record) = self.load_record_unchecked(tenant_id, page_id).await? else {
            return Ok(());
        };
        let bodies = page_body::Entity::find()
            .filter(page_body::Column::PageId.eq(page_id))
            .filter(page_body::Column::Format.eq(CONTENT_FORMAT_GRAPESJS))
            .all(&self.db)
            .await?;
        if bodies.is_empty() {
            return Err(PagesError::validation(
                "Page Builder scenario baseline exists but no grapesjs body is available",
            ));
        }
        for body in bodies {
            let project_data = serde_json::from_str(&body.content).map_err(|error| {
                PagesError::validation(format!(
                    "Stored Page Builder project for locale `{}` is not valid JSON: {error}",
                    body.locale
                ))
            })?;
            let evaluation =
                self.evaluate_candidate_with_baseline(project_data, record.baseline.clone())?;
            ensure_evaluation_allowed(evaluation)?;
        }
        Ok(())
    }

    pub async fn ensure_candidate_allowed(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
        project_data: Value,
    ) -> PagesResult<()> {
        match self
            .evaluate_candidate(tenant_id, page_id, project_data)
            .await?
        {
            Some(evaluation) => ensure_evaluation_allowed(evaluation),
            None => Ok(()),
        }
    }

    pub async fn ensure_candidates_allowed(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
        project_data: Vec<Value>,
    ) -> PagesResult<()> {
        let Some(record) = self.load_record_unchecked(tenant_id, page_id).await? else {
            return Ok(());
        };
        for candidate in project_data {
            let evaluation =
                self.evaluate_candidate_with_baseline(candidate, record.baseline.clone())?;
            ensure_evaluation_allowed(evaluation)?;
        }
        Ok(())
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
        evaluate_page_builder_runtime_scenario_release(PageBuilderRuntimeScenarioReleaseRequest {
            project_data,
            baseline: Some(baseline),
            policy: RuntimeScenarioReleasePolicy::block_broken(),
        })
        .map(|response| response.evaluation)
        .map_err(|error| PagesError::validation(error.to_string()))
    }

    async fn load_record_unchecked(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> PagesResult<Option<PageBuilderScenarioBaselineRecord>> {
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
        Ok(Some(PageBuilderScenarioBaselineRecord {
            baseline,
            previous_baseline_hash: model.previous_baseline_hash,
            promoted_by: model.promoted_by,
            promotion_note: model.promotion_note,
            promoted_at: model.promoted_at,
        }))
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

fn normalized_promotion_note(note: Option<&str>) -> Option<String> {
    note.map(str::trim)
        .filter(|note| !note.is_empty())
        .map(ToString::to_string)
}

fn baseline_conflict(page_id: Uuid) -> PagesError {
    PagesError::validation(format!(
        "{PAGE_BUILDER_SCENARIO_BASELINE_CONFLICT_ERROR_CODE}: scenario baseline for page `{page_id}` changed since it was loaded"
    ))
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
