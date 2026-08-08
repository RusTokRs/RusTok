# M6 locale replay checkpoint and one-page worker contract

Status: `checkpoint_worker_source_complete_runner_pending`.

## Purpose

Locale-aware source scans and durable locale replay jobs now exist. This slice connects the one-page application worker and PostgreSQL checkpoint store to the same canonical locale identity, without exposing locale replay through the multi-page runner or GraphQL yet.

## One-page request

`IndexReplayPageRequest` now carries `locale: Option<LocaleKey>`.

The existing public `IndexReplayPageRequest::new(...)` remains schema-wide. The crate-private locale constructor accepts an already canonical `LocaleKey`; it is reserved for the multi-page runner follow-up and retained crate tests.

Before any checkpoint read or source scan, `IndexReplayWorker` resolves the exact `SchemaRegistry` entry:

- a missing schema fails closed as `SchemaNotRegistered`;
- a locale request against `LocaleMode::None` fails closed as `LocaleScopeUnsupported`;
- locale requests against `Optional` or `Required` are admitted;
- schema-wide requests remain admitted for every existing locale mode, including `Required`, so the current full rebuild path is preserved.

## Checkpoint identity

`IndexReplayCheckpointKey` now carries the same optional canonical locale. Its existing `new(...)` constructor remains schema-wide; the locale constructor builds an exact locale key.

The worker derives the checkpoint key and source scan request from the same `IndexReplayPageRequest.locale` value. It does not infer locale from a cursor or returned mutation.

For locale pages the worker calls `IndexSourceScanRequest::for_locale(...)`; schema-wide pages continue to call `IndexSourceScanRequest::new(...)`.

The already-retained source-page validator remains the second fail-closed layer: a locale source cannot return a mutation outside the requested locale.

## PostgreSQL checkpoint adapter

`PostgresIndexReplayCheckpointStore` now requires checkpoint key locale equality with `IndexReplayJobLease.locale` in addition to tenant/source/schema equality.

Checkpoint SQL continues to use the existing composite key:

`tenant + kind + source + module + entity + version + locale_key + partition_key`.

The adapter now binds the canonical locale string when the key is locale-scoped and preserves the historical empty-string locale for schema-wide replay. `partition_key` remains the empty string in both paths.

A schema lease therefore cannot read or commit an `en-US` checkpoint, an `en-US` lease cannot read or commit the schema-wide checkpoint, and `de` remains distinct from both.

## Durable completion

The job-store completion probe introduced with durable locale job scope already looks up the lease's exact locale checkpoint. Once this slice writes locale checkpoints, a locale job can complete honestly.

Retained SQLite source now proves:

1. a schema-wide checkpoint completes only the schema job;
2. the same schema checkpoint leaves `en-US` with `CheckpointMissing`;
3. an `en-US` checkpoint is stored under locale key `en-US` and completes only the `en-US` job;
4. the `de` job remains `CheckpointMissing`;
5. stored checkpoint rows contain separate empty-string and `en-US` locale identities.

## Boundary

This slice does not change:

- `IndexReplayRunRequest` or multi-page runner scope;
- replay cancellation GraphQL transport;
- public GraphQL input;
- `partition_key` behavior;
- targeted/full/shadow rebuild modes;
- Product source SQL (already locale-aware from the previous slice);
- Storefront serving behavior.

Locale page/checkpoint constructors remain internal until the runner uses the same durable locale job identity. External callers therefore cannot create a partial locale replay through the production command transport in this intermediate state.

## Next source slice

The next PR should carry optional locale through `IndexReplayRunRequest` and `PostgresIndexReplayRunner`:

- acquire schema or locale job lease from the same request;
- build schema-wide or exact-locale page requests consistently;
- preserve cancellation, heartbeat, graceful-stop and retry semantics;
- add optional GraphQL `locale` only after authorization, canonicalize through `LocaleKey`, and keep omission exactly schema-wide;
- retain end-to-end locale replay/restart evidence.

`partition_key` and rebuild modes remain separate later contracts.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or `git diff --check` were executed by the implementation agent.
