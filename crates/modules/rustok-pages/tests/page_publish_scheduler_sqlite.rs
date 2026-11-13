use std::error::Error;
use std::sync::Arc;

use chrono::{Duration as ChronoDuration, Utc};
#[cfg(feature = "runtime")]
use rustok_api::{ModuleWorkItem, ModuleWorkOutcome, ModuleWorkSource};
use rustok_channel::ChannelModule;
use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_pages::PagesModule;
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageBodyRevisionInput, PageTranslationInput, PublishPageInput,
    ReviewedPagePublishRuntimeInput, SavePageDocumentInput, SchedulePagePublishInput,
};
use rustok_pages::entities::{page, page_publish_job};
#[cfg(feature = "runtime")]
use rustok_pages::services::PageScheduledPublishWorkAdapter;
use rustok_pages::services::{
    MAX_PAGE_PUBLISH_JOB_ATTEMPTS, PAGE_PUBLISH_JOB_LEASE_EXPIRED,
    PAGE_PUBLISH_SCHEDULE_NOT_CANCELABLE, PAGE_PUBLISH_SWEEP_INTERVAL_SECS,
    PAGE_SCHEDULED_PUBLISH_WORKER, PagePublishScheduler, PageService,
};
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    EntityTrait, QueryFilter, Statement, sea_query::Expr,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use serde_json::json;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

fn reviewed_input() -> ReviewedPagePublishRuntimeInput {
    let reviewed = PageBuilderReviewedPublishRuntime::new(
        "scheduled-publish",
        json!({ "surface": "storefront", "channel": "web" }),
    )
    .expect("valid reviewed runtime");
    ReviewedPagePublishRuntimeInput {
        format: reviewed.format,
        scenario_id: reviewed.scenario_id,
        context: reviewed.context,
        review_hash: reviewed.review_hash,
    }
}

fn project(heading: &str) -> serde_json::Value {
    json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": heading,
                "description": "Scheduled publish regression",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": heading
                }]
            }
        }]
    })
}

struct Fixture {
    db: DatabaseConnection,
    service: PageService,
    tenant_id: Uuid,
}

async fn fixture() -> TestResult<Fixture> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    Ok(Fixture {
        service: PageService::new(db.clone(), event_bus),
        db,
        tenant_id,
    })
}

impl Fixture {
    async fn create_draft_page(&self, heading: &str) -> TestResult<(Uuid, String, i32)> {
        let draft = self
            .service
            .create(
                self.tenant_id,
                SecurityContext::system(),
                CreatePageInput {
                    translations: vec![PageTranslationInput {
                        locale: "en".to_string(),
                        title: heading.to_string(),
                        slug: Some(format!("home-{}", Uuid::new_v4())),
                        meta_title: None,
                        meta_description: None,
                    }],
                    template: Some("default".to_string()),
                    body: Some(PageBodyInput {
                        locale: "en".to_string(),
                        document: project(heading),
                    }),
                    channel_slugs: Some(vec!["web".to_string()]),
                    publish: false,
                },
            )
            .await?;
        let revision = draft
            .body
            .as_ref()
            .ok_or_else(|| std::io::Error::other("draft body is missing"))?
            .updated_at
            .clone();
        Ok((draft.id, revision, draft.version))
    }

    fn schedule_input(
        &self,
        publish_at: &str,
        page_id: Uuid,
        revision: &str,
        version: i32,
    ) -> SchedulePagePublishInput {
        SchedulePagePublishInput {
            publish_at: publish_at.to_string(),
            command: PublishPageInput {
                expected_version: version,
                expected_body_revisions: vec![PageBodyRevisionInput {
                    locale: "en".to_string(),
                    revision: revision.to_string(),
                }],
                idempotency_key: format!("page-publish-job:{page_id}:{revision}"),
                runtime: reviewed_input(),
            },
        }
    }

