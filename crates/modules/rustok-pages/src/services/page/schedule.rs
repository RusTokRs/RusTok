//! Scheduled page publishing: a Pages-owned job queue around `publish_reviewed`.
//!
//! Scheduling captures the exact reviewed publish command at request time; the bounded
//! sweep replays due commands unchanged as a system actor. See the
//! `2026-10-09-scheduled-page-publishing` decision record for the drift semantics.

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, sea_query::Expr,
};
use serde_json::from_value;
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use rustok_core::error::{ErrorKind, RichError};

use crate::dto::{
    PagePublishJobState, PagePublishScheduleResponse, PublishPageInput, SchedulePagePublishInput,
};
use crate::entities::page_publish_job;
use crate::error::{PagesError, PagesResult};
use crate::services::rbac::enforce_owned_scope;

use super::PageService;
use super::helpers::body_revision_timestamp;
use super::reviewed_publish::{
    normalize_expected_body_revisions, normalize_idempotency_key, validate_reviewed_runtime,
};

/// Jobs give up after this many executions and stay `failed`.
pub const MAX_PAGE_PUBLISH_JOB_ATTEMPTS: i32 = 5;

/// Upper bound of jobs one sweep executes.
pub const MAX_PAGE_PUBLISH_JOBS_PER_SWEEP: u64 = 50;

pub const PAGE_PUBLISH_SCHEDULE_NOT_FOUND: &str = "PAGE_PUBLISH_SCHEDULE_NOT_FOUND";
pub const PAGE_PUBLISH_SCHEDULE_NOT_CANCELABLE: &str = "PAGE_PUBLISH_SCHEDULE_NOT_CANCELABLE";
pub const PAGE_PUBLISH_SCHEDULE_TIME_INVALID: &str = "PAGE_PUBLISH_SCHEDULE_TIME_INVALID";

fn schedule_time_invalid(message: impl Into<String>) -> PagesError {
    PagesError::Rich(Box::new(
        RichError::new(ErrorKind::Validation, message.into())
            .with_user_message("Choose a publication time in the future.")
            .with_error_code(PAGE_PUBLISH_SCHEDULE_TIME_INVALID),
    ))
}

fn schedule_not_found(page_id: Uuid) -> PagesError {
    PagesError::Rich(Box::new(
        RichError::new(
            ErrorKind::NotFound,
            format!("Page `{page_id}` has no publish schedule"),
        )
        .with_error_code(PAGE_PUBLISH_SCHEDULE_NOT_FOUND),
    ))
}

fn schedule_not_cancelable(state: PagePublishJobState) -> PagesError {
    PagesError::Rich(Box::new(
        RichError::new(
            ErrorKind::Conflict,
            format!(
                "Page publish schedule is `{}` and cannot be canceled",
                state.as_str()
            ),
        )
        .with_user_message("Only a pending scheduled publication can be canceled.")
        .with_error_code(PAGE_PUBLISH_SCHEDULE_NOT_CANCELABLE),
    ))
}

fn schedule_response_from_model(model: page_publish_job::Model) -> PagePublishScheduleResponse {
    PagePublishScheduleResponse {
        id: model.id,
        page_id: model.page_id,
        publish_at: model.publish_at.to_string(),
        state: PagePublishJobState::parse(&model.state)
            .expect("job state is constrained by the storage check"),
        attempts: model.attempts,
        last_error_code: model.last_error_code,
        last_error_message: model.last_error_message,
        publish_operation_id: model.publish_operation_id,
        created_by: model.created_by,
        created_at: model.created_at.to_string(),
        updated_at: model.updated_at.to_string(),
    }
}

/// Failures of the replayed publish command that must not be retried: they are the
/// deterministic reviewed-publish contract (drift, validation, authorization).
fn is_transient_publish_failure(kind: ErrorKind) -> bool {
    matches!(
        kind,
        ErrorKind::Database
            | ErrorKind::ExternalService
            | ErrorKind::Internal
            | ErrorKind::RateLimited
            | ErrorKind::Timeout
    )
}

fn publish_failure_signature(error: PagesError) -> (ErrorKind, Option<String>, String) {
    let rich: RichError = error.into();
    let code = rich
        .error_code
        .clone()
        .unwrap_or_else(|| rich.kind.error_code().to_string());
    (rich.kind, Some(code), rich.message)
}

