# Index reconciliation operator runtime

Status: `sealed_source_page_graphql_source_complete_owner_execution_pending`.

## Purpose

The server publishes guarded reconciliation, exact-entity diagnosis, one-page missing-entity
diagnosis, and one bounded sealed GraphQL page transport after replay composition freezes the source
and schema registries.

`IndexDriftSourcePageDiagnosisRuntime` privately composes:

- the frozen `SharedIndexSourceRegistry`;
- the guarded `IndexDriftDiagnosisOperatorRuntime`;
- an optional private server-owned continuation keyring containing bounded key IDs, `SecretRef` values, lifetime,
  and the process-owned resolver registry.

The keyring is not published as an independent extension handle. Raw AES key bytes exist only in one
short-lived codec created inside a sealed request.

## Request-bound authority

Every operator method requires a non-nil tenant/actor context, a current request-local RBAC snapshot,
and effective `Permission::MODULES_MANAGE`.

Exact diagnosis authorizes before key validation or dependency access. The raw internal page method
authorizes before page-limit validation and scan-request construction.

`diagnose_source_page_sealed(context, schema, continuation, limit)` additionally:

1. authorizes before untrusted continuation parsing;
2. validates the `1..=32` limit;
3. derives canonical source scope from the frozen registry;
4. resolves exact 32-byte keys through the private keyring;
5. opens and validates the token before constructing `IndexSourceScanRequest`;
6. diagnoses one page exactly once;
7. seals any outgoing raw cursor before returning.

## Published operator surface

`IndexReconciliationOperatorRuntime` exposes:

- `run(context, request)`;
- `request_cancel(context, job_id)`;
- `inspect_dead_letter(context, job_id)`;
- `inspect_drift_finding(context, finding_id)`;
- `requeue_dead_letter(context, job_id, reason)`.

The boundary uses `PostgresIndexDriftFindingInspector` for bounded read-only open-finding diagnosis.
Both inspection methods and requeue authorize before adapter or recovery-request validation and before database access.
The operator runtime does not expose or own that scheduler.
Drift inspection is read-only and is not scheduled.

`IndexDriftDiagnosisOperatorRuntime` exposes:

- `diagnose_entity(context, key)`;
- `diagnose_missing_entity_candidate(context, key)`.

`IndexDriftSourcePageDiagnosisRuntime` exposes:

- internal `diagnose_source_page(context, schema, cursor, limit)`;
- transport-safe `diagnose_source_page_sealed(context, schema, continuation, limit)`.

The sealed result contains only bounded current-page counters, missing-finding receipts, completion
state, and an optional opaque token.

## GraphQL transports

The root mutation now contains two deliberately separate operations:

- `diagnoseIndexEntity(input: IndexDriftDiagnosisInput!)` for one caller-known exact entity;
- `diagnoseIndexSourcePage(input: IndexDriftSourcePageDiagnosisInput!)` for one bounded owner-source
  page through the sealed continuation boundary.

The page input contains module, entity, schema version, limit, and optional opaque token strings only.
Tenant and actor come from authenticated context. Effective `modules:manage` is checked before schema,
limit, or token parsing.

The page resolver delegates exactly once to `diagnose_source_page_sealed`. It does not call the raw
page method and accepts no tenant, actor, owner/source identity, raw cursor, entity ID, entity list,
checkpoint, scheduler, lifecycle, or repair input.

The payload exposes current-page aggregate counters, bounded finding receipts, completion state, and
one opaque token. It exposes no raw cursor, owner/index payload, fields, links, source identity,
secret reference, key material, SQL, or database cause.

## Confidential continuation composition

`IndexSourceContinuationCodec` uses AES-256-GCM and binds encrypted claims to tenant, exact schema,
canonical owner/source identity, contract version, issued-at, and expiry.

`RUSTOK_INDEX_SOURCE_CONTINUATION_KEYRING_JSON` stores only bounded key IDs and secret references.
The JSON is bounded to 16 KiB before parsing; at most 16 unique references are admitted. Secret
values must be canonical URL-safe unpadded base64 and decode to exactly 32 bytes.

Synchronous composition validates configuration shape and resolver policy. Asynchronous secret
resolution occurs inside the sealed method before token parsing or source scan. Resolver causes,
reference keys, token contents, and key material are never copied into GraphQL errors or debug output.

## Composition order

1. selected source factories are materialized;
2. the source registry is frozen;
3. replay and reconciliation runtimes are composed;
4. exact diagnosis and optional absence proof are composed;
5. deployment-owned continuation configuration is validated;
6. the private keyring is passed directly into the page runtime;
7. GraphQL schema construction mounts exact diagnosis and the separate sealed source-page mutation.

Composition performs no secret resolution, source scan, diagnosis SQL, or task spawn.

## Explicitly open

- retained authorization, key-resolution, rotation, expiry, PostgreSQL, and GraphQL evidence;
- persisted continuation, multi-page accumulation, background scanning, scheduling, or restart state;
- stale Index-only and orphan-link discovery;
- finding resolve/ignore lifecycle;
- targeted/full/shadow repair;
- reconciliation command transports and operator-visible scheduler health.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, cryptographic integration, database or GraphQL
scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