    async fn job(&self, page_id: Uuid) -> TestResult<page_publish_job::Model> {
        let job = page_publish_job::Entity::find()
            .filter(page_publish_job::Column::TenantId.eq(self.tenant_id))
            .filter(page_publish_job::Column::PageId.eq(page_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| std::io::Error::other("publish job is missing"))?;
        Ok(job)
    }
}

/// Simulates a worker that claimed the job and crashed before reporting an
/// outcome: leaves it `executing` with the given attempts and lease.
async fn strand_job(
    fixture: &Fixture,
    page_id: Uuid,
    attempts: i32,
    lease_until: Option<sea_orm::prelude::DateTimeWithTimeZone>,
) -> TestResult<()> {
    page_publish_job::Entity::update_many()
        .col_expr(page_publish_job::Column::State, Expr::value("executing"))
        .col_expr(page_publish_job::Column::Attempts, Expr::value(attempts))
        .col_expr(
            page_publish_job::Column::LeaseUntil,
            Expr::value(lease_until),
        )
        .filter(page_publish_job::Column::TenantId.eq(fixture.tenant_id))
        .filter(page_publish_job::Column::PageId.eq(page_id))
        .exec(&fixture.db)
        .await?;
    Ok(())
}

#[tokio::test]
async fn scheduled_publish_executes_the_captured_command_once() -> TestResult<()> {
    let fixture = fixture().await?;
    let (page_id, revision, version) = fixture.create_draft_page("Scheduled once").await?;
    let publish_at = (Utc::now() + ChronoDuration::hours(1)).to_rfc3339();

    let scheduled = fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(&publish_at, page_id, &revision, version),
        )
        .await?;
    assert_eq!(scheduled.state.as_str(), "scheduled");
    assert_eq!(scheduled.attempts, 0);

    let due = (Utc::now() + ChronoDuration::hours(2)).into();
    let published = fixture
        .service
        .process_due_publish_jobs_as_of(due, 10)
        .await?;
    assert_eq!(published, 1);

    let job = fixture.job(page_id).await?;
    assert_eq!(job.state, "published");
    assert_eq!(job.attempts, 1);
    assert!(job.publish_operation_id.is_some());
    assert!(job.last_error_code.is_none());

    let page = page::Entity::find_by_id(page_id)
        .one(&fixture.db)
        .await?
        .ok_or_else(|| std::io::Error::other("page is missing"))?;
    assert_eq!(page.status, "published");

    // A second sweep must not re-execute the completed job.
    let again = fixture
        .service
        .process_due_publish_jobs_as_of(due, 10)
        .await?;
    assert_eq!(again, 0);
    let job = fixture.job(page_id).await?;
    assert_eq!(job.state, "published");
    assert_eq!(job.attempts, 1);
    Ok(())
}

#[tokio::test]
async fn non_due_schedule_is_untouched() -> TestResult<()> {
    let fixture = fixture().await?;
    let (page_id, revision, version) = fixture.create_draft_page("Not due yet").await?;
    let publish_at = (Utc::now() + ChronoDuration::hours(1)).to_rfc3339();
    fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(&publish_at, page_id, &revision, version),
        )
        .await?;

    let published = fixture
        .service
        .process_due_publish_jobs_as_of(Utc::now().into(), 10)
        .await?;
    assert_eq!(published, 0);
    let job = fixture.job(page_id).await?;
    assert_eq!(job.state, "scheduled");
    assert_eq!(job.attempts, 0);
    Ok(())
}

#[tokio::test]
async fn canceled_schedule_never_executes() -> TestResult<()> {
    let fixture = fixture().await?;
    let (page_id, revision, version) = fixture.create_draft_page("Canceled schedule").await?;
    let publish_at = (Utc::now() + ChronoDuration::hours(1)).to_rfc3339();
    fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(&publish_at, page_id, &revision, version),
        )
        .await?;

    let canceled = fixture
        .service
        .cancel_scheduled_publish(fixture.tenant_id, SecurityContext::system(), page_id)
        .await?;
    assert_eq!(canceled.state.as_str(), "canceled");

    let due = (Utc::now() + ChronoDuration::hours(2)).into();
    let published = fixture
        .service
        .process_due_publish_jobs_as_of(due, 10)
        .await?;
    assert_eq!(published, 0);
    let job = fixture.job(page_id).await?;
    assert_eq!(job.state, "canceled");
    Ok(())
}

