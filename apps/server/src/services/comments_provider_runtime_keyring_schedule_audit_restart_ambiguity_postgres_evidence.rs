#[cfg(all(test, feature = "mod-blog"))]
mod retained_restart_ambiguity_evidence {
    use std::{sync::Arc, time::Duration};

    use anyhow::{Context, Result, anyhow, ensure};
    use rustok_core::MigrationSource;
    use sea_orm::{
        ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection,
        QueryResult, Statement,
    };
    use sea_orm_migration::SchemaManager;
    use uuid::Uuid;

    use super::super::{
        keyring_schedule_audit_canonical_writer::RustokOutboxCommentsTcpDelegationScheduleAuditCanonicalWriter,
        keyring_schedule_audit_handoff_postgres::{
            CommentsTcpDelegationScheduleAuditHandoffError,
            PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff,
        },
        keyring_schedule_audit_publication::SharedCommentsTcpDelegationScheduleAuditCanonicalWriter,
        keyring_schedule_audit_recovery_postgres::{
            CommentsTcpDelegationScheduleAuditRecoveryError,
            CommentsTcpDelegationScheduleAuditRecoveryOutcome,
            CommentsTcpDelegationScheduleAuditRecoveryRequest,
            PostgresCommentsTcpDelegationScheduleAuditRecoveryStore,
        },
    };

    const POSTGRES_DATABASE_ENV: &str = "RUSTOK_BLOG_COMMENTS_AUDIT_TEST_DATABASE_URL";
    const SOURCE_TABLE: &str = "blog_comments_tcp_delegation_schedule_audit_outbox";
    const RECOVERY_AUDIT_TABLE: &str =
        "blog_comments_tcp_delegation_schedule_audit_recovery_audits";
    const STATE_KEY: &str = "comments_tcp_delegation_schedule";
    const CANONICAL_EVENT_TYPE: &str = "blog.comments_delegation_schedule.replacement_succeeded";
    const SELECTED_MIGRATIONS: [&str; 5] = [
        "m20260801_000007_create_blog_comments_delegation_schedule_state",
        "m20260801_000008_create_blog_comments_delegation_schedule_audit_outbox",
        "m20260803_000009_add_blog_comments_audit_canonical_handoff",
        "m20260803_000010_add_blog_comments_audit_source_retry_policy",
        "m20260803_000011_create_blog_comments_audit_recovery",
    ];

    struct PostgresRestartEvidenceDb {
        control: DatabaseConnection,
        database_url: String,
        db: DatabaseConnection,
        schema_name: String,
    }

    impl PostgresRestartEvidenceDb {
        async fn setup(prefix: &str) -> Result<Option<Self>> {
            let Some(database_url) = postgres_database_url() else {
                eprintln!(
                    "{POSTGRES_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Blog Comments restart/ambiguity evidence"
                );
                return Ok(None);
            };

            let control = connect(&database_url).await?;
            let schema_name = format!(
                "rustok_blog_comments_audit_{}_{}",
                sanitize_identifier(prefix),
                Uuid::new_v4().simple()
            );
            control
                .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
                .await?;

            let db = connect(&database_url).await?;
            set_search_path(&db, &schema_name).await?;
            let manager = SchemaManager::new(&db);
            let migration_result = async {
                for migration in rustok_outbox::OutboxModule.migrations() {
                    migration.up(&manager).await?;
                }
                let mut applied = Vec::new();
                for migration in rustok_blog::BlogModule.migrations() {
                    let name = migration.name().to_string();
                    if SELECTED_MIGRATIONS.contains(&name.as_str()) {
                        migration.up(&manager).await?;
                        applied.push(name);
                    }
                }
                Ok::<Vec<String>, sea_orm::DbErr>(applied)
            }
            .await;

            let applied = match migration_result {
                Ok(applied) => applied,
                Err(error) => {
                    let _ = drop_schema(&control, &schema_name).await;
                    return Err(error.into());
                }
            };
            let expected = SELECTED_MIGRATIONS
                .iter()
                .map(|name| (*name).to_string())
                .collect::<Vec<_>>();
            if applied != expected {
                let _ = drop_schema(&control, &schema_name).await;
                return Err(anyhow!(
                    "unexpected Blog audit migration subset: {applied:?}"
                ));
            }
            if let Err(error) = seed_schedule_state(&db).await {
                let _ = drop_schema(&control, &schema_name).await;
                return Err(error);
            }

            Ok(Some(Self {
                control,
                database_url,
                db,
                schema_name,
            }))
        }

