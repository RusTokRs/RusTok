# Index operations and recovery

## Storage cutover

The pending Index migrations define one storage shape: `index_entities.record_fingerprint`,
`index_jobs.failure_count` and `index_jobs.retry_epoch`, and `index_job_recovery_audits` for both
`rebuild` and `reconcile`. SQLite is a contract-test backend; its locale-scope table rebuild
copies both counters and the lease generation. The locale-scope rollback refuses to discard live
locale jobs and leaves the previous constraint and data intact. PostgreSQL is the production
backend.

**Amending a migration does not change databases that already recorded it as applied.** Validate
this branch on a fresh disposable database. Do not reset a database containing valuable state,
edit its migration history, or deploy the amended bodies over any independently published
migration set. If either case applies, preserve the owner data and audit history and plan a
forward migration or explicit export/rebuild before deployment. Do not synthesize revision
fingerprints from a locally repaired edge set: rebuild from the owner record with its exact
revision.

## Replay and reconciliation

- A job's `attempt_count` is its lease generation. It increases on acquisition, including
  reclaim. A page-budget yield or a host interruption does not consume the retry budget.
- Retryable failures increment `failure_count` and persist a bounded backoff in `available_at`.
  A permanent failure or exhausted budget leaves the original job in `failed`. Diagnostic
  details retain the dependency code and classification, not raw owner or driver errors.
- The worker only runs due work for selected sources with the active tenant schema contract.
  Discovery is not acquisition. Dispatch is fenced to the discovered job, generation and
  retry epoch; a malformed due row is deferred, not acknowledged as completed or replaced.
- A page is driven for at most 90 seconds while the runner can renew its lease. A shorter
  dependency timeout keeps its own code. Mutation and checkpoint commits validate the exact
  active lease and cancellation flag on the write path. PostgreSQL locks the exact job row
  before checking wall-clock expiry; a heartbeat that finishes waiting after expiry cannot
  revive that generation. Already committed mutations are not
  rolled back when a later mutation or checkpoint fails. The checkpoint advances only after
  all mutations finish. A network timeout by itself says nothing about commit.
- A completed replay requires the original job and its durable terminal checkpoint for the
  exact source, schema and locale. Reconciliation requires the requested number of completed
  passes and no continuation. Terminal acknowledgments require the same retry epoch and the
  generation acquired by the dispatch (or the generation already completed at admission).
  The scheduler only checks the durable result; it does not set success.
- An equal source revision is a duplicate only if the original normalized record and schema
  fingerprints match. Different content at the same revision is a source contract violation.
  Local orphan-edge repair does not change the authoritative record fingerprint.

After resolving a failure, select the exact job kind in **Retry / Requeue Failed Job** in
admin, or use `requeueIndexReplay` / `requeueIndexReconciliation` with `{jobId, reason}`. The operation checks
the authenticated tenant, actor, effective `modules:manage` permission, selected source and current persisted scope.
It requeues the existing ID, preserves the request and lease generation, resets the failure
budget, advances the retry epoch and appends one actor/reason audit entry in the same transaction.
Replay retains its last checkpoint. Reconciliation restarts the configured passes. `NotFound`,
`NotFailed` and `ScopeUnavailable` are not successful requeues. The Cancel control targets
replay only. Both run transports honor the host's graceful stop at durable safe points.