impl PageService {
    /// Captures one reviewed publish command for later execution.
    ///
    /// A pending schedule is rewritten in place (one pending job per page); completed jobs
    /// stay as append-only history.
    #[instrument(skip(self, security))]
    pub async fn schedule_publish(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        input: SchedulePagePublishInput,
    ) -> PagesResult<PagePublishScheduleResponse> {
        let page = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(&security, Resource::Pages, Action::Publish, page.author_id)?;

        let publish_at = parse_schedule_time(&input.publish_at)?;
        let command = validate_schedule_command(input.command)?;

        let now = body_revision_timestamp(Utc::now());
        let pending = page_publish_job::Entity::find()
            .filter(page_publish_job::Column::TenantId.eq(tenant_id))
            .filter(page_publish_job::Column::PageId.eq(page_id))
            .filter(page_publish_job::Column::State.eq("scheduled"))
            .one(&self.db)
            .await?;

        let model = if let Some(existing) = pending {
            let mut active: page_publish_job::ActiveModel = existing.into();
            active.publish_at = Set(publish_at);
            active.command = Set(command);
            active.attempts = Set(0);
            active.last_error_code = Set(None);
            active.last_error_message = Set(None);
            active.updated_at = Set(now);
            active.update(&self.db).await?
        } else {
            page_publish_job::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                page_id: Set(page_id),
                publish_at: Set(publish_at),
                state: Set(PagePublishJobState::Scheduled.as_str().to_string()),
                command: Set(command),
                attempts: Set(0),
                last_error_code: Set(None),
                last_error_message: Set(None),
                publish_operation_id: Set(None),
                created_by: Set(security.user_id),
                created_at: Set(now),
                updated_at: Set(now),
            }
            .insert(&self.db)
            .await?
        };
        Ok(schedule_response_from_model(model))
    }

    /// Cancels the pending schedule of one page.
    #[instrument(skip(self, security))]
    pub async fn cancel_scheduled_publish(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<PagePublishScheduleResponse> {
        let page = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(&security, Resource::Pages, Action::Publish, page.author_id)?;

        let job = self.latest_publish_job(tenant_id, page_id).await?;
        let Some(job) = job else {
            return Err(schedule_not_found(page_id));
        };
        let state = PagePublishJobState::parse(&job.state).expect("job state is storage-checked");
        if state != PagePublishJobState::Scheduled {
            return Err(schedule_not_cancelable(state));
        }

        let mut active: page_publish_job::ActiveModel = job.into();
        active.state = Set(PagePublishJobState::Canceled.as_str().to_string());
        active.updated_at = Set(body_revision_timestamp(Utc::now()));
        let model = active.update(&self.db).await?;
        Ok(schedule_response_from_model(model))
    }

    /// Returns the most recent schedule job of one page, whatever its state.
    ///
    /// `None` means the page was never scheduled.
    #[instrument(skip(self, security))]
    pub async fn page_publish_schedule(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<Option<PagePublishScheduleResponse>> {
        let page = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(&security, Resource::Pages, Action::Read, page.author_id)?;

        Ok(self
            .latest_publish_job(tenant_id, page_id)
            .await?
            .map(schedule_response_from_model))
    }

    /// Executes due jobs now; see [`Self::process_due_publish_jobs_as_of`].
    pub async fn process_due_publish_jobs(&self, limit: u64) -> PagesResult<u64> {
        self.process_due_publish_jobs_as_of(body_revision_timestamp(Utc::now()), limit)
            .await
    }

    /// Claims and executes at most `limit` scheduled jobs that are due at `now`.
    ///
    /// Returns how many jobs reached `published`. Safe to run from concurrent workers:
    /// the claim is an optimistic `scheduled -> executing` transition.
    #[instrument(skip(self))]
    pub async fn process_due_publish_jobs_as_of(
        &self,
        now: sea_orm::DateTimeWithTimeZone,
        limit: u64,
    ) -> PagesResult<u64> {
        let limit = limit.clamp(1, MAX_PAGE_PUBLISH_JOBS_PER_SWEEP);
        let due = page_publish_job::Entity::find()
            .filter(page_publish_job::Column::State.eq("scheduled"))
            .filter(page_publish_job::Column::PublishAt.lte(now))
            .order_by_asc(page_publish_job::Column::PublishAt)
            .limit(limit)
            .all(&self.db)
            .await?;

        let mut published = 0_u64;
        for job in due {
            let claimed = page_publish_job::Entity::update_many()
                .col_expr(
                    page_publish_job::Column::State,
                    Expr::value(PagePublishJobState::Executing.as_str()),
                )
                .col_expr(
                    page_publish_job::Column::Attempts,
                    Expr::value(job.attempts + 1),
                )
                .col_expr(
                    page_publish_job::Column::UpdatedAt,
                    Expr::value(body_revision_timestamp(Utc::now())),
                )
                .filter(page_publish_job::Column::Id.eq(job.id))
                .filter(page_publish_job::Column::State.eq("scheduled"))
                .exec(&self.db)
                .await?;
            if claimed.rows_affected == 0 {
                continue;
            }

            let command: PublishPageInput = from_value(job.command.clone()).map_err(|error| {
                PagesError::validation(format!(
                    "Stored page publish command for job `{}` is invalid: {error}",
                    job.id
                ))
            })?;
            let attempt = job.attempts + 1;
            match self
                .publish_reviewed(
                    job.tenant_id,
                    SecurityContext::system(),
                    job.page_id,
                    command,
                )
                .await
            {
                Ok(receipt) => {
                    page_publish_job::Entity::update_many()
                        .col_expr(
                            page_publish_job::Column::State,
                            Expr::value(PagePublishJobState::Published.as_str()),
                        )
                        .col_expr(
                            page_publish_job::Column::LastErrorCode,
                            Expr::value(None::<String>),
                        )
                        .col_expr(
                            page_publish_job::Column::LastErrorMessage,
                            Expr::value(None::<String>),
                        )
                        .col_expr(
                            page_publish_job::Column::PublishOperationId,
                            Expr::value(Some(receipt.operation_id)),
                        )
                        .col_expr(
                            page_publish_job::Column::UpdatedAt,
                            Expr::value(body_revision_timestamp(Utc::now())),
                        )
                        .filter(page_publish_job::Column::Id.eq(job.id))
                        .filter(page_publish_job::Column::State.eq("executing"))
                        .exec(&self.db)
                        .await?;
                    published += 1;
                }
                Err(error) => {
                    let (kind, code, message) = publish_failure_signature(error);
                    let terminal = !is_transient_publish_failure(kind)
                        || attempt >= MAX_PAGE_PUBLISH_JOB_ATTEMPTS;
                    let next_state = if terminal {
                        PagePublishJobState::Failed
                    } else {
                        PagePublishJobState::Scheduled
                    };
                    page_publish_job::Entity::update_many()
                        .col_expr(
                            page_publish_job::Column::State,
                            Expr::value(next_state.as_str()),
                        )
                        .col_expr(page_publish_job::Column::LastErrorCode, Expr::value(code))
                        .col_expr(
                            page_publish_job::Column::LastErrorMessage,
                            Expr::value(Some(message)),
                        )
                        .col_expr(
                            page_publish_job::Column::UpdatedAt,
                            Expr::value(body_revision_timestamp(Utc::now())),
                        )
                        .filter(page_publish_job::Column::Id.eq(job.id))
                        .filter(page_publish_job::Column::State.eq("executing"))
                        .exec(&self.db)
                        .await?;
                }
            }
        }
        Ok(published)
    }

    async fn latest_publish_job(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> PagesResult<Option<page_publish_job::Model>> {
        Ok(page_publish_job::Entity::find()
            .filter(page_publish_job::Column::TenantId.eq(tenant_id))
            .filter(page_publish_job::Column::PageId.eq(page_id))
            .order_by_desc(page_publish_job::Column::CreatedAt)
            .order_by_desc(page_publish_job::Column::Id)
            .one(&self.db)
            .await?)
    }
}

fn parse_schedule_time(value: &str) -> PagesResult<sea_orm::DateTimeWithTimeZone> {
    let parsed = chrono::DateTime::parse_from_rfc3339(value.trim()).map_err(|error| {
        schedule_time_invalid(format!("publish_at must be an RFC 3339 timestamp: {error}"))
    })?;
    let publish_at = body_revision_timestamp(parsed.with_timezone(&Utc));
    if publish_at <= body_revision_timestamp(Utc::now()) {
        return Err(schedule_time_invalid(
            "publish_at must be later than the current time",
        ));
    }
    Ok(publish_at)
}

fn validate_schedule_command(command: PublishPageInput) -> PagesResult<serde_json::Value> {
    normalize_idempotency_key(&command.idempotency_key)?;
    normalize_expected_body_revisions(command.expected_body_revisions.clone())?;
    validate_reviewed_runtime(command.runtime.clone())?;
    serde_json::to_value(command).map_err(|error| {
        PagesError::validation(format!("Publish command is not serializable: {error}"))
    })
}

/// Relay-style runner hosts embed next to the outbox relay.
pub struct PagePublishScheduler {
    service: PageService,
    interval: std::time::Duration,
    batch: u64,
}

impl PagePublishScheduler {
    pub fn new(service: PageService) -> Self {
        Self {
            service,
            interval: std::time::Duration::from_secs(30),
            batch: MAX_PAGE_PUBLISH_JOBS_PER_SWEEP,
        }
    }

    pub fn with_interval(mut self, interval: std::time::Duration) -> Self {
        self.interval = interval;
        self
    }

    /// Runs sweeps until the process stops.
    pub async fn run(&self) -> PagesResult<()> {
        loop {
            self.process_due_once().await?;
            tokio::time::sleep(self.interval).await;
        }
    }

    /// One bounded sweep; returns how many jobs reached `published`.
    pub async fn process_due_once(&self) -> PagesResult<u64> {
        self.service.process_due_publish_jobs(self.batch).await
    }
}