/// A page can hold terminal job rows newer than its live schedule. Reading "the newest row of any
/// state" then reports the finished publication, so the editor sees a past schedule and
/// cancellation rejects a job that is still going to fire.
#[tokio::test]
async fn a_second_schedule_is_not_shadowed_by_the_published_job() -> TestResult<()> {
    let fixture = fixture().await?;
    let (page_id, revision, version) = fixture
        .create_draft_page("Rescheduled after publish")
        .await?;
    let first_publish_at = (Utc::now() + ChronoDuration::hours(1)).to_rfc3339();
    fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(&first_publish_at, page_id, &revision, version),
        )
        .await?;

    let due = (Utc::now() + ChronoDuration::hours(2)).into();
    assert_eq!(
        fixture
            .service
            .process_due_publish_jobs_as_of(due, 10)
            .await?,
        1
    );
    let published = fixture.job(page_id).await?;
    assert_eq!(published.state, "published");

    // The page now holds a terminal row older than the schedule that follows it.
    let second_publish_at = (Utc::now() + ChronoDuration::hours(3)).to_rfc3339();
    fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(&second_publish_at, page_id, &revision, version),
        )
        .await?;

    let schedule = fixture
        .service
        .page_publish_schedule(fixture.tenant_id, SecurityContext::system(), page_id)
        .await?
        .ok_or_else(|| std::io::Error::other("the page has a schedule"))?;
    assert_eq!(schedule.state.as_str(), "scheduled");
    assert_ne!(schedule.id, published.id);

    let canceled = fixture
        .service
        .cancel_scheduled_publish(fixture.tenant_id, SecurityContext::system(), page_id)
        .await?;
    assert_eq!(canceled.id, schedule.id);
    assert_eq!(canceled.state.as_str(), "canceled");
    Ok(())
}

/// A page with a finished job and no pending one reports the finished job and refuses the cancel
/// with its state, instead of claiming the page was never scheduled.
#[tokio::test]
async fn cancel_without_a_pending_job_reports_the_last_state() -> TestResult<()> {
    let fixture = fixture().await?;
    let (page_id, revision, version) = fixture.create_draft_page("Nothing pending").await?;
    let publish_at = (Utc::now() + ChronoDuration::hours(1)).to_rfc3339();
    fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(&publish_at, page_id, &revision, version),
        )
        .await?;
    fixture
        .service
        .cancel_scheduled_publish(fixture.tenant_id, SecurityContext::system(), page_id)
        .await?;

    let schedule = fixture
        .service
        .page_publish_schedule(fixture.tenant_id, SecurityContext::system(), page_id)
        .await?
        .ok_or_else(|| std::io::Error::other("the canceled job is history"))?;
    assert_eq!(schedule.state.as_str(), "canceled");

    let error = fixture
        .service
        .cancel_scheduled_publish(fixture.tenant_id, SecurityContext::system(), page_id)
        .await
        .expect_err("a canceled schedule cannot be canceled again");
    let rich: rustok_core::error::RichError = error.into();
    assert_eq!(
        rich.error_code.as_deref(),
        Some(PAGE_PUBLISH_SCHEDULE_NOT_CANCELABLE)
    );
    Ok(())
}

#[tokio::test]
async fn reschedule_rewrites_the_pending_job() -> TestResult<()> {
    let fixture = fixture().await?;
    let (page_id, revision, version) = fixture.create_draft_page("Reschedule").await?;
    let first = fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(
                &(Utc::now() + ChronoDuration::hours(1)).to_rfc3339(),
                page_id,
                &revision,
                version,
            ),
        )
        .await?;
    let second = fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(
                &(Utc::now() + ChronoDuration::hours(3)).to_rfc3339(),
                page_id,
                &revision,
                version,
            ),
        )
        .await?;
    assert_eq!(first.id, second.id);
    assert_ne!(first.publish_at, second.publish_at);
    Ok(())
}

