use std::{
    future::Future,
    sync::{Arc, Barrier},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rustok_api::AuthPrincipalKind;
use rustok_migrations::Migrator;
use rustok_server::services::comments_provider_runtime::{
    COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_EVENT_TYPE,
    COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE,
    COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY,
    COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_TABLE,
    CommentsTcpDelegationPersistedScheduleAuditOutcome,
    CommentsTcpDelegationSchedulePersistenceDocument, CommentsTcpDelegationSchedulePersistenceKey,
    CommentsTcpDelegationSchedulePersistenceStartupMode,
    CommentsTcpDelegationScheduleTriggerAuthorizationError,
    CommentsTcpDelegationScheduleTriggerAuthorizationRequest,
    CommentsTcpDelegationScheduleTriggerAuthorizer, CommentsTcpDelegationScheduleTriggerContext,
    PostgresCommentsTcpDelegationScheduleAuditedPersistenceStore,
    SharedCommentsTcpDelegationPostgresAuditedScheduleTrigger,
    SharedCommentsTcpDelegationScheduleTriggerAuthorizer,
};
use rustok_test_utils::{
    assert_postgres_url, connect_postgres, create_postgres_database,
    drop_postgres_database_if_exists, postgres_database_url, unique_postgres_database_name,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const PROPAGATION_BUDGET_MS: u64 = 1_000;
const MAX_TTL_MS: u64 = 5_000;
const SUCCESSOR_DELAY_MS: u64 = 120_000;
const SECRET_A: &str = "comments-audit-postgres-secret-a-000000000001";
const SECRET_B: &str = "comments-audit-postgres-secret-b-000000000002";

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Copy)]
struct ScheduleAnchor {
    primary_activation_ms: u64,
    successor_activation_ms: u64,
    primary_retirement_ms: u64,
}

#[derive(Debug)]
struct StateRow {
    schema_version: i16,
    source: String,
    generation: i64,
    digest_hex: String,
}

#[derive(Debug)]
struct AuditRow {
    request_id: Uuid,
    actor_id: Uuid,
    principal_kind: String,
    operation: String,
    source: String,
    previous_generation: i64,
    candidate_generation: i64,
    outcome: String,
    unpublished: bool,
}

struct AllowAuthorizer;

