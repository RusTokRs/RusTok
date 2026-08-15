#[cfg(all(test, feature = "mod-blog"))]
mod retained_postgres_evidence {
    use std::{sync::Arc, time::Duration};

    use anyhow::{Context, Result, anyhow, ensure};
    use async_trait::async_trait;
    use rustok_api::Permission;
    use rustok_core::{MigrationSource, UserRole};
    use sea_orm::{
        ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection,
        DatabaseTransaction, QueryResult, Statement,
    };
    use sea_orm_migration::SchemaManager;
    use uuid::Uuid;

    use super::super::{
        keyring_schedule_audit_handoff_postgres::{
            CommentsTcpDelegationScheduleAuditHandoffError,
            PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff,
        },
        keyring_schedule_audit_publication::{
            CommentsTcpDelegationScheduleAuditCanonicalPublication,
            CommentsTcpDelegationScheduleAuditCanonicalWriteError,
            CommentsTcpDelegationScheduleAuditCanonicalWriter,
            SharedCommentsTcpDelegationScheduleAuditCanonicalWriter,
        },
        keyring_schedule_audit_recovery_postgres::{
            CommentsTcpDelegationScheduleAuditRecoveryError,
            CommentsTcpDelegationScheduleAuditRecoveryOutcome,
            PostgresCommentsTcpDelegationScheduleAuditRecoveryStore,
        },
        keyring_schedule_audit_source_retry_postgres::CommentsTcpDelegationScheduleAuditSourceFailureCode,
    };
    use super::{
        CommentsTcpDelegationScheduleAuditOperatorContext,
        CommentsTcpDelegationScheduleAuditOperatorError,
        CommentsTcpDelegationScheduleAuditOperatorRuntime,
    };
    use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};

    const POSTGRES_DATABASE_ENV: &str = "RUSTOK_BLOG_COMMENTS_AUDIT_TEST_DATABASE_URL";
    const SOURCE_TABLE: &str = "blog_comments_tcp_delegation_schedule_audit_outbox";
    const RECOVERY_AUDIT_TABLE: &str =
        "blog_comments_tcp_delegation_schedule_audit_recovery_audits";
    const STATE_KEY: &str = "comments_tcp_delegation_schedule";
    const SELECTED_MIGRATIONS: [&str; 5] = [
        "m20260801_000007_create_blog_comments_delegation_schedule_state",
        "m20260801_000008_create_blog_comments_delegation_schedule_audit_outbox",
        "m20260803_000009_add_blog_comments_audit_canonical_handoff",
        "m20260803_000010_add_blog_comments_audit_source_retry_policy",
        "m20260803_000011_create_blog_comments_audit_recovery",
    ];

    struct PostgresRecoveryEvidenceDb {
        control: DatabaseConnection,
        database_url: String,
        db: DatabaseConnection,
        schema_name: String,
    }

    impl PostgresRecoveryEvidenceDb {
        async fn setup(prefix: &str, apply_migrations: bool) -> Result<Option<Self>> {
            let Some(database_url) = postgres_database_url() else {
                eprintln!(
                    "{POSTGRES_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Blog Comments recovery PostgreSQL evidence"
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

            if apply_migrations {
                let manager = SchemaManager::new(&db);
                let mut applied = Vec::new();
                let migration_result = async {
                    for migration in rustok_blog::BlogModule.migrations() {
                        let name = migration.name().to_string();
                        if SELECTED_MIGRATIONS.contains(&name.as_str()) {
                            migration.up(&manager).await?;
                            applied.push(name);
                        }
                    }
                    Ok::<(), sea_orm::DbErr>(())
                }
                .await;

                if let Err(error) = migration_result {
                    let _ = control
                        .execute_unprepared(&format!(
                            r#"DROP SCHEMA IF EXISTS "{schema_name}" CASCADE"#
                        ))
                        .await;
                    return Err(error.into());
                }
                ensure!(
                    applied == SELECTED_MIGRATIONS,
                    "unexpected Blog audit migration subset: {applied:?}"
                );
                seed_schedule_state(&db).await?;
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
            self.control
                .execute_unprepared(&format!(
                    r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                    self.schema_name
                ))
                .await?;
            Ok(())
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

    struct UnusedCanonicalWriter;

    #[async_trait]
    impl CommentsTcpDelegationScheduleAuditCanonicalWriter for UnusedCanonicalWriter {
        async fn write_once_in_transaction(
            &self,
            _transaction: &DatabaseTransaction,
            _publication: &CommentsTcpDelegationScheduleAuditCanonicalPublication,
        ) -> std::result::Result<Uuid, CommentsTcpDelegationScheduleAuditCanonicalWriteError>
        {
            Err(CommentsTcpDelegationScheduleAuditCanonicalWriteError::Conflict)
        }
    }

    #[tokio::test]
    #[ignore = "requires maintainer PostgreSQL execution"]
    async fn authorization_precedes_validation_and_storage() -> Result<()> {
        let Some(context) = PostgresRecoveryEvidenceDb::setup("authorization_order", false).await?
        else {
            return Ok(());
        };

        let outcome = async {
            let tenant_id = Uuid::new_v4();
            let actor_id = Uuid::new_v4();
            let operator = operator(context.db.clone(), tenant_id)?;
            let operator_context =
                CommentsTcpDelegationScheduleAuditOperatorContext::new(
                    tenant_id,
                    actor_id,
                )?;
            let request_id = Uuid::new_v4();

            let missing = operator
                .inspect_dead_letter(operator_context, request_id)
                .await
                .expect_err("missing authority must fail before empty-schema storage");
            ensure!(
                matches!(
                    missing,
                    CommentsTcpDelegationScheduleAuditOperatorError::MissingRequestAuthority
                ),
                "unexpected missing-authority result: {missing:?}"
            );

            let forbidden = with_rbac_request_scope(
                Some(scope(
                    tenant_id,
                    actor_id,
                    vec![Permission::MODULES_READ],
                )),
                operator.inspect_dead_letter(operator_context, request_id),
            )
            .await
            .expect_err("modules:read must fail before empty-schema storage");
            ensure!(
                matches!(
                    forbidden,
                    CommentsTcpDelegationScheduleAuditOperatorError::Forbidden
                ),
                "unexpected forbidden result: {forbidden:?}"
            );

            let mismatched_context =
                CommentsTcpDelegationScheduleAuditOperatorContext::new(
                    Uuid::new_v4(),
                    actor_id,
                )?;
            let mismatch = operator
                .inspect_dead_letter(mismatched_context, request_id)
                .await
                .expect_err("tenant mismatch must fail before permission lookup");
            ensure!(
                matches!(
                    mismatch,
                    CommentsTcpDelegationScheduleAuditOperatorError::TenantMismatch
                ),
                "unexpected tenant mismatch result: {mismatch:?}"
            );

            let delegated = with_rbac_request_scope(
                Some(scope(
                    tenant_id,
                    actor_id,
                    vec![Permission::MODULES_MANAGE],
                )),
                operator.inspect_dead_letter(operator_context, request_id),
            )
            .await
            .expect_err("authorized inspection must reach the empty schema");
            ensure!(
                matches!(
                    delegated,
                    CommentsTcpDelegationScheduleAuditOperatorError::Recovery(
                        CommentsTcpDelegationScheduleAuditRecoveryError::Unavailable
                    )
                ),
                "authorized inspection did not delegate to storage: {delegated:?}"
            );

            let denied_invalid_requeue = operator
                .requeue_dead_letter(
                    operator_context,
                    Uuid::nil(),
                    0,
                    -1,
                    "",
                )
                .await
                .expect_err("authorization must precede recovery DTO validation");
            ensure!(
                matches!(
                    denied_invalid_requeue,
                    CommentsTcpDelegationScheduleAuditOperatorError::MissingRequestAuthority
                ),
                "invalid denied requeue escaped authorization ordering: {denied_invalid_requeue:?}"
            );

            let validated_after_authorization = with_rbac_request_scope(
                Some(scope(
                    tenant_id,
                    actor_id,
                    vec![Permission::MODULES_MANAGE],
                )),
                operator.requeue_dead_letter(
                    operator_context,
                    Uuid::nil(),
                    0,
                    -1,
                    "",
                ),
            )
            .await
            .expect_err("authorized invalid requeue must reach DTO validation");
            ensure!(
                matches!(
                    validated_after_authorization,
                    CommentsTcpDelegationScheduleAuditOperatorError::Recovery(
                        CommentsTcpDelegationScheduleAuditRecoveryError::InvalidRequest(_)
                    )
                ),
                "authorized invalid requeue did not reach DTO validation: {validated_after_authorization:?}"
            );

            Ok(())
        }
        .await;

        context.cleanup().await?;
        outcome
    }

    #[tokio::test]
    #[ignore = "requires maintainer PostgreSQL execution"]
    async fn exact_inspection_requeue_and_append_only_audit_are_atomic() -> Result<()> {
        let Some(context) = PostgresRecoveryEvidenceDb::setup("exact_requeue", true).await? else {
            return Ok(());
        };

        let outcome = async {
            let tenant_id = Uuid::new_v4();
            let actor_id = Uuid::new_v4();
            let source_actor_id = Uuid::new_v4();
            let request_id = Uuid::new_v4();
            seed_source_row(
                &context.db,
                request_id,
                source_actor_id,
                1,
                2,
                SourceFixtureState::DeadLettered {
                    attempt_count: 8,
                    failure_code: "unavailable",
                },
            )
            .await?;

            let operator = operator(context.db.clone(), tenant_id)?;
            let operator_context =
                CommentsTcpDelegationScheduleAuditOperatorContext::new(
                    tenant_id,
                    actor_id,
                )?;
            let authority = scope(
                tenant_id,
                actor_id,
                vec![Permission::MODULES_MANAGE],
            );

            let inspection = with_rbac_request_scope(
                Some(authority.clone()),
                operator.inspect_dead_letter(operator_context, request_id),
            )
            .await?
            .context("exact dead letter was not inspectable")?;
            ensure!(inspection.request_id() == request_id);
            ensure!(inspection.attempt_count() == 8);
            ensure!(inspection.recovery_epoch() == 0);
            ensure!(
                inspection.last_failure_code()
                    == Some(
                        CommentsTcpDelegationScheduleAuditSourceFailureCode::Unavailable
                    )
            );
            ensure!(inspection.reason() == "attempt_budget_exhausted");

            let recovery_reason = "operator approved exact source retry";
            let requeued = with_rbac_request_scope(
                Some(authority.clone()),
                operator.requeue_dead_letter(
                    operator_context,
                    request_id,
                    inspection.attempt_count(),
                    inspection.recovery_epoch(),
                    recovery_reason,
                ),
            )
            .await?;
            let (audit_id, recovery_epoch) = match requeued {
                CommentsTcpDelegationScheduleAuditRecoveryOutcome::Requeued {
                    audit_id,
                    request_id: returned_request_id,
                    recovery_epoch,
                } => {
                    ensure!(returned_request_id == request_id);
                    (audit_id, recovery_epoch)
                }
                other => return Err(anyhow!("unexpected requeue outcome: {other:?}")),
            };
            ensure!(recovery_epoch == 1);

            let source = source_state(&context.db, request_id).await?;
            ensure!(source.attempt_count == 0);
            ensure!(source.recovery_epoch == 1);
            ensure!(!source.claimed);
            ensure!(!source.deferred);
            ensure!(!source.failed);
            ensure!(!source.dead_lettered);

            let audit = recovery_audit(&context.db, audit_id).await?;
            ensure!(audit.control_plane_tenant_id == tenant_id);
            ensure!(audit.request_id == request_id);
            ensure!(audit.actor_id == actor_id);
            ensure!(audit.reason == recovery_reason);
            ensure!(audit.prior_attempt_count == 8);
            ensure!(audit.recovery_epoch == 1);

            ensure!(
                context
                    .db
                    .execute_unprepared(&format!(
                        "UPDATE {RECOVERY_AUDIT_TABLE} SET reason = 'changed' WHERE audit_id = '{audit_id}'"
                    ))
                    .await
                    .is_err(),
                "append-only recovery audit accepted UPDATE"
            );
            ensure!(
                context
                    .db
                    .execute_unprepared(&format!(
                        "DELETE FROM {RECOVERY_AUDIT_TABLE} WHERE audit_id = '{audit_id}'"
                    ))
                    .await
                    .is_err(),
                "append-only recovery audit accepted DELETE"
            );

            let after = with_rbac_request_scope(
                Some(authority),
                operator.inspect_dead_letter(operator_context, request_id),
            )
            .await?;
            ensure!(after.is_none(), "requeued row remained inspectable as terminal");

            Ok(())
        }
        .await;

        context.cleanup().await?;
        outcome
    }

    #[tokio::test]
    #[ignore = "requires maintainer PostgreSQL execution"]
    async fn stale_and_non_terminal_requeue_fail_closed() -> Result<()> {
        let Some(context) = PostgresRecoveryEvidenceDb::setup("closed_denials", true).await? else {
            return Ok(());
        };

        let outcome = async {
            let tenant_id = Uuid::new_v4();
            let actor_id = Uuid::new_v4();
            let source_actor_id = Uuid::new_v4();
            let stale_request_id = Uuid::new_v4();
            let pending_request_id = Uuid::new_v4();
            seed_source_row(
                &context.db,
                stale_request_id,
                source_actor_id,
                1,
                2,
                SourceFixtureState::DeadLettered {
                    attempt_count: 8,
                    failure_code: "conflict",
                },
            )
            .await?;
            seed_source_row(
                &context.db,
                pending_request_id,
                source_actor_id,
                2,
                3,
                SourceFixtureState::Pending,
            )
            .await?;

            let operator = operator(context.db.clone(), tenant_id)?;
            let operator_context =
                CommentsTcpDelegationScheduleAuditOperatorContext::new(tenant_id, actor_id)?;
            let authority = scope(tenant_id, actor_id, vec![Permission::MODULES_MANAGE]);

            let stale_attempt = with_rbac_request_scope(
                Some(authority.clone()),
                operator.requeue_dead_letter(
                    operator_context,
                    stale_request_id,
                    7,
                    0,
                    "stale attempt must not recover",
                ),
            )
            .await?;
            ensure!(
                stale_attempt == CommentsTcpDelegationScheduleAuditRecoveryOutcome::StaleInspection
            );

            let stale_epoch = with_rbac_request_scope(
                Some(authority.clone()),
                operator.requeue_dead_letter(
                    operator_context,
                    stale_request_id,
                    8,
                    1,
                    "stale epoch must not recover",
                ),
            )
            .await?;
            ensure!(
                stale_epoch == CommentsTcpDelegationScheduleAuditRecoveryOutcome::StaleInspection
            );

            let non_terminal = with_rbac_request_scope(
                Some(authority),
                operator.requeue_dead_letter(
                    operator_context,
                    pending_request_id,
                    1,
                    0,
                    "pending row must not recover",
                ),
            )
            .await?;
            ensure!(
                non_terminal == CommentsTcpDelegationScheduleAuditRecoveryOutcome::NotDeadLetter
            );

            let stale_state = source_state(&context.db, stale_request_id).await?;
            ensure!(stale_state.attempt_count == 8);
            ensure!(stale_state.recovery_epoch == 0);
            ensure!(stale_state.dead_lettered);
            let pending_state = source_state(&context.db, pending_request_id).await?;
            ensure!(pending_state.attempt_count == 0);
            ensure!(pending_state.recovery_epoch == 0);
            ensure!(!pending_state.dead_lettered);
            ensure!(recovery_audit_count(&context.db).await? == 0);

            Ok(())
        }
        .await;

        context.cleanup().await?;
        outcome
    }

    #[tokio::test]
    #[ignore = "requires maintainer PostgreSQL execution"]
    async fn concurrent_requeue_admits_one_epoch_and_next_worker_claim_starts_at_one() -> Result<()>
    {
        let Some(context) = PostgresRecoveryEvidenceDb::setup("concurrent_epoch", true).await?
        else {
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

            let peer = context.peer().await?;
            let first = operator(context.db.clone(), tenant_id)?;
            let second = operator(peer, tenant_id)?;
            let operator_context =
                CommentsTcpDelegationScheduleAuditOperatorContext::new(tenant_id, actor_id)?;
            let first_scope = scope(tenant_id, actor_id, vec![Permission::MODULES_MANAGE]);
            let second_scope = first_scope.clone();

            let first_task = tokio::spawn(async move {
                with_rbac_request_scope(
                    Some(first_scope),
                    first.requeue_dead_letter(
                        operator_context,
                        request_id,
                        8,
                        0,
                        "concurrent operator recovery",
                    ),
                )
                .await
            });
            let second_task = tokio::spawn(async move {
                with_rbac_request_scope(
                    Some(second_scope),
                    second.requeue_dead_letter(
                        operator_context,
                        request_id,
                        8,
                        0,
                        "concurrent operator recovery",
                    ),
                )
                .await
            });

            let first_outcome = first_task
                .await
                .context("first recovery task failed to join")??;
            let second_outcome = second_task
                .await
                .context("second recovery task failed to join")??;
            let outcomes = [first_outcome, second_outcome];
            ensure!(
                outcomes
                    .iter()
                    .filter(|outcome| matches!(
                        outcome,
                        CommentsTcpDelegationScheduleAuditRecoveryOutcome::Requeued { .. }
                    ))
                    .count()
                    == 1,
                "concurrent recovery did not admit exactly one winner: {outcomes:?}"
            );
            ensure!(
                outcomes.iter().any(|outcome| matches!(
                    outcome,
                    CommentsTcpDelegationScheduleAuditRecoveryOutcome::NotDeadLetter
                        | CommentsTcpDelegationScheduleAuditRecoveryOutcome::StaleInspection
                )),
                "concurrent recovery loser was not closed: {outcomes:?}"
            );
            ensure!(recovery_audit_count(&context.db).await? == 1);

            let recovered = source_state(&context.db, request_id).await?;
            ensure!(recovered.attempt_count == 0);
            ensure!(recovered.recovery_epoch == 1);
            ensure!(!recovered.dead_lettered);

            let writer: SharedCommentsTcpDelegationScheduleAuditCanonicalWriter =
                Arc::new(UnusedCanonicalWriter);
            let handoff = PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff::new(
                context.db.clone(),
                tenant_id,
                writer,
                Duration::from_secs(60),
            )
            .map_err(anyhow::Error::msg)?;
            let claim = handoff
                .claim_next_retry_ready(8)
                .await
                .map_err(map_handoff_error)?
                .context("recovered source row was not admitted by retry-aware claim")?;
            ensure!(claim.request_id() == request_id);
            ensure!(claim.attempt_count() == 1);
            ensure!(!claim.claim_token().is_nil());

            let claimed = source_state(&context.db, request_id).await?;
            ensure!(claimed.attempt_count == 1);
            ensure!(claimed.recovery_epoch == 1);
            ensure!(claimed.claimed);
            ensure!(!claimed.dead_lettered);

            Ok(())
        }
        .await;

        context.cleanup().await?;
        outcome
    }

    fn operator(
        db: DatabaseConnection,
        control_plane_tenant_id: Uuid,
    ) -> Result<CommentsTcpDelegationScheduleAuditOperatorRuntime> {
        let recovery = PostgresCommentsTcpDelegationScheduleAuditRecoveryStore::new(db)
            .map_err(|error| anyhow!(error))?;
        Ok(CommentsTcpDelegationScheduleAuditOperatorRuntime::new(
            control_plane_tenant_id,
            recovery,
        ))
    }

    fn scope(tenant_id: Uuid, actor_id: Uuid, permissions: Vec<Permission>) -> RbacRequestScope {
        RbacRequestScope::new(tenant_id, actor_id, permissions, UserRole::Admin)
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
        claimed: bool,
        deferred: bool,
        failed: bool,
        dead_lettered: bool,
    }

    async fn source_state(db: &DatabaseConnection, request_id: Uuid) -> Result<StoredSourceState> {
        let row = query_one(
            db,
            format!(
                "SELECT handoff_attempt_count, handoff_recovery_epoch, \
                        handoff_claim_token IS NOT NULL AS claimed, \
                        handoff_next_attempt_at IS NOT NULL AS deferred, \
                        handoff_last_failure_at IS NOT NULL AS failed, \
                        handoff_dead_lettered_at IS NOT NULL AS dead_lettered \
                 FROM {SOURCE_TABLE} WHERE request_id = '{request_id}'"
            ),
        )
        .await?;
        Ok(StoredSourceState {
            attempt_count: row.try_get("", "handoff_attempt_count")?,
            recovery_epoch: row.try_get("", "handoff_recovery_epoch")?,
            claimed: row.try_get("", "claimed")?,
            deferred: row.try_get("", "deferred")?,
            failed: row.try_get("", "failed")?,
            dead_lettered: row.try_get("", "dead_lettered")?,
        })
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

    async fn recovery_audit_count(db: &DatabaseConnection) -> Result<i64> {
        let row = query_one(
            db,
            format!("SELECT COUNT(*)::bigint AS value FROM {RECOVERY_AUDIT_TABLE}"),
        )
        .await?;
        Ok(row.try_get("", "value")?)
    }

    async fn query_one(db: &DatabaseConnection, sql: String) -> Result<QueryResult> {
        db.query_one(Statement::from_string(DatabaseBackend::Postgres, sql))
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
        anyhow!("retry-aware handoff claim failed: {error:?}")
    }
}