#[tokio::test]
async fn content_drift_after_scheduling_marks_the_job_failed() -> TestResult<()> {
    let fixture = fixture().await?;
    let (page_id, revision, version) = fixture.create_draft_page("Drifts away").await?;
    let publish_at = (Utc::now() + ChronoDuration::hours(1)).to_rfc3339();
    fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(&publish_at, page_id, &revision, version),
        )
        .await?;

    // The working copy changes after the reviewed command was captured.
    fixture
        .service
        .save_document(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            SavePageDocumentInput {
                expected_revision: revision.clone(),
                body: PageBodyInput {
                    locale: "en".to_string(),
                    document: project("Drifts away, edited"),
                },
            },
        )
        .await?;

    let due = (Utc::now() + ChronoDuration::hours(2)).into();
    let published = fixture
        .service
        .process_due_publish_jobs_as_of(due, 10)
        .await?;
    assert_eq!(published, 0);
    let job = fixture.job(page_id).await?;
    assert_eq!(job.state, "failed");
    assert_eq!(job.attempts, 1);
    assert_eq!(
        job.last_error_code.as_deref(),
        Some("PAGE_DOCUMENT_REVISION_CONFLICT")
    );
    Ok(())
}

#[tokio::test]
async fn schedule_requires_a_future_time() -> TestResult<()> {
    let fixture = fixture().await?;
    let (page_id, revision, version) = fixture.create_draft_page("Past time").await?;
    let past = (Utc::now() - ChronoDuration::hours(1)).to_rfc3339();
    let error = fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(&past, page_id, &revision, version),
        )
        .await
        .expect_err("past publish_at must be rejected");
    assert!(error.to_string().contains("publish_at"), "{error}");
    Ok(())
}

#[tokio::test]
async fn page_publish_schedule_reports_the_latest_job() -> TestResult<()> {
    let fixture = fixture().await?;
    let (page_id, revision, version) = fixture.create_draft_page("Schedule view").await?;
    let missing = fixture
        .service
        .page_publish_schedule(fixture.tenant_id, SecurityContext::system(), page_id)
        .await?;
    assert!(missing.is_none());

    let publish_at = (Utc::now() + ChronoDuration::hours(1)).to_rfc3339();
    fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(&publish_at, page_id, &revision, version),
        )
        .await?;
    let view = fixture
        .service
        .page_publish_schedule(fixture.tenant_id, SecurityContext::system(), page_id)
        .await?
        .ok_or_else(|| std::io::Error::other("schedule is missing"))?;
    assert_eq!(view.state.as_str(), "scheduled");
    assert_eq!(view.page_id, page_id);
    Ok(())
}

#[tokio::test]
async fn sweep_reclaims_a_stranded_executing_job_and_publishes_it() -> TestResult<()> {
    let fixture = fixture().await?;
    let (page_id, revision, version) = fixture.create_draft_page("Stranded claim").await?;
    let publish_at = (Utc::now() + ChronoDuration::hours(1)).to_rfc3339();
    fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(&publish_at, page_id, &revision, version),
        )
        .await?;

    // The worker claimed the job and crashed before reporting an outcome.
    strand_job(
        &fixture,
        page_id,
        1,
        Some((Utc::now() - ChronoDuration::hours(1)).into()),
    )
    .await?;

    // One sweep must both reclaim the stranded job and publish it.
    let due = (Utc::now() + ChronoDuration::hours(2)).into();
    let outcome = fixture
        .service
        .sweep_due_publish_jobs_as_of(due, 10)
        .await?;
    assert_eq!(outcome.reaped.requeued, 1);
    assert_eq!(outcome.reaped.failed, 0);
    assert_eq!(outcome.reaped.superseded, 0);
    assert_eq!(outcome.published, 1);

    let job = fixture.job(page_id).await?;
    assert_eq!(job.state, "published");
    assert_eq!(job.attempts, 2);
    assert!(job.lease_until.is_none());
    assert!(job.publish_operation_id.is_some());
    // The success transition clears the reaper marker alongside the lease.
    assert_eq!(job.last_error_code, None);

    let page = page::Entity::find_by_id(page_id)
        .one(&fixture.db)
        .await?
        .ok_or_else(|| std::io::Error::other("page is missing"))?;
    assert_eq!(page.status, "published");
    Ok(())
}

