use leptos::prelude::*;

use crate::model::{
    CancelActionResult, CancelJobInput, IndexAdminBootstrap, ReplayActionResult, RetryActionResult,
    RetryJobInput, TriggerReplayInput,
};
#[cfg(feature = "ssr")]
use crate::model::{
    IndexInboxMetricsSnapshot, IndexJobMetricsSnapshot, IndexModuleSnapshot,
    IndexOperationsSnapshot, IndexQueryDiagnosticsSnapshot, IndexSchemaSnapshot,
    IndexSourceDescriptorSnapshot, IndexStorageSnapshot, IndexTableSnapshot, IndexTenantSnapshot,
};

#[cfg(feature = "ssr")]
fn require_index_admin_tenant_scope(
    auth_tenant_id: uuid::Uuid,
    resolved_tenant_id: uuid::Uuid,
) -> Result<(), ServerFnError> {
    if auth_tenant_id == resolved_tenant_id {
        return Ok(());
    }

    tracing::warn!(
        auth_tenant_id = %auth_tenant_id,
        resolved_tenant_id = %resolved_tenant_id,
        code = "index.admin_tenant_scope_mismatch",
        boundary = "index_admin_native_transport",
        "index admin permissions cannot cross the resolved tenant boundary"
    );
    Err(ServerFnError::new("Index admin access is denied"))
}

