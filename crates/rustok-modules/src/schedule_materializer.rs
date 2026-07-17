//! Owner clock materialization for immutable artifact Schedule bindings.
//!
//! The generic runtime scheduler calls this service before claiming durable
//! Schedule work. The service parses only an admitted cron/timezone contract,
//! advances a tenant-RLS cursor, and creates immutable slot records through the
//! owner queue. It never starts its own timer or accepts a guest clock value.

use std::{collections::BTreeMap, str::FromStr};

use chrono::{DateTime, Duration, Timelike, Utc};
use chrono_tz::Tz;
use cron::Schedule;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, TransactionTrait,
    Value as SqlValue,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    artifact::schedule_cron_expression,
    data::{configure_tenant_scope, now_expression, placeholder, uuid_from_row, uuid_value},
    schedule_binding_digest, ArtifactScheduleDeliveryError, ArtifactScheduleDeliveryRequest,
    ModuleArtifactDescriptor, ModuleRuntimeBindingKind, ModuleScheduleBinding,
    ModuleScheduleMisfirePolicy, ModuleScheduleOverlapPolicy, SeaOrmArtifactScheduleDeliveryQueue,
};

/// Bounded owner policy for clock catch-up. The descriptor selects behavior;
/// this configuration limits how much durable work one scheduler poll creates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactScheduleMaterializationConfig {
    pub misfire_grace_seconds: u32,
    pub max_slots_per_binding: u32,
}

impl Default for ArtifactScheduleMaterializationConfig {
    fn default() -> Self {
        Self {
            misfire_grace_seconds: 60,
            max_slots_per_binding: 100,
        }
    }
}

