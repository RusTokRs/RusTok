use crate::i18n::t;
use crate::model::{
    CancelActionResult, IndexAdminBootstrap, ReplayActionResult, RetryActionResult,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexStatCardViewModel {
    pub label: String,
    pub value: String,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSchemaRowViewModel {
    pub module: String,
    pub entity: String,
    pub raw_version: u32,
    pub qualified_name: String,
    pub version: String,
    pub fingerprint: String,
    pub fields_count: String,
    pub links_count: String,
    pub owner_module: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexTableRowViewModel {
    pub name: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSourceRowViewModel {
    pub name: String,
    pub entity: String,
    pub mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexAdminOverviewViewModel {
    pub stat_cards: Vec<IndexStatCardViewModel>,
    pub module_description: String,
    pub engine_type: String,
    pub current_milestone: String,
    pub rewrite_status: String,
    pub partition_status: String,
    pub tables: Vec<IndexTableRowViewModel>,
    pub schemas: Vec<IndexSchemaRowViewModel>,
    pub sources: Vec<IndexSourceRowViewModel>,
    pub operations_summary: Vec<IndexStatCardViewModel>,
    pub inbox_summary: Vec<IndexStatCardViewModel>,
    pub job_summary: Vec<IndexStatCardViewModel>,
    pub query_diagnostics_summary: Vec<IndexStatCardViewModel>,
}

pub fn build_index_admin_overview_view_model(
    locale: Option<&str>,
    bootstrap: IndexAdminBootstrap,
) -> IndexAdminOverviewViewModel {
    let stat_cards = vec![
        IndexStatCardViewModel {
            label: t(locale, "index.info.tenant", "Tenant"),
            value: bootstrap.tenant.slug,
            hint: Some(bootstrap.tenant.name),
        },
        IndexStatCardViewModel {
            label: t(locale, "index.info.locale", "Default Locale"),
            value: bootstrap.tenant.default_locale,
            hint: None,
        },
        IndexStatCardViewModel {
            label: t(locale, "index.info.engine", "Storage Engine"),
            value: bootstrap.module.engine_type.clone(),
            hint: Some(format!(
                "{} / {}",
                bootstrap.storage.backend, bootstrap.storage.layout
            )),
        },
        IndexStatCardViewModel {
            label: t(locale, "index.info.milestone", "Active Milestone"),
            value: bootstrap.module.current_milestone.clone(),
            hint: Some(bootstrap.module.rewrite_status.clone()),
        },
        IndexStatCardViewModel {
            label: t(locale, "index.info.schemasCount", "Registered Schemas"),
            value: bootstrap.schemas.len().to_string(),
            hint: None,
        },
        IndexStatCardViewModel {
            label: t(locale, "index.info.sourcesCount", "Replay Sources"),
            value: bootstrap.operations.registered_sources.len().to_string(),
            hint: None,
        },
        IndexStatCardViewModel {
            label: t(locale, "index.info.tablesCount", "Canonical Tables"),
            value: bootstrap.storage.tables.len().to_string(),
            hint: Some(bootstrap.storage.partition_status.clone()),
        },
    ];

    let schemas = bootstrap
        .schemas
        .into_iter()
        .map(|s| IndexSchemaRowViewModel {
            module: s.module.clone(),
            entity: s.entity.clone(),
            raw_version: s.version,
            qualified_name: format!("{}.{}", s.module, s.entity),
            version: format!("v{}", s.version),
            fingerprint: if s.fingerprint.len() > 16 {
                format!("{}...", &s.fingerprint[..16])
            } else {
                s.fingerprint
            },
            fields_count: s.field_count.to_string(),
            links_count: s.link_count.to_string(),
            owner_module: s.owner_module,
        })
        .collect();

    let tables = bootstrap
        .storage
        .tables
        .into_iter()
        .map(|t| IndexTableRowViewModel {
            name: t.name,
            role: t.role,
        })
        .collect();

    let sources = bootstrap
        .operations
        .registered_sources
        .into_iter()
        .map(|src| IndexSourceRowViewModel {
            name: src.name,
            entity: src.entity,
            mode: src.mode,
        })
        .collect();

    let operations_summary = vec![
        IndexStatCardViewModel {
            label: t(locale, "index.ops.replayRunner", "Replay Runner"),
            value: bootstrap.operations.replay_runner,
            hint: None,
        },
        IndexStatCardViewModel {
            label: t(locale, "index.ops.reconciliation", "Reconciliation"),
            value: bootstrap.operations.reconciliation_mode,
            hint: None,
        },
        IndexStatCardViewModel {
            label: t(locale, "index.ops.driftRepair", "Drift Repair"),
            value: bootstrap.operations.drift_repair,
            hint: None,
        },
    ];

    let inbox = &bootstrap.operations.inbox_metrics;
    let inbox_summary = vec![
        IndexStatCardViewModel {
            label: t(locale, "index.inbox.total", "Inbox Total"),
            value: inbox.total_messages.to_string(),
            hint: None,
        },
        IndexStatCardViewModel {
            label: t(locale, "index.inbox.pending", "Pending"),
            value: inbox.pending_messages.to_string(),
            hint: inbox
                .oldest_pending_age_seconds
                .map(|s| format!("{} s lag", s)),
        },
        IndexStatCardViewModel {
            label: t(locale, "index.inbox.deadLetter", "Dead-letter"),
            value: inbox.dead_letter_messages.to_string(),
            hint: None,
        },
        IndexStatCardViewModel {
            label: t(locale, "index.inbox.failed", "Failed"),
            value: inbox.failed_messages.to_string(),
            hint: None,
        },
    ];

    let jobs = &bootstrap.operations.job_metrics;
    let job_summary = vec![
        IndexStatCardViewModel {
            label: t(locale, "index.jobs.total", "Jobs Total"),
            value: jobs.total_jobs.to_string(),
            hint: None,
        },
        IndexStatCardViewModel {
            label: t(locale, "index.jobs.running", "Running"),
            value: jobs.running_jobs.to_string(),
            hint: None,
        },
        IndexStatCardViewModel {
            label: t(locale, "index.jobs.failed", "Failed"),
            value: jobs.failed_jobs.to_string(),
            hint: None,
        },
        IndexStatCardViewModel {
            label: t(locale, "index.jobs.recoveryAudits", "Recovery Audits"),
            value: jobs.retry_recovery_count.to_string(),
            hint: None,
        },
    ];

    let qd = &bootstrap.operations.query_diagnostics;
    let query_diagnostics_summary = vec![
        IndexStatCardViewModel {
            label: t(locale, "index.query.catalogStatus", "Query Catalog Status"),
            value: qd.catalog_status.clone(),
            hint: None,
        },
        IndexStatCardViewModel {
            label: t(locale, "index.query.admissionRules", "Admission Rules"),
            value: qd.admission_rules_count.to_string(),
            hint: None,
        },
        IndexStatCardViewModel {
            label: t(locale, "index.query.linkRules", "Link Availability Rules"),
            value: qd.link_availability_rules_count.to_string(),
            hint: None,
        },
        IndexStatCardViewModel {
            label: t(
                locale,
                "index.query.partitionStrategy",
                "Partition Strategy",
            ),
            value: qd.partition_strategy.clone(),
            hint: None,
        },
    ];

    IndexAdminOverviewViewModel {
        stat_cards,
        module_description: bootstrap.module.description,
        engine_type: bootstrap.module.engine_type,
        current_milestone: bootstrap.module.current_milestone,
        rewrite_status: bootstrap.module.rewrite_status,
        partition_status: bootstrap.storage.partition_status,
        tables,
        schemas,
        sources,
        operations_summary,
        inbox_summary,
        job_summary,
        query_diagnostics_summary,
    }
}

pub fn format_index_admin_bootstrap_error(
    locale: Option<&str>,
    error: impl std::fmt::Display,
) -> String {
    format!(
        "{}: {error}",
        t(
            locale,
            "index.error.loadBootstrap",
            "Failed to load index bootstrap"
        )
    )
}

pub fn format_replay_action_result(locale: Option<&str>, result: &ReplayActionResult) -> String {
    if result.success {
        format!(
            "{}: {} ({}: {}, {}: {})",
            t(
                locale,
                "index.action.rebuildSuccess",
                "Rebuild completed successfully"
            ),
            result.status,
            t(locale, "index.action.pagesProcessed", "Pages processed"),
            result.pages_processed,
            t(locale, "index.action.mutationsApplied", "Mutations applied"),
            result.mutations_applied,
        )
    } else {
        result.message.clone()
    }
}

pub fn format_retry_action_result(locale: Option<&str>, result: &RetryActionResult) -> String {
    if result.success {
        let epoch_hint = result
            .retry_epoch
            .map(|e| format!(" (epoch {e})"))
            .unwrap_or_default();
        format!(
            "{}: {}{} (job: {})",
            t(
                locale,
                "index.action.retrySuccess",
                "Job successfully requeued"
            ),
            result.outcome,
            epoch_hint,
            result.job_id,
        )
    } else {
        result.message.clone()
    }
}

pub fn format_cancel_action_result(locale: Option<&str>, result: &CancelActionResult) -> String {
    if result.success {
        format!(
            "{}: {} (job: {})",
            t(
                locale,
                "index.action.cancelSuccess",
                "Cancellation requested successfully"
            ),
            result.outcome,
            result.job_id,
        )
    } else {
        result.message.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn sample_bootstrap() -> IndexAdminBootstrap {
        IndexAdminBootstrap {
            tenant: IndexTenantSnapshot {
                id: "tenant-1".to_string(),
                slug: "demo".to_string(),
                name: "Demo Store".to_string(),
                default_locale: "en".to_string(),
            },
            module: IndexModuleSnapshot {
                slug: "index".to_string(),
                name: "Index".to_string(),
                description: "Cross-module relational index and query engine.".to_string(),
                rewrite_status: "in_progress".to_string(),
                current_milestone: "M7 (Product / Variant / SalesChannel Graph)".to_string(),
                engine_type: "PostgreSQL JSONB".to_string(),
            },
            storage: IndexStorageSnapshot {
                backend: "PostgreSQL".to_string(),
                layout: "JSONB".to_string(),
                partition_status: "fail_closed_shadow_only".to_string(),
                tables: vec![
                    IndexTableSnapshot {
                        name: "index_entities".to_string(),
                        role: "Primary entity records".to_string(),
                    },
                    IndexTableSnapshot {
                        name: "index_links".to_string(),
                        role: "Relational graph links".to_string(),
                    },
                    IndexTableSnapshot {
                        name: "index_schemas".to_string(),
                        role: "Persisted schemas and fingerprints".to_string(),
                    },
                ],
            },
            schemas: vec![
                IndexSchemaSnapshot {
                    module: "catalog".to_string(),
                    entity: "product".to_string(),
                    version: 4,
                    fingerprint: "abcdef1234567890abcdef1234567890".to_string(),
                    field_count: 12,
                    link_count: 2,
                    owner_module: "rustok-distribution".to_string(),
                },
                IndexSchemaSnapshot {
                    module: "catalog".to_string(),
                    entity: "sales_channel".to_string(),
                    version: 1,
                    fingerprint: "1234567890abcdef1234567890abcdef".to_string(),
                    field_count: 4,
                    link_count: 0,
                    owner_module: "rustok-distribution".to_string(),
                },
            ],
            operations: IndexOperationsSnapshot {
                replay_runner: "bounded_1024_pages".to_string(),
                reconciliation_mode: "multi_pass_cursor".to_string(),
                drift_repair: "recovery_aware".to_string(),
                registered_sources: vec![IndexSourceDescriptorSnapshot {
                    name: "product-postgres-primary".to_string(),
                    entity: "product".to_string(),
                    mode: "scan_and_load".to_string(),
                }],
                inbox_metrics: IndexInboxMetricsSnapshot {
                    total_messages: 14,
                    pending_messages: 3,
                    processing_messages: 1,
                    completed_messages: 9,
                    failed_messages: 1,
                    dead_letter_messages: 0,
                    oldest_pending_age_seconds: Some(42),
                },
                job_metrics: IndexJobMetricsSnapshot {
                    total_jobs: 8,
                    pending_jobs: 1,
                    running_jobs: 1,
                    succeeded_jobs: 5,
                    failed_jobs: 1,
                    cancelled_jobs: 0,
                    retry_recovery_count: 2,
                },
                query_diagnostics: IndexQueryDiagnosticsSnapshot {
                    catalog_status: "Active".to_string(),
                    admission_rules_count: 2,
                    link_availability_rules_count: 1,
                    partition_strategy: "fail_closed_shadow_only".to_string(),
                },
            },
        }
    }

    #[test]
    fn overview_view_model_formats_full_state_without_framework_runtime() {
        let view_model = build_index_admin_overview_view_model(Some("en"), sample_bootstrap());

        assert_eq!(view_model.stat_cards.len(), 7);
        assert_eq!(view_model.stat_cards[0].value, "demo");
        assert_eq!(view_model.stat_cards[2].value, "PostgreSQL JSONB");
        assert_eq!(
            view_model.stat_cards[3].value,
            "M7 (Product / Variant / SalesChannel Graph)"
        );
        assert_eq!(view_model.stat_cards[4].value, "2"); // 2 schemas
        assert_eq!(view_model.stat_cards[5].value, "1"); // 1 source
        assert_eq!(view_model.stat_cards[6].value, "3"); // 3 tables

        assert_eq!(view_model.schemas.len(), 2);
        assert_eq!(view_model.schemas[0].qualified_name, "catalog.product");
        assert_eq!(view_model.schemas[0].version, "v4");
        assert_eq!(view_model.schemas[0].fingerprint, "abcdef1234567890...");
        assert_eq!(view_model.schemas[0].fields_count, "12");
        assert_eq!(view_model.schemas[0].links_count, "2");

        assert_eq!(view_model.tables.len(), 3);
        assert_eq!(view_model.tables[0].name, "index_entities");

        assert_eq!(view_model.sources.len(), 1);
        assert_eq!(view_model.sources[0].name, "product-postgres-primary");

        assert_eq!(view_model.operations_summary.len(), 3);
        assert_eq!(view_model.operations_summary[0].value, "bounded_1024_pages");

        // Inbox metrics summary
        assert_eq!(view_model.inbox_summary.len(), 4);
        assert_eq!(view_model.inbox_summary[0].value, "14"); // total
        assert_eq!(view_model.inbox_summary[1].value, "3"); // pending
        assert_eq!(
            view_model.inbox_summary[1].hint,
            Some("42 s lag".to_string())
        );
        assert_eq!(view_model.inbox_summary[2].value, "0"); // dead-letter

        // Job metrics summary
        assert_eq!(view_model.job_summary.len(), 4);
        assert_eq!(view_model.job_summary[0].value, "8"); // total
        assert_eq!(view_model.job_summary[1].value, "1"); // running
        assert_eq!(view_model.job_summary[2].value, "1"); // failed
        assert_eq!(view_model.job_summary[3].value, "2"); // recovery audits

        // Query diagnostics summary
        assert_eq!(view_model.query_diagnostics_summary.len(), 4);
        assert_eq!(view_model.query_diagnostics_summary[1].value, "2"); // admission rules
        assert_eq!(view_model.query_diagnostics_summary[2].value, "1"); // link rules
    }

    #[test]
    fn overview_view_model_localizes_properly_in_russian() {
        let view_model = build_index_admin_overview_view_model(Some("ru"), sample_bootstrap());

        assert_eq!(view_model.stat_cards[0].label, "Тенант");
        assert_eq!(view_model.stat_cards[1].label, "Основная локаль");
        assert_eq!(view_model.stat_cards[2].label, "Движок хранения");
        assert_eq!(view_model.stat_cards[3].label, "Текущий майлстоун");
    }

    #[test]
    fn format_replay_action_result_works_in_english_and_russian() {
        let result = ReplayActionResult {
            success: true,
            job_id: Some("00000000-0000-0000-0000-000000000001".to_string()),
            status: "complete".to_string(),
            pages_processed: 5,
            mutations_applied: 42,
            message: "Replay completed successfully".to_string(),
        };

        let en_str = format_replay_action_result(Some("en"), &result);
        assert!(en_str.contains("Rebuild completed successfully: complete"));
        assert!(en_str.contains("Pages processed: 5"));
        assert!(en_str.contains("Mutations applied: 42"));

        let ru_str = format_replay_action_result(Some("ru"), &result);
        assert!(ru_str.contains("Переиндексация успешно выполнена: complete"));
        assert!(ru_str.contains("Обработано страниц: 5"));
        assert!(ru_str.contains("Применено мутаций: 42"));

        let err_result = ReplayActionResult {
            success: false,
            job_id: None,
            status: "failed".to_string(),
            pages_processed: 0,
            mutations_applied: 0,
            message: "Database connection failed".to_string(),
        };
        assert_eq!(
            format_replay_action_result(Some("en"), &err_result),
            "Database connection failed"
        );
    }

    #[test]
    fn format_cancel_action_result_works_in_english_and_russian() {
        let result = CancelActionResult {
            success: true,
            job_id: "00000000-0000-0000-0000-000000000002".to_string(),
            outcome: "requested".to_string(),
            message: "Cancellation requested".to_string(),
        };

        let en_str = format_cancel_action_result(Some("en"), &result);
        assert!(en_str.contains("Cancellation requested successfully: requested"));
        assert!(en_str.contains("job: 00000000-0000-0000-0000-000000000002"));

        let ru_str = format_cancel_action_result(Some("ru"), &result);
        assert!(ru_str.contains("Запрос на отмену задачи отправлен: requested"));

        let err_result = CancelActionResult {
            success: false,
            job_id: "00000000-0000-0000-0000-000000000002".to_string(),
            outcome: "not_found".to_string(),
            message: "Job not found".to_string(),
        };
        assert_eq!(
            format_cancel_action_result(Some("en"), &err_result),
            "Job not found"
        );
    }

    #[test]
    fn input_structs_can_be_instantiated() {
        let trigger_input = TriggerReplayInput {
            schema_module: "catalog".to_string(),
            schema_entity: "product".to_string(),
            schema_version: 1,
            locale: Some("en".to_string()),
        };
        assert_eq!(trigger_input.schema_module, "catalog");

        let cancel_input = CancelJobInput {
            job_id: "test-uuid".to_string(),
        };
        assert_eq!(cancel_input.job_id, "test-uuid");

        let retry_input = RetryJobInput {
            job_id: "test-uuid-retry".to_string(),
            reason: Some("manual operator retry".to_string()),
        };
        assert_eq!(retry_input.job_id, "test-uuid-retry");
    }

    #[test]
    fn format_retry_action_result_works_in_english_and_russian() {
        let result = RetryActionResult {
            success: true,
            job_id: "00000000-0000-0000-0000-000000000003".to_string(),
            outcome: "requeued".to_string(),
            retry_epoch: Some(3),
            message: "Job successfully requeued (epoch 3)".to_string(),
        };

        let en_str = format_retry_action_result(Some("en"), &result);
        assert!(en_str.contains("requeued"));
        assert!(en_str.contains("epoch 3"));
        assert!(en_str.contains("00000000-0000-0000-0000-000000000003"));

        let ru_str = format_retry_action_result(Some("ru"), &result);
        assert!(!ru_str.is_empty());

        let not_failed = RetryActionResult {
            success: false,
            job_id: "00000000-0000-0000-0000-000000000003".to_string(),
            outcome: "not_failed".to_string(),
            retry_epoch: None,
            message: "Job is not in a failed state".to_string(),
        };
        assert_eq!(
            format_retry_action_result(Some("en"), &not_failed),
            "Job is not in a failed state"
        );
    }
}