        async fn peer(&self) -> Result<DatabaseConnection> {
            let db = connect(&self.database_url).await?;
            set_search_path(&db, &self.schema_name).await?;
            Ok(db)
        }

        async fn cleanup(self) -> Result<()> {
            drop_schema(&self.control, &self.schema_name).await
        }
    }

    #[derive(Clone, Copy)]
    enum SourceFixtureState {
        Pending,
        DeadLettered {
            attempt_count: i64,
            failure_code: &'static str,
        },
    }

    #[tokio::test]
    #[ignore = "requires maintainer PostgreSQL execution"]
    async fn active_claim_ack_reconciles_after_owner_restart() -> Result<()> {
        let Some(context) = PostgresRestartEvidenceDb::setup("claim_ack").await? else {
            return Ok(());
        };

        let outcome = async {
            let tenant_id = Uuid::new_v4();
            let request_id = Uuid::new_v4();
            seed_source_row(
                &context.db,
                request_id,
                Uuid::new_v4(),
                1,
                2,
                SourceFixtureState::Pending,
            )
            .await?;

            let original = handoff(context.db.clone(), tenant_id)?;
            let committed_claim = original
                .claim_next_retry_ready(8)
                .await
                .map_err(map_handoff_error)?
                .context("pending source row was not claimed")?;
            drop(original);

            let restarted = handoff(context.peer().await?, tenant_id)?;
            let reconciled = restarted
                .reconcile_claim_for_test(committed_claim.claim_token())
                .await
                .map_err(map_handoff_error)?;
            ensure!(reconciled == committed_claim);

            let wrong_token = restarted
                .reconcile_claim_for_test(Uuid::new_v4())
                .await
                .expect_err("unrelated claim token must not reconcile");
            ensure!(wrong_token == CommentsTcpDelegationScheduleAuditHandoffError::Unavailable);

            let stored = source_state(&context.db, request_id).await?;
            ensure!(stored.attempt_count == 1);
            ensure!(stored.claim_token == Some(committed_claim.claim_token()));
            ensure!(!stored.published);
            ensure!(stored.canonical_envelope_id.is_none());
            Ok(())
        }
        .await;

        context.cleanup().await?;
        outcome
    }

    #[tokio::test]
    #[ignore = "requires maintainer PostgreSQL execution"]
    async fn expired_claim_is_reclaimed_after_restart_and_old_token_is_fenced() -> Result<()> {
        let Some(context) = PostgresRestartEvidenceDb::setup("claim_restart").await? else {
            return Ok(());
        };

        let outcome = async {
            let tenant_id = Uuid::new_v4();
            let request_id = Uuid::new_v4();
            seed_source_row(
                &context.db,
                request_id,
                Uuid::new_v4(),
                1,
                2,
                SourceFixtureState::Pending,
            )
            .await?;

            let first_owner = handoff(context.db.clone(), tenant_id)?;
            let first_claim = first_owner
                .claim_next_retry_ready(8)
                .await
                .map_err(map_handoff_error)?
                .context("first claim was not admitted")?;
            context
                .db
                .execute_unprepared(&format!(
                    "UPDATE {SOURCE_TABLE} SET handoff_claim_expires_at = NOW() - INTERVAL '1 second' WHERE request_id = '{request_id}'"
                ))
                .await?;
            drop(first_owner);

            let restarted = handoff(context.peer().await?, tenant_id)?;
            let second_claim = restarted
                .claim_next_retry_ready(8)
                .await
                .map_err(map_handoff_error)?
                .context("expired claim was not reclaimed after restart")?;
            ensure!(second_claim.request_id() == request_id);
            ensure!(second_claim.attempt_count() == 2);
            ensure!(second_claim.claim_token() != first_claim.claim_token());

            let stale = restarted
                .reconcile_claim_for_test(first_claim.claim_token())
                .await
                .expect_err("replaced claim token must be fenced");
            ensure!(stale == CommentsTcpDelegationScheduleAuditHandoffError::Unavailable);
            let current = restarted
                .reconcile_claim_for_test(second_claim.claim_token())
                .await
                .map_err(map_handoff_error)?;
            ensure!(current == second_claim);

            let stored = source_state(&context.db, request_id).await?;
            ensure!(stored.attempt_count == 2);
            ensure!(stored.claim_token == Some(second_claim.claim_token()));
            Ok(())
        }
        .await;

        context.cleanup().await?;
        outcome
    }

