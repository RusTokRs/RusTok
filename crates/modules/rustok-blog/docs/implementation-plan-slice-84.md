# rustok-blog implementation plan — slice 84 continuation

This document continues `crates/rustok-blog/docs/implementation-plan-slice-83.md`.

Slices 1–83 retain the typed Comments remote boundary, signed user-write
delegation, scheduled key lifecycle, explicitly authorized mutation, canonical
schedule persistence digest, PostgreSQL state CAS, and one transactionally
coupled successful-authorization audit/outbox row.

## 2026-08-01 continuation audit

Slice 83 is source-ready but deliberately has no PostgreSQL execution artifact.
The highest-value bounded continuation is therefore an ignored, isolated-database
integration harness that exercises the public audited trigger and the full
workspace migrator without introducing a private test-only mutation path.

The harness is committed but intentionally not executed by the implementation
agent. Maintainers retain ownership of PostgreSQL availability and execution.

## Slice 84 — audited PostgreSQL integration harness

### Artifact

The source-ready harness is:

```text
apps/server/tests/blog_comments_schedule_audit_postgres.rs
```

It is compiled under the existing `mod-blog`/`mod-comments` feature composition
and uses existing dependencies only:

- `rustok-server` public Comments runtime facade;
- `rustok-migrations::Migrator`;
- `rustok-test-utils` isolated PostgreSQL helpers;
- SeaORM statements and independent database connections;
- Tokio blocking tasks for synchronous trigger calls.

No manifest, feature, dependency, or `Cargo.lock` change is required.

Both tests are `#[ignore = "requires PostgreSQL admin access"]` and use
`RUSTOK_MIGRATION_SMOKE_ADMIN_URL`, falling back to the repository's standard
local PostgreSQL admin URL.

Each scenario creates a unique database, runs the full workspace migrator,
opens two independent SeaORM connections, executes the scenario, closes the
connections, and removes the database.

### Production API only

The harness constructs:

- canonical `CommentsTcpDelegationSchedulePersistenceDocument` values;
- valid active and future scheduled keys;
- a mandatory host authorizer implementing the public authorizer trait;
- `PostgresCommentsTcpDelegationScheduleAuditedPersistenceStore`;
- `SharedCommentsTcpDelegationPostgresAuditedScheduleTrigger`;
- non-nil service-principal trigger contexts.

It does not call private SQL functions, construct arbitrary persistence records,
bypass the authorizer, mutate a schedule handle directly, or replace the
production PostgreSQL transaction owner.

The only direct SQL mutation is a bounded fixture row used to create a valid
unique-candidate-generation collision. That fixture does not mutate the state
row and exists specifically to prove that a later state update is rolled back
when the outbox insert conflicts.

### Schedule fixture

The initial generation contains one currently active terminal key.

The replacement generations:

- retain the active key identity, secret, and activation time;
- add a retirement time that covers propagation budget, maximum TTL, default
  clock skew, and an additional margin;
- add one successor key whose activation is two minutes in the future;
- keep the same runtime TTL and clock-skew policy;
- use increasing generation numbers.

This follows the production replacement validator rather than weakening it for
tests.

### Atomic success and exact resume scenario

`audited_schedule_success_resume_and_conflicts_are_atomic` performs:

1. bootstrap generation 1 through the audited trigger;
2. require one valid state row and zero synthetic audit rows;
3. replace with generation 2 using a service principal and unique request/actor
   identities;
4. require the returned transition `1 -> 2`;
5. require PostgreSQL generation 2 and the exact accepted digest;
6. require one durable outbox row containing the exact request, actor, principal,
   operation, source, previous generation, candidate generation, success outcome,
   and null `published_at`;
7. construct a second audited trigger on an independent connection with
   `ResumeExact` and require generation 2;
8. require stale generation-1 exact resume to fail.

Bootstrap remains state-only and no actor is invented for startup.

### Request identity conflict rollback

The same scenario reuses the successful request UUID while proposing generation
3.

The production transaction may update the state row before attempting the
outbox insert, but the request primary-key collision makes the outbox insert
affect zero rows. The harness then requires:

- a normal persistence conflict;
- PostgreSQL state still at generation 2;
- exactly one durable outbox row;
- the process-local trigger snapshot still at generation 2;
- the latest bounded process-local audit outcome to be `PersistenceConflict`.

This demonstrates rollback of the state update when durable event identity is
reused.

### Candidate generation conflict rollback

The harness inserts one valid unpublished outbox fixture for candidate generation
3 with a different request UUID. It then attempts generation 3 through the
production audited trigger.