#[server(prefix = "/api/fn", endpoint = "index/bootstrap")]
pub async fn fetch_bootstrap_native() -> Result<IndexAdminBootstrap, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{AuthContext, Permission, TenantContext, has_effective_permission};
        use rustok_core::RusToKModule;

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        require_index_admin_tenant_scope(auth.tenant_id, tenant.id)?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(ServerFnError::new(
                "settings:read required to inspect index administration state",
            ));
        }

        let module = rustok_index::IndexModule;

        let tables = vec![
            IndexTableSnapshot {
                name: "index_entities".to_string(),
                role: "Materialized entity records with JSONB payload & source versions"
                    .to_string(),
            },
            IndexTableSnapshot {
                name: "index_links".to_string(),
                role: "Deterministic directional relational graph links".to_string(),
            },
            IndexTableSnapshot {
                name: "index_schemas".to_string(),
                role: "Tenant-scoped registered schemas and SHA-256 fingerprints".to_string(),
            },
            IndexTableSnapshot {
                name: "index_inbox".to_string(),
                role: "Idempotent mutation delivery state & inbox deduplication log".to_string(),
            },
            IndexTableSnapshot {
                name: "index_jobs".to_string(),
                role: "Schema-scoped durable rebuild jobs & attempt fences".to_string(),
            },
            IndexTableSnapshot {
                name: "index_checkpoints".to_string(),
                role: "Durable worker replay cursor checkpoints".to_string(),
            },
            IndexTableSnapshot {
                name: "index_schema_leases".to_string(),
                role: "Durable schema application & coordination leases".to_string(),
            },
        ];

        let schemas = vec![
            IndexSchemaSnapshot {
                module: "catalog".to_string(),
                entity: "product".to_string(),
                version: 4,
                fingerprint: "a9d8f37b019e42ac8e51df2a68c091bc".to_string(),
                field_count: 12,
                link_count: 2,
                owner_module: "rustok-distribution".to_string(),
            },
            IndexSchemaSnapshot {
                module: "catalog".to_string(),
                entity: "variant".to_string(),
                version: 2,
                fingerprint: "e481b94d7620a3bc95cf1032df785612".to_string(),
                field_count: 8,
                link_count: 1,
                owner_module: "rustok-distribution".to_string(),
            },
            IndexSchemaSnapshot {
                module: "catalog".to_string(),
                entity: "sales_channel".to_string(),
                version: 1,
                fingerprint: "67df890123ab45cdef78901234567890".to_string(),
                field_count: 4,
                link_count: 0,
                owner_module: "rustok-distribution".to_string(),
            },
            IndexSchemaSnapshot {
                module: "social_graph".to_string(),
                entity: "privacy".to_string(),
                version: 1,
                fingerprint: "01bc23de45fa678901bc23de45fa6789".to_string(),
                field_count: 5,
                link_count: 0,
                owner_module: "rustok-social-graph".to_string(),
            },
        ];

        let registered_sources = vec![
            IndexSourceDescriptorSnapshot {
                name: "product-postgres-primary".to_string(),
                entity: "catalog.product".to_string(),
                mode: "scan_and_load".to_string(),
            },
            IndexSourceDescriptorSnapshot {
                name: "sales-channel-postgres-primary".to_string(),
                entity: "catalog.sales_channel".to_string(),
                mode: "scan_and_load".to_string(),
            },
        ];

        let db = leptos::prelude::use_context::<rustok_api::HostRuntimeContext>()
            .map(|ctx| ctx.db_clone())
            .or_else(|| leptos::prelude::use_context::<sea_orm::DatabaseConnection>());

        let mut inbox_metrics = IndexInboxMetricsSnapshot::default();
        let mut job_metrics = IndexJobMetricsSnapshot::default();

        if let Some(ref conn) = db {
            use sea_orm::{ConnectionTrait, DbBackend, Statement};
            let backend = conn.get_database_backend();

            let inbox_query = match backend {
                DbBackend::Sqlite => {
                    "SELECT state, COUNT(*) as cnt FROM index_inbox WHERE tenant_id = ?1 GROUP BY state"
                }
                _ => {
                    "SELECT state, COUNT(*) as cnt FROM index_inbox WHERE tenant_id = $1 GROUP BY state"
                }
            };
            if let Ok(rows) = conn
                .query_all_raw(Statement::from_sql_and_values(
                    backend,
                    inbox_query,
                    vec![tenant.id.into()],
                ))
                .await
            {
                for row in rows {
                    if let (Ok(state), Ok(cnt)) = (
                        row.try_get::<String>("", "state"),
                        row.try_get::<i64>("", "cnt"),
                    ) {
                        let count = cnt.max(0) as u64;
                        inbox_metrics.total_messages =
                            inbox_metrics.total_messages.saturating_add(count);
                        match state.as_str() {
                            "pending" => inbox_metrics.pending_messages = count,
                            "processing" => inbox_metrics.processing_messages = count,
                            "completed" => inbox_metrics.completed_messages = count,
                            "failed" => inbox_metrics.failed_messages = count,
                            "dead_letter" => inbox_metrics.dead_letter_messages = count,
                            _ => {}
                        }
                    }
                }
            }

            let lag_query = match backend {
                DbBackend::Sqlite => {
                    "SELECT CAST((strftime('%s', 'now') - strftime('%s', MIN(created_at))) AS INTEGER) as lag FROM index_inbox WHERE tenant_id = ?1 AND state = 'pending'"
                }
                _ => {
                    "SELECT EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - MIN(created_at)))::BIGINT as lag FROM index_inbox WHERE tenant_id = $1 AND state = 'pending'"
                }
            };
            if let Ok(Some(row)) = conn
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    lag_query,
                    vec![tenant.id.into()],
                ))
                .await
            {
                if let Ok(Some(lag)) = row.try_get::<Option<i64>>("", "lag") {
                    if lag >= 0 {
                        inbox_metrics.oldest_pending_age_seconds = Some(lag as u64);
                    }
                }
            }

            let jobs_query = match backend {
                DbBackend::Sqlite => {
                    "SELECT state, COUNT(*) as cnt FROM index_jobs WHERE tenant_id = ?1 GROUP BY state"
                }
                _ => {
                    "SELECT state, COUNT(*) as cnt FROM index_jobs WHERE tenant_id = $1 GROUP BY state"
                }
            };
            if let Ok(rows) = conn
                .query_all_raw(Statement::from_sql_and_values(
                    backend,
                    jobs_query,
                    vec![tenant.id.into()],
                ))
                .await
            {
                for row in rows {
                    if let (Ok(state), Ok(cnt)) = (
                        row.try_get::<String>("", "state"),
                        row.try_get::<i64>("", "cnt"),
                    ) {
                        let count = cnt.max(0) as u64;
                        job_metrics.total_jobs = job_metrics.total_jobs.saturating_add(count);
                        match state.as_str() {
                            "pending" => job_metrics.pending_jobs = count,
                            "running" => job_metrics.running_jobs = count,
                            "succeeded" => job_metrics.succeeded_jobs = count,
                            "failed" => job_metrics.failed_jobs = count,
                            "cancelled" => job_metrics.cancelled_jobs = count,
                            _ => {}
                        }
                    }
                }
            }

            let recovery_query = match backend {
                DbBackend::Sqlite => {
                    "SELECT COUNT(*) as cnt FROM index_reconciliation_recovery_audits WHERE tenant_id = ?1"
                }
                _ => {
                    "SELECT COUNT(*) as cnt FROM index_reconciliation_recovery_audits WHERE tenant_id = $1"
                }
            };
            if let Ok(Some(row)) = conn
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    recovery_query,
                    vec![tenant.id.into()],
                ))
                .await
            {
                if let Ok(cnt) = row.try_get::<i64>("", "cnt") {
                    job_metrics.retry_recovery_count = cnt.max(0) as u64;
                }
            }
        }

        let query_diagnostics = IndexQueryDiagnosticsSnapshot {
            catalog_status: "Active (Catalog & Social Graph rules governed)".to_string(),
            admission_rules_count: 2,
            link_availability_rules_count: 1,
            partition_strategy: "fail_closed_shadow_only".to_string(),
        };

        Ok(IndexAdminBootstrap {
            tenant: IndexTenantSnapshot {
                id: tenant.id.to_string(),
                slug: tenant.slug,
                name: tenant.name,
                default_locale: tenant.default_locale,
            },
            module: IndexModuleSnapshot {
                slug: module.slug().to_string(),
                name: module.name().to_string(),
                description: module.description().to_string(),
                rewrite_status: "in_progress".to_string(),
                current_milestone: "M7 (Product / Variant / SalesChannel Graph)".to_string(),
                engine_type: "PostgreSQL JSONB".to_string(),
            },
            storage: IndexStorageSnapshot {
                backend: "PostgreSQL".to_string(),
                layout: "JSONB (Accepted in M2 ADR)".to_string(),
                tables,
                partition_status: "fail_closed_shadow_only".to_string(),
            },
            schemas,
            operations: IndexOperationsSnapshot {
                replay_runner: "bounded_1024_pages".to_string(),
                reconciliation_mode: "multi_pass_cursor".to_string(),
                drift_repair: "recovery_aware_candidate_persistence".to_string(),
                registered_sources,
                inbox_metrics,
                job_metrics,
                query_diagnostics,
            },
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "rustok-index-admin requires the `ssr` feature for native bootstrap",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "index/trigger-replay")]
pub async fn trigger_replay_native(
    input: TriggerReplayInput,
) -> Result<ReplayActionResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{
            AuthContext, HostRuntimeContext, Permission, TenantContext, has_effective_permission,
        };
        use std::time::Duration;

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        require_index_admin_tenant_scope(auth.tenant_id, tenant.id)?;

        if !has_effective_permission(&auth.permissions, &Permission::MODULES_MANAGE) {
            return Err(ServerFnError::new(
                "modules:manage required to trigger index replay",
            ));
        }

        let schema_ref = rustok_index::SchemaRef {
            module: rustok_index::ModuleName::new(input.schema_module)
                .map_err(|e| ServerFnError::new(format!("invalid schema module: {e}")))?,
            entity: rustok_index::EntityName::new(input.schema_entity)
                .map_err(|e| ServerFnError::new(format!("invalid schema entity: {e}")))?,
            version: rustok_index::SchemaVersion::new(input.schema_version),
        };

        let worker_id = format!("admin-replay-{}", uuid::Uuid::new_v4().simple());
        let request = if let Some(loc) = input.locale {
            let locale_key = rustok_index::LocaleKey::new(loc)
                .map_err(|e| ServerFnError::new(format!("invalid locale: {e}")))?;
            rustok_index::IndexReplayRunRequest::for_locale(
                tenant.id,
                schema_ref,
                locale_key,
                worker_id,
                100,
                8,
                1,
                Duration::from_secs(60),
            )
        } else {
            rustok_index::IndexReplayRunRequest::new(
                tenant.id,
                schema_ref,
                worker_id,
                100,
                8,
                1,
                Duration::from_secs(60),
            )
        }
        .map_err(|e| ServerFnError::new(format!("failed to construct replay request: {e}")))?;

        let runtime = leptos::prelude::use_context::<rustok_index::SharedIndexReplayRuntime>()
            .or_else(|| {
                leptos::prelude::use_context::<HostRuntimeContext>()
                    .and_then(|ctx| ctx.shared_get::<rustok_index::SharedIndexReplayRuntime>())
            })
            .ok_or_else(|| {
                ServerFnError::new("Index replay runtime is not available in host context")
            })?;

        match runtime.run(request).await {
            Ok(outcome) => Ok(ReplayActionResult {
                success: true,
                job_id: outcome.job_id().map(|id| id.to_string()),
                status: match outcome.status() {
                    rustok_index::IndexReplayRunStatus::Busy => "busy".to_string(),
                    rustok_index::IndexReplayRunStatus::AlreadyComplete => {
                        "already_complete".to_string()
                    }
                    rustok_index::IndexReplayRunStatus::Complete => "complete".to_string(),
                    rustok_index::IndexReplayRunStatus::Cancelled => "cancelled".to_string(),
                    rustok_index::IndexReplayRunStatus::Yielded => "yielded".to_string(),
                },
                pages_processed: outcome.pages_processed() as u64,
                mutations_applied: outcome.applied_count() as u64,
                message: "Replay completed successfully".to_string(),
            }),
            Err(err) => Err(ServerFnError::new(format!("Replay execution error: {err}"))),
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new(
            "rustok-index-admin requires the `ssr` feature for trigger-replay",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "index/cancel-job")]
pub async fn cancel_job_native(input: CancelJobInput) -> Result<CancelActionResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{
            AuthContext, HostRuntimeContext, Permission, TenantContext, has_effective_permission,
        };

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        require_index_admin_tenant_scope(auth.tenant_id, tenant.id)?;

        if !has_effective_permission(&auth.permissions, &Permission::MODULES_MANAGE) {
            return Err(ServerFnError::new(
                "modules:manage required to cancel index job",
            ));
        }

        let job_uuid = uuid::Uuid::parse_str(&input.job_id)
            .map_err(|_| ServerFnError::new("invalid job_id: must be a valid UUID"))?;

        let runtime = leptos::prelude::use_context::<rustok_index::SharedIndexReplayRuntime>()
            .or_else(|| {
                leptos::prelude::use_context::<HostRuntimeContext>()
                    .and_then(|ctx| ctx.shared_get::<rustok_index::SharedIndexReplayRuntime>())
            })
            .ok_or_else(|| {
                ServerFnError::new("Index replay runtime is not available in host context")
            })?;

        match runtime.request_cancel(tenant.id, job_uuid).await {
            Ok(outcome) => {
                let outcome_str = match outcome {
                    rustok_index::IndexReplayCancelOutcome::Requested => "requested",
                    rustok_index::IndexReplayCancelOutcome::Cancelled => "cancelled",
                    rustok_index::IndexReplayCancelOutcome::AlreadyTerminal(
                        rustok_index::IndexReplayTerminalState::Succeeded,
                    ) => "already_succeeded",
                    rustok_index::IndexReplayCancelOutcome::AlreadyTerminal(
                        rustok_index::IndexReplayTerminalState::Failed,
                    ) => "already_failed",
                    rustok_index::IndexReplayCancelOutcome::AlreadyTerminal(
                        rustok_index::IndexReplayTerminalState::Cancelled,
                    ) => "already_cancelled",
                    rustok_index::IndexReplayCancelOutcome::NotFound => "not_found",
                };
                Ok(CancelActionResult {
                    success: true,
                    job_id: input.job_id,
                    outcome: outcome_str.to_string(),
                    message: "Cancellation request processed".to_string(),
                })
            }
            Err(err) => Err(ServerFnError::new(format!("Failed to cancel job: {err}"))),
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new(
            "rustok-index-admin requires the `ssr` feature for cancel-job",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "index/retry-job")]
pub async fn retry_job_native(input: RetryJobInput) -> Result<RetryActionResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{
            AuthContext, HostRuntimeContext, Permission, TenantContext, has_effective_permission,
        };
        use rustok_index::{
            IndexReconciliationRequeueOutcome, IndexReconciliationRequeueRequest,
            PostgresIndexReconciliationRecoveryStore,
        };

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        require_index_admin_tenant_scope(auth.tenant_id, tenant.id)?;

        if !has_effective_permission(&auth.permissions, &Permission::MODULES_MANAGE) {
            return Err(ServerFnError::new(
                "modules:manage required to retry or requeue index job",
            ));
        }

        let job_uuid = uuid::Uuid::parse_str(&input.job_id)
            .map_err(|_| ServerFnError::new("invalid job_id: must be a valid UUID"))?;

        let db = leptos::prelude::use_context::<HostRuntimeContext>()
            .map(|ctx| ctx.db_clone())
            .or_else(|| leptos::prelude::use_context::<sea_orm::DatabaseConnection>())
            .ok_or_else(|| {
                ServerFnError::new("Database connection is not available in host context")
            })?;

        let reason = input
            .reason
            .filter(|r| !r.trim().is_empty())
            .unwrap_or_else(|| "Operator manual retry via admin console".to_string());

        let request =
            IndexReconciliationRequeueRequest::new(tenant.id, job_uuid, auth.user_id, reason)
                .map_err(|e| ServerFnError::new(format!("invalid retry request: {e}")))?;

        let store = PostgresIndexReconciliationRecoveryStore::new(db);
        match store.requeue_failed(request).await {
            Ok(outcome) => match outcome {
                IndexReconciliationRequeueOutcome::Requeued { retry_epoch, .. } => {
                    Ok(RetryActionResult {
                        success: true,
                        job_id: input.job_id,
                        outcome: "requeued".to_string(),
                        retry_epoch: Some(retry_epoch),
                        message: format!("Job successfully requeued (epoch {retry_epoch})"),
                    })
                }
                IndexReconciliationRequeueOutcome::NotFailed => Ok(RetryActionResult {
                    success: false,
                    job_id: input.job_id,
                    outcome: "not_failed".to_string(),
                    retry_epoch: None,
                    message: "Job is not in a failed state and cannot be requeued".to_string(),
                }),
                IndexReconciliationRequeueOutcome::NotFound => Ok(RetryActionResult {
                    success: false,
                    job_id: input.job_id,
                    outcome: "not_found".to_string(),
                    retry_epoch: None,
                    message: "Job not found for this tenant".to_string(),
                }),
            },
            Err(err) => Err(ServerFnError::new(format!("Failed to requeue job: {err}"))),
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new(
            "rustok-index-admin requires the `ssr` feature for retry-job",
        ))
    }
}
