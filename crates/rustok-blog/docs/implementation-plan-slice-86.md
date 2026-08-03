# rustok-blog implementation plan — slice 86 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-85.md`.

Slices 1–85 retain the typed Comments remote boundary, signed user-write
delegation, scheduled key lifecycle, authorized mutation, canonical schedule
persistence digest, PostgreSQL state CAS, transactionally coupled successful
authorization outbox, isolated PostgreSQL success/conflict/concurrency evidence,
and a source-ready commit-acknowledgement fault harness.

## 2026-08-03 continuation audit

The slice-85 artifacts are present on current `main`:

- `apps/server/tests/blog_comments_schedule_audit_postgres_faults.rs`;
- `crates/rustok-blog/docs/implementation-plan-slice-85.md`;
- `crates/rustok-blog/contracts/evidence/blog-comments-tcp-delegation-schedule-audit-postgres-faults.json`;
- `scripts/verify/verify-blog-comments-tcp-delegation-schedule-audit-postgres-faults.mjs`.

The production audited PostgreSQL store still maps a dropped worker response to
`CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable`. The outer
audited trigger still treats that result as indeterminate after a submitted
audited write and calls `std::process::abort()`.

Slice 85 documented that behavior but deliberately left deterministic
worker-response-disconnect evidence open. It covered PostgreSQL commit response
loss, third-state mismatch, and reconciliation read exhaustion instead.

## Slice 86 — audited worker-response disconnect fail-stop harness

### Scope

Slice 86 adds one private diagnostic backend and one ignored subprocess harness
inside:

```text
apps/server/src/services/comments_provider_runtime_keyring_schedule_postgres_audited_trigger.rs
```

The diagnostic backend is compiled only under `cfg(test)`. It is not exported,
selected by configuration, reachable from server startup, or present as a
production enum variant.

The ordinary constructors still accept the original concrete
`PostgresCommentsTcpDelegationScheduleAuditedPersistenceStore`. In non-test
builds the private backend enum has exactly one variant, `Production`, which
delegates unchanged to that store.

No migration, manifest, feature, dependency, environment profile, public type,
public constructor, listener path, or runtime composition changes.

### Diagnostic worker boundary

The test-only `AuditedStoreResponseDisconnectHarness` owns a bounded synchronous
command queue of one, matching the production store queue bound.

It accepts the same three logical operations needed by the outer persistence
bridge:

1. `VerifyCurrent` responds with success;
2. `BootstrapEmpty` responds with success;
3. `CompareAndStoreWithAudit` receives the response sender, drops it without
   sending a value, and terminates the diagnostic worker.

The candidate replacement therefore passes schedule construction,
authorization, audit-context installation, durable-store command submission,
and the persisted-trigger compare-and-store call before the response channel is
disconnected.

The harness does not return a synthetic conflict, inject a database error, call
the abort helper directly, panic the caller, or modify the production worker.

### Existing fail-stop path under test

The diagnostic backend converts the disconnected response receiver to the same
`Unavailable` result used by the production store.

The unchanged outer `PostgresAuditedPersistenceBridge::compare_and_store`
matches that result and enters
`abort_on_indeterminate_audited_store_response()`.

The test therefore exercises the real audited wrapper fail-stop boundary rather
than asserting only that `std::process::abort()` exists.

### Subprocess discrimination

The ignored parent test is:

```text
audited_worker_response_disconnect_aborts
```

It re-executes the same `rustok-server` unit-test binary with only the ignored
child entry selected:

```text
audited_worker_response_disconnect_child
```

The child:

- builds a valid generation-1 host schedule;
- bootstraps through the test-only backend;
- builds a lifecycle-preserving generation-2 successor schedule;
- uses a non-nil request UUID, non-nil actor UUID, and `Service` principal;
- emits a readiness marker immediately before the audited replacement;
- calls the ordinary public `replace_host_schedule` method.

The parent applies a ten-second bound. If the child does not terminate, it kills
and reaps it.

On Unix, success requires signal 6 (`SIGABRT`). An ordinary panic, setup failure,
missing test filter, or successful return does not satisfy the gate. On
non-Unix platforms the source harness retains the weaker abnormal-exit
expectation plus the readiness marker.

### Preserved behavior

Slice 86 does not change:

- schedule lifecycle or replacement validation;
- principal admission or host authorization;
- audit context installation and cleanup;
- process-local audit ring behavior;
- canonical persistence digest;
- PostgreSQL worker, SQL, transaction, reconciliation, or retry policy;
- state or outbox migrations;
- durable request identity;
- outbox publication state;
- TCP transport, listener, channel, keyring, replay, or scheduling behavior;
- manifests, features, dependencies, or `Cargo.lock`.

### Explicit non-claims

Slice 86 does not claim:

- that Rust unit tests compiled or ran;
- that the subprocess harness executed;
- that `SIGABRT` was observed;
- PostgreSQL contact, migration execution, or database evidence;
- that the production worker itself was forcibly terminated;
- operating-system thread-kill support;
- commit-acknowledgement runtime evidence from slice 85;
- outbox publishing, leasing, retry/backoff, retention, or delivery;
- durable audit completeness for denied or failed attempts;
- automatic fail-stop recovery;
- clock synchronization or distributed activation;
- shared, durable, multi-replica, or restart-safe replay prevention;
- workflow, CI, or production validation.

Status: `source_verified_no_compile`.
Compile policy: `not_run_by_request`.
Runtime status: `not_run`.

## Updated implementation results

1. Execute slices 84, 85, and 86 at an exact revision and retain PostgreSQL
   version, commands, child exit statuses, signal observations, bounded trace
   summaries, and cleanup results.
2. Add the outbox dispatcher contract: bounded claim leases, stable request
   identity, idempotent delivery, retry/backoff, publication fencing, and
   retention.
3. Define the operator recovery ceremony for fail-stop, corruption, lost state,
   externally advanced state, and partially delivered outbox events.
4. Add clock-health ownership and replace process-local replay admission before
   distributed activation claims.

The worker-response-disconnect source gate is now represented and is no longer
listed as an unimplemented design item. Runtime evidence remains pending until
maintainers execute the ignored test.

## Suggested verification — intentionally not run

```bash
node scripts/verify/verify-blog-comments-audited-worker-response-disconnect.mjs
cargo test -p rustok-server \
  services::comments_provider_runtime::keyring_schedule_postgres_audited_trigger::tests::audited_worker_response_disconnect_aborts \
  -- --ignored --nocapture --test-threads=1
cargo check -p rustok-server --features mod-blog --locked
```

## Ownership retained

- Comments owns schedule lifecycle validation, keyring selection, signing,
  verification, request binding, and process-local replay admission.
- The server host owns authorization, audited trigger composition, PostgreSQL
  state/outbox transaction logic, and fail-stop behavior.
- Blog owns persistence migrations and implementation evidence.
- Maintainers own executable subprocess and signal evidence.