    #[tokio::test]
    #[ignore = "requires maintainer PostgreSQL execution"]
    async fn publication_ack_reconciles_after_restart_without_running_relay() -> Result<()> {
        let Some(context) = PostgresRestartEvidenceDb::setup("publication_ack").await? else {
            return Ok(());
        };

        let outcome = async {
            let tenant_id = Uuid::new_v4();
            let request_id = Uuid::new_v4();
            seed_source_row(
                &context.db,
                request_id,
                Uuid::new_v4(),
                1,
                2,
                SourceFixtureState::Pending,
            )
            .await?;

            let original = handoff(context.db.clone(), tenant_id)?;
            let claim = original
                .claim_next_retry_ready(8)
                .await
                .map_err(map_handoff_error)?
                .context("publication source row was not claimed")?;
            let committed_envelope = original
                .publish_claimed(claim)
                .await
                .map_err(map_handoff_error)?;
            ensure!(committed_envelope == request_id);
            drop(original);

            let restarted = handoff(context.peer().await?, tenant_id)?;
            let reconciled = restarted
                .reconcile_publication_for_test(request_id)
                .await
                .map_err(map_handoff_error)?;
            ensure!(reconciled == request_id);
            let absent = restarted
                .reconcile_publication_for_test(Uuid::new_v4())
                .await
                .expect_err("unknown publication must not reconcile");
            ensure!(absent == CommentsTcpDelegationScheduleAuditHandoffError::Unavailable);

            let stored = source_state(&context.db, request_id).await?;
            ensure!(stored.published);
            ensure!(stored.canonical_envelope_id == Some(request_id));
            ensure!(stored.claim_token.is_none());

            let canonical = canonical_event(&context.db, request_id).await?;
            ensure!(canonical.event_type == CANONICAL_EVENT_TYPE);
            ensure!(canonical.schema_version == 1);
            ensure!(canonical.status == "pending");
            ensure!(canonical.retry_count == 0);
            ensure!(!canonical.claimed);
            ensure!(canonical_event_count(&context.db, request_id).await? == 1);
            Ok(())
        }
        .await;

        context.cleanup().await?;
        outcome
    }

    #[tokio::test]
    #[ignore = "requires maintainer PostgreSQL execution"]
    async fn requeue_ack_reconciles_exact_audit_facts_after_restart() -> Result<()> {
        let Some(context) = PostgresRestartEvidenceDb::setup("requeue_ack").await? else {
            return Ok(());
        };

        let outcome = async {
            let tenant_id = Uuid::new_v4();
            let actor_id = Uuid::new_v4();
            let request_id = Uuid::new_v4();
            seed_source_row(
                &context.db,
                request_id,
                Uuid::new_v4(),
                1,
                2,
                SourceFixtureState::DeadLettered {
                    attempt_count: 8,
                    failure_code: "unavailable",
                },
            )
            .await?;

            let request = CommentsTcpDelegationScheduleAuditRecoveryRequest::new(
                tenant_id,
                request_id,
                actor_id,
                8,
                0,
                "restart-safe operator recovery",
            )?;
            let original = recovery(context.db.clone())?;
            let committed = original.requeue_dead_letter(request.clone()).await?;
            let (audit_id, recovery_epoch) = match committed {
                CommentsTcpDelegationScheduleAuditRecoveryOutcome::Requeued {
                    audit_id,
                    request_id: returned_request_id,
                    recovery_epoch,
                } => {
                    ensure!(returned_request_id == request_id);
                    (audit_id, recovery_epoch)
                }
                other => return Err(anyhow!("unexpected recovery outcome: {other:?}")),
            };
            ensure!(recovery_epoch == 1);
            drop(original);

            let restarted = recovery(context.peer().await?)?;
            let reconciled = restarted
                .reconcile_requeue_for_test(audit_id, &request, recovery_epoch)
                .await?;
            ensure!(reconciled == committed);

            let mismatched = CommentsTcpDelegationScheduleAuditRecoveryRequest::new(
                tenant_id,
                request_id,
                actor_id,
                8,
                0,
                "different recovery reason",
            )?;
            let mismatch = restarted
                .reconcile_requeue_for_test(audit_id, &mismatched, recovery_epoch)
                .await
                .expect_err("mismatched immutable audit facts must fail reconciliation");
            ensure!(
                mismatch == CommentsTcpDelegationScheduleAuditRecoveryError::InvalidStoredState
            );

            let stored = source_state(&context.db, request_id).await?;
            ensure!(stored.attempt_count == 0);
            ensure!(stored.recovery_epoch == 1);
            ensure!(!stored.dead_lettered);
            ensure!(stored.claim_token.is_none());

            let audit = recovery_audit(&context.db, audit_id).await?;
            ensure!(audit.control_plane_tenant_id == tenant_id);
            ensure!(audit.request_id == request_id);
            ensure!(audit.actor_id == actor_id);
            ensure!(audit.reason == request.reason());
            ensure!(audit.prior_attempt_count == 8);
            ensure!(audit.recovery_epoch == 1);
            ensure!(recovery_audit_count(&context.db, request_id).await? == 1);
            Ok(())
        }
        .await;

        context.cleanup().await?;
        outcome
    }

