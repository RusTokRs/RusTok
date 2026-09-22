# M6 replay graceful interruption and restart

Status: `host_binding_source_complete_execution_pending`.

## Purpose

The one-page replay worker exposes cooperative interruption safe points around durable boundaries, and
`PostgresIndexReplayRunner::run_interruptible` carries that contract through the lease/job/checkpoint state
machine without changing ordinary replay or persisted operator cancellation semantics.

The shared Index replay runtime and guarded server operator now expose the same lifecycle-neutral interruptible
entry point. The server GraphQL transport binds that entry point to the existing server-owned
`StopHandle::is_stopping` probe; GraphQL input does not contain or control shutdown state.

## Durable safe points

`PostgresIndexReplayRunner::run_interruptible` adapts one synchronous boolean probe to
`IndexReplayWorker::run_next_page_interruptible`, which checks it:

1. after checkpoint readiness and before source scan;
2. before each mutation application;
3. after page mutations are durable and before checkpoint commit.

The existing `PostgresIndexReplayRunner::run` remains unchanged and does not manufacture an interruption probe.

## Host interruption is not user cancellation

`IndexReplayError::Interrupted` is handled separately from page failure. The runner:

1. first checks whether a persisted operator cancellation won the race;
2. if cancellation is present, returns the existing terminal `Cancelled` outcome;
3. otherwise calls the existing fenced `yield_for_resume` transition;
4. returns `Yielded` with the same durable job UUID and attempt number;
5. leaves the job `pending`, clears lease ownership, records no failure payload, and preserves the last committed
   checkpoint.

Host interruption never sets `cancel_requested` and never publishes `failed`. User-requested cancellation keeps
its existing contract and continues to be persisted through `request_cancel`.

## Server lifecycle binding

`SharedIndexReplayRuntime::run_interruptible` delegates only the boolean probe to the PostgreSQL runner; the
Index crate does not import the server `StopHandle` type.

`IndexReplayOperatorRuntime::run_interruptible` preserves the existing exact tenant/actor and effective
`modules:manage` authorization before delegating the request/probe to the shared Index runtime. The operator also
remains lifecycle-type neutral.

`init_graphql_schema` resolves one shared `StopHandle` from `ServerRuntimeContext`. If no worker/module-work host
created one yet, schema initialization atomically publishes one. Because all `ServerRuntimeContext` clones share
the same typed-value map, later worker/module-work initialization reuses the same lifecycle handle.

For API-only hosts schema initialization retains one `watch::Receiver<bool>` keepalive. That guarantees the
existing `StopHandle::stop()` sender has a receiver and can publish the terminal stop value even when no
background worker subscribed. The keepalive is private server state.

`runIndexReplay` obtains the server-owned handle from GraphQL schema data only after request authorization/input
preparation and calls:

`IndexReplayOperatorRuntime::run_interruptible` -> `SharedIndexReplayRuntime::run_interruptible` ->
`PostgresIndexReplayRunner::run_interruptible`

with `|| stop_handle.is_stopping()` as the probe.

The transport never calls `StopHandle::stop`; callers cannot supply the handle, stop value, probe, worker ID, or
resource budgets. `cancelIndexReplay` remains on the existing persisted cancellation path and does not read or
mutate shutdown state.

## Restart and redelivery

An interruption before source scan performs no source work and leaves no checkpoint change. A new worker can
claim the same pending job, increment the attempt fence, and continue normally.

The important crash-equivalent window is interruption after a mutation has committed but before the page
checkpoint commits. The durable mutation remains in `index_inbox` / materialization while the replay checkpoint
still points at the previous page. A later attempt therefore scans the same page again. Stable delivery identity
causes the already-durable mutation to return `Duplicate`; only then does the resumed page commit its checkpoint
and complete the job.

This intentionally relies on the existing inbox deduplication and monotonic source-version guarantees rather
than introducing rollback or an unsafe synthetic checkpoint advance.

## Retained source evidence

`source_replay_graceful_shutdown_tests.rs` retains deterministic SQLite source for two runner-level cases:

- interruption at the first safe point proves zero source scans, a pending lease-free job, no checkpoint, and
  successful attempt-2 restart;
- interruption at the third safe point of a one-mutation page proves one durable entity/inbox receipt, no
  checkpoint, pending yield, then attempt-2 duplicate redelivery and successful checkpoint/job completion.

Source guards additionally retain the server binding chain, shared lifecycle handle reuse, API-host receiver
keepalive, and the absence of shutdown controls in GraphQL input.

The packet and actual server-shutdown path have not been executed by the implementation agent.

## Still open

- execute/admit the retained runner interruption/restart evidence;
- retain/execute an end-to-end GraphQL request that begins replay, triggers the server lifecycle stop signal, and
  observes pending yield/restart without relying on timing-sensitive sleeps;
- execute/admit GraphQL replay command/cancellation evidence;
- locale/partition replay checkpoint scope;
- explicit rebuild modes;
- broader multi-host/timing evidence.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