The snapshot requires `settings:read`. Native browser requests use `AuthorizedBrowserClient`
with bearer and tenant headers. Standalone SSR relays to the admin host's configured API
origin; the backend validates the bearer and tenant. Relay requests do not rotate the
standalone host's SSR refresh cookie. Trunk CSR sends commands through `/api/graphql`, not
the native command endpoints. `/api/fn/index/bootstrap` is still needed for the combined
snapshot (see the [internal bootstrap exception](./implementation-plan.md#ffafba-status)).
Replay runs within the request. The server's HTTP timeout (30 seconds by default) can end
observation before the runner's page deadline. A timeout does not prove cancellation or that
no writes committed. Refresh the same tenant's status before retrying; do not fall back to
another transport.

## PostgreSQL secondary indexes

`maintainIndexSecondaryIndex` runs `Ensure`, `Reindex` or `Retire` for a selected schema field
under the same tenant/operator permission boundary. No DDL runs during host startup. Use
`Ensure` for a filterable/sortable scalar field or a containment-compatible list. Decimal lists
use numeric equality, not string containment; they do not get a containment GIN.

The compiler and DDL share validated schema/field expressions and partial-index predicates.
Predicate literals are validated tenant UUIDs, schema identifiers, versions and fingerprints;
query data and page limits stay bound. String ordering and canonical UTC timestamp keys use `C`
collation. Timestamp keys retain nanosecond precision and avoid a volatile text-to-timestamp
cast. Plan selection is still PostgreSQL's decision; see the custom/generic `EXPLAIN` regression
in the equivalence harness rather than assuming DDL alone is evidence of an index scan.

Only one cooperating worker may execute DDL for a physical name. An interrupted `Ensure` is
recovered by reissuing the **same** request for the same schema field. The original job reserves
an absent name before `CREATE INDEX CONCURRENTLY`. Its recorded target table OID and definition
hash allow recovery in the gap between CREATE and the owner comment, including an invalid
matching index left by a failed CREATE. Recovery also checks the actual physical definition.
Unmarked objects without the original reservation, comments for different definitions, and
objects on a different table are not adopted or removed. An ownership conflict requires an
operator investigation; do not drop the object to make the request pass. The lock does not
coordinate arbitrary external DDL. PostgreSQL's interrupted concurrent REINDEX can also leave
invalid auxiliary indexes; the operator does not adopt or drop them by suffix.

The operation runs inside the request, not in a background worker. The default server HTTP
budget (30 seconds) can end observation before the 90-second lease expires. For planned DDL,
check `RUSTOK_HTTP_REQUEST_TIMEOUT_SECONDS` and the deployment's reverse-proxy deadline;
keep the lease budget in mind and do not assume a longer HTTP timeout extends it. Wait for
lease expiry (or the deferred `available_at`) before retrying the same request. Large
operations that cannot finish within the configured budgets are not supported by this route.

## Query budgets and repair boundaries

The PostgreSQL port bounds bind count/size, per-value and nested projection sizes, and total
streamed page size; overflow returns an error rather than a partial success. It runs readiness
checks and the page with a two-second statement timeout, and exact count with a one-second
statement timeout, in one read-only repeatable-read snapshot. These are statement budgets,
not a guarantee about total request time, memory used in all PostgreSQL operators, or scan
and sort work. Linked-target checks apply to referenced first hops when selected by the owner
and use the current target schema fingerprint and owner admission in both page and count.

Orphan-edge removal requires an owner-provided `PostgresIndexOrphanLinkAuthorityFence` in the
same PostgreSQL transaction as the Index write. Two evidence reads do not serialize owner
writes. The fence must prove the exact source edge and target absence and hold until commit.
Acquire owner locks before canonical Index entity locks; cooperating owners must agree on
lock ordering. Owners must guarantee permanent absence or advance affected source revisions
on resurrection. Redelivery at the old revision is a duplicate, not edge restoration. If a
command committed before receipt finalization, a matching applied inbox delivery permits
read-only receipt recovery. If the original authority or materialization has since advanced,
retain the owner's receipt but do not claim present convergence or delete the restored edge.
No source owner fence is selected in the production host by this change. No automatic repair
is enabled. Inventory delivery acknowledgment and Product revision/shadow behavior remain
owner contracts and are not established by Index tests.

## Maintainer validation

[Index Contract CI](../../../../.github/workflows/index-contract-ci.yml) configures Rust 1.96 and
an isolated PostgreSQL 16 service for Index crate, repair and recovery tests. It also runs the
non-embedded operator transport fixtures and checks the admin host's SSR, hydrate and CSR profiles.
The hydrate and CSR jobs check the Index admin crate before composing the entire host. For failed
Cargo checks or event digest comparisons, see the `Index CI diagnostics` annotation and the full
job log. These checks do not replace retained deployment evidence or an authenticated browser run.

Run locally with the repository toolchain and a disposable PostgreSQL database. Without a
PostgreSQL URL the real-engine tests skip. Keep the tests serial on a shared test database:
the test schemas are separate, but PostgreSQL advisory lock keys are database-wide.

```sh
RUSTOK_INDEX_TEST_DATABASE_URL=postgres://... cargo test --locked -p rustok-index --lib -- --test-threads=1
RUSTOK_INDEX_TEST_DATABASE_URL=postgres://... cargo test --locked -p rustok-index \
  --test drift_repair_concrete_execution_postgres_test \
  --test drift_repair_recovery_postgres_test \
  --test source_reconciliation_dead_letter_admission_postgres_test \
  -- --test-threads=1
cargo test --locked -p rustok-index-admin --no-default-features --features ssr
cargo check --locked -p rustok-admin --bin rustok-admin --no-default-features --features ssr
for profile in hydrate csr; do
  cargo check --locked -p rustok-index-admin --lib --no-default-features --features "$profile" --target wasm32-unknown-unknown
  cargo check --locked -p rustok-admin --lib --no-default-features --features "$profile" --target wasm32-unknown-unknown
done
cargo test --locked -p rustok-server --no-default-features --features mod-product --lib index_native_json_routes_enforce_request_scope_without_embedded_admin
cargo test --locked -p rustok-server --no-default-features --features mod-product --lib index_standalone_relay_preserves_scope_and_exact_recovery_kind
cargo test --locked -p rustok-server --no-default-features --features mod-product --lib index_admin_csr_commands_validate_against_the_operator_schema
```

Repeat the server fixtures with embedded-admin assets and features before validating the embedded
profile. Exercise real authenticated bootstrap, trigger, cancel and requeue in both host modes and
via Trunk CSR against different tenants and read-only versus operator roles. Verify cross-tab tenant
changes, auth restoration, late responses and graceful stop with an in-flight replay. The loopback
relay and in-process schema fixtures do not verify the production auth stack or browser hydration.
Compare the retained M4 snapshots with compiler output and verify PostgreSQL ordering, planner
choice and projection budgets before claiming parity.
