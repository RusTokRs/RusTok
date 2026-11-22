use rustok_runtime::ModuleWorkOutcome;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, Value as SqlValue};
use uuid::Uuid;

use crate::{LocaleMode, SchemaRegistry, SharedIndexSourceRegistry};

use super::{
    source_job_lease::{deadline_after_seconds, due_job_where, placeholder_prefix},
    source_replay_job::uuid_value,
};

pub(super) const MAX_DISCARDED_CANDIDATES_PER_POLL: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DiscoveredJobVersion {
    pub(super) generation: i64,
    pub(super) retry_epoch: i64,
}

impl DiscoveredJobVersion {
    pub(super) fn matches(self, generation: u32, retry_epoch: i64) -> bool {
        self.generation == i64::from(generation) && self.retry_epoch == retry_epoch
    }

    /// Terminal results can only belong to this dispatch or to the exact completed generation
    /// it observed without acquiring. Held/retryable report no terminal work: another runner
    /// may have already advanced the row. This check never authorizes a write.
    pub(super) fn verifies_outcome(
        self,
        persisted: Self,
        state: &str,
        outcome: ModuleWorkOutcome,
    ) -> bool {
        if persisted.generation < self.generation || persisted.retry_epoch < self.retry_epoch {
            return false;
        }
        let same_epoch = persisted.retry_epoch == self.retry_epoch;
        let acquired = self.generation.checked_add(1) == Some(persisted.generation);
        match outcome {
            ModuleWorkOutcome::Completed => {
                state == "succeeded" && same_epoch && (acquired || persisted == self)
            }
            ModuleWorkOutcome::Cancelled => state == "cancelled" && same_epoch && acquired,
            ModuleWorkOutcome::Rejected { .. } => state == "failed" && same_epoch && acquired,
            ModuleWorkOutcome::Held { .. } | ModuleWorkOutcome::Retryable { .. } => matches!(
                state,
                "pending" | "running" | "succeeded" | "failed" | "cancelled"
            ),
        }
    }
}

/// Discovery is not a lease. Filter by the exact tenant's active persisted registration and
/// immutable owner selection, after choosing the same authoritative row per scope as acquisition.
/// An unselected source or retired registration is held, not discarded or silently replaced.
pub(super) fn discovery_scope(
    backend: DbBackend,
    schemas: &SchemaRegistry,
    sources: &SharedIndexSourceRegistry,
) -> (String, Vec<SqlValue>) {
    let source_name = match backend {
        DbBackend::Postgres => "j.request->>'source_name'",
        DbBackend::Sqlite => {
            "CASE WHEN json_valid(j.request) THEN json_extract(j.request, '$.source_name') END"
        }
        _ => "j.request->>'source_name'",
    };
    let mut clauses = Vec::new();
    let mut values = Vec::new();
    for registered in schemas.iter() {
        let reference = &registered.schema.reference;
        let Some(source) = sources.source_for_schema(reference) else {
            continue;
        };
        let offset = values.len();
        let locale_scope = if registered.schema.locale_mode == LocaleMode::None {
            " AND j.scope_kind = 'schema'"
        } else {
            ""
        };
        clauses.push(format!(
            "(j.module_name = ${} AND j.entity_name = ${} AND j.schema_version = ${} \
             AND s.schema_fingerprint = ${} AND {source_name} = ${}{locale_scope})",
            offset + 1,
            offset + 2,
            offset + 3,
            offset + 4,
            offset + 5,
        ));
        values.extend([
            reference.module.as_str().to_owned().into(),
            reference.entity.as_str().to_owned().into(),
            i64::from(reference.version.get()).into(),
            registered.fingerprint.to_string().into(),
            source.source_name().to_owned().into(),
        ]);
    }
    let scopes = if clauses.is_empty() {
        "FALSE".to_owned()
    } else {
        clauses.join(" OR ")
    };
    let predicate = format!(
        "EXISTS (SELECT 1 FROM index_schemas AS s WHERE s.tenant_id = j.tenant_id \
         AND s.module_name = j.module_name AND s.entity_name = j.entity_name \
         AND s.schema_version = j.schema_version AND s.status = 'active' AND ({scopes}))"
    );
    (predicate, values)
}

/// Park a due row whose pre-lease processing failed. Preserve request, job ID, checkpoint and
/// retry budget. If another host claimed, requeued or completed the row, this write is a no-op.
pub(super) async fn defer_due_job(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    job_id: Uuid,
    kind: &'static str,
    lease_generation: i64,
    retry_epoch: i64,
    code: &'static str,
) -> Result<(), sea_orm::DbErr> {
    let backend = db.get_database_backend();
    db.execute_raw(Statement::from_sql_and_values(
        backend,
        defer_due_job_sql(backend, kind),
        vec![
            uuid_value(tenant_id, backend),
            uuid_value(job_id, backend),
            lease_generation.into(),
            retry_epoch.into(),
            code.into(),
            60_i64.into(),
        ],
    ))
    .await?;
    Ok(())
}

fn defer_due_job_sql(backend: DbBackend, kind: &'static str) -> String {
    let prefix = placeholder_prefix(backend);
    let later = deadline_after_seconds(backend, 6);
    let due = due_job_where(backend, kind);
    format!(
        "UPDATE index_jobs SET available_at = {later}, updated_at = CURRENT_TIMESTAMP, \
         last_error_code = {prefix}5, last_error_details = NULL WHERE {due} \
         AND attempt_count = {prefix}3 AND retry_epoch = {prefix}4"
    )
}