impl CommentsTcpDelegationScheduleTriggerAuthorizer for AllowAuthorizer {
    fn authorize(
        &self,
        _request: &CommentsTcpDelegationScheduleTriggerAuthorizationRequest,
    ) -> Result<(), CommentsTcpDelegationScheduleTriggerAuthorizationError> {
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn audited_schedule_success_resume_and_conflicts_are_atomic() {
    with_blog_postgres_database(
        "rustok_blog_comments_audit_atomic",
        |db_a, db_b| async move {
            let anchor = schedule_anchor()?;
            let initial = schedule_document(1, anchor, false)?;
            let replacement = schedule_document(2, anchor, true)?;
            let generation_three = schedule_document(3, anchor, true)?;

            let trigger = audited_trigger(
                db_a.clone(),
                initial.clone(),
                CommentsTcpDelegationSchedulePersistenceStartupMode::BootstrapEmpty,
            )?;
            let bootstrapped = read_state(&db_a).await?;
            assert_state(&bootstrapped, 1)?;
            if count_outbox(&db_a).await? != 0 {
                return Err("bootstrap must not create a synthetic durable audit row".into());
            }

            let request_id = Uuid::new_v4();
            let actor_id = Uuid::new_v4();
            let outcome = trigger.replace_host_schedule(
                trigger_context(request_id, actor_id)?,
                replacement.clone(),
            )?;
            if outcome.previous_generation != 1 || outcome.current.generation != 2 {
                return Err(
                    format!("unexpected successful replacement outcome: {outcome:?}").into(),
                );
            }

            let state_after_success = read_state(&db_a).await?;
            assert_state(&state_after_success, 2)?;
            let persisted = trigger.current_persistence_record()?;
            if state_after_success.digest_hex != persisted.schedule_digest().to_hex() {
                return Err(
                    "PostgreSQL state digest does not match the accepted trigger record".into(),
                );
            }
            let audit = read_audit(&db_a, request_id)
                .await?
                .ok_or("successful replacement did not create its durable outbox row")?;
            assert_audit(&audit, request_id, actor_id, 1, 2)?;
            if count_outbox(&db_a).await? != 1 {
                return Err("successful replacement must create exactly one outbox row".into());
            }

            let resumed = audited_trigger(
                db_b.clone(),
                replacement.clone(),
                CommentsTcpDelegationSchedulePersistenceStartupMode::ResumeExact,
            )?;
            if resumed.current_selection()?.generation != 2 {
                return Err("exact resume did not recover accepted generation 2".into());
            }
            let stale_resume = audited_trigger(
                db_b.clone(),
                initial,
                CommentsTcpDelegationSchedulePersistenceStartupMode::ResumeExact,
            );
            if stale_resume.is_ok() {
                return Err("exact resume unexpectedly accepted stale generation 1".into());
            }

            let reused_request = trigger.replace_host_schedule(
                trigger_context(request_id, actor_id)?,
                generation_three.clone(),
            );
            if reused_request.is_ok() {
                return Err("reused durable request identity unexpectedly committed".into());
            }
            assert_state(&read_state(&db_a).await?, 2)?;
            if count_outbox(&db_a).await? != 1 {
                return Err("request conflict changed the durable outbox".into());
            }
            if trigger.current_selection()?.generation != 2 {
                return Err("request conflict published an in-memory generation".into());
            }
            assert_last_conflict_audit(&trigger)?;

            let seeded_request_id = Uuid::new_v4();
            seed_generation_conflict(&db_a, seeded_request_id, Uuid::new_v4()).await?;
            if count_outbox(&db_a).await? != 2 {
                return Err(
                    "generation-conflict fixture did not create exactly one seed row".into(),
                );
            }

            let generation_conflict = trigger.replace_host_schedule(
                trigger_context(Uuid::new_v4(), Uuid::new_v4())?,
                generation_three,
            );
            if generation_conflict.is_ok() {
                return Err("reused candidate generation unexpectedly committed".into());
            }
            assert_state(&read_state(&db_a).await?, 2)?;
            if count_outbox(&db_a).await? != 2 {
                return Err(
                    "generation conflict did not roll back the attempted outbox insert".into(),
                );
            }
            if trigger.current_selection()?.generation != 2 {
                return Err("generation conflict published an in-memory generation".into());
            }
            assert_last_conflict_audit(&trigger)?;

            drop(resumed);
            drop(trigger);
            Ok(())
        },
    )
    .await
    .unwrap_or_else(|error| panic!("Comments audited PostgreSQL atomic evidence failed: {error}"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn concurrent_audited_schedule_cas_commits_one_state_and_one_outbox() {
    with_blog_postgres_database(
        "rustok_blog_comments_audit_concurrent",
        |db_a, db_b| async move {
            let anchor = schedule_anchor()?;
            let initial = schedule_document(1, anchor, false)?;
            let replacement = schedule_document(2, anchor, true)?;

            let trigger_a = audited_trigger(
                db_a.clone(),
                initial.clone(),
                CommentsTcpDelegationSchedulePersistenceStartupMode::BootstrapEmpty,
            )?;
            let trigger_b = audited_trigger(
                db_b.clone(),
                initial,
                CommentsTcpDelegationSchedulePersistenceStartupMode::ResumeExact,
            )?;

            let request_a = Uuid::new_v4();
            let request_b = Uuid::new_v4();
            let barrier = Arc::new(Barrier::new(2));
            let barrier_a = Arc::clone(&barrier);
            let barrier_b = Arc::clone(&barrier);
            let replacement_a = replacement.clone();
            let replacement_b = replacement;
            let first = tokio::task::spawn_blocking(move || {
                barrier_a.wait();
                let result = trigger_a.replace_host_schedule(
                    trigger_context(request_a, Uuid::new_v4())?,
                    replacement_a,
                );
                let generation = trigger_a.current_selection()?.generation;
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>((request_a, result, generation))
            });
            let second = tokio::task::spawn_blocking(move || {
                barrier_b.wait();
                let result = trigger_b.replace_host_schedule(
                    trigger_context(request_b, Uuid::new_v4())?,
                    replacement_b,
                );
                let generation = trigger_b.current_selection()?.generation;
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>((request_b, result, generation))
            });

            let first = first.await??;
            let second = second.await??;
            let outcomes = [&first, &second];
            let successes = outcomes.iter().filter(|(_, result, _)| result.is_ok()).count();
            let conflicts = outcomes.iter().filter(|(_, result, _)| result.is_err()).count();
            if successes != 1 || conflicts != 1 {
                return Err(format!(
                    "concurrent audited CAS produced {successes} successes and {conflicts} conflicts"
                )
                .into());
            }

            let mut local_generations = [first.2, second.2];
            local_generations.sort_unstable();
            if local_generations != [1, 2] {
                return Err(format!(
                    "concurrent triggers retained unexpected local generations {local_generations:?}"
                )
                .into());
            }
            assert_state(&read_state(&db_a).await?, 2)?;
            if count_outbox(&db_a).await? != 1 {
                return Err("concurrent CAS must retain exactly one durable success event".into());
            }

            let winning_request = outcomes
                .iter()
                .find_map(|(request_id, result, _)| result.is_ok().then_some(*request_id))
                .ok_or("concurrent CAS did not identify a winner")?;
            let durable_request = read_only_outbox_request(&db_a)
                .await?
                .ok_or("concurrent CAS did not retain an outbox request")?;
            if durable_request != winning_request {
                return Err(format!(
                    "durable request {durable_request} does not match winner {winning_request}"
                )
                .into());
            }
            Ok(())
        },
    )
    .await
    .unwrap_or_else(|error| panic!("Comments audited PostgreSQL concurrency evidence failed: {error}"));
}

fn audited_trigger(
    database: DatabaseConnection,
    document: CommentsTcpDelegationSchedulePersistenceDocument,
    startup_mode: CommentsTcpDelegationSchedulePersistenceStartupMode,
) -> TestResult<SharedCommentsTcpDelegationPostgresAuditedScheduleTrigger> {
    let store = PostgresCommentsTcpDelegationScheduleAuditedPersistenceStore::new(database)?;
    Ok(
        SharedCommentsTcpDelegationPostgresAuditedScheduleTrigger::from_host_document(
            document,
            Duration::from_millis(MAX_TTL_MS),
            shared_authorizer(),
            store,
            startup_mode,
            32,
        )?,
    )
}

fn shared_authorizer() -> SharedCommentsTcpDelegationScheduleTriggerAuthorizer {
    Arc::new(AllowAuthorizer)
}

fn trigger_context(
    request_id: Uuid,
    actor_id: Uuid,
) -> TestResult<CommentsTcpDelegationScheduleTriggerContext> {
    Ok(CommentsTcpDelegationScheduleTriggerContext::new(
        request_id,
        actor_id,
        AuthPrincipalKind::Service,
    )?)
}

fn schedule_anchor() -> TestResult<ScheduleAnchor> {
    let now = unix_ms()?;
    let successor_activation_ms = now
        .checked_add(SUCCESSOR_DELAY_MS)
        .ok_or("successor activation overflow")?;
    let primary_retirement_ms = successor_activation_ms
        .checked_add(PROPAGATION_BUDGET_MS)
        .and_then(|value| value.checked_add(MAX_TTL_MS))
        .and_then(|value| {
            value.checked_add(rustok_comments::DEFAULT_COMMENTS_TCP_DELEGATION_CLOCK_SKEW_MS)
        })
        .and_then(|value| value.checked_add(1_000))
        .ok_or("primary retirement overflow")?;
    Ok(ScheduleAnchor {
        primary_activation_ms: now.saturating_sub(60_000).max(1),
        successor_activation_ms,
        primary_retirement_ms,
    })
}

fn schedule_document(
    generation: u64,
    anchor: ScheduleAnchor,
    include_successor: bool,
) -> TestResult<CommentsTcpDelegationSchedulePersistenceDocument> {
    let primary = CommentsTcpDelegationSchedulePersistenceKey::new(
        "audit-key-a",
        SECRET_A,
        anchor.primary_activation_ms,
        include_successor.then_some(anchor.primary_retirement_ms),
    )?;
    let mut keys = vec![primary];
    if include_successor {
        keys.push(CommentsTcpDelegationSchedulePersistenceKey::new(
            "audit-key-b",
            SECRET_B,
            anchor.successor_activation_ms,
            None,
        )?);
    }
    Ok(CommentsTcpDelegationSchedulePersistenceDocument::new(
        generation,
        Duration::from_millis(PROPAGATION_BUDGET_MS),
        keys,
        None,
    )?)
}

fn assert_state(state: &StateRow, expected_generation: i64) -> TestResult<()> {
    if state.schema_version != 1
        || state.source != "host_provided"
        || state.generation != expected_generation
        || state.digest_hex.len() != 64
        || hex::decode(&state.digest_hex).is_err()
    {
        return Err(format!("invalid persisted schedule state: {state:?}").into());
    }
    Ok(())
}

fn assert_audit(
    audit: &AuditRow,
    request_id: Uuid,
    actor_id: Uuid,
    previous_generation: i64,
    candidate_generation: i64,
) -> TestResult<()> {
    if audit.request_id != request_id
        || audit.actor_id != actor_id
        || audit.principal_kind != "service"
        || audit.operation != "replace_host_schedule"
        || audit.source != "host_provided"
        || audit.previous_generation != previous_generation
        || audit.candidate_generation != candidate_generation
        || audit.outcome != "replacement_succeeded"
        || !audit.unpublished
    {
        return Err(format!("invalid durable audit row: {audit:?}").into());
    }
    Ok(())
}

fn assert_last_conflict_audit(
    trigger: &SharedCommentsTcpDelegationPostgresAuditedScheduleTrigger,
) -> TestResult<()> {
    let records = trigger.audit_records()?;
    let last = records.last().ok_or("process-local audit ring is empty")?;
    if last.outcome != CommentsTcpDelegationPersistedScheduleAuditOutcome::PersistenceConflict
        || last.current_generation != Some(2)
    {
        return Err(format!("unexpected process-local conflict audit: {last:?}").into());
    }
    Ok(())
}

async fn read_state(database: &DatabaseConnection) -> TestResult<StateRow> {
    let row = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "SELECT schema_version, source, generation, schedule_digest_hex \
                 FROM {COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_TABLE} \
                 WHERE state_key = $1"
            ),
            vec![COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY.into()],
        ))
        .await?
        .ok_or("schedule state row is missing")?;
    Ok(StateRow {
        schema_version: row.try_get("", "schema_version")?,
        source: row.try_get("", "source")?,
        generation: row.try_get("", "generation")?,
        digest_hex: row.try_get("", "schedule_digest_hex")?,
    })
}

async fn read_audit(
    database: &DatabaseConnection,
    request_id: Uuid,
) -> TestResult<Option<AuditRow>> {
    let row = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "SELECT request_id, actor_id, principal_kind, operation, source, \
                 previous_generation, candidate_generation, outcome, \
                 (published_at IS NULL) AS unpublished \
                 FROM {COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE} \
                 WHERE request_id = $1"
            ),
            vec![request_id.into()],
        ))
        .await?;
    row.map(|row| {
        Ok(AuditRow {
            request_id: row.try_get("", "request_id")?,
            actor_id: row.try_get("", "actor_id")?,
            principal_kind: row.try_get("", "principal_kind")?,
            operation: row.try_get("", "operation")?,
            source: row.try_get("", "source")?,
            previous_generation: row.try_get("", "previous_generation")?,
            candidate_generation: row.try_get("", "candidate_generation")?,
            outcome: row.try_get("", "outcome")?,
            unpublished: row.try_get("", "unpublished")?,
        })
    })
    .transpose()
}

