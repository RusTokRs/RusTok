# M6 drift finding inspection

Status: `source_complete_server_authorized_repair_pending`.

## Purpose

`PostgresIndexDriftFindingInspector` provides one bounded read-only diagnosis boundary over an exact open row in `index_consistency_findings`.

The adapter accepts one non-nil tenant UUID and one non-nil finding UUID. Its query is restricted to that exact pair and `state = 'open'`. Resolved, ignored, cross-tenant, and unknown findings return no inspection.

This slice consumes the existing canonical findings table. It does not create a finding writer, compare source and Index state, infer drift, or mutate repair state.

## Returned contract

A successful inspection contains only:

- finding UUID;
- exact lowercase SHA-256 finding key;
- bounded check name;
- typed `info`, `warning`, or `error` severity;
- typed global, schema, or entity scope;
- optional exact lowercase SHA-256 expected digest;
- optional exact lowercase SHA-256 actual digest.

Schema scope returns a validated `SchemaRef`. Entity scope additionally returns one non-nil entity UUID and one canonical `LocaleKey`.

Stored scope columns must match the existing migration contract exactly. Global scope carries no schema/entity/locale identity. Schema scope carries module, entity, and positive schema version only. Entity scope carries that schema plus non-nil entity UUID and canonical locale. Any mismatch fails closed through `InvalidStoredFinding`.

Finding keys and digests must be exactly 64 lowercase hexadecimal bytes. Check names must be non-empty, trimmed, control-character-free, and at most 128 UTF-8 bytes.

## Privacy and read-only boundary

The SELECT deliberately excludes:

- tenant identity;
- raw `details` JSON;
- first- and last-detected timestamps;
- closure timestamp;
- job, worker, lease, cursor, source, mutation, and transport data;
- SQL and database causes.

Database failures map to one stable detail-free `Storage` error.

The adapter performs no insert, update, delete, state transition, finding acknowledgement, repair admission, source scan, targeted load, scheduling, polling, sleep, or task creation.

## Authorized server boundary

The guarded server `IndexReconciliationOperatorRuntime` now composes one private `PostgresIndexDriftFindingInspector` beside the canonical runner, dead-letter inspector, and recovery store.

`inspect_drift_finding(context, finding_id)` accepts no tenant or actor parameter. It:

1. validates the existing request-bound operator context;
2. resolves the current exact tenant/actor RBAC snapshot;
3. requires effective `modules:manage`;
4. only then calls `inspect(context.tenant_id(), finding_id)`.

Missing request authority and `modules:read` fail before nil-finding validation or database access. An authorized nil finding reaches the bounded crate adapter and returns typed `NilFindingId`.

The server returns only `Option<IndexDriftFindingInspection>`. It contains no direct SQL, does not decode or copy raw `details`, and exposes neither the adapter nor the database connection. GraphQL, HTTP, CLI, MCP, native admin, and other transports remain open.

## Repair boundary

This inspection does not claim that an open finding is correct, current, repairable, or admitted for production mutation.

Targeted repair requires separate contracts for:

- authoritative source snapshot or high-watermark identity;
- supported finding/check classes;
- exact target keys and bounded targeted loads;
- advisory locking and concurrency fencing;
- immutable actor/reason and before/after evidence;
- dry-run, targeted, full, and shadow modes;
- post-repair digest verification and retained PostgreSQL evidence.

No automatic finding closure or mutation is allowed from inspection alone.

## Explicitly open

- source/index digest comparison and finding persistence;
- orphan diagnosis;
- targeted repair request/admission and immutable repair audit;
- full and shadow repair modes;
- automatic finding resolution after admitted repair;
- locale or partition checkpoint dimensions;
- retained PostgreSQL authorization, inspection, diagnosis, repair, and concurrency evidence;
- public/admin command transport.

The canonical roadmap item `Add drift diagnosis, targeted repair commands, and admitted repair evidence` remains open. This slice establishes bounded read-only inspection plus internal request-bound authorization only.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, SQLite/PostgreSQL scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
