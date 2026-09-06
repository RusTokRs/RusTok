# M6 durable locale replay job scope

Status: `job_scope_source_complete_checkpoint_worker_pending`.

## Purpose

The generic source request and current Product PostgreSQL source can now scan one exact canonical locale before pagination. Durable replay identity must match that source scope before checkpoints or GraphQL can safely expose locale replay.

This slice introduces an Index-owned durable **locale job scope** while preserving the existing schema-wide replay job contract byte-for-byte at its public constructor and stored v1 request shape.

## Durable scope model

`index_jobs` gains a permitted `scope_kind = 'locale'` shape:

- tenant, module, entity and schema version are required;
- `entity_id` remains null;
- `locale_key` is required, non-empty and bounded to 32 bytes;
- schema-wide jobs remain `scope_kind = 'schema'` with `locale_key IS NULL`;
- entity/global job shapes remain unchanged.

The scope index now includes `locale_key` after `schema_version`, so distinct locale jobs do not share one schema-only index prefix.

`partition_key` is not introduced into `index_jobs` and remains outside this slice.

## Migration boundary

`m20260808_000009_add_index_job_locale_scope` follows the existing Index cross-backend constraint-relaxation pattern:

- PostgreSQL discovers and replaces exactly one `index_jobs` scope CHECK, then recreates `idx_index_jobs_scope` with locale dimension;
- SQLite rebuilds `index_jobs`, copies all existing rows and recreates the claim/scope indexes;
- down migration restores the previous scope CHECK/index contract.

The migration does not reinterpret existing schema jobs and does not manufacture locale values for old rows.

## Job request/lease identity

`IndexReplayJobLeaseRequest::new(...)` remains schema-wide and keeps the stored exact request contract:

```json
{"contract":"index_replay_job_v1","source_name":"..."}
```

The crate-private locale constructor carries an already canonical `LocaleKey`. Locale jobs persist:

```json
{"contract":"index_replay_job_v2","source_name":"...","locale":"en-US"}
```

The v2 JSON locale must exactly equal the persisted canonical `locale_key`. Stored jobs fail closed if scope kind, locale column, request version or request locale disagree.

The locale constructor is intentionally crate-private in this slice. No external caller or GraphQL transport can create a locale job until worker/checkpoint semantics are source-complete.

## Schema admission

Before any job lookup/insert, the job store reads the exact active persisted `index_schemas` row. Locale jobs additionally deserialize the persisted `IndexSchema` and reject `LocaleMode::None` with `LocaleScopeUnsupported`.

`LocaleMode::Optional` and `LocaleMode::Required` are eligible for locale job identity. Schema-wide replay remains allowed for all existing schemas, including `LocaleMode::Required`, preserving the current full-rebuild path.

## Lock, lookup and lease semantics

The PostgreSQL advisory scope lock includes explicit scope kind plus canonical locale so schema-wide, `en-US` and `de` are different lock identities.

Active-job lookup is null-safe and exact:

- PostgreSQL uses `locale_key IS NOT DISTINCT FROM $6`;
- SQLite uses `locale_key IS ?6`;
- both also require exact `scope_kind`.

A claimed lease retains the optional canonical locale. Existing heartbeat/failure/retry fencing remains based on tenant/job/worker/attempt and therefore needs no locale duplication after the job UUID has been selected.

## Completion fail-closed boundary

`PostgresIndexReplayJobStore::succeed` now looks for a complete checkpoint with the lease's exact locale key (empty string only for schema-wide jobs) and still requires `partition_key = ''`.

This slice deliberately does **not** yet teach `IndexReplayCheckpointKey` or `PostgresIndexReplayCheckpointStore` to write a locale checkpoint. Therefore a locale job cannot accidentally succeed through a schema-wide checkpoint; it remains `CheckpointMissing` until the next source slice adds locale checkpoint identity.

That temporary fail-closed state is intentional and is why the locale job constructor remains crate-private.

## Retained source evidence

`source_replay_locale_job_tests.rs` retains SQLite source that requires:

- independent schema-wide, `en-US` and `de` jobs for one tenant/schema/source;
- canonical `EN-us` input persisted as `en-US`;
- a second `en-US` worker to observe `Busy` rather than collide with another locale;
- exact v1 schema and v2 locale request JSON;
- a completed schema checkpoint to finish only the schema job, while locale completion remains `CheckpointMissing`;
- locale job admission against `LocaleMode::None` to fail before creating an `index_jobs` row.

The packet applies the real Index migration chain, including the new cross-backend migration source, but the implementation agent did not execute it.

## Still open

- locale-bearing `IndexReplayCheckpointKey` and checkpoint PostgreSQL adapter;
- locale-aware one-page replay request/worker admission and exact source scan construction;
- locale-bearing multi-page run request and cancellation/runner scope;
- optional GraphQL `locale` input with canonicalization after authorization;
- retained end-to-end locale replay/restart evidence;
- `partition_key` source/job/checkpoint semantics;
- explicit targeted/full/shadow rebuild modes.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or `git diff --check` were executed by the implementation agent.