    fn handoff(
        db: DatabaseConnection,
        control_plane_tenant_id: Uuid,
    ) -> Result<PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff> {
        let writer: SharedCommentsTcpDelegationScheduleAuditCanonicalWriter =
            Arc::new(RustokOutboxCommentsTcpDelegationScheduleAuditCanonicalWriter);
        PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff::new(
            db,
            control_plane_tenant_id,
            writer,
            Duration::from_secs(60),
        )
        .map_err(anyhow::Error::msg)
    }

    fn recovery(
        db: DatabaseConnection,
    ) -> Result<PostgresCommentsTcpDelegationScheduleAuditRecoveryStore> {
        PostgresCommentsTcpDelegationScheduleAuditRecoveryStore::new(db).map_err(anyhow::Error::msg)
    }

    async fn seed_schedule_state(db: &DatabaseConnection) -> Result<()> {
        db.execute_unprepared(&format!(
            "INSERT INTO blog_comments_tcp_delegation_schedule_state \
                 (state_key, schema_version, source, generation, schedule_digest_hex) \
             VALUES ('{STATE_KEY}', 1, 'host_provided', 1, '{}')",
            "a".repeat(64)
        ))
        .await?;
        Ok(())
    }

    async fn seed_source_row(
        db: &DatabaseConnection,
        request_id: Uuid,
        actor_id: Uuid,
        previous_generation: i64,
        candidate_generation: i64,
        state: SourceFixtureState,
    ) -> Result<()> {
        let (attempt_count, failure_at, failure_code, dead_lettered_at, dead_letter_reason) =
            match state {
                SourceFixtureState::Pending => (0, "NULL", "NULL", "NULL", "NULL"),
                SourceFixtureState::DeadLettered {
                    attempt_count,
                    failure_code,
                } => (
                    attempt_count,
                    "CURRENT_TIMESTAMP",
                    if failure_code == "conflict" {
                        "'conflict'"
                    } else {
                        "'unavailable'"
                    },
                    "CURRENT_TIMESTAMP",
                    "'attempt_budget_exhausted'",
                ),
            };
        db.execute_unprepared(&format!(
            "INSERT INTO {SOURCE_TABLE} ( \
                 request_id, state_key, audit_schema_version, event_type, \
                 occurred_at_unix_ms, actor_id, principal_kind, operation, source, \
                 previous_generation, candidate_generation, outcome, \
                 handoff_attempt_count, handoff_last_failure_at, \
                 handoff_last_failure_code, handoff_dead_lettered_at, \
                 handoff_dead_letter_reason \
             ) VALUES ( \
                 '{request_id}', '{STATE_KEY}', 1, \
                 'comments_tcp_delegation_schedule_replaced', 1, '{actor_id}', \
                 'direct_user', 'replace_host_schedule', 'host_provided', \
                 {previous_generation}, {candidate_generation}, \
                 'replacement_succeeded', {attempt_count}, {failure_at}, \
                 {failure_code}, {dead_lettered_at}, {dead_letter_reason} \
             )"
        ))
        .await?;
        Ok(())
    }

    #[derive(Debug)]
    struct StoredSourceState {
        attempt_count: i64,
        recovery_epoch: i64,
        claim_token: Option<Uuid>,
        dead_lettered: bool,
        canonical_envelope_id: Option<Uuid>,
        published: bool,
    }

    async fn source_state(db: &DatabaseConnection, request_id: Uuid) -> Result<StoredSourceState> {
        let row = query_one(
            db,
            format!(
                "SELECT handoff_attempt_count, handoff_recovery_epoch, \
                        handoff_claim_token, handoff_dead_lettered_at IS NOT NULL AS dead_lettered, \
                        canonical_envelope_id, published_at IS NOT NULL AS published \
                 FROM {SOURCE_TABLE} WHERE request_id = '{request_id}'"
            ),
        )
        .await?;
        Ok(StoredSourceState {
            attempt_count: row.try_get("", "handoff_attempt_count")?,
            recovery_epoch: row.try_get("", "handoff_recovery_epoch")?,
            claim_token: row.try_get("", "handoff_claim_token")?,
            dead_lettered: row.try_get("", "dead_lettered")?,
            canonical_envelope_id: row.try_get("", "canonical_envelope_id")?,
            published: row.try_get("", "published")?,
        })
    }