The complete expected state predicate can match and the transaction can update
the state row, but the unique `(state_key, candidate_generation)` index rejects
the new outbox row. The harness requires:

- normal persistence conflict;
- PostgreSQL state still at generation 2;
- no additional outbox row beyond the successful generation-2 event and the
  explicit generation-3 fixture;
- no in-memory snapshot publication;
- process-local `PersistenceConflict` audit.

This is the direct rollback evidence for the state-plus-outbox atomicity rule.

### Concurrent CAS scenario

`concurrent_audited_schedule_cas_commits_one_state_and_one_outbox` creates two
independent audited triggers that both resume generation 1 from separate
connections.

A barrier releases both synchronous replacement calls together. Each call uses
a unique request UUID and proposes the same valid generation-2 schedule.

The harness requires:

- exactly one successful replacement;
- exactly one normal conflict;
- local trigger generations `[1, 2]` after completion;
- PostgreSQL state generation 2;
- exactly one durable outbox row;
- the durable request UUID to equal the winning call's request UUID.

This uses the production complete-record predicate update as the serialization
point. No application-level mutex is shared between the two trigger instances.

### Commit reconciliation boundary

The normal-success scenario reads the exact state/outbox pair that the production
commit-reconciliation path expects. It therefore validates the persisted pair
shape and equality inputs.

This slice does **not** inject a PostgreSQL commit acknowledgement failure,
connection loss, process abort, or third-state pair. A normal committed readback
is not equivalent to ambiguous-commit evidence. Those fault paths remain an
explicit later gate requiring subprocess/crash orchestration.

### Worker shutdown boundary

The audited store owns a dedicated worker thread. Test triggers are dropped
before database cleanup, and the harness gives closed command channels a bounded
settling interval before closing the retained SeaORM connections and dropping
the isolated database.

This is test cleanup behavior only. It does not add a production shutdown/join
API and does not claim deterministic worker termination evidence.

### Preserved production owners

Slice 84 adds test, documentation, evidence, and a source verifier only. It does
not change:

- schedule lifecycle and replacement validation;
- authorization or delegated-principal rejection;
- canonical digest construction;
- slice-81 persisted trigger and persist-before-publish bridge;
- slice-82 ordinary PostgreSQL adapter;
- slice-83 audited PostgreSQL store, wrapper, guard, or migration;
- TCP framing, listener lifecycle, channel selection, or replay behavior;
- any manifest, feature, dependency, or lockfile.

### Explicit non-claims

Slice 84 does not claim:

- that the Rust integration tests compiled or ran;
- that PostgreSQL was contacted;
- successful migration, transaction, conflict, concurrency, or cleanup execution;
- commit-error, crash, abort, network-partition, or third-state reconciliation
  evidence;
- outbox publishing, leasing, retries, retention, or external delivery;
- durable audit completeness for denied or failed attempts;
- automatic recovery after fail-stop;
- shared or restart-safe replay protection;
- clock synchronization or distributed activation;
- workflow, CI, or production validation.

Status: `source_verified_no_compile`.
Compile policy: `not_run_by_request`.
Runtime status: `not_run`.

## Next implementation results

1. Execute the isolated PostgreSQL harness and retain the exact revision,
   PostgreSQL version, command output, and database cleanup result.
2. Add subprocess fault injection for commit acknowledgement loss, exact pair
   recovery, third-state fail-stop, unreadable retry exhaustion, and worker
   response disconnect.
3. Add an outbox dispatcher contract with bounded claim leases, idempotent
   delivery identity, retry/backoff, and retention.
4. Define the operator recovery ceremony for fail-stop, corruption, lost state,
   and externally advanced state.
5. Add clock-health ownership and replace process-local replay admission before
   distributed activation claims.

## Suggested verification — intentionally not run

```bash
node scripts/verify/verify-blog-comments-tcp-delegation-schedule-audit-postgres-harness.mjs
cargo test -p rustok-server --features mod-blog --test blog_comments_schedule_audit_postgres -- --ignored --nocapture
cargo check -p rustok-server --features mod-blog --locked
```

## Ownership retained

- Comments owns schedule lifecycle validation, keyring selection, signing,
  verification, request binding, and process-local replay admission.
- The server host owns authorization, audited trigger composition, PostgreSQL
  state/outbox transaction logic, and this integration harness.
- Blog owns the persistence migrations and implementation evidence.
- Maintainers own execution, PostgreSQL provisioning, fault injection, and
  retention of runtime evidence.