#[tokio::test]
async fn reap_returns_expired_jobs_to_scheduled_with_a_marker() -> TestResult<()> {
    let fixture = fixture().await?;
    let (page_id, revision, version) = fixture.create_draft_page("Expired lease").await?;
    let publish_at = (Utc::now() + ChronoDuration::hours(1)).to_rfc3339();
    fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(&publish_at, page_id, &revision, version),
        )
        .await?;
    strand_job(
        &fixture,
        page_id,
        1,
        Some((Utc::now() - ChronoDuration::minutes(5)).into()),
    )
    .await?;

    let outcome = fixture
        .service
        .reap_expired_publish_jobs_as_of(Utc::now().into())
        .await?;
    assert_eq!(outcome.requeued, 1);
    assert_eq!(outcome.failed, 0);
    assert_eq!(outcome.superseded, 0);

    let job = fixture.job(page_id).await?;
    assert_eq!(job.state, "scheduled");
    assert_eq!(job.attempts, 1);
    assert!(job.lease_until.is_none());
    assert_eq!(
        job.last_error_code.as_deref(),
        Some(PAGE_PUBLISH_JOB_LEASE_EXPIRED)
    );
    let reaper_message = job.last_error_message.unwrap_or_default();
    assert!(reaper_message.contains("lease"));
    Ok(())
}

#[tokio::test]
async fn reap_leaves_live_leases_alone() -> TestResult<()> {
    let fixture = fixture().await?;
    let (page_id, revision, version) = fixture.create_draft_page("Live lease").await?;
    let publish_at = (Utc::now() + ChronoDuration::hours(1)).to_rfc3339();
    fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(&publish_at, page_id, &revision, version),
        )
        .await?;
    let live_lease = (Utc::now() + ChronoDuration::minutes(30)).into();
    strand_job(&fixture, page_id, 1, Some(live_lease)).await?;

    let outcome = fixture
        .service
        .reap_expired_publish_jobs_as_of(Utc::now().into())
        .await?;
    assert_eq!(outcome.requeued, 0);
    assert_eq!(outcome.failed, 0);
    assert_eq!(outcome.superseded, 0);

    let job = fixture.job(page_id).await?;
    assert_eq!(job.state, "executing");
    assert!(job.lease_until.is_some());
    assert_eq!(job.last_error_code, None);
    Ok(())
}

#[tokio::test]
async fn reap_treats_a_null_lease_as_expired() -> TestResult<()> {
    let fixture = fixture().await?;
    let (page_id, revision, version) = fixture.create_draft_page("Legacy executing row").await?;
    let publish_at = (Utc::now() + ChronoDuration::hours(1)).to_rfc3339();
    fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(&publish_at, page_id, &revision, version),
        )
        .await?;
    // Rows claimed before the lease column existed carry no lease.
    strand_job(&fixture, page_id, 1, None).await?;

    let outcome = fixture
        .service
        .reap_expired_publish_jobs_as_of(Utc::now().into())
        .await?;
    assert_eq!(outcome.requeued, 1);

    let job = fixture.job(page_id).await?;
    assert_eq!(job.state, "scheduled");
    assert!(job.lease_until.is_none());
    Ok(())
}

