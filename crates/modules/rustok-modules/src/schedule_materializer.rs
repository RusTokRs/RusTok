//! Owner clock materialization for immutable artifact Schedule bindings.
//!
//! The generic runtime scheduler calls this service before claiming durable
//! Schedule work. The service parses only an admitted cron/timezone contract,
//! advances a tenant-RLS cursor, and creates immutable slot records through the
//! owner queue. It never starts its own timer or accepts a guest clock value.

use std::str::FromStr;

use chrono::{DateTime, Duration, Timelike, Utc};
use chrono_tz::Tz;
use cron::Schedule;
use sea_orm::{ConnectionTrait, DatabaseConnection, QueryResult, Statement, TransactionTrait};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::backend::SupportedBackend;
use crate::{
    ArtifactScheduleDeliveryError, ArtifactScheduleDeliveryRequest, ModuleArtifactDescriptor,
    ModuleRuntimeBindingKind, ModuleScheduleBinding, ModuleScheduleMisfirePolicy,
    ModuleScheduleOverlapPolicy, SeaOrmArtifactScheduleDeliveryQueue,
    artifact::schedule_cron_expression,
    data::{configure_tenant_scope, uuid_from_row},
    schedule_binding_digest,
};

/// Upper bound on installations evaluated for one tenant in a single poll.
/// Exceeding it fails the poll with `BacklogLimitExceeded` rather than
/// silently truncating the schedule set.
pub const MAX_SCHEDULE_CANDIDATES_PER_POLL: usize = 10_000;

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
            // INVARIANT: setting nanosecond to 0 on a valid DateTime<Utc> is infallible
            .with_nanosecond(0)
            .expect("valid UTC clock instant must support second precision");
        let mut report = ArtifactScheduleMaterializationReport::default();
        for binding in self.effective_bindings(tenant_id).await? {
            report.scanned_bindings = report.scanned_bindings.saturating_add(1);
            // One transaction per binding: the cursor row is locked
            // (`FOR UPDATE` on PostgreSQL; SQLite serializes writers), so
            // the due-window decision, the Forbid-overlap probe, slot
            // creation, and the cursor advance commit atomically. Two
            // concurrent materializers can no longer both observe "no
            // active delivery" and create overlapping slots.
            let transaction = self.db.begin().await.map_err(storage_error)?;
            configure_tenant_scope(&transaction, tenant_id)
                .await
                .map_err(storage_error)?;
            let backend =
                SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
            let Some(through) =
                Self::cursor_through_in_transaction(&transaction, backend, tenant_id, &binding)
                    .await?
            else {
                // New or changed binding: initialize (or reset) its cursor
                // at `now` inside the same transaction; old schedule
                // semantics are never replayed under a new digest.
                Self::store_cursor_in_transaction(&transaction, backend, tenant_id, &binding, now)
                    .await?;
                transaction.commit().await.map_err(storage_error)?;
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
                && Self::has_active_delivery_in_transaction(
                    &transaction,
                    backend,
                    tenant_id,
                    &binding,
                )
                .await?
            {
                report.skipped_overlaps = report
                    .skipped_overlaps
                    .saturating_add(u32::try_from(selected.len()).unwrap_or(u32::MAX));
                selected.clear();
            }
            for scheduled_for in selected {
                let receipt = self
                    .deliveries
                    .enqueue_in_transaction(
                        &transaction,
                        backend,
                        ArtifactScheduleDeliveryRequest {
                            tenant_id,
                            installation_id: binding.installation_id,
                            binding_id: binding.binding_id.clone(),
                            scheduled_for,
                        },
                    )
                    .await
                    .map_err(ArtifactScheduleMaterializationError::Queue)?;
                if receipt.created {
                    report.created_slots = report.created_slots.saturating_add(1);
                }
            }
            Self::store_cursor_in_transaction(
                &transaction,
                backend,
                tenant_id,
                &binding,
                decision.advance_through,
            )
            .await?;
            transaction.commit().await.map_err(storage_error)?;
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
        let backend =
            SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
        let enabled = match backend {
            SupportedBackend::Postgres => "COALESCE(lifecycle.enabled, TRUE) = TRUE",
            SupportedBackend::Sqlite => "COALESCE(lifecycle.enabled, 1) = 1",
        };
        let rows = transaction
            .query_all_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT installation.installation_id, installation.slug, installation.scope_kind, \
                            installation.version, installation.payload_digest, \
                            admission.payload_digest AS admission_payload_digest, \
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
                            OR (installation.scope_kind = 'platform' AND installation.tenant_id IS NULL)) \
                     LIMIT {candidate_limit}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    candidate_limit = MAX_SCHEDULE_CANDIDATES_PER_POLL + 1,
                ),
                vec![backend.uuid_value(tenant_id), backend.uuid_value(tenant_id)],
            ))
            .await
            .map_err(storage_error)?;
        if rows.len() > MAX_SCHEDULE_CANDIDATES_PER_POLL {
            // The extra SQL row detects overflow without returning a partial
            // schedule set or loading an unbounded number of rows into Rust.
            return Err(ArtifactScheduleMaterializationError::BacklogLimitExceeded);
        }
        // Select by identity before parsing descriptors: invalid tenant
        // metadata must not restore a lower-priority platform installation.
        let selected =
            crate::installation_selection::select_projection_rows(rows).map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        if selected.ambiguous_count != 0 {
            tracing::warn!(%tenant_id, ambiguous_count = selected.ambiguous_count,
                "artifact schedule selection is ambiguous; affected slugs are omitted");
        }
        let mut bindings = Vec::new();
        for row in selected.rows {
            let candidate = match candidate_from_row(&row, backend) {
                Ok(candidate) => candidate,
                Err(ArtifactScheduleMaterializationError::InvalidState) => {
                    tracing::warn!(%tenant_id, "invalid artifact schedule descriptor or release identity; affected slug is omitted");
                    continue;
                }
                Err(error) => return Err(error),
            };
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

    /// Reads the binding cursor inside the caller's transaction, locking
    /// the row on PostgreSQL so concurrent materializers serialize on
    /// the cursor (SQLite serializes write transactions instead). `None`
    /// means the cursor is absent or belongs to a previous schedule
    /// digest; the caller initializes it at `now` in the same
    /// transaction.
    async fn cursor_through_in_transaction(
        transaction: &sea_orm::DatabaseTransaction,
        backend: SupportedBackend,
        tenant_id: Uuid,
        binding: &EffectiveScheduleBinding,
    ) -> Result<Option<DateTime<Utc>>, ArtifactScheduleMaterializationError> {
        let lock_clause = if backend == SupportedBackend::Postgres {
            " FOR UPDATE"
        } else {
            ""
        };
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT schedule_digest, materialized_through \
                     FROM module_artifact_schedule_cursors \
                     WHERE tenant_id = {} AND installation_id = {} AND binding_id = {}{lock_clause}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                ),
                vec![
                    backend.uuid_value(tenant_id),
                    backend.uuid_value(binding.installation_id),
                    binding.binding_id.clone().into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        match row {
            Some(row) => {
                let digest: String = row.try_get("", "schedule_digest").map_err(storage_error)?;
                if digest == binding.schedule_digest {
                    Ok(Some(datetime_from_row(
                        &row,
                        "materialized_through",
                        backend,
                    )?))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    async fn store_cursor_in_transaction(
        transaction: &sea_orm::DatabaseTransaction,
        backend: SupportedBackend,
        tenant_id: Uuid,
        binding: &EffectiveScheduleBinding,
        materialized_through: DateTime<Utc>,
    ) -> Result<(), ArtifactScheduleMaterializationError> {
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
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
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                    backend.placeholder(5),
                    backend.now_sql(),
                ),
                vec![
                    backend.uuid_value(tenant_id),
                    backend.uuid_value(binding.installation_id),
                    binding.binding_id.clone().into(),
                    binding.schedule_digest.clone().into(),
                    backend.datetime_value(materialized_through),
                ],
            ))
            .await
            .map_err(storage_error)?;
        Ok(())
    }

    async fn has_active_delivery_in_transaction(
        transaction: &sea_orm::DatabaseTransaction,
        backend: SupportedBackend,
        tenant_id: Uuid,
        binding: &EffectiveScheduleBinding,
    ) -> Result<bool, ArtifactScheduleMaterializationError> {
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT 1 AS active_delivery FROM module_artifact_schedule_deliveries \
                     WHERE tenant_id = {} AND installation_id = {} AND binding_id = {} \
                       AND schedule_digest = {} AND status IN ('pending', 'running') LIMIT 1",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                ),
                vec![
                    backend.uuid_value(tenant_id),
                    backend.uuid_value(binding.installation_id),
                    binding.binding_id.clone().into(),
                    binding.schedule_digest.clone().into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        Ok(row.is_some())
    }
}

#[derive(Clone)]
struct ScheduleCandidate {
    installation_id: Uuid,
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
    backend: SupportedBackend,
) -> Result<ScheduleCandidate, ArtifactScheduleMaterializationError> {
    let descriptor = crate::installation_selection::descriptor_with_identity(row)
        .map_err(storage_error)?
        .ok_or(ArtifactScheduleMaterializationError::InvalidState)?;
    Ok(ScheduleCandidate {
        installation_id: uuid_from_row(row, "installation_id", backend).map_err(storage_error)?,
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
            // Slots older than the grace cutoff are skipped, and counted
            // honestly instead of vanishing from the report.
            let start = through.max(cutoff - Duration::seconds(1));
            let skipped_misfires = count_due_between(&schedule, timezone, through, start);
            let slots = due_after(
                &schedule,
                timezone,
                start,
                now,
                config.max_slots_per_binding,
            );
            (slots, skipped_misfires, now)
        }
        ModuleScheduleMisfirePolicy::RunOnce => {
            // Run the most recently missed slot once; every earlier missed
            // slot is a skipped misfire. (Previously the *oldest* slot ran,
            // which executes the stalest possible work and hides the rest.)
            let (latest, due_count, exhausted) = latest_due(&schedule, timezone, through, now);
            match latest {
                Some(latest) => {
                    let skipped_misfires = due_count.saturating_sub(1);
                    // If the scan hit its bound before reaching `now`, do
                    // not pretend the backlog is drained: advance only to
                    // the last scanned slot so the next poll continues.
                    let advance_through = if exhausted { now } else { latest };
                    (vec![latest], skipped_misfires, advance_through)
                }
                None => (Vec::new(), 0, now),
            }
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

/// Upper bound on cron occurrences inspected per binding per poll while
/// counting skipped misfires or finding the latest due slot. The bound
/// keeps one poll's CPU independent of backlog age: when a scan stops at
/// the bound, `RunOnce` advances the cursor only to the last scanned
/// slot, so the next poll (seconds later) continues draining the backlog
/// instead of pretending it is gone. At the admitted minimum interval
/// of one minute, one pass covers ~7 days of backlog per binding.
const MAX_MISFIRE_COUNT_SCAN: u32 = 10_000;

/// Counts occurrences in `(after, through_inclusive]`, saturating at
/// `MAX_MISFIRE_COUNT_SCAN`.
fn count_due_between(
    schedule: &Schedule,
    timezone: Tz,
    after: DateTime<Utc>,
    through_inclusive: DateTime<Utc>,
) -> u32 {
    let mut count = 0u32;
    for occurrence in schedule.after(&after.with_timezone(&timezone)) {
        if occurrence.with_timezone(&Utc) > through_inclusive || count >= MAX_MISFIRE_COUNT_SCAN {
            break;
        }
        count += 1;
    }
    count
}

/// Finds the latest occurrence in `(after, now]` and how many occurrences
/// the bounded scan saw. `exhausted` is false when the scan stopped at
/// `MAX_MISFIRE_COUNT_SCAN` before reaching `now`.
fn latest_due(
    schedule: &Schedule,
    timezone: Tz,
    after: DateTime<Utc>,
    now: DateTime<Utc>,
) -> (Option<DateTime<Utc>>, u32, bool) {
    let mut latest = None;
    let mut count = 0u32;
    for occurrence in schedule.after(&after.with_timezone(&timezone)) {
        let occurrence = occurrence.with_timezone(&Utc);
        if occurrence > now {
            return (latest, count, true);
        }
        if count >= MAX_MISFIRE_COUNT_SCAN {
            return (latest, count, false);
        }
        latest = Some(occurrence);
        count += 1;
    }
    (latest, count, true)
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
    backend: SupportedBackend,
) -> Result<DateTime<Utc>, ArtifactScheduleMaterializationError> {
    match backend {
        SupportedBackend::Postgres => row
            .try_get::<DateTime<Utc>>("", column)
            .map_err(storage_error),
        SupportedBackend::Sqlite => row
            .try_get::<String>("", column)
            .map_err(storage_error)
            .and_then(|value| crate::backend::parse_sqlite_datetime(&value).map_err(storage_error)),
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
    #[error("artifact schedule cron or timezone is invalid")]
    InvalidSchedule,
    #[error("artifact schedule backlog exceeds the configured materialization limit")]
    BacklogLimitExceeded,
    #[error("artifact schedule delivery queue failed: {0}")]
    Queue(ArtifactScheduleDeliveryError),
    #[error("artifact schedule materializer storage failed: {0}")]
    Storage(String),
}

#[cfg(test)]
mod due_slot_tests {
    use super::*;
    use crate::{ModuleScheduleDeduplication, ModuleScheduleOverlapPolicy};

    fn binding(misfire: ModuleScheduleMisfirePolicy) -> ModuleScheduleBinding {
        ModuleScheduleBinding {
            // Every 10 minutes.
            cron: "*/10 * * * *".to_string(),
            timezone: "UTC".to_string(),
            misfire,
            overlap: ModuleScheduleOverlapPolicy::Queue,
            deduplication: ModuleScheduleDeduplication::PerSlot,
        }
    }

    fn at(hour: u32, minute: u32) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(&format!("2026-10-21T{hour:02}:{minute:02}:00Z"))
            .expect("test timestamp")
            .with_timezone(&Utc)
    }

    #[tokio::test]
    async fn schedule_projection_selects_scope_before_decoding_and_quarantines_ambiguity() {
        use crate::ArtifactScheduleDeliveryConfig;
        use crate::installation_selection::test_support::ProjectionFixture;
        use sea_orm::ConnectionTrait;
        use std::collections::BTreeSet;

        async fn ids(materializer: &ArtifactScheduleMaterializer, tenant: Uuid) -> BTreeSet<Uuid> {
            materializer
                .effective_bindings(tenant)
                .await
                .unwrap()
                .into_iter()
                .map(|binding| binding.installation_id)
                .collect()
        }

        let fixture = ProjectionFixture::new().await;
        let queue = SeaOrmArtifactScheduleDeliveryQueue::new(
            fixture.db.clone(),
            ArtifactScheduleDeliveryConfig::default(),
        )
        .unwrap();
        let materializer = ArtifactScheduleMaterializer::new(
            fixture.db.clone(),
            queue,
            ArtifactScheduleMaterializationConfig::default(),
        )
        .unwrap();
        assert_eq!(
            ids(&materializer, fixture.tenant).await,
            BTreeSet::from([fixture.other])
        );
        fixture.repair_tenant().await;
        assert_eq!(
            ids(&materializer, fixture.tenant).await,
            BTreeSet::from([fixture.other, fixture.tenant_winner])
        );
        fixture.set_active(fixture.tenant_duplicate, true).await;
        assert_eq!(
            ids(&materializer, fixture.tenant).await,
            BTreeSet::from([fixture.other])
        );
        fixture.set_active(fixture.tenant_winner, false).await;
        fixture.set_active(fixture.tenant_duplicate, false).await;
        assert_eq!(
            ids(&materializer, fixture.tenant).await,
            BTreeSet::from([fixture.other])
        );
        fixture.set_active(fixture.platform_duplicate, false).await;
        assert_eq!(
            ids(&materializer, fixture.tenant).await,
            BTreeSet::from([fixture.other, fixture.platform])
        );
        fixture.set_active(fixture.tenant_winner, true).await;
        fixture.set_tenant_enabled(false).await;
        assert_eq!(
            ids(&materializer, fixture.tenant).await,
            BTreeSet::from([fixture.other, fixture.platform])
        );
        fixture.set_tenant_enabled(true).await;
        assert_eq!(
            ids(&materializer, fixture.tenant).await,
            BTreeSet::from([fixture.other, fixture.tenant_winner])
        );
        fixture
            .db
            .execute_raw(sea_orm::Statement::from_sql_and_values(
                sea_orm::DbBackend::Sqlite,
                "INSERT INTO module_artifact_uninstall_operations VALUES (?1)",
                vec![fixture.tenant_winner.to_string().into()],
            ))
            .await
            .unwrap();
        assert_eq!(
            ids(&materializer, fixture.tenant).await,
            BTreeSet::from([fixture.other, fixture.platform])
        );
    }

    #[tokio::test]
    async fn schedule_projection_omits_only_identity_mismatched_winner() {
        use crate::ArtifactScheduleDeliveryConfig;
        use crate::installation_selection::test_support::ProjectionFixture;
        use sea_orm::{ConnectionTrait, DbBackend, Statement};
        use std::collections::BTreeSet;

        let fixture = ProjectionFixture::new().await;
        fixture.repair_tenant().await;
        let queue = SeaOrmArtifactScheduleDeliveryQueue::new(
            fixture.db.clone(),
            ArtifactScheduleDeliveryConfig::default(),
        )
        .unwrap();
        let materializer = ArtifactScheduleMaterializer::new(
            fixture.db.clone(),
            queue,
            ArtifactScheduleMaterializationConfig::default(),
        )
        .unwrap();
        let good_digest = format!("sha256:{}", "a".repeat(64));
        let bad_digest = format!("sha256:{}", "b".repeat(64));
        for (table, column, bad, good) in [
            (
                "module_artifact_installations",
                "version",
                "2.0.0".to_string(),
                "1.0.0".to_string(),
            ),
            (
                "module_artifact_installations",
                "payload_digest",
                bad_digest.clone(),
                good_digest.clone(),
            ),
            (
                "module_artifact_admissions",
                "payload_digest",
                bad_digest.clone(),
                good_digest.clone(),
            ),
        ] {
            let update = format!("UPDATE {table} SET {column} = ?1 WHERE installation_id = ?2");
            fixture
                .db
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    update.clone(),
                    vec![bad.into(), fixture.tenant_winner.to_string().into()],
                ))
                .await
                .unwrap();
            let ids: BTreeSet<_> = materializer
                .effective_bindings(fixture.tenant)
                .await
                .unwrap()
                .into_iter()
                .map(|binding| binding.installation_id)
                .collect();
            assert_eq!(ids, BTreeSet::from([fixture.other]));
            fixture
                .db
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    update,
                    vec![good.into(), fixture.tenant_winner.to_string().into()],
                ))
                .await
                .unwrap();
        }
        let ids: BTreeSet<_> = materializer
            .effective_bindings(fixture.tenant)
            .await
            .unwrap()
            .into_iter()
            .map(|binding| binding.installation_id)
            .collect();
        assert_eq!(ids, BTreeSet::from([fixture.other, fixture.tenant_winner]));
    }

    #[tokio::test]
    async fn schedule_projection_checks_the_sentinel_before_materializing() {
        use crate::ArtifactScheduleDeliveryConfig;
        use crate::installation_selection::test_support::ProjectionFixture;
        use std::collections::BTreeSet;

        let fixture = ProjectionFixture::new().await;
        fixture.repair_tenant().await;
        fixture
            .add_limit_candidates(MAX_SCHEDULE_CANDIDATES_PER_POLL - 4)
            .await;
        let queue = SeaOrmArtifactScheduleDeliveryQueue::new(
            fixture.db.clone(),
            ArtifactScheduleDeliveryConfig::default(),
        )
        .unwrap();
        let materializer = ArtifactScheduleMaterializer::new(
            fixture.db.clone(),
            queue,
            ArtifactScheduleMaterializationConfig::default(),
        )
        .unwrap();
        // At exactly the limit, unrelated selected bindings are retained.
        let ids: BTreeSet<_> = materializer
            .effective_bindings(fixture.tenant)
            .await
            .unwrap()
            .into_iter()
            .map(|binding| binding.installation_id)
            .collect();
        assert_eq!(ids, BTreeSet::from([fixture.other, fixture.tenant_winner]));
        fixture.set_active(fixture.tenant_duplicate, true).await;
        assert!(matches!(
            materializer
                .materialize_tenant(fixture.tenant, Utc::now())
                .await,
            Err(ArtifactScheduleMaterializationError::BacklogLimitExceeded)
        ));
        fixture.set_active(fixture.tenant_duplicate, false).await;
        let ids: BTreeSet<_> = materializer
            .effective_bindings(fixture.tenant)
            .await
            .unwrap()
            .into_iter()
            .map(|binding| binding.installation_id)
            .collect();
        assert_eq!(ids, BTreeSet::from([fixture.other, fixture.tenant_winner]));
    }

    #[test]
    fn run_once_selects_latest_missed_slot_and_counts_the_rest() {
        let config = ArtifactScheduleMaterializationConfig::default();
        // Cursor at 00:00, now 01:05 → due 00:10..01:00 (6 slots).
        let decision = due_slots(
            &binding(ModuleScheduleMisfirePolicy::RunOnce),
            at(0, 0),
            at(1, 5),
            &config,
        )
        .expect("due slots");
        assert_eq!(decision.slots, vec![at(1, 0)]);
        assert_eq!(decision.skipped_misfires, 5);
        assert_eq!(decision.advance_through, at(1, 5));
    }

    #[test]
    fn skip_counts_slots_older_than_the_grace_cutoff() {
        let config = ArtifactScheduleMaterializationConfig {
            misfire_grace_seconds: 60,
            max_slots_per_binding: 100,
        };
        // Cursor at 00:00, now 01:05, grace 60s → cutoff 01:04, start
        // 01:03:59: every due slot (00:10..01:00) is older than the grace
        // window, so all six are skipped and none is materialized.
        let decision = due_slots(
            &binding(ModuleScheduleMisfirePolicy::Skip),
            at(0, 0),
            at(1, 5),
            &config,
        )
        .expect("due slots");
        assert!(decision.slots.is_empty());
        assert_eq!(decision.skipped_misfires, 6);
    }

    #[test]
    fn catch_up_paginates_from_the_cursor() {
        let config = ArtifactScheduleMaterializationConfig {
            misfire_grace_seconds: 60,
            max_slots_per_binding: 2,
        };
        let decision = due_slots(
            &binding(ModuleScheduleMisfirePolicy::CatchUp),
            at(0, 0),
            at(1, 5),
            &config,
        )
        .expect("due slots");
        assert_eq!(decision.slots, vec![at(0, 10), at(0, 20)]);
        assert_eq!(decision.advance_through, at(0, 20));
        assert_eq!(decision.skipped_misfires, 0);
    }
}
