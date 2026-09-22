# M6 drift finding inspection

Status: `source_complete_server_authorized_snapshot_reader_and_repair_pending`.

## Purpose

`PostgresIndexDriftFindingInspector` provides one bounded read-only diagnosis boundary over an exact open row in `index_consistency_findings`.

The adapter accepts one non-nil tenant UUID and one non-nil finding UUID. Its query is restricted to that exact pair and `state = 'open'`. Resolved, ignored, cross-tenant, and unknown findings return no inspection.

This slice consumes the canonical findings table. The separate `PostgresIndexDriftFindingWriter` can now persist already-computed bounded digest mismatches for both locale-bearing and locale-free entity keys. The database-neutral producer compares typed snapshot views and delegates unequal digests, while authoritative production snapshot capture and repair remain separate boundaries.

## Returned contract

A successful inspection contains only:

- finding UUID;
- exact lowercase SHA-256 finding key;
- bounded check name;
- typed `info`, `warning`, or `error` severity;
- typed global, schema, locale-bearing entity, or locale-free entity scope;
- optional exact lowercase SHA-256 expected digest;
- optional exact lowercase SHA-256 actual digest.

Schema scope returns a validated `SchemaRef`. Both entity variants additionally return one non-nil entity UUID. Locale-bearing entity scope returns one canonical `LocaleKey`; locale-free entity scope is represented explicitly as `EntityWithoutLocale` and never uses an empty or invented locale.

Stored scope columns must match the forward-migrated contract exactly. Global scope carries no schema/entity/locale identity. Schema scope carries module, entity, and positive schema version only. Entity scope carries that schema plus a non-nil entity UUID and an optional locale column. A non-null locale must parse canonically and preserve its exact stored bytes. Any mismatch fails closed through `InvalidStoredFinding`.

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

The guarded server `IndexReconciliationOperatorRuntime` composes one private `PostgresIndexDriftFindingInspector` beside the canonical runner, dead-letter inspector, and recovery store.

`inspect_drift_finding(context, finding_id)` accepts no tenant or actor parameter. It:

1. validates the existing request-bound operator context;
2. resolves the current exact tenant/actor RBAC snapshot;
3. requires effective `modules:manage`;
4. only then calls `inspect(context.tenant_id(), finding_id)`.

Missing request authority and `modules:read` fail before nil-finding validation or database access. An authorized nil finding reaches the bounded crate adapter and returns typed `NilFindingId`.

The server returns only `Option<IndexDriftFindingInspection>`. It contains no direct SQL, does not decode or copy raw `details`, and exposes neither the adapter nor the database connection. GraphQL, HTTP, CLI, MCP, native admin, and other transports remain open.

The writer is not composed into this server runtime. The digest producer is also not composed into this server runtime.

## Repair boundary

Inspection and finding persistence do not claim that an open finding is correct, current, repairable, or admitted for production mutation.

Targeted repair requires separate contracts for:

- authoritative source snapshot or high-watermark identity;
- supported finding/check classes;
- exact target keys and bounded targeted loads;
- advisory locking and concurrency fencing;
- immutable actor/reason and before/after evidence;
- dry-run, targeted, full, and shadow modes;
- post-repair digest verification and retained PostgreSQL evidence.

No automatic finding closure or mutation is allowed from inspection alone. Writer persistence also cannot close a finding or mutate indexed data.

## Explicitly open

- authoritative production source/index snapshot reader composition;
- orphan diagnosis;
- targeted repair request/admission and immutable repair audit;
- full and shadow repair modes;
- automatic finding resolution after admitted repair;
- locale or partition checkpoint dimensions;
- retained PostgreSQL authorization, inspection, diagnosis, writer, migration, repair, and concurrency evidence;
- public/admin command transport.

The former open item `authoritative source/index digest computation and producer composition` has narrowed to production snapshot-reader composition. The canonical roadmap item `Add drift diagnosis, targeted repair commands, and admitted repair evidence` remains open. Current source establishes bounded digest production, persistence, read-only inspection, locale-complete entity scope, and internal request-bound inspection authorization only.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, SQLite/PostgreSQL scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