#[tokio::test]
async fn reap_fails_expired_jobs_with_exhausted_attempts() -> TestResult<()> {
    let fixture = fixture().await?;
    let (page_id, revision, version) = fixture.create_draft_page("Exhausted stranded job").await?;
    let publish_at = (Utc::now() + ChronoDuration::hours(1)).to_rfc3339();
    fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(&publish_at, page_id, &revision, version),
        )
        .await?;
    strand_job(
        &fixture,
        page_id,
        MAX_PAGE_PUBLISH_JOB_ATTEMPTS,
        Some((Utc::now() - ChronoDuration::minutes(5)).into()),
    )
    .await?;

    let outcome = fixture
        .service
        .reap_expired_publish_jobs_as_of(Utc::now().into())
        .await?;
    assert_eq!(outcome.requeued, 0);
    assert_eq!(outcome.failed, 1);
    assert_eq!(outcome.superseded, 0);

    let job = fixture.job(page_id).await?;
    assert_eq!(job.state, "failed");
    assert_eq!(job.attempts, MAX_PAGE_PUBLISH_JOB_ATTEMPTS);
    assert_eq!(
        job.last_error_code.as_deref(),
        Some(PAGE_PUBLISH_JOB_LEASE_EXPIRED)
    );

    // A later sweep must not resurrect the terminal job.
    let due = (Utc::now() + ChronoDuration::hours(2)).into();
    let published = fixture
        .service
        .process_due_publish_jobs_as_of(due, 10)
        .await?;
    assert_eq!(published, 0);
    let job = fixture.job(page_id).await?;
    assert_eq!(job.state, "failed");
    Ok(())
}

#[tokio::test]
async fn reap_cancels_a_stranded_job_superseded_by_a_newer_schedule() -> TestResult<()> {
    let fixture = fixture().await?;
    let (page_id, revision, version) = fixture.create_draft_page("Superseded strand").await?;

    // First schedule gets claimed, then the worker crashes.
    fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(
                &(Utc::now() + ChronoDuration::hours(1)).to_rfc3339(),
                page_id,
                &revision,
                version,
            ),
        )
        .await?;
    strand_job(
        &fixture,
        page_id,
        1,
        Some((Utc::now() - ChronoDuration::minutes(5)).into()),
    )
    .await?;

    // While the claim is stranded, the author schedules a new publication:
    // no `scheduled` row exists, so a second job is inserted.
    fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(
                &(Utc::now() + ChronoDuration::hours(1)).to_rfc3339(),
                page_id,
                &revision,
                version,
            ),
        )
        .await?;

    let outcome = fixture
        .service
        .reap_expired_publish_jobs_as_of(Utc::now().into())
        .await?;
    assert_eq!(outcome.requeued, 0);
    assert_eq!(outcome.superseded, 1);

    // The newer schedule wins; the stranded claim is terminal.
    let jobs = page_publish_job::Entity::find()
        .filter(page_publish_job::Column::TenantId.eq(fixture.tenant_id))
        .filter(page_publish_job::Column::PageId.eq(page_id))
        .all(&fixture.db)
        .await?;
    assert_eq!(jobs.len(), 2);
    let stranded = jobs
        .iter()
        .find(|job| job.last_error_code.as_deref() == Some(PAGE_PUBLISH_JOB_LEASE_EXPIRED))
        .ok_or_else(|| std::io::Error::other("stranded job is missing"))?;
    assert_eq!(stranded.state, "canceled");
    assert!(stranded.lease_until.is_none());
    let scheduled = jobs.iter().filter(|job| job.state == "scheduled").count();
    assert_eq!(scheduled, 1);

    // The newer job still executes.
    let due = (Utc::now() + ChronoDuration::hours(2)).into();
    let published = fixture
        .service
        .process_due_publish_jobs_as_of(due, 10)
        .await?;
    assert_eq!(published, 1);
    let jobs = page_publish_job::Entity::find()
        .filter(page_publish_job::Column::TenantId.eq(fixture.tenant_id))
        .filter(page_publish_job::Column::PageId.eq(page_id))
        .all(&fixture.db)
        .await?;
    let published_count = jobs.iter().filter(|job| job.state == "published").count();
    assert_eq!(published_count, 1);
    let canceled_count = jobs.iter().filter(|job| job.state == "canceled").count();
    assert_eq!(canceled_count, 1);
    Ok(())
}

