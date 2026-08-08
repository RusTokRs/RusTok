# M6 locale replay command/restart evidence

Status: `source_complete_execution_pending`.

## Purpose

This retained packet exercises the source-complete locale replay contract through the real server command stack rather than calling the runner or job store directly:

`IndexReplayMutation::runIndexReplay` -> `IndexReplayOperatorRuntime` -> `SharedIndexReplayRuntime` -> `PostgresIndexReplayRunner` -> one-page replay worker -> PostgreSQL adapters running against SQLite test storage.

The packet is intentionally deterministic and storage-backed. It uses production Index migrations, persisted schema registration, durable `index_jobs`, `index_checkpoints`, `index_inbox` and materialized Index rows. It does not use sleeps, polling, direct runner invocation, direct lease acquisition or hand-written checkpoint rows.

## Fixture contract

The retained schema uses `LocaleMode::Required` and one source with stable owner event IDs.

Source behavior is deliberately shaped around the fixed GraphQL replay budget:

- exact `en-US` scope exposes 9 one-mutation pages;
- exact `de` scope exposes 2 one-mutation pages;
- schema-wide scope exposes the same 11 stable mutations in one page.

The exact-locale pages return mutations carrying exactly the requested canonical locale. The schema-wide page returns the same owner events across both locales, so cross-scope replay exercises canonical inbox idempotency instead of inventing separate deliveries for the same owner facts.

## Evidence sequence

The packet performs the following ordered command sequence on one durable database.

### 1. Canonicalized `en-US` yield

GraphQL receives `locale: "EN-us"` and therefore exercises transport canonicalization before the request reaches the runner.

The fixed GraphQL cap permits 8 pages per invocation, so the first `en-US` run must:

- return `YIELDED`;
- process/apply exactly 8 mutations;
- persist one `scope_kind = 'locale'`, `locale_key = 'en-US'` replay job in `pending` state at attempt 1;
- persist the matching `en-US` checkpoint with cursor `8`.

### 2. Other-locale isolation

While `en-US` is still pending, an independent `de` command runs to completion.

The packet requires:

- a different job UUID;
- `scope_kind = 'locale'`, `locale_key = 'de'`;
- attempt 1 success;
- a complete `de` checkpoint;
- the `en-US` job and cursor to remain pending/unchanged.

This proves another locale cannot acquire, complete or advance the pending `en-US` durable scope.

### 3. Schema-wide isolation plus stable redelivery

The command is then invoked with locale omitted. Omission must preserve schema-wide identity rather than infer a default locale.

The schema-wide source page contains all 11 stable owner events. At this point ten were already applied by the two locale commands, while the ninth `en-US` event was not.

The packet therefore requires the schema-wide command to:

- use a third job UUID with `scope_kind = 'schema'` and `locale_key IS NULL`;
- complete in one page;
- report 11 mutations, 1 applied and 10 duplicate deliveries;
- write the historical schema-wide checkpoint identity with empty `checkpoint.locale_key`;
- leave the pending `en-US` job/checkpoint cursor at `8`;
- materialize exactly 11 owner entities and retain exactly 11 applied inbox deliveries.

This demonstrates that job/checkpoint scope is independent from canonical delivery idempotency: the schema-wide rebuild may redeliver and materialize the missing owner fact without stealing the locale checkpoint.

### 4. Fresh-runtime locale resume

A completely fresh distribution/runtime/operator/GraphQL composition is built over the same durable database.

A new authorized `en-US` command must reclaim the original pending locale job and:

- return the same job UUID;
- run as attempt 2;
- process the final source page;
- report the final owner event as `Duplicate` because the schema-wide command already durably applied it;
- commit the `en-US` completion checkpoint;
- terminalize the original locale job as `succeeded`.

At the end, exactly three replay jobs and three checkpoints exist: schema-wide, `en-US` and `de`; all jobs are succeeded and all checkpoints are complete.

## Authority and lifecycle boundary

The packet uses the real GraphQL mutation with request-scoped `modules:manage` authority. Worker IDs and resource budgets remain server-owned.

A real `StopHandle` is present in schema data because the production command samples it, but this packet never calls `.stop()`. Graceful-shutdown semantics remain covered by the separate shutdown evidence packet and are not conflated with bounded-page yield/restart evidence here.

## Non-goals

This packet does not add or claim:

- PostgreSQL execution evidence;
- HTTP/process bootstrap evidence;
- partition-scoped replay;
- targeted/full/shadow rebuild modes;
- cancellation races;
- scheduler/retry-policy changes;
- production admission.

It also does not change Product source behavior. The source is a deterministic test module whose only purpose is to prove the generic command/job/checkpoint/idempotency contract.

## Admission state

The source packet is retained in `apps/server/src/graphql/index_replay_locale_tests.rs` and registered only under `#[cfg(test)]`.

Execution and admission remain maintainer-owned. Until the packet is actually run and reviewed, the plan item `Execute/admit retained locale replay/restart command evidence` remains open.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or `git diff --check` were executed by the implementation agent.
