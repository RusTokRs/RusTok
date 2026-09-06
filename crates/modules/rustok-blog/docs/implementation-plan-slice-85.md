# rustok-blog implementation plan — slice 85 continuation

This document continues
`crates/rustok-blog/docs/implementation-plan-slice-84.md`.

Slices 1–84 retain the typed Comments remote boundary, signed user-write
delegation, scheduled key lifecycle, explicitly authorized mutation, canonical
schedule persistence digest, PostgreSQL state CAS, one transactionally coupled
successful-authorization audit/outbox row, and a source-ready isolated
PostgreSQL success/conflict/concurrency harness.

## 2026-08-01 continuation audit

Slice 84 deliberately stops before ambiguous-commit evidence. Reading the exact
state/outbox pair after an ordinary successful commit does not prove that the
slice-83 store recovers when PostgreSQL committed but the client did not receive
the commit acknowledgement.

Slice 85 adds a source-ready subprocess and loopback PostgreSQL wire harness. It
drops the first audited replacement commit response after PostgreSQL emits
`ReadyForQuery`, then lets the unchanged production store execute its real
bounded reconciliation path.

The harness is committed but intentionally not compiled or executed by the
implementation agent.

## Slice 85 — PostgreSQL commit-acknowledgement fault harness

### Artifact

The ignored integration harness is:

```text
apps/server/tests/blog_comments_schedule_audit_postgres_faults.rs
```

It uses existing dependencies only:

- the public `rustok-server` Comments runtime facade;
- the full `rustok-migrations::Migrator`;
- `rustok-test-utils` isolated PostgreSQL helpers;
- SeaORM for direct assertion and one deliberate third-state mutation;
- Tokio TCP/process primitives;
- the existing `url` server dev dependency.

No manifest, feature, direct dependency, or `Cargo.lock` change is required.

### Production owner preservation

Slice 85 does not add a fault constructor, environment switch, mock database, or
test branch inside the slice-83 store.

The following owners remain byte-for-byte unchanged:

- `comments_provider_runtime_keyring_schedule_persistence_postgres_audit.rs`;
- `comments_provider_runtime_keyring_schedule_postgres_audited_trigger.rs`;
- the state and outbox migrations;
- the persisted trigger, persistence bridge, schedule lifecycle, and trigger
  authorization owners.

The child process constructs the public audited PostgreSQL store and trigger.
The commit failure is introduced outside the process at the PostgreSQL wire
boundary.

### Isolated database setup

Each parent scenario:

1. reads `RUSTOK_MIGRATION_SMOKE_ADMIN_URL`, with the repository local fallback;
2. creates a unique PostgreSQL database;
3. runs the full workspace migrator;
4. constructs one valid generation-1 host schedule;
5. bootstraps generation 1 through the production audited trigger;
6. drops that trigger and waits a bounded settling interval;
7. starts a loopback PostgreSQL proxy;
8. re-executes the same ignored integration test binary as a child process;
9. inspects durable state and outbox rows after the child exits;
10. stops the proxy, closes retained connections, and removes the database.

The child uses a one-connection SeaORM pool, resumes generation 1 exactly, and
attempts one authorized generation-2 replacement with parent-provided request
and actor UUIDs.

### Plaintext PostgreSQL proxy boundary

The child proxy URL is forced to `sslmode=disable`. The proxy supports a
transparent PostgreSQL startup packet and rejects encrypted negotiation.

After startup, it parses bounded PostgreSQL frontend and backend frames:

- simple Query messages;
- Parse messages used by extended query flow;
- typed server response messages.

It recognizes only normalized `COMMIT` or `COMMIT TRANSACTION`.

When the first replacement COMMIT is forwarded upstream, the proxy:

1. forwards the COMMIT request to PostgreSQL;
2. reads and withholds every response frame for that COMMIT;
3. waits until PostgreSQL emits backend `ReadyForQuery`;
4. applies the selected fault action;
5. closes the client-side connection without forwarding the commit response.

Waiting for `ReadyForQuery` means the server has completed the transaction
command before the acknowledgement is lost. This is a protocol-level source
design; execution remains pending.

The proxy limits startup and typed frames to one MiB. It is not a general
PostgreSQL proxy and is not added to production composition.

### Exact-pair recovery scenario

`commit_ack_loss_exact_pair_reconciles_successfully` selects no post-commit
database mutation.

After the proxy drops the COMMIT response:

- SeaORM must report commit failure to the production store;
- the store must enter `reconcile_ambiguous_audited_commit`;
- the pool reconnects through the now-transparent proxy;
- the exact generation-2 state and exact request outbox row must be visible;
- reconciliation returns success;
- slice 81 publishes the generation-2 in-memory snapshot;
- the child exits successfully.

The parent then requires:

- child exit success;
- state generation 2;
- one 64-character digest different from the deliberate third-state digest;
- one unpublished outbox row;
- the exact request UUID;
- previous generation 1;
- candidate generation 2;
- exactly one outbox row.

This is the source-ready path for committed-exact-pair recovery.

### Third-state fail-stop scenario

`commit_ack_loss_third_state_fail_stops` uses an independent direct PostgreSQL
connection owned by the parent proxy.

After the server has completed the generation-2 commit but before the proxy
closes the child connection, the proxy performs one deliberate external state
advance:

```text
generation = 3
schedule_digest_hex = ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff
```

The generation-2 outbox row remains unchanged.

The child then receives commit failure and production reconciliation observes:

- a valid but unexpected generation-3 state row;
- the exact generation-2 request outbox row.

That is neither the exact candidate pair nor an unreadable retry. The production
owner must call `std::process::abort()`.

On Unix the parent expects `SIGABRT`. It then requires the durable mismatch to
remain visible: state generation 3 and one candidate-generation-2 outbox row.

The mutation is deliberately unaudited corruption used only inside the isolated
fault harness. It is not an operator repair mechanism.

### Unreadable retry-exhaustion scenario

`commit_ack_loss_unreadable_reconciliation_fail_stops` leaves the committed
generation-2 state/outbox pair intact.

After dropping the COMMIT response, the proxy accepts and immediately closes
every subsequent child connection. The child pool has one connection, so every
production reconciliation read must reconnect through the refusing proxy.

The unchanged production owner retries unreadable state/outbox results 20 times
with its 100 ms delay and then calls `std::process::abort()`.

On Unix the parent expects `SIGABRT`. After the child exits, the parent bypasses
the proxy and requires the exact generation-2 state/outbox pair to exist.

This is a deterministic loopback connection-loss design. It does not claim a
kernel, router, cloud load balancer, PostgreSQL failover, or real network
partition result until executed.

### Child-process discrimination

The parent invokes only the ignored child entry point with:

```text
--exact blog_comments_schedule_audit_fault_child
--ignored
--nocapture
--test-threads=1
```

The child has a 30-second parent timeout and is killed on drop if it does not
finish.

For fail-stop scenarios, Unix evidence requires signal 6 rather than merely a
non-zero test status. A normal Rust panic or setup failure therefore does not
satisfy the expected outcome.

Non-Unix builds retain only the weaker abnormal-exit source expectation.

### Schedule fixture

The schedule fixture matches slice 84:

- generation 1 contains one currently active terminal key;
- generation 2 retains that key identity, secret, and activation;
- generation 2 adds a retirement covering propagation, maximum TTL, default
  clock skew, and margin;
- one successor activates two minutes in the future;
- runtime TTL and clock-skew policy do not change.

The parent passes all timestamps to the child so `ResumeExact` reconstructs the
same canonical digest.

### Worker-response disconnect boundary

Slice 85 does not add an internal worker-kill seam and does not duplicate the
slice-83 worker implementation.

The public wrapper already fail-stops when its response channel disconnects
after a submitted audited write, but deterministic worker-only termination
remains a separate gate. That gate requires either an explicitly reviewed
diagnostic seam or operating-system thread/process orchestration that does not
weaken production composition.

### Preserved behavior

Slice 85 does not change:

- authorization and delegated-principal admission;
- schedule lifecycle and replacement validation;
- canonical digest construction;
- state/outbox SQL or transaction ordering;
- commit reconciliation attempts or delays;
- process-local audit behavior;
- outbox schema or publication state;
- TCP Comments transport, listener lifecycle, channels, or replay;
- manifests, features, dependencies, or lockfile.

### Explicit non-claims

Slice 85 does not claim:

- that the Rust tests compiled or ran;
- that PostgreSQL, migrations, proxy, subprocesses, or signals executed;
- retained exact-pair, third-state, or retry-exhaustion runtime evidence;
- deterministic worker-response disconnect evidence;
- encrypted PostgreSQL proxy support;
- production suitability of the bounded proxy;
- real network partition, PostgreSQL failover, or crash-injection evidence;
- outbox publishing, leasing, retry, retention, or external delivery;
- durable audit completeness for denied or failed attempts;
- automatic recovery after fail-stop;
- shared or restart-safe replay protection;
- clock synchronization or distributed activation;
- workflow, CI, or production validation.

Status: `source_verified_no_compile`.
Compile policy: `not_run_by_request`.
Runtime status: `not_run`.

## Next implementation results

1. Execute slices 84 and 85 against an isolated PostgreSQL server and retain the
   exact revision, PostgreSQL version, commands, child exit statuses, signal
   observations, proxy trace summary, and cleanup result.
2. Add a narrowly reviewed worker-response-disconnect diagnostic seam or
   subprocess/thread orchestration and prove the outer audited wrapper aborts.
3. Add an outbox dispatcher contract with bounded claim leases, idempotent
   delivery identity, retry/backoff, and retention.
4. Define the operator recovery ceremony for fail-stop, corruption, lost state,
   and externally advanced state.
5. Add clock-health ownership and replace process-local replay admission before
   distributed activation claims.

## Suggested verification — intentionally not run

```bash
node scripts/verify/verify-blog-comments-tcp-delegation-schedule-audit-postgres-faults.mjs
cargo test -p rustok-server --features mod-blog --test blog_comments_schedule_audit_postgres_faults -- --ignored --nocapture
cargo check -p rustok-server --features mod-blog --locked
```

## Ownership retained

- Comments owns schedule lifecycle validation, keyring selection, signing,
  verification, request binding, and process-local replay admission.
- The server host owns authorization, audited trigger composition, PostgreSQL
  state/outbox transaction logic, and the fault harness.
- Blog owns persistence migrations and implementation evidence.
- Maintainers own PostgreSQL provisioning, executable fault evidence, and
  retained runtime artifacts.
