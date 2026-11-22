use rustok_grid::{ColumnAlign, ColumnFilters, FilterValue, GridColumnDef, GridFilterType};

use crate::i18n::t;
use crate::model::{
    CancelActionResult, CancelActionStatus, IndexAdminBootstrap, IndexJobSnapshot,
    ReplayActionResult, ReplayActionStatus, RetryActionResult, RetryActionStatus, SchemaReadiness,
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
    pub readiness: String,
    pub can_replay: bool,
}

impl IndexSchemaRowViewModel {
    pub fn key(&self) -> String {
        format!("{}::{}@{}", self.module, self.entity, self.raw_version)
    }
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
    pub owner_module: String,
}

impl IndexSourceRowViewModel {
    pub fn key(&self) -> String {
        format!("{} / {}", self.name, self.entity)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexAdminOverviewViewModel {
    pub stat_cards: Vec<IndexStatCardViewModel>,
    pub module_description: String,
    pub engine_type: String,
    pub module_version: String,
    pub partition_status: String,
    pub tables: Vec<IndexTableRowViewModel>,
    pub schemas: Vec<IndexSchemaRowViewModel>,
    pub sources: Vec<IndexSourceRowViewModel>,
    pub source_catalog_available: bool,
    pub can_manage_jobs: bool,
    pub operator_unavailable: Option<String>,
    pub recent_jobs: Vec<IndexJobSnapshot>,
    pub operations_summary: Vec<IndexStatCardViewModel>,
    pub inbox_summary: Vec<IndexStatCardViewModel>,
    pub job_summary: Vec<IndexStatCardViewModel>,
    pub query_diagnostics_summary: Vec<IndexStatCardViewModel>,
}

pub fn build_index_admin_overview_view_model(
    locale: Option<&str>,
    bootstrap: IndexAdminBootstrap,
) -> IndexAdminOverviewViewModel {
    let source_catalog_available = bootstrap.operations.registered_sources.is_some();
    let can_manage_jobs = bootstrap.operations.replay_available && bootstrap.operations.can_manage;
    let operator_unavailable = if !bootstrap.operations.replay_available {
        Some(t(
            locale,
            "index.action.runtimeUnavailable",
            "The replay runtime is not mounted.",
        ))
    } else if !bootstrap.operations.can_manage {
        Some(t(
            locale,
            "index.action.readOnly",
            "Read-only: changing jobs requires MODULES_MANAGE.",
        ))
    } else {
        None
    };
    let engine_type = format!("{} {}", bootstrap.storage.backend, bootstrap.storage.layout);
    let unavailable = t(locale, "index.info.unavailable", "Unavailable");
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
            value: engine_type.clone(),
            hint: Some(format!(
                "{} / {}",
                bootstrap.storage.backend, bootstrap.storage.layout
            )),
        },
        IndexStatCardViewModel {
            label: t(locale, "index.info.version", "Module Version"),
            value: bootstrap.module.version.clone(),
            hint: None,
        },
        IndexStatCardViewModel {
            label: t(locale, "index.info.schemasCount", "Schema Contracts"),
            value: bootstrap.schemas.len().to_string(),
            hint: None,
        },
        IndexStatCardViewModel {
            label: t(
                locale,
                "index.info.sourcesCount",
                "Selected Source Bindings",
            ),
            value: bootstrap
                .operations
                .registered_sources
                .as_ref()
                .map_or_else(
                    || unavailable.to_owned(),
                    |sources| sources.len().to_string(),
                ),
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
            fingerprint: if s.fingerprint.chars().count() > 16 {
                format!("{}...", s.fingerprint.chars().take(16).collect::<String>())
            } else {
                s.fingerprint
            },
            fields_count: s.field_count.to_string(),
            links_count: s.link_count.to_string(),
            owner_module: s.owner_module.unwrap_or_else(|| unavailable.to_owned()),
            readiness: describe_readiness(locale, s.readiness).to_owned(),
            can_replay: can_manage_jobs
                && s.source_selected
                && s.readiness == SchemaReadiness::Ready,
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
        .unwrap_or_default()
        .into_iter()
        .map(|src| IndexSourceRowViewModel {
            name: src.name,
            entity: src.entity,
            owner_module: src.owner_module,
        })
        .collect();

    let operations_summary = vec![IndexStatCardViewModel {
        label: t(locale, "index.ops.replayRunner", "Replay Runner"),
        value: if bootstrap.operations.replay_available {
            t(locale, "index.info.mounted", "Mounted")
        } else {
            unavailable.clone()
        },
        hint: None,
    }];

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
            label: t(locale, "index.inbox.applied", "Applied"),
            value: inbox.applied_messages.to_string(),
            hint: None,
        },
        IndexStatCardViewModel {
            label: t(locale, "index.inbox.rejected", "Rejected"),
            value: inbox.rejected_messages.to_string(),
            hint: None,
        },
        IndexStatCardViewModel {
            label: t(locale, "index.inbox.processing", "Processing"),
            value: inbox.processing_messages.to_string(),
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

    let qd = bootstrap.operations.query_diagnostics.as_ref();
    let query_diagnostics_summary = vec![
        IndexStatCardViewModel {
            label: t(locale, "index.query.catalogStatus", "Query Catalog Status"),
            value: if qd.is_some() {
                t(locale, "index.info.mounted", "Mounted")
            } else {
                unavailable.clone()
            },
            hint: None,
        },
        IndexStatCardViewModel {
            label: t(locale, "index.query.admissionRules", "Admission Rules"),
            value: qd.map_or_else(
                || unavailable.clone(),
                |qd| qd.admission_rules_count.to_string(),
            ),
            hint: None,
        },
        IndexStatCardViewModel {
            label: t(locale, "index.query.linkRules", "Link Availability Rules"),
            value: qd.map_or_else(
                || unavailable.clone(),
                |qd| qd.link_availability_rules_count.to_string(),
            ),
            hint: None,
        },
        IndexStatCardViewModel {
            label: t(
                locale,
                "index.query.partitionStrategy",
                "Partition Strategy",
            ),
            value: bootstrap.storage.partition_status.clone(),
            hint: None,
        },
    ];

    IndexAdminOverviewViewModel {
        stat_cards,
        module_description: bootstrap.module.description,
        engine_type,
        module_version: bootstrap.module.version,
        partition_status: bootstrap.storage.partition_status,
        tables,
        schemas,
        sources,
        source_catalog_available,
        can_manage_jobs,
        operator_unavailable,
        recent_jobs: bootstrap.operations.recent_jobs,
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

fn describe_readiness(locale: Option<&str>, readiness: SchemaReadiness) -> String {
    match readiness {
        SchemaReadiness::Ready => t(
            locale,
            "index.schema.ready",
            "Registered and matches runtime",
        ),
        SchemaReadiness::Missing => t(
            locale,
            "index.schema.missing",
            "Not registered for this tenant",
        ),
        SchemaReadiness::Inactive => t(locale, "index.schema.inactive", "Inactive"),
        SchemaReadiness::Unselected => t(
            locale,
            "index.schema.unselected",
            "Not selected by the runtime",
        ),
        SchemaReadiness::ContractMismatch => t(
            locale,
            "index.schema.mismatch",
            "Stored schema differs from runtime",
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackTone {
    Success,
    Neutral,
    Error,
}

pub fn format_replay_action_result(
    locale: Option<&str>,
    result: &ReplayActionResult,
) -> (FeedbackTone, String) {
    use FeedbackTone::*;
    let (tone, key, fallback) = match result.status {
        ReplayActionStatus::Busy => (
            Neutral,
            "index.action.replayBusy",
            "Replay is busy or in backoff",
        ),
        ReplayActionStatus::AlreadyComplete => (
            Success,
            "index.action.replayAlreadyComplete",
            "Replay was already complete",
        ),
        ReplayActionStatus::Complete => {
            (Success, "index.action.rebuildSuccess", "Replay completed")
        }
        ReplayActionStatus::Cancelled => {
            (Neutral, "index.action.replayCancelled", "Replay cancelled")
        }
        ReplayActionStatus::Yielded => (
            Neutral,
            "index.action.replayYielded",
            "Replay yielded; more pages remain",
        ),
        ReplayActionStatus::RetryScheduled => (
            Neutral,
            "index.action.replayRetry",
            "Replay retry scheduled",
        ),
        ReplayActionStatus::FailedPermanent => (
            Error,
            "index.action.replayPermanent",
            "Replay failed permanently",
        ),
        ReplayActionStatus::FailedExhausted => (
            Error,
            "index.action.replayExhausted",
            "Replay retry budget exhausted",
        ),
    };
    let mut text = t(locale, key, fallback);
    if let Some(seconds) = result.retry_after_seconds {
        text.push_str(&format!(
            " ({}: {seconds})",
            t(locale, "index.action.retryDelay", "Retry delay in seconds")
        ));
    }
    if let Some(job_id) = &result.job_id {
        text.push_str(&format!(" · {job_id}"));
    }
    if result.pages_processed > 0 {
        text.push_str(&format!(
            " · {}: {}, {}: {}",
            t(locale, "index.action.pagesProcessed", "Pages processed"),
            result.pages_processed,
            t(locale, "index.action.mutationsApplied", "Mutations applied"),
            result.mutations_applied
        ));
    }
    (tone, text)
}

pub fn format_retry_action_result(
    locale: Option<&str>,
    result: &RetryActionResult,
) -> (FeedbackTone, String) {
    use FeedbackTone::*;
    let (tone, key, fallback) = match result.outcome {
        RetryActionStatus::Requeued => (
            Success,
            "index.action.retrySuccess",
            "Original job requeued",
        ),
        RetryActionStatus::NotFound => (
            Error,
            "index.action.jobNotFound",
            "Job not found for this tenant",
        ),
        RetryActionStatus::NotFailed => (
            Neutral,
            "index.action.retryNotFailed",
            "Job is not failed; nothing was requeued",
        ),
        RetryActionStatus::ScopeUnavailable => (
            Error,
            "index.action.retryScopeUnavailable",
            "The original job scope is no longer available",
        ),
    };
    let mut text = format!("{} · {}", t(locale, key, fallback), result.job_id);
    if let Some(epoch) = result.retry_epoch {
        text.push_str(&format!(
            " ({} {epoch})",
            t(locale, "index.action.retryEpoch", "epoch")
        ));
    }
    (tone, text)
}

pub fn format_cancel_action_result(
    locale: Option<&str>,
    result: &CancelActionResult,
) -> (FeedbackTone, String) {
    use FeedbackTone::*;
    let (tone, key, fallback) = match result.outcome {
        CancelActionStatus::Requested => (
            Neutral,
            "index.action.cancelRequested",
            "Cancellation requested; the job has not stopped yet",
        ),
        CancelActionStatus::Cancelled => {
            (Success, "index.action.replayCancelled", "Replay cancelled")
        }
        CancelActionStatus::AlreadySucceeded => (
            Neutral,
            "index.action.replayAlreadyComplete",
            "Replay was already complete",
        ),
        CancelActionStatus::AlreadyFailed => (
            Neutral,
            "index.action.cancelAlreadyFailed",
            "Replay had already failed",
        ),
        CancelActionStatus::AlreadyCancelled => (
            Neutral,
            "index.action.cancelAlreadyCancelled",
            "Replay was already cancelled",
        ),
        CancelActionStatus::NotFound => (
            Error,
            "index.action.jobNotFound",
            "Job not found for this tenant",
        ),
    };
    (
        tone,
        format!("{} · {}", t(locale, key, fallback), result.job_id),
    )
}

pub fn source_catalog_empty_message(locale: Option<&str>, available: bool) -> String {
    if available {
        t(
            locale,
            "index.source.empty",
            "No replay sources registered.",
        )
    } else {
        t(
            locale,
            "index.source.unavailable",
            "Source registry is not mounted.",
        )
    }
}

pub fn index_schema_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);
    vec![
        GridColumnDef::new(
            "name",
            if is_ru {
                "Схема / Сущность"
            } else {
                "Schema / Entity"
            },
        )
        .width(240)
        .align(ColumnAlign::Left)
        .filter(GridFilterType::Text {
            placeholder: Some(if is_ru {
                "Фильтр схемы...".to_string()
            } else {
                "Filter schema...".to_string()
            }),
        }),
        GridColumnDef::new("version", if is_ru { "Версия" } else { "Version" })
            .width(100)
            .align(ColumnAlign::Center),
        GridColumnDef::new(
            "fingerprint",
            if is_ru {
                "Контрольная сумма"
            } else {
                "Fingerprint"
            },
        )
        .width(200)
        .align(ColumnAlign::Left),
        GridColumnDef::new("fields", if is_ru { "Поля" } else { "Fields" })
            .width(90)
            .align(ColumnAlign::Right),
        GridColumnDef::new("links", if is_ru { "Связи" } else { "Links" })
            .width(90)
            .align(ColumnAlign::Right),
        GridColumnDef::new(
            "owner",
            if is_ru {
                "Модуль-владелец"
            } else {
                "Owner Module"
            },
        )
        .width(180)
        .align(ColumnAlign::Left)
        .filter(GridFilterType::Text {
            placeholder: Some(if is_ru {
                "Фильтр владельца...".to_string()
            } else {
                "Filter owner...".to_string()
            }),
        }),
        GridColumnDef::new("actions", if is_ru { "Действия" } else { "Actions" })
            .width(120)
            .align(ColumnAlign::Right)
            .not_sortable(),
        GridColumnDef::new(
            "readiness",
            t(locale, "index.schema.readiness", "Registration"),
        )
        .width(220),
    ]
}

pub fn matches_index_schema_filter(
    schema: &IndexSchemaRowViewModel,
    filters: &ColumnFilters,
) -> bool {
    for (col_id, filter_val) in filters.iter() {
        match (col_id.as_str(), filter_val) {
            ("name", FilterValue::Text(q)) => {
                let term = q.to_lowercase();
                if !schema.qualified_name.to_lowercase().contains(&term)
                    && !schema.module.to_lowercase().contains(&term)
                    && !schema.entity.to_lowercase().contains(&term)
                {
                    return false;
                }
            }
            ("owner", FilterValue::Text(q)) => {
                let term = q.to_lowercase();
                if !schema.owner_module.to_lowercase().contains(&term) {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

pub fn filter_index_schemas(
    schemas: &[IndexSchemaRowViewModel],
    filters: &ColumnFilters,
    search: Option<&str>,
) -> Vec<IndexSchemaRowViewModel> {
    let search_term = search
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());

    schemas
        .iter()
        .filter(|schema| {
            if let Some(ref term) = search_term {
                let matches_global = schema.qualified_name.to_lowercase().contains(term)
                    || schema.owner_module.to_lowercase().contains(term)
                    || schema.fingerprint.to_lowercase().contains(term);
                if !matches_global {
                    return false;
                }
            }

            matches_index_schema_filter(schema, filters)
        })
        .cloned()
        .collect()
}

pub fn index_table_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);
    vec![
        GridColumnDef::new(
            "name",
            if is_ru {
                "Имя таблицы"
            } else {
                "Table Name"
            },
        )
        .width(280)
        .align(ColumnAlign::Left)
        .filter(GridFilterType::Text {
            placeholder: Some(if is_ru {
                "Фильтр таблиц...".to_string()
            } else {
                "Filter tables...".to_string()
            }),
        }),
        GridColumnDef::new(
            "role",
            if is_ru {
                "Назначение таблицы"
            } else {
                "Table Role"
            },
        )
        .align(ColumnAlign::Left),
    ]
}

pub fn matches_index_table_filter(table: &IndexTableRowViewModel, filters: &ColumnFilters) -> bool {
    for (col_id, filter_val) in filters.iter() {
        match (col_id.as_str(), filter_val) {
            ("name", FilterValue::Text(q))
                if !table
                    .name
                    .to_ascii_lowercase()
                    .contains(&q.to_ascii_lowercase()) =>
            {
                return false;
            }
            ("role", FilterValue::Text(q))
                if !table
                    .role
                    .to_ascii_lowercase()
                    .contains(&q.to_ascii_lowercase()) =>
            {
                return false;
            }
            _ => {}
        }
    }
    true
}

pub fn filter_index_tables(
    tables: &[IndexTableRowViewModel],
    filters: &ColumnFilters,
    search: Option<&str>,
) -> Vec<IndexTableRowViewModel> {
    tables
        .iter()
        .filter(|t| {
            if let Some(s) = search {
                let term = s.trim().to_ascii_lowercase();
                if !term.is_empty()
                    && !t.name.to_ascii_lowercase().contains(&term)
                    && !t.role.to_ascii_lowercase().contains(&term)
                {
                    return false;
                }
            }
            matches_index_table_filter(t, filters)
        })
        .cloned()
        .collect()
}

pub fn index_source_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);
    vec![
        GridColumnDef::new(
            "name",
            if is_ru {
                "Имя источника"
            } else {
                "Source Name"
            },
        )
        .width(260)
        .align(ColumnAlign::Left)
        .filter(GridFilterType::Text {
            placeholder: Some(if is_ru {
                "Фильтр источника...".to_string()
            } else {
                "Filter source...".to_string()
            }),
        }),
        GridColumnDef::new(
            "entity",
            if is_ru {
                "Целевая сущность"
            } else {
                "Target Entity"
            },
        )
        .width(200)
        .align(ColumnAlign::Left),
        GridColumnDef::new(
            "owner",
            if is_ru {
                "Режим воспроизведения"
            } else {
                "Replay Mode"
            },
        )
        .align(ColumnAlign::Left),
    ]
}

pub fn matches_index_source_filter(
    source: &IndexSourceRowViewModel,
    filters: &ColumnFilters,
) -> bool {
    for (col_id, filter_val) in filters.iter() {
        match (col_id.as_str(), filter_val) {
            ("name", FilterValue::Text(q))
                if !source
                    .name
                    .to_ascii_lowercase()
                    .contains(&q.to_ascii_lowercase()) =>
            {
                return false;
            }
            ("entity", FilterValue::Text(q))
                if !source
                    .entity
                    .to_ascii_lowercase()
                    .contains(&q.to_ascii_lowercase()) =>
            {
                return false;
            }
            ("owner", FilterValue::Text(q))
                if !source
                    .owner_module
                    .to_ascii_lowercase()
                    .contains(&q.to_ascii_lowercase()) =>
            {
                return false;
            }
            _ => {}
        }
    }
    true
}

pub fn filter_index_sources(
    sources: &[IndexSourceRowViewModel],
    filters: &ColumnFilters,
    search: Option<&str>,
) -> Vec<IndexSourceRowViewModel> {
    sources
        .iter()
        .filter(|s| {
            if let Some(q) = search {
                let term = q.trim().to_ascii_lowercase();
                if !term.is_empty()
                    && !s.name.to_ascii_lowercase().contains(&term)
                    && !s.entity.to_ascii_lowercase().contains(&term)
                    && !s.owner_module.to_ascii_lowercase().contains(&term)
                {
                    return false;
                }
            }
            matches_index_source_filter(s, filters)
        })
        .cloned()
        .collect()
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
                version: "0.1.0".to_owned(),
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
                    owner_module: Some("rustok-distribution".to_string()),
                    readiness: SchemaReadiness::Ready,
                    source_selected: true,
                },
                IndexSchemaSnapshot {
                    module: "catalog".to_string(),
                    entity: "sales_channel".to_string(),
                    version: 1,
                    fingerprint: "1234567890abcdef1234567890abcdef".to_string(),
                    field_count: 4,
                    link_count: 0,
                    owner_module: Some("rustok-distribution".to_string()),
                    readiness: SchemaReadiness::Ready,
                    source_selected: true,
                },
            ],
            operations: IndexOperationsSnapshot {
                replay_available: true,
                can_manage: true,
                registered_sources: Some(vec![IndexSourceDescriptorSnapshot {
                    name: "product-postgres-primary".to_string(),
                    entity: "product".to_string(),
                    owner_module: "distribution".to_string(),
                }]),
                inbox_metrics: IndexInboxMetricsSnapshot {
                    total_messages: 14,
                    pending_messages: 3,
                    processing_messages: 1,
                    applied_messages: 9,
                    rejected_messages: 1,
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
                query_diagnostics: Some(IndexQueryDiagnosticsSnapshot {
                    admission_rules_count: 2,
                    link_availability_rules_count: 1,
                }),
                recent_jobs: Vec::new(),
            },
        }
    }

    #[test]
    fn overview_view_model_formats_full_state_without_framework_runtime() {
        let view_model = build_index_admin_overview_view_model(Some("en"), sample_bootstrap());

        assert_eq!(view_model.stat_cards.len(), 7);
        assert_eq!(view_model.stat_cards[0].value, "demo");
        assert_eq!(view_model.stat_cards[2].value, "PostgreSQL JSONB");
        assert_eq!(view_model.stat_cards[3].value, "0.1.0");
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

        assert_eq!(view_model.operations_summary.len(), 1);
        assert_eq!(view_model.operations_summary[0].value, "Mounted");

        // Inbox metrics summary
        assert_eq!(view_model.inbox_summary.len(), 5);
        assert_eq!(view_model.inbox_summary[0].value, "14"); // total
        assert_eq!(view_model.inbox_summary[1].value, "3"); // pending
        assert_eq!(
            view_model.inbox_summary[1].hint,
            Some("42 s lag".to_string())
        );
        assert_eq!(view_model.inbox_summary[2].value, "9"); // applied

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
    fn schema_keys_do_not_use_the_ambiguous_display_label() {
        let mut sample = sample_bootstrap();
        sample.schemas[0].module = "demo.catalog".into();
        sample.schemas[1].module = "demo".into();
        sample.schemas[1].entity = "catalog.product".into();
        sample.schemas[1].version = sample.schemas[0].version;
        let model = build_index_admin_overview_view_model(Some("en"), sample);
        assert_eq!(
            model.schemas[0].qualified_name,
            model.schemas[1].qualified_name
        );
        assert_ne!(model.schemas[0].key(), model.schemas[1].key());
    }

    #[test]
    fn source_rows_are_distinct_per_schema_and_unavailable_is_not_empty() {
        let mut sample = sample_bootstrap();
        let source = sample.operations.registered_sources.as_mut().unwrap();
        let mut second = source[0].clone();
        second.entity = "catalog::variant@1".to_owned();
        source.push(second);
        sample.operations.can_manage = false;
        let rows = build_index_admin_overview_view_model(Some("en"), sample.clone());
        assert_eq!(rows.sources.len(), 2);
        assert_ne!(rows.sources[0].key(), rows.sources[1].key());
        assert!(!rows.can_manage_jobs);
        assert!(rows.schemas.iter().all(|row| !row.can_replay));

        sample.operations.can_manage = true;
        sample.operations.replay_available = false;
        sample.operations.registered_sources = None;
        let unavailable = build_index_admin_overview_view_model(Some("en"), sample);
        assert!(!unavailable.can_manage_jobs);
        assert!(!unavailable.source_catalog_available);
    }

    #[test]
    fn overview_view_model_localizes_properly_in_russian() {
        let view_model = build_index_admin_overview_view_model(Some("ru"), sample_bootstrap());

        assert_eq!(view_model.stat_cards[0].label, "Тенант");
        assert_eq!(view_model.stat_cards[1].label, "Основная локаль");
        assert_eq!(view_model.stat_cards[2].label, "Движок хранения");
        assert_eq!(view_model.stat_cards[3].label, "Версия модуля");
    }

    #[test]
    fn pending_and_terminal_job_receipts_are_not_success_aliases() {
        let mut result = ReplayActionResult {
            job_id: Some("original-job".to_owned()),
            status: ReplayActionStatus::Yielded,
            retry_after_seconds: None,
            pages_processed: 8,
            mutations_applied: 5,
        };
        for status in [
            ReplayActionStatus::Busy,
            ReplayActionStatus::Yielded,
            ReplayActionStatus::Cancelled,
            ReplayActionStatus::RetryScheduled,
        ] {
            result.status = status;
            let (tone, message) = format_replay_action_result(Some("en"), &result);
            assert_eq!(tone, FeedbackTone::Neutral);
            assert!(message.contains("original-job"));
            assert!(!message.contains("completed"));
        }
        result.status = ReplayActionStatus::FailedExhausted;
        assert_eq!(
            format_replay_action_result(Some("ru"), &result).0,
            FeedbackTone::Error
        );
        let missing = CancelActionResult {
            job_id: "absent".to_owned(),
            outcome: CancelActionStatus::NotFound,
        };
        assert_eq!(
            format_cancel_action_result(Some("en"), &missing).0,
            FeedbackTone::Error
        );
        let recovery = RetryActionResult {
            job_id: "original-job".to_owned(),
            outcome: RetryActionStatus::Requeued,
            retry_epoch: Some(3),
        };
        let (tone, message) = format_retry_action_result(Some("en"), &recovery);
        assert_eq!(tone, FeedbackTone::Success);
        assert!(message.contains("epoch 3"));
    }

    #[test]
    fn missing_runtime_does_not_create_fake_counts_or_enable_replay() {
        let mut bootstrap = sample_bootstrap();
        bootstrap.operations.registered_sources = None;
        bootstrap.operations.replay_available = false;
        bootstrap.operations.query_diagnostics = None;
        let model = build_index_admin_overview_view_model(Some("en"), bootstrap);
        assert!(!model.source_catalog_available);
        assert!(model.sources.is_empty());
        assert_eq!(model.stat_cards[5].value, "Unavailable");
        assert_eq!(model.query_diagnostics_summary[1].value, "Unavailable");
        assert!(model.schemas.iter().all(|schema| !schema.can_replay));
    }

    #[test]
    fn index_schema_grid_columns_localization() {
        let cols_en = index_schema_grid_columns(Some("en"));
        assert_eq!(cols_en[0].title, "Schema / Entity");
        assert_eq!(cols_en[1].title, "Version");
        assert_eq!(cols_en[2].title, "Fingerprint");
        assert_eq!(cols_en[6].title, "Actions");

        let cols_ru = index_schema_grid_columns(Some("ru"));
        assert_eq!(cols_ru[0].title, "Схема / Сущность");
        assert_eq!(cols_ru[1].title, "Версия");
        assert_eq!(cols_ru[2].title, "Контрольная сумма");
        assert_eq!(cols_ru[6].title, "Действия");
    }

    #[test]
    fn filter_index_schemas_by_search_and_column() {
        let s1 = IndexSchemaRowViewModel {
            module: "catalog".into(),
            entity: "product".into(),
            raw_version: 1,
            qualified_name: "catalog.product".into(),
            version: "v1".into(),
            fingerprint: "abcdef123".into(),
            fields_count: "5".into(),
            links_count: "1".into(),
            owner_module: "rustok-catalog".into(),
            readiness: "Ready".into(),
            can_replay: true,
        };
        let s2 = IndexSchemaRowViewModel {
            module: "order".into(),
            entity: "order_item".into(),
            raw_version: 2,
            qualified_name: "order.order_item".into(),
            version: "v2".into(),
            fingerprint: "987654321".into(),
            fields_count: "8".into(),
            links_count: "2".into(),
            owner_module: "rustok-order".into(),
            readiness: "Ready".into(),
            can_replay: true,
        };
        let list = vec![s1.clone(), s2.clone()];

        // Search
        let res = filter_index_schemas(&list, &ColumnFilters::new(), Some("order"));
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].qualified_name, "order.order_item");

        // Column filter
        let mut filters = ColumnFilters::new();
        filters.set("name", FilterValue::Text("product".into()));
        let res = filter_index_schemas(&list, &filters, None);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].qualified_name, "catalog.product");
    }

    #[test]
    fn index_table_grid_and_filtering() {
        let cols_en = index_table_grid_columns(Some("en"));
        assert_eq!(cols_en.len(), 2);
        assert_eq!(cols_en[0].title, "Table Name");
        assert_eq!(cols_en[1].title, "Table Role");

        let cols_ru = index_table_grid_columns(Some("ru"));
        assert_eq!(cols_ru[0].title, "Имя таблицы");
        assert_eq!(cols_ru[1].title, "Назначение таблицы");

        let tables = vec![
            IndexTableRowViewModel {
                name: "idx_schemas".to_string(),
                role: "Schema catalog".to_string(),
            },
            IndexTableRowViewModel {
                name: "idx_events".to_string(),
                role: "Event stream buffer".to_string(),
            },
        ];

        let res = filter_index_tables(&tables, &ColumnFilters::new(), Some("schema"));
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].name, "idx_schemas");
    }

    #[test]
    fn index_source_grid_and_filtering() {
        let cols_en = index_source_grid_columns(Some("en"));
        assert_eq!(cols_en.len(), 3);
        assert_eq!(cols_en[0].title, "Source Name");
        assert_eq!(cols_en[1].title, "Target Entity");
        assert_eq!(cols_en[2].title, "Replay Mode");

        let cols_ru = index_source_grid_columns(Some("ru"));
        assert_eq!(cols_ru[0].title, "Имя источника");
        assert_eq!(cols_ru[1].title, "Целевая сущность");
        assert_eq!(cols_ru[2].title, "Режим воспроизведения");

        let sources = vec![
            IndexSourceRowViewModel {
                name: "products_replay".to_string(),
                entity: "product".to_string(),
                owner_module: "streaming".to_string(),
            },
            IndexSourceRowViewModel {
                name: "orders_replay".to_string(),
                entity: "order".to_string(),
                owner_module: "snapshot".to_string(),
            },
        ];

        let res = filter_index_sources(&sources, &ColumnFilters::new(), Some("snapshot"));
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].name, "orders_replay");
    }
}