#[tokio::test]
async fn scheduler_metrics_count_sweeps_and_strand_recoveries() -> TestResult<()> {
    let fixture = fixture().await?;
    let (page_id, revision, version) = fixture.create_draft_page("Scheduler metrics").await?;
    let publish_at = (Utc::now() + ChronoDuration::hours(1)).to_rfc3339();
    fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(&publish_at, page_id, &revision, version),
        )
        .await?;
    strand_job(
        &fixture,
        page_id,
        1,
        Some((Utc::now() - ChronoDuration::minutes(5)).into()),
    )
    .await?;

    let scheduler = PagePublishScheduler::new(fixture.service.clone());
    let outcome = scheduler.process_due_once().await?;
    assert_eq!(outcome.reaped.requeued, 1);
    assert_eq!(outcome.published, 0, "publish_at is still in the future");

    let metrics = scheduler.metrics();
    assert_eq!(metrics.sweeps_total, 1);
    assert_eq!(metrics.requeued_total, 1);
    assert_eq!(metrics.published_total, 0);
    assert_eq!(metrics.reaped_failed_total, 0);
    assert_eq!(metrics.superseded_total, 0);

    // The recovered job is no longer stranded: counters stay put.
    let outcome = scheduler.process_due_once().await?;
    assert_eq!(outcome.reaped.requeued, 0);
    let metrics = scheduler.metrics();
    assert_eq!(metrics.sweeps_total, 2);
    assert_eq!(metrics.requeued_total, 1);
    Ok(())
}

/// Discovery has to reach a stranded `executing` job: with the job claimed and its lease expired
/// there is no due `scheduled` row, so a claim that only looked for due work would leave that
/// publication unreachable. The adapter is the only path from the host scheduler into this queue.
#[cfg(feature = "runtime")]
#[tokio::test]
async fn scheduler_adapter_claims_stranded_jobs_and_publishes_them() -> TestResult<()> {
    let fixture = fixture().await?;
    // Pacing exists to keep a 1s host poll from burning the attempt budget; it is not part of
    // discovery semantics, so the claim assertions run unpaced.
    let adapter = PageScheduledPublishWorkAdapter::new(fixture.service.clone())
        .with_sweep_interval(std::time::Duration::ZERO);

    assert_eq!(
        adapter.claim(PAGE_SCHEDULED_PUBLISH_WORKER).await?,
        None,
        "an empty queue claims nothing"
    );
    assert_eq!(
        adapter.claim("another_module_worker").await?,
        None,
        "the adapter answers only for its own worker slug"
    );

    let (page_id, revision, version) = fixture.create_draft_page("Adapter claim").await?;
    let publish_at = (Utc::now() + ChronoDuration::hours(1)).to_rfc3339();
    fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(&publish_at, page_id, &revision, version),
        )
        .await?;

    // A future schedule is not runnable yet: nothing is due and no lease is stranded.
    assert_eq!(
        adapter.claim(PAGE_SCHEDULED_PUBLISH_WORKER).await?,
        None,
        "a schedule that is not due yet is not claimed"
    );

    // A worker that died mid-publish leaves the job `executing` with an expired lease.
    strand_job(
        &fixture,
        page_id,
        1,
        Some((Utc::now() - ChronoDuration::minutes(5)).into()),
    )
    .await?;
    let item = adapter
        .claim(PAGE_SCHEDULED_PUBLISH_WORKER)
        .await?
        .ok_or_else(|| std::io::Error::other("a stranded job must be claimed"))?;
    assert_eq!(item.tenant_id, fixture.tenant_id);
    assert_eq!(item.id, fixture.tenant_id);

    // The sweep reaps the stranded row and returns it to `scheduled`; `publish_at` is still in
    // the future, so that is the whole outcome of one execution.
    let outcome = adapter.execute(item).await?;
    assert_eq!(outcome, ModuleWorkOutcome::Completed);
    let job = fixture.job(page_id).await?;
    assert_eq!(job.state, "scheduled");
    assert_eq!(
        job.last_error_code.as_deref(),
        Some(PAGE_PUBLISH_JOB_LEASE_EXPIRED)
    );
    assert!(job.lease_until.is_none());

    // A due job is published by the same adapter.
    page_publish_job::Entity::update_many()
        .col_expr(
            page_publish_job::Column::PublishAt,
            Expr::value((Utc::now() - ChronoDuration::minutes(1)).into()),
        )
        .filter(page_publish_job::Column::TenantId.eq(fixture.tenant_id))
        .filter(page_publish_job::Column::PageId.eq(page_id))
        .exec(&fixture.db)
        .await?;
    let item = adapter
        .claim(PAGE_SCHEDULED_PUBLISH_WORKER)
        .await?
        .ok_or_else(|| std::io::Error::other("a due job must be claimed"))?;
    adapter.execute(item).await?;
    let job = fixture.job(page_id).await?;
    assert_eq!(job.state, "published");
    assert!(job.publish_operation_id.is_some());
    Ok(())
}