impl ArtifactScheduleMaterializationConfig {
    fn validate(&self) -> Result<(), ArtifactScheduleMaterializationError> {
        if self.misfire_grace_seconds == 0
            || self.misfire_grace_seconds > 3_600
            || self.max_slots_per_binding == 0
            || self.max_slots_per_binding > 1_000
        {
            return Err(ArtifactScheduleMaterializationError::InvalidConfiguration);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactScheduleMaterializationReport {
    pub scanned_bindings: u32,
    pub created_slots: u32,
    pub skipped_misfires: u32,
    pub skipped_overlaps: u32,
    pub initialized_cursors: u32,
}

/// Materializes each effective tenant Schedule binding. All state is durable:
/// retries after a crash can repeat enqueue safely because the queue owns the
/// immutable per-slot uniqueness key.
#[derive(Clone)]
pub struct ArtifactScheduleMaterializer {
    db: DatabaseConnection,
    deliveries: SeaOrmArtifactScheduleDeliveryQueue,
    config: ArtifactScheduleMaterializationConfig,
}

impl ArtifactScheduleMaterializer {
    pub fn new(
        db: DatabaseConnection,
        deliveries: SeaOrmArtifactScheduleDeliveryQueue,
        config: ArtifactScheduleMaterializationConfig,
    ) -> Result<Self, ArtifactScheduleMaterializationError> {
        config.validate()?;
        Ok(Self {
            db,
            deliveries,
            config,
        })
    }

    pub fn config(&self) -> &ArtifactScheduleMaterializationConfig {
        &self.config
    }

    /// Evaluates due slots against an owner-supplied UTC clock instant. A new
    /// or changed binding initializes its cursor at `now`; old schedule
    /// semantics are never replayed under a new immutable digest.
    pub async fn materialize_tenant(
        &self,
        tenant_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<ArtifactScheduleMaterializationReport, ArtifactScheduleMaterializationError> {
        if tenant_id.is_nil() {
            return Err(ArtifactScheduleMaterializationError::InvalidRequest);
        }
        let now = now
            .with_nanosecond(0)
            .expect("valid UTC clock instant must support second precision");
        let mut report = ArtifactScheduleMaterializationReport::default();
        for binding in self.effective_bindings(tenant_id).await? {
            report.scanned_bindings = report.scanned_bindings.saturating_add(1);
            let Some(through) = self.cursor_through(tenant_id, &binding, now).await? else {
                report.initialized_cursors = report.initialized_cursors.saturating_add(1);
                continue;
            };
            let decision = due_slots(&binding.schedule, through, now, &self.config)?;
            let mut selected = decision.slots;
            report.skipped_misfires = report
                .skipped_misfires
                .saturating_add(decision.skipped_misfires);
            if binding.schedule.overlap == ModuleScheduleOverlapPolicy::Forbid
                && !selected.is_empty()
                && self.has_active_delivery(tenant_id, &binding).await?
            {
                report.skipped_overlaps = report
                    .skipped_overlaps
                    .saturating_add(u32::try_from(selected.len()).unwrap_or(u32::MAX));
                selected.clear();
            }
            for scheduled_for in selected {
                let receipt = self
                    .deliveries
                    .enqueue(ArtifactScheduleDeliveryRequest {
                        tenant_id,
                        installation_id: binding.installation_id,
                        binding_id: binding.binding_id.clone(),
                        scheduled_for,
                    })
                    .await
                    .map_err(ArtifactScheduleMaterializationError::Queue)?;
                if receipt.created {
                    report.created_slots = report.created_slots.saturating_add(1);
                }
            }
            self.store_cursor(tenant_id, &binding, decision.advance_through)
                .await?;
        }
        Ok(report)
    }

    async fn effective_bindings(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<EffectiveScheduleBinding>, ArtifactScheduleMaterializationError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(storage_error)?;
        let backend = transaction.get_database_backend();
        let enabled = match backend {
            DbBackend::Postgres => "COALESCE(lifecycle.enabled, TRUE) = TRUE",
            _ => "COALESCE(lifecycle.enabled, 1) = 1",
        };
        let rows = transaction
            .query_all(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT installation.installation_id, installation.slug, installation.scope_kind, \
                            CAST(installation.descriptor AS TEXT) AS descriptor \
                     FROM module_artifact_installations installation \
                     JOIN module_artifact_admissions admission \
                       ON admission.installation_id = installation.installation_id \
                     LEFT JOIN module_artifact_tenant_lifecycle lifecycle \
                       ON lifecycle.installation_id = installation.installation_id \
                      AND lifecycle.tenant_id = {} \
                     WHERE admission.status = 'active' \
                       AND NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                                       WHERE uninstall.installation_id = installation.installation_id) \
                       AND {enabled} \
                       AND ((installation.scope_kind = 'tenant' AND installation.tenant_id = {}) \
                            OR (installation.scope_kind = 'platform' AND installation.tenant_id IS NULL))",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                ),
                vec![uuid_value(tenant_id, backend), uuid_value(tenant_id, backend)],
            ))
            .await
            .map_err(storage_error)?;
        let mut effective = BTreeMap::<String, ScheduleCandidate>::new();
        for row in rows {
            let candidate = candidate_from_row(&row, backend)?;
            match effective.get(&candidate.slug) {
                None => {
                    effective.insert(candidate.slug.clone(), candidate);
                }
                Some(existing) if candidate.tenant_scoped && !existing.tenant_scoped => {
                    effective.insert(candidate.slug.clone(), candidate);
                }
                Some(existing) if !candidate.tenant_scoped && existing.tenant_scoped => {}
                Some(_) => {
                    return Err(ArtifactScheduleMaterializationError::AmbiguousBinding(
                        candidate.slug,
                    ));
                }
            }
        }
        transaction.commit().await.map_err(storage_error)?;
        let mut bindings = Vec::new();
        for candidate in effective.into_values() {
            for binding in candidate.descriptor.bindings {
                if binding.kind != ModuleRuntimeBindingKind::Schedule {
                    continue;
                }
                let Some(schedule) = binding.schedule else {
                    return Err(ArtifactScheduleMaterializationError::BindingUnavailable);
                };
                bindings.push(EffectiveScheduleBinding {
                    installation_id: candidate.installation_id,
                    binding_id: binding.id,
                    schedule_digest: schedule_binding_digest(&schedule),
                    schedule,
                });
            }
        }
        Ok(bindings)
    }

    /// `None` means a new or replaced immutable schedule, whose cursor was
    /// initialized at `now`. Existing cursors are returned without mutation.
    async fn cursor_through(
        &self,
        tenant_id: Uuid,
        binding: &EffectiveScheduleBinding,
        now: DateTime<Utc>,
    ) -> Result<Option<DateTime<Utc>>, ArtifactScheduleMaterializationError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(storage_error)?;
        let backend = transaction.get_database_backend();
        let row = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT schedule_digest, materialized_through \
                     FROM module_artifact_schedule_cursors \
                     WHERE tenant_id = {} AND installation_id = {} AND binding_id = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                vec![
                    uuid_value(tenant_id, backend),
                    uuid_value(binding.installation_id, backend),
                    binding.binding_id.clone().into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        let cursor = match row {
            Some(row) => {
                let digest: String = row.try_get("", "schedule_digest").map_err(storage_error)?;
                if digest == binding.schedule_digest {
                    Some(datetime_from_row(&row, "materialized_through", backend)?)
                } else {
                    None
                }
            }
            None => None,
        };
        transaction.commit().await.map_err(storage_error)?;
        if cursor.is_none() {
            self.store_cursor(tenant_id, binding, now).await?;
        }
        Ok(cursor)
    }

    async fn store_cursor(
        &self,
        tenant_id: Uuid,
        binding: &EffectiveScheduleBinding,
        materialized_through: DateTime<Utc>,
    ) -> Result<(), ArtifactScheduleMaterializationError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(storage_error)?;
        let backend = transaction.get_database_backend();
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_schedule_cursors \
                     (tenant_id, installation_id, binding_id, schedule_digest, materialized_through, updated_at) \
                     VALUES ({}, {}, {}, {}, {}, {}) \
                     ON CONFLICT (tenant_id, installation_id, binding_id) DO UPDATE \
                     SET schedule_digest = excluded.schedule_digest, \
                         materialized_through = CASE \
                             WHEN module_artifact_schedule_cursors.schedule_digest = excluded.schedule_digest \
                              AND module_artifact_schedule_cursors.materialized_through > excluded.materialized_through \
                             THEN module_artifact_schedule_cursors.materialized_through \
                             ELSE excluded.materialized_through \
                         END, \
                         updated_at = excluded.updated_at",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    now_expression(backend),
                ),
                vec![
                    uuid_value(tenant_id, backend),
                    uuid_value(binding.installation_id, backend),
                    binding.binding_id.clone().into(),
                    binding.schedule_digest.clone().into(),
                    datetime_value(materialized_through, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(())
    }

    async fn has_active_delivery(
        &self,
        tenant_id: Uuid,
        binding: &EffectiveScheduleBinding,
    ) -> Result<bool, ArtifactScheduleMaterializationError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(storage_error)?;
        let backend = transaction.get_database_backend();
        let row = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT 1 AS active_delivery FROM module_artifact_schedule_deliveries \
                     WHERE tenant_id = {} AND installation_id = {} AND binding_id = {} \
                       AND schedule_digest = {} AND status IN ('pending', 'running') LIMIT 1",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                vec![
                    uuid_value(tenant_id, backend),
                    uuid_value(binding.installation_id, backend),
                    binding.binding_id.clone().into(),
                    binding.schedule_digest.clone().into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(row.is_some())
    }
}

#[derive(Clone)]
struct ScheduleCandidate {
    installation_id: Uuid,
    slug: String,
    tenant_scoped: bool,
    descriptor: ModuleArtifactDescriptor,
}

#[derive(Clone)]
struct EffectiveScheduleBinding {
    installation_id: Uuid,
    binding_id: String,
    schedule_digest: String,
    schedule: ModuleScheduleBinding,
}

struct DueSlots {
    slots: Vec<DateTime<Utc>>,
    advance_through: DateTime<Utc>,
    skipped_misfires: u32,
}

fn candidate_from_row(
    row: &QueryResult,
    backend: DbBackend,
) -> Result<ScheduleCandidate, ArtifactScheduleMaterializationError> {
    let slug: String = row.try_get("", "slug").map_err(storage_error)?;
    let tenant_scoped = match row
        .try_get::<String>("", "scope_kind")
        .map_err(storage_error)?
        .as_str()
    {
        "tenant" => true,
        "platform" => false,
        _ => return Err(ArtifactScheduleMaterializationError::InvalidState),
    };
    let descriptor: ModuleArtifactDescriptor = serde_json::from_str(
        &row.try_get::<String>("", "descriptor")
            .map_err(storage_error)?,
    )
    .map_err(|_| ArtifactScheduleMaterializationError::InvalidState)?;
    descriptor
        .validate()
        .map_err(|_| ArtifactScheduleMaterializationError::InvalidState)?;
    if descriptor.slug != slug {
        return Err(ArtifactScheduleMaterializationError::InvalidState);
    }
    Ok(ScheduleCandidate {
        installation_id: uuid_from_row(row, "installation_id", backend).map_err(storage_error)?,
        slug,
        tenant_scoped,
        descriptor,
    })
}

fn due_slots(
    binding: &ModuleScheduleBinding,
    through: DateTime<Utc>,
    now: DateTime<Utc>,
    config: &ArtifactScheduleMaterializationConfig,
) -> Result<DueSlots, ArtifactScheduleMaterializationError> {
    if through >= now {
        return Ok(DueSlots {
            slots: Vec::new(),
            advance_through: now,
            skipped_misfires: 0,
        });
    }
    let timezone = Tz::from_str(&binding.timezone)
        .map_err(|_| ArtifactScheduleMaterializationError::InvalidSchedule)?;
    let expression = schedule_cron_expression(&binding.cron)
        .ok_or(ArtifactScheduleMaterializationError::InvalidSchedule)?;
    let schedule = Schedule::from_str(&expression)
        .map_err(|_| ArtifactScheduleMaterializationError::InvalidSchedule)?;
    let cutoff = now - Duration::seconds(i64::from(config.misfire_grace_seconds));
    let (slots, skipped_misfires, advance_through) = match binding.misfire {
        ModuleScheduleMisfirePolicy::Skip => {
            let start = through.max(cutoff - Duration::seconds(1));
            let slots = due_after(
                &schedule,
                timezone,
                start,
                now,
                config.max_slots_per_binding,
            );
            (slots, 0, now)
        }
        ModuleScheduleMisfirePolicy::RunOnce => {
            let slots = due_after(&schedule, timezone, through, now, 1);
            (slots, 0, now)
        }
        ModuleScheduleMisfirePolicy::CatchUp => {
            let slots = due_after(
                &schedule,
                timezone,
                through,
                now,
                config.max_slots_per_binding,
            );
            let advance_through = slots.last().copied().unwrap_or(now);
            (slots, 0, advance_through)
        }
    };
    Ok(DueSlots {
        slots,
        advance_through,
        skipped_misfires,
    })
}

fn due_after(
    schedule: &Schedule,
    timezone: Tz,
    through: DateTime<Utc>,
    now: DateTime<Utc>,
    limit: u32,
) -> Vec<DateTime<Utc>> {
    let mut due = Vec::new();
    for occurrence in schedule.after(&through.with_timezone(&timezone)) {
        let occurrence = occurrence.with_timezone(&Utc);
        if occurrence > now || due.len() >= limit as usize {
            break;
        }
        due.push(occurrence);
    }
    due
}

fn datetime_from_row(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<DateTime<Utc>, ArtifactScheduleMaterializationError> {
    match backend {
        DbBackend::Postgres => row
            .try_get::<DateTime<Utc>>("", column)
            .map_err(storage_error),
        _ => row
            .try_get::<String>("", column)
            .map_err(storage_error)
            .and_then(|value| {
                DateTime::parse_from_rfc3339(&value)
                    .map(|timestamp| timestamp.with_timezone(&Utc))
                    .map_err(storage_error)
            }),
    }
}

fn datetime_value(value: DateTime<Utc>, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => SqlValue::ChronoDateTimeUtc(Some(Box::new(value))),
        _ => value.to_rfc3339().into(),
    }
}

fn storage_error(error: impl std::fmt::Display) -> ArtifactScheduleMaterializationError {
    ArtifactScheduleMaterializationError::Storage(error.to_string())
}

#[derive(Debug, Error)]
pub enum ArtifactScheduleMaterializationError {
    #[error("artifact schedule materializer configuration is invalid")]
    InvalidConfiguration,
    #[error("artifact schedule materializer request is invalid")]
    InvalidRequest,
    #[error("artifact schedule materializer state is invalid")]
    InvalidState,
    #[error("artifact schedule binding is unavailable")]
    BindingUnavailable,
    #[error("artifact schedule for a module is ambiguous: {0}")]
    AmbiguousBinding(String),
    #[error("artifact schedule cron or timezone is invalid")]
    InvalidSchedule,
    #[error("artifact schedule backlog exceeds the configured materialization limit")]
    BacklogLimitExceeded,
    #[error("artifact schedule delivery queue failed: {0}")]
    Queue(ArtifactScheduleDeliveryError),
    #[error("artifact schedule materializer storage failed: {0}")]
    Storage(String),
}
