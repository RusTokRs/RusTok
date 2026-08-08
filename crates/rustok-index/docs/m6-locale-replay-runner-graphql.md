# M6 locale replay runner and GraphQL command scope

Status: `runner_graphql_source_complete_execution_pending`.

## Purpose

This slice completes the source-code path that began with the generic locale source-scan contract and now carries one optional canonical locale through the public replay command boundary, multi-page runner, durable job lease, one-page worker, checkpoint identity and terminal success fence.

The retained sequence is:

1. generic `IndexSourceScanRequest` gained optional canonical locale scope;
2. current Product PostgreSQL replay scans filter that locale before pagination;
3. durable replay jobs gained an explicit `locale` scope and canonical `locale_key` identity;
4. one-page replay requests/checkpoints gained the same locale identity and fail-closed `LocaleMode` admission;
5. this slice carries that identity through `IndexReplayRunRequest`, both runner paths and the authorized GraphQL command.

## Multi-page run request

`IndexReplayRunRequest::new(...)` remains schema-wide and preserves the historical public call shape.

`IndexReplayRunRequest::for_locale(...)` accepts an already canonical `LocaleKey`. Both constructors delegate to one scoped initializer and therefore retain the same page limit, maximum-page, heartbeat and lease-duration validation.

The request exposes its optional locale read-only. It does not gain partition or rebuild-mode state.

## Durable job and page identity

Both ordinary `PostgresIndexReplayRunner::run` and graceful `run_interruptible` call the same `lease_request_for_run(...)` helper:

- no locale -> `IndexReplayJobLeaseRequest::new(...)`;
- locale -> `IndexReplayJobLeaseRequest::for_locale(...)` with the exact request locale.

The page request stored inside `IndexReplayRunRequest` was built from that same locale. The worker therefore scans and checkpoints the same scope as the durable job lease.

The runner never reconstructs locale from source cursor contents, returned mutations, schema defaults, or checkpoint rows.

## Terminal success fence

Before this slice, multi-page `finish_success` required `checkpoint.locale_key = ''` unconditionally. That was correct for schema-wide replay but would make a durable locale job unable to finish honestly after writing a locale checkpoint.

The terminal write now binds one ninth value from `IndexReplayJobLease.locale`:

- schema lease binds the historical empty string;
- locale lease binds the exact canonical locale string.

The completion `EXISTS` probe requires `checkpoint.locale_key` to equal that value. `partition_key` remains the empty string.

Job acquisition, page source scan, checkpoint read/write and final success therefore share one locale identity.

## GraphQL command

`runIndexReplay` now accepts one optional `locale` string in addition to module, entity and schema routing key.

Authority ordering is unchanged and explicit:

1. derive tenant and actor from authenticated request context;
2. require the request-bound effective `modules:manage` permission;
3. only then parse schema input and optional locale;
4. bound locale to 32 bytes and canonicalize through `LocaleKey`;
5. build a schema-wide or exact-locale `IndexReplayRunRequest` with server-owned worker identity and fixed transport budgets.

Omitting locale remains exactly schema-wide. A schema's `LocaleMode::Required` does not cause omission to be rewritten into a default locale. A supplied locale reaches the existing worker/job admission, where `LocaleMode::None` fails closed.

GraphQL still accepts no tenant, actor, worker identity, source name, partition, scheduler handle, replay budget, stop handle or shutdown flag.

## Graceful shutdown and cancellation

The interruptible runner uses the same locale-aware lease helper as ordinary replay. Yield-to-pending, heartbeat, persisted cancellation precedence and duplicate-safe restart semantics are otherwise unchanged.

User cancellation remains job-UUID based inside the authenticated tenant. Locale is not accepted on the cancellation mutation and is not needed to widen or reconstruct cancellation scope.

## Compatibility

Schema-wide callers continue to use `IndexReplayRunRequest::new(...)` and bind the same empty checkpoint locale as before.

The existing Product schema-wide replay path remains available even for locale-required schemas, preserving full rebuild behavior. Locale-scoped commands are additive.

No migration is introduced in this slice because the durable locale job/checkpoint storage shapes were already established by the preceding locale job/checkpoint slices.

## Boundary

This slice does not add:

- partition-scoped source scans, jobs or checkpoints;
- targeted/full/shadow rebuild modes;
- automatic scheduling or retry policy changes;
- Storefront serving changes;
- a new source implementation;
- new cancellation semantics;
- production evidence admission.

Partition replay must remain blocked until a real source contract can scan exactly one partition before pagination. Explicit rebuild modes remain a separate later contract.

## Evidence state and next source boundary

Retained source assertions cover schema-wide omission, canonical locale request construction, common runner lease selection, locale-aware terminal checkpoint SQL and GraphQL authorization-before-locale-parsing.

The next M6 evidence boundary is end-to-end retained locale replay/restart execution through the real runner/command composition, including yielded/restarted locale identity and isolation from the schema-wide and other-locale jobs. Execution/admission remains maintainer-owned.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or `git diff --check` were executed by the implementation agent.