/// The default cadence is what keeps a per-second host poll from spending a transiently failing
/// job's whole attempt budget in seconds. This pins the pace to the claim path.
#[cfg(feature = "runtime")]
#[tokio::test]
async fn scheduler_adapter_paces_executions_at_the_default_interval() -> TestResult<()> {
    let fixture = fixture().await?;
    let adapter = PageScheduledPublishWorkAdapter::new(fixture.service.clone());
    // The pace must outlast a few host polls, otherwise `MAX_PAGE_PUBLISH_JOB_ATTEMPTS` covers
    // seconds of outage instead of minutes.
    assert!(PAGE_PUBLISH_SWEEP_INTERVAL_SECS >= 10);

    let (page_id, revision, version) = fixture.create_draft_page("Adapter pace").await?;
    let publish_at = (Utc::now() - ChronoDuration::minutes(1)).to_rfc3339();
    fixture
        .service
        .schedule_publish(
            fixture.tenant_id,
            SecurityContext::system(),
            page_id,
            fixture.schedule_input(&publish_at, page_id, &revision, version),
        )
        .await?;

    let first = adapter
        .claim(PAGE_SCHEDULED_PUBLISH_WORKER)
        .await?
        .ok_or_else(|| std::io::Error::other("a due job must be claimed"))?;
    adapter.execute(first).await?;
    assert!(
        adapter
            .claim(PAGE_SCHEDULED_PUBLISH_WORKER)
            .await?
            .is_none(),
        "a claim immediately after an execution must wait out the pace window"
    );
    Ok(())
}

#[cfg(feature = "runtime")]
#[tokio::test]
async fn scheduler_adapter_rejects_an_envelope_that_is_not_its_own() -> TestResult<()> {
    let fixture = fixture().await?;
    let adapter = PageScheduledPublishWorkAdapter::new(fixture.service.clone());
    let foreign = ModuleWorkItem {
        id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        worker_slug: "blog_scheduled_publish".to_string(),
        lease_token: Uuid::new_v4().to_string(),
        payload: json!({}),
    };
    assert!(adapter.execute(foreign).await.is_err());

    let mismatched = ModuleWorkItem {
        id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        worker_slug: PAGE_SCHEDULED_PUBLISH_WORKER.to_string(),
        lease_token: Uuid::new_v4().to_string(),
        payload: json!({ "tenant_id": Uuid::new_v4().to_string() }),
    };
    assert!(
        adapter.execute(mismatched).await.is_err(),
        "the payload tenant must be the envelope tenant"
    );
    Ok(())
}

async fn setup_db(tenant_id: Uuid) -> TestResult<DatabaseConnection> {
    let database_url = format!(
        "sqlite:file:pages_publish_scheduler_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;

    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)".to_string(),
    ))
    .await?;
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenants (id) VALUES (?)",
        [tenant_id.into()],
    ))
    .await?;
    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE tenant_modules (\
            id TEXT PRIMARY KEY NOT NULL, \
            tenant_id TEXT NOT NULL, \
            module_slug TEXT NOT NULL, \
            enabled INTEGER NOT NULL, \
            settings TEXT NOT NULL, \
            created_at TEXT NOT NULL, \
            updated_at TEXT NOT NULL\
        )"
        .to_string(),
    ))
    .await?;

    let manager = SchemaManager::new(&db);
    SysEventsMigration.up(&manager).await?;
    for migration in ChannelModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in PagesModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(db)
}