    #[derive(Debug)]
    struct StoredCanonicalEvent {
        event_type: String,
        schema_version: i16,
        status: String,
        retry_count: i32,
        claimed: bool,
    }

    async fn canonical_event(
        db: &DatabaseConnection,
        request_id: Uuid,
    ) -> Result<StoredCanonicalEvent> {
        let row = query_one(
            db,
            format!(
                "SELECT event_type, schema_version, status::text AS status, retry_count, \
                        claimed_by IS NOT NULL AS claimed \
                 FROM sys_events WHERE id = '{request_id}'"
            ),
        )
        .await?;
        Ok(StoredCanonicalEvent {
            event_type: row.try_get("", "event_type")?,
            schema_version: row.try_get("", "schema_version")?,
            status: row.try_get("", "status")?,
            retry_count: row.try_get("", "retry_count")?,
            claimed: row.try_get("", "claimed")?,
        })
    }

    async fn canonical_event_count(db: &DatabaseConnection, request_id: Uuid) -> Result<i64> {
        scalar_i64(
            db,
            format!("SELECT COUNT(*)::bigint AS value FROM sys_events WHERE id = '{request_id}'"),
        )
        .await
    }

    #[derive(Debug)]
    struct StoredRecoveryAudit {
        control_plane_tenant_id: Uuid,
        request_id: Uuid,
        actor_id: Uuid,
        reason: String,
        prior_attempt_count: i64,
        recovery_epoch: i64,
    }

    async fn recovery_audit(
        db: &DatabaseConnection,
        audit_id: Uuid,
    ) -> Result<StoredRecoveryAudit> {
        let row = query_one(
            db,
            format!(
                "SELECT control_plane_tenant_id, request_id, actor_id, reason, \
                        prior_attempt_count, recovery_epoch \
                 FROM {RECOVERY_AUDIT_TABLE} WHERE audit_id = '{audit_id}'"
            ),
        )
        .await?;
        Ok(StoredRecoveryAudit {
            control_plane_tenant_id: row.try_get("", "control_plane_tenant_id")?,
            request_id: row.try_get("", "request_id")?,
            actor_id: row.try_get("", "actor_id")?,
            reason: row.try_get("", "reason")?,
            prior_attempt_count: row.try_get("", "prior_attempt_count")?,
            recovery_epoch: row.try_get("", "recovery_epoch")?,
        })
    }

    async fn recovery_audit_count(db: &DatabaseConnection, request_id: Uuid) -> Result<i64> {
        scalar_i64(
            db,
            format!(
                "SELECT COUNT(*)::bigint AS value FROM {RECOVERY_AUDIT_TABLE} WHERE request_id = '{request_id}'"
            ),
        )
        .await
    }

    async fn scalar_i64(db: &DatabaseConnection, sql: String) -> Result<i64> {
        let row = query_one(db, sql).await?;
        Ok(row.try_get("", "value")?)
    }

    async fn query_one(db: &DatabaseConnection, sql: String) -> Result<QueryResult> {
        db.query_one_raw(Statement::from_string(DatabaseBackend::Postgres, sql))
            .await?
            .context("expected PostgreSQL evidence row")
    }

    fn postgres_database_url() -> Option<String> {
        std::env::var(POSTGRES_DATABASE_ENV)
            .or_else(|_| std::env::var("DATABASE_URL"))
            .ok()
            .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
    }

    async fn connect(database_url: &str) -> Result<DatabaseConnection> {
        let mut options = ConnectOptions::new(database_url.to_owned());
        options
            .max_connections(1)
            .min_connections(1)
            .sqlx_logging(false);
        Ok(Database::connect(options).await?)
    }

    async fn set_search_path(db: &DatabaseConnection, schema_name: &str) -> Result<()> {
        db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}""#))
            .await?;
        Ok(())
    }

    async fn drop_schema(control: &DatabaseConnection, schema_name: &str) -> Result<()> {
        control
            .execute_unprepared(&format!(r#"DROP SCHEMA IF EXISTS "{schema_name}" CASCADE"#))
            .await?;
        Ok(())
    }

    fn sanitize_identifier(value: &str) -> String {
        let normalized = value
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() {
                    character.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect::<String>();
        let normalized = normalized.trim_matches('_');
        if normalized.is_empty() {
            "test".to_string()
        } else {
            normalized.to_string()
        }
    }

    fn map_handoff_error(error: CommentsTcpDelegationScheduleAuditHandoffError) -> anyhow::Error {
        anyhow!("canonical handoff failed: {error:?}")
    }
}