async fn count_outbox(database: &DatabaseConnection) -> TestResult<i64> {
    let row = database
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "SELECT COUNT(*)::BIGINT AS row_count \
                 FROM {COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE}"
            ),
        ))
        .await?
        .ok_or("outbox count query returned no row")?;
    Ok(row.try_get("", "row_count")?)
}

async fn read_only_outbox_request(database: &DatabaseConnection) -> TestResult<Option<Uuid>> {
    let row = database
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "SELECT request_id \
                 FROM {COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE} \
                 ORDER BY created_at ASC LIMIT 1"
            ),
        ))
        .await?;
    row.map(|row| row.try_get("", "request_id"))
        .transpose()
        .map_err(Into::into)
}

async fn seed_generation_conflict(
    database: &DatabaseConnection,
    request_id: Uuid,
    actor_id: Uuid,
) -> TestResult<()> {
    let occurred_at_unix_ms = i64::try_from(unix_ms()?)?;
    let result = database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "INSERT INTO {COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE} \
                 (audit_schema_version, request_id, state_key, event_type, occurred_at_unix_ms, \
                  actor_id, principal_kind, operation, source, previous_generation, \
                  candidate_generation, outcome, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW())"
            ),
            vec![
                1i16.into(),
                request_id.into(),
                COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY.into(),
                COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_EVENT_TYPE.into(),
                occurred_at_unix_ms.into(),
                actor_id.into(),
                "service".into(),
                "replace_host_schedule".into(),
                "host_provided".into(),
                2i64.into(),
                3i64.into(),
                "replacement_succeeded".into(),
            ],
        ))
        .await?;
    if result.rows_affected() != 1 {
        return Err("generation-conflict fixture insert affected an unexpected row count".into());
    }
    Ok(())
}

async fn with_blog_postgres_database<T, F, Fut>(prefix: &str, test: F) -> TestResult<T>
where
    F: FnOnce(DatabaseConnection, DatabaseConnection) -> Fut,
    Fut: Future<Output = TestResult<T>>,
{
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);

    let database_name = unique_postgres_database_name(prefix);
    let target_url = postgres_database_url(&admin_url, &database_name);
    let admin = connect_postgres(&admin_url)
        .await
        .map_err(|error| format!("PostgreSQL admin database must be reachable: {error}"))?;
    drop_postgres_database_if_exists(&admin, &database_name).await?;
    create_postgres_database(&admin, &database_name).await?;

    let test_result = async {
        let db_a = connect_postgres(&target_url).await?;
        Migrator::up(&db_a, None).await?;
        let db_b = connect_postgres(&target_url).await?;
        let result = test(db_a.clone(), db_b.clone()).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        db_a.close().await?;
        db_b.close().await?;
        result
    }
    .await;

    drop_postgres_database_if_exists(&admin, &database_name).await?;
    admin.close().await?;
    test_result
}

fn unix_ms() -> TestResult<u64> {
    Ok(u64::try_from(
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
    )?)
}
