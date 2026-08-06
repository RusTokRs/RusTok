# Index reconciliation operator runtime

Status: `sealed_source_page_source_complete_transport_and_owner_execution_pending`.

## Purpose

The server publishes guarded Index reconciliation, exact-entity diagnosis, and one-page
missing-entity diagnosis capabilities after replay composition freezes the immutable source and
schema registries.

`IndexReconciliationOperatorRuntime` privately wraps bounded reconciliation run, cancellation,
dead-letter inspection, finding inspection, and same-job recovery adapters.

The sibling `IndexDriftDiagnosisOperatorRuntime` privately wraps:

- `PostgresIndexDriftSnapshotReader`;
- `IndexDriftDigestProducer`;
- `PostgresIndexDriftFindingWriter`.

`IndexDriftSourcePageDiagnosisRuntime` privately composes:

- the frozen `SharedIndexSourceRegistry` for one bounded owner-source page;
- the guarded `IndexDriftDiagnosisOperatorRuntime` for sequential missing-only candidate diagnosis;
- an optional server-owned continuation keyring containing only bounded key IDs, `SecretRef` values,
  lifetime, and the process-owned resolver registry.

The keyring is not published as a separate extension handle. Raw AES key bytes are resolved into one
short-lived codec for one sealed call and are not retained in settings, logs, errors, or debug output.

## Request-bound authority

Every operation requires a non-nil tenant/actor `IndexReconciliationOperatorContext`, a current
request-scoped RBAC snapshot, and effective `Permission::MODULES_MANAGE`.

`diagnose_entity(context, key)` and `diagnose_missing_entity_candidate(context, key)` each accept one
typed `EntityKey`. Authorization runs before request validation, owner-source access, materialized
reads, typed-state classification, digest production, or finding persistence.

The raw internal method `diagnose_source_page(context, schema, cursor, limit)` checks authority before
validating its maximum page size of 32 and constructing `IndexSourceScanRequest`.

The sealed method
`diagnose_source_page_sealed(context, schema, continuation, limit)` preserves a stronger boundary:

1. authorization precedes untrusted continuation parsing;
2. the page limit is validated before secret resolution or token work;
3. canonical tenant/schema/owner/source scope comes from the frozen source registry;
4. referenced keys are resolved and must decode to exactly 32 bytes;
5. the token is authenticated, decrypted, scope-checked, and expired;
6. `IndexSourceScanRequest` is constructed only after token admission;
7. the existing one-page missing-only path is called exactly once;
8. any outgoing raw cursor is sealed before the result leaves the service boundary.

## Published operator surface

`IndexReconciliationOperatorRuntime` exposes only:

- `run(context, request)`;
- `request_cancel(context, job_id)`;
- `inspect_dead_letter(context, job_id)`;
- `inspect_drift_finding(context, finding_id)`;
- `requeue_dead_letter(context, job_id, reason)`.

`IndexDriftDiagnosisOperatorRuntime` exposes only:

- `diagnose_entity(context, key)`;
- `diagnose_missing_entity_candidate(context, key)`.

`IndexDriftSourcePageDiagnosisRuntime` exposes:

- the internal compatibility method `diagnose_source_page(context, schema, cursor, limit)`;
- the transport-safe internal method
  `diagnose_source_page_sealed(context, schema, continuation, limit)`.

The sealed result contains current-page counts, bounded missing-finding receipts, and one optional
opaque token. It exposes no raw source cursor, source entity identifier, owner/index record, field,
link, SQL, database cause, secret reference, key bytes, registry handle, scheduler handle, or repair
handle.

The source-page runtime skips retained source `Delete` mutations and sequentially diagnoses only
source `Upsert` candidates. Materialized `Upsert` or `Delete`, including stale field/link/version
mismatches, returns `NotCandidate` and does not call the finding recorder through this path.

## GraphQL diagnosis transport

The existing root GraphQL mutation remains only:

- `diagnoseIndexEntity(input: IndexDriftDiagnosisInput!): IndexDriftDiagnosisPayload!`.

It diagnoses one caller-known exact entity and exposes only bounded digest/finding-receipt metadata.
The source-page runtime, sealed method, opaque token, counters, source scan, and server-owned
continuation keyring are not attached to GraphQL by this slice.

## Confidential source continuation

The database-neutral `IndexSourceContinuationCodec` uses AES-256-GCM with exact tenant, schema,
canonical owner/source, version, issued-at, and expiry claims. One active key seals new tokens while
bounded retained keys support decryption during rotation.

Server configuration is deployment-owned:

- `RUSTOK_INDEX_SOURCE_CONTINUATION_KEYRING_JSON` stores key IDs and `SecretRef` values only;
- secret values use URL-safe unpadded base64 and decode to exactly 32 bytes;
- this slice admits only `env` and `mounted_file` resolver aliases;
- at most 16 keys and a lifetime of 1 through 900 seconds are allowed.

Synchronous composition validates configuration and resolver policy. Actual asynchronous secret
resolution occurs inside `diagnose_source_page_sealed` before token parsing or source scan. Failures
map to bounded server errors without resolver causes, reference names, or key material.

## Composition

The server replay composition remains the single source-freezing point:

1. selected PostgreSQL source factories construct ordinary replay sources and optional owner
   absence providers without executing SQL;
2. `SharedIndexSourceRegistry` is frozen;
3. replay runtime and reconciliation work registration are published;
4. the guarded reconciliation operator is constructed;
5. exact diagnosis materializes the optional `SharedIndexSourceAbsenceRegistry`, verifying owner
   parity against the frozen replay registry;
6. general and missing-only exact diagnosis methods are inserted through one guarded runtime;
7. if a source registry exists, deployment-owned continuation configuration is validated and one
   private keyring runtime is built;
8. one-page diagnosis is inserted with the frozen source registry, exact diagnosis runtime, and the
   optional private keyring;
9. GraphQL schema construction receives the frozen `ModuleRuntimeExtensions` and mounts only the
   bounded exact-entity mutation.

The keyring is passed directly to the page runtime and is never inserted as its own extension.
Composition performs no diagnosis SQL, secret resolution, source scan, or task spawn.

## Explicit Product locale absence

For Product v1/v2, an empty ordinary targeted load is accepted as source `Missing` only with the
exact positive `products.index_revision` absence watermark. The reader reloads owner state and the
same watermark around its materialized snapshot. Changed state/version returns retryable
`index_drift_source_changed_during_capture`; unavailable proof remains
`index_drift_source_watermark_missing`.

## Explicitly open

- a GraphQL, HTTP, CLI, MCP, or native-admin source-page transport;
- retained secret-resolution, rotation, expiry, authorization, PostgreSQL, and GraphQL execution
  evidence;
- cursor persistence, multi-page accumulation, background iteration, scheduling, or restart state;
- reconciliation run/cancel/inspection/requeue transports;
- bounded Index-only stale enumeration and orphan-link diagnosis;
- finding resolution or ignore transitions with actor/reason audit;
- targeted/full/shadow repair admission, execution, audit, and evidence;
- operator-visible scheduler health and metrics;
- locale or partition checkpoint dimensions.

Exact-entity diagnosis, Product locale-absence fencing, exact GraphQL transport, missing-only page
diagnosis, confidential continuation codec, server-owned SecretRef keyring, and the sealed internal
page boundary are source complete. Public source-page transport, broader diagnosis, repair, and
retained execution evidence remain open.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, cryptographic integration, database or GraphQL
scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.