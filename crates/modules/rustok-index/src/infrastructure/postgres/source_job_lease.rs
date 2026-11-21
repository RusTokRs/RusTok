use sea_orm::DbBackend;

pub(super) fn placeholder_prefix(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => "$",
        DbBackend::Sqlite => "?",
        _ => unreachable!("unsupported database backend was validated"),
    }
}

pub(super) fn lease_clock(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => "clock_timestamp()",
        _ => "CURRENT_TIMESTAMP",
    }
}

/// The four consecutive bindings are tenant, job, worker and lease generation.
/// `kind` is a storage contract literal, never a value from a request.
pub(super) fn live_lease_where(
    backend: DbBackend,
    kind: &'static str,
    first: usize,
    cancelled: bool,
) -> String {
    let prefix = placeholder_prefix(backend);
    let clock = lease_clock(backend);
    let cancel_requested = if cancelled { "TRUE" } else { "FALSE" };
    let guard = format!(
        "kind = '{kind}' AND state = 'running' AND lease_owner = {prefix}{} \
         AND attempt_count = {prefix}{} AND cancel_requested = {cancel_requested}",
        first + 2,
        first + 3,
    );
    locked_job_where(
        backend,
        first,
        &guard,
        "lease_expires_at",
        &format!("lease_expires_at > {clock}"),
    )
}

pub(super) fn job_is_due(backend: DbBackend) -> String {
    let clock = lease_clock(backend);
    format!(
        "(available_at <= {clock} AND (state = 'pending' OR \
         (state = 'running' AND lease_expires_at <= {clock})))"
    )
}

pub(super) fn due_job_where(backend: DbBackend, kind: &'static str) -> String {
    locked_job_where(
        backend,
        1,
        &format!("kind = '{kind}' AND state IN ('pending', 'running')"),
        "available_at, state, lease_expires_at",
        &job_is_due(backend),
    )
}

fn locked_job_where(
    backend: DbBackend,
    first: usize,
    guard: &str,
    columns: &str,
    timing: &str,
) -> String {
    let prefix = placeholder_prefix(backend);
    let key = format!(
        "tenant_id = {prefix}{first} AND job_id = {prefix}{}",
        first + 1
    );
    match backend {
        // A WHERE filter (or even a SELECT projection) can run before LockRows waits.
        // This materialization boundary forces the wall-clock check after the lock,
        // against the current generation, including when the blocker never updated it.
        DbBackend::Postgres => format!(
            "{key} AND EXISTS (WITH locked_job AS MATERIALIZED (\
             SELECT {columns} FROM index_jobs WHERE {key} AND {guard} FOR NO KEY UPDATE\
             ) SELECT 1 FROM locked_job WHERE {timing})"
        ),
        DbBackend::Sqlite => format!("{key} AND {guard} AND {timing}"),
        _ => unreachable!("unsupported database backend was validated"),
    }
}

pub(super) fn deadline_after_seconds(backend: DbBackend, parameter: usize) -> String {
    let clock = lease_clock(backend);
    match backend {
        DbBackend::Postgres => format!("{clock} + (${parameter} * INTERVAL '1 second')"),
        DbBackend::Sqlite => format!("datetime('now', '+' || ?{parameter} || ' seconds')"),
        _ => unreachable!("unsupported database backend was validated"),
    }
}
