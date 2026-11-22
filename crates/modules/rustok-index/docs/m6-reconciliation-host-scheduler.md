# M6 reconciliation host scheduler

Status: `source_complete_owner_execution_pending`.

## Purpose

Index now publishes one module-owned work registration for due reconciliation jobs. The registration attaches `PostgresIndexReconciliationWorkAdapter` to the platform `ModuleWorkScheduler` only when the immutable Index source and schema registries are present in the host runtime.

The generic host scheduler remains the only polling and lifecycle owner. Index does not start a second Tokio task, create another stop channel, or add a server-specific polling loop.

## Host lifecycle ownership

The existing server module-work bootstrap owns:

- one scheduler instance for all registered module workers;
- a one-second polling cadence;
- one sequential claim per worker slug per iteration;
- deployment background-worker admission;
- the shared `StopHandle` subscription;
- stopping before any new claim after shutdown while allowing already claimed work to finish its canonical completion path.

Index contributes only the worker registration and adapter. An absent source registry registers no Index worker. A source registry without the shared schema registry fails closed during worker registration.

## Due discovery

`ModuleWorkSource::claim` first verifies the exact worker slug `index_reconciliation`. It then executes one bounded read-only query over `index_jobs`.

The query considers only schema-scoped reconciliation rows in `pending`, `running`, `succeeded`, or `failed` state. A window rank mirrors the runner's authority order for each exact tenant/module/entity/schema-version scope:

1. `succeeded`;
2. `running`;
3. `pending`;
4. `failed`.

Only rank one may become work, and only when it is:

- `pending` and due; or
- `running` and due with an expired lease.

In both cases the schema must have a currently selected source and active tenant registration
matching the runtime's fingerprint.

Cancellation-requested expired running rows remain discoverable so the canonical runner can complete their fenced cancellation transition after takeover. Discovery is globally bounded by `LIMIT 1` and deterministically ordered by due time, creation time, tenant, and job identity.

The discovery SELECT is read-only and is not lease acquisition. A malformed due row may be
deferred with a bounded reason only while its original generation, epoch and due state match.

## Stored contract validation

Before a work item is published, the adapter validates:

- non-nil tenant and job UUIDs;
- valid module/entity identifiers and a positive schema version;
- the strict stored `index_reconciliation_job_v1` request object;
- a positive bounded pass count accepted by `IndexReconciliationRunRequest`;
- exact source-name ownership against the immutable source registry.

Malformed stored work fails closed with the stable code `index.reconciliation_scheduler.invalid_stored_job`. Database discovery failures use `index.reconciliation_scheduler.discovery_failed`. Neither error retains SQL, database causes, request JSON, source payloads, tenant values, worker values, or stack text.

## Work item boundary

The module-work item contains only:

- existing reconciliation job UUID as the work identity;
- tenant UUID required by the generic scheduler contract;
- the fixed worker slug;
- a fresh bounded invocation UUID used only to derive the runner worker identity;
- strict payload contract `index_reconciliation_scheduler_item_v1` with module, entity, schema version, pass count, lease generation and retry epoch.

The payload uses `deny_unknown_fields`. It contains no cursor, error details, retry policy, source name, live lease token from Index storage, request JSON, actor, transport, or database detail.

## Canonical execution and fleet safety

`ModuleWorkHandler::execute` reconstructs one bounded `IndexReconciliationRunRequest` and delegates to `PostgresIndexReconciliationRunner`.

Default scheduler invocation policy:

- page limit: `100`;
- maximum pages per invocation: `8`;
- heartbeat every page boundary;
- lease duration: `300 seconds`;
- pass count: retained from the original strict job request.

The adapter never claims `index_jobs` directly. Multiple hosts may discover the same due row, but the runner's existing advisory scope lock and exact pending/running lease transition determine one durable winner. Losing invocations receive the existing `Busy`, completion, or stale-state result without calling the source under a second active attempt.

The runner continues to own:

- exact scope admission;
- attempt increment;
- pending claim and expired-running takeover;
- lease and heartbeat fencing;
- source scan and mutation application;
- progress, yield, success, cancellation, retry, exhaustion, and dead-letter transitions.

`ModuleWorkSource::complete` verifies the original row, selected owner and pass count. Terminal
outcomes require the same retry epoch and the exact lease generation acquired by the dispatch;
an already-completed row can instead match the discovered generation. Success also requires a
valid terminal cursor with all requested passes and no continuation. A later row can confirm a
nonterminal observation (`Held` or `Retryable`), but can't confirm terminal work by an old dispatch.
Completion never writes a job transition or a separate acknowledgment.

## Outcome mapping

`AlreadyComplete` and `Complete` map to module-work `Completed`. `Cancelled` remains
`Cancelled`; `Busy` is `Held`; `Yielded` and `RetryScheduled` are `Retryable`; terminal
permanent or exhausted failures are `Rejected`.

A run error is not completion. The adapter conditionally defers the originally discovered due
row with `index.reconciliation_scheduler.run_failed`. A newer claim or requeue is untouched;
a post-claim failure remains protected by its lease and takeover path.

## Explicitly open

- retained PostgreSQL due scheduling, multi-host contention, retry exhaustion, restart, and shutdown evidence;
- operator-visible scheduler health and metrics;
- per-source policy, jitter, and dynamic configuration;
- GraphQL, HTTP, CLI, MCP, or admin scheduling controls;
- source/index digest comparison, orphan diagnosis, and targeted/full/shadow repair.

The canonical retry/backoff/dead-letter/global-scheduling plan item remains open until the repository owner retains and admits production PostgreSQL and multi-host lifecycle evidence.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, SQLite/PostgreSQL scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
