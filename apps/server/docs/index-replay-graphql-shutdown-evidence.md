# Index replay GraphQL shutdown evidence

Status: `source_complete_execution_pending`.

## Purpose

This retained packet closes the source-level gap between the guarded GraphQL replay command and the shared
server shutdown signal without relying on sleeps, polling, or a test-only replay runner entry point.

The packet uses the real `IndexReplayMutation`, real `IndexReplayOperatorRuntime`, real
`SharedIndexReplayRuntime`, real PostgreSQL replay-runner implementation over SQLite, production Index
migrations, schema registration, replay jobs, checkpoints, mutation store, and inbox/materialization state.

It intentionally builds a minimal async-graphql schema at the same schema-data boundary used by the production
resolver instead of constructing the full application `AppSchema`. Production `init_graphql_schema` lifecycle
reservation/keepalive is retained separately by source guards. This packet therefore does not claim full
HTTP/process bootstrap execution.

## Deterministic shutdown handoff

The first runtime publishes a source whose `scan` method uses two `tokio::sync::Notify` values:

1. the GraphQL request runs in its own task with the real request-scoped RBAC permission snapshot;
2. source scan records that it has started and then waits for an explicit release notification;
3. the test waits for that source-start notification, so the replay pre-scan safe point has already passed;
4. the test calls the real shared `StopHandle::stop()`;
5. the test releases source scan;
6. the next replay safe point, immediately before mutation application, observes
   `StopHandle::is_stopping()` through the real GraphQL -> guarded operator -> shared runtime -> runner chain;
7. the command returns `YIELDED` and the durable replay job returns to `pending`.

No wall-clock delay or status polling determines ordering.

## First-attempt assertions

After the shutdown-observed GraphQL command:

- the source was scanned exactly once;
- GraphQL returns `YIELDED` and exposes the durable job UUID;
- the replay job is `pending`, uncancelled, lease-free, incomplete, and still on attempt `1`;
- no replay checkpoint exists;
- no Index entity was materialized;
- no applied inbox delivery exists.

This proves that server shutdown observation remains distinct from persisted user cancellation and does not
manufacture a terminal failure.

## Fresh runtime restart

The packet then constructs fresh module/runtime/operator/GraphQL composition over the same SQLite database and a
new non-stopping `StopHandle`. The source contract and schema identity remain the same, but the scan gate is
removed.

The same authorized GraphQL command:

- reacquires the same durable replay job;
- increments the attempt fence to attempt `2`;
- returns `COMPLETE` with the same job UUID;
- materializes exactly one Index entity and one applied inbox delivery;
- commits the rebuild checkpoint;
- leaves the replay job `succeeded`.

This is restart composition evidence, not an in-memory resume through the first GraphQL schema.

## Duplicate-redelivery boundary

This GraphQL-level packet deliberately stops while source scan is pending, so the first attempt has not yet
applied a mutation. It therefore does not manufacture a duplicate solely to make the assertion richer.

The retained runner-level graceful-shutdown packet remains the owner of the harder crash-equivalent window:
mutation durability succeeds, shutdown is observed before checkpoint commit, and attempt 2 safely observes the
stable delivery as `Duplicate` before completing the checkpoint/job.

Together the packets cover command/lifecycle binding and duplicate-safe durable replay without conflating their
coordination mechanisms.

## Deliberate limits

This source packet does not:

- execute a real HTTP request;
- execute full application/bootstrap startup or OS signal handling;
- invoke production shutdown orchestration beyond the real shared `StopHandle::stop()` primitive;
- test user-requested cancellation races;
- add automatic replay scheduling;
- add locale/partition replay checkpoint scope;
- add targeted/full/shadow rebuild modes;
- claim runtime or CI admission.

The production source guards still require `init_graphql_schema` to publish/reuse one shared StopHandle and keep
an API-host receiver alive, while the transport may only observe `is_stopping()` and may never call `.stop()`.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
