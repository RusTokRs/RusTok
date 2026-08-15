# M6 concrete repair PostgreSQL execution harness

Status: `source_ready_owner_execution_pending`.

## Purpose

This slice adds an environment-gated PostgreSQL integration packet for the complete concrete Index
repair boundary introduced across the missing-entity, prepared-command recovery, and orphan-link
slices.

The packet is executable source, not admitted production evidence. It retains the exact scenarios and
assertions the repository owner must run before a public command surface, automatic iterator, or
time-derived ownership policy can be considered.

The locked retained-evidence contract, clean-commit capture runner, and admission verifier are
documented separately in
[`m6-repair-retained-evidence-admission.md`](./m6-repair-retained-evidence-admission.md).

## Executable targets

The harness lives in:

```text
crates/rustok-index/tests/drift_repair_postgres_environment_test.rs
crates/rustok-index/tests/drift_repair_recovery_postgres_test.rs
crates/rustok-index/tests/drift_repair_concrete_execution_postgres_test.rs
crates/rustok-index/tests/support/drift_repair.rs
```

All targets read `RUSTOK_INDEX_TEST_DATABASE_URL`, with PostgreSQL `DATABASE_URL` as a fallback. When
no PostgreSQL URL is available, each target reports a skip and succeeds. The retained-evidence runner
rejects that skip state and therefore cannot promote an environment-less run.

Every test creates a unique PostgreSQL schema, creates the tenant-owner fixture table, applies the
real `IndexModule` migrations, uses production schema registration, mutation, finding, repair, and
recovery stores, and drops the isolated schema after successful assertions.

The shared fixture intentionally creates independent one-connection pools for the recovery-aware
store, base repair store, recovery-aware owner, concrete owner, and evidence reader. This preserves
the real nested transaction shape without allowing an outer command fence to starve an inner owner
transaction on a single pooled connection.

## Environment metadata target

`drift_repair_postgres_environment_test` queries PostgreSQL through the same isolated-schema fixture
and emits exactly two bounded capture markers:

- `postgres_server_version`;
- `postgres_server_version_num`.

It does not emit the connection URL, host, database name, username, or password. URL source and a
bounded URL class are derived only by the capture runner and are retained without credentials.

## Migration and recovery target

`drift_repair_recovery_postgres_test` covers:

- concurrent commands for one finding, requiring exactly one durable reservation and one
  `FindingBusy` result;
- immutable revision `0` activation for the winning reservation;
- command UUID payload reuse rejection;
- authorized pause and exact decision replay;
- stale recovery revision rejection;
- database-trigger rejection of `prepared -> completed` while paused;
- authorized resume followed by one valid completion;
- immutable completed command identity;
- reverse execution of all real Index migrations and removal of both repair tables.

The test stops immediately after reservation through an injected evidence failure. It does not use a
parallel test-only reservation implementation.

## Concrete crash and retry target

`drift_repair_concrete_execution_postgres_test` covers both concrete owners through the production
repair service contract.

### Missing entity

The target:

1. seeds one live entity through `PostgresMutationStore`;
2. publishes an exact retained absence watermark;
3. records the exact confirmed-missing finding commitment;
4. lets the production delete owner commit its command-bound inbox mutation;
5. injects a retryable failure after owner commit and before repair receipt completion;
6. requires the tombstone and applied delivery to survive while the repair command remains
   `prepared`;
7. retries the exact command UUID and requires inbox duplicate convergence plus one repaired receipt;
8. requires a later exact retry to return `AlreadyCompleted`.

### Orphan link

The target seeds one source entity with two ordered links, repairs ordinal `0`, and requires:

- source entity version and live state remain unchanged;
- only the exact committed edge is removed;
- the unrelated ordinal `1` edge remains at ordinal `1`;
- the command-bound orphan delivery is `applied` in the same committed side effect;
- an injected post-owner failure leaves the repair command `prepared`;
- exact retry accepts the absent edge only with the matching applied inbox proof;
- one repaired receipt is completed and later exact replay is terminally idempotent.

## Recovery race target

Two deterministic barriers retain the two recovery race windows:

- **pause before owner admission**: admitted before-evidence is held, pause wins, the recovery-aware
  owner rejects execution, no mutation delivery exists, authorized resume then permits exact repair;
- **abandon after side effect but before completion**: the owner mutation and admitted after-evidence
  exist, abandon wins before receipt persistence, both application completion and the database trigger
  fail closed, and no repaired receipt is inferred from the side effect.

The barriers are test adapters around the production evidence readers. They do not alter the repair
store, recovery store, concrete owner, or inbox implementation.

## Changed commitment and concurrency target

The orphan target additionally requires fail-closed behavior for:

- source-version movement;
- exact link substitution at the committed ordinal;
- authoritative target restoration;
- target absence-version movement;
- a normal full source mutation committed after admitted before-evidence but before owner admission.

The first four cases must complete as `NotRepaired(before_not_repairable)` without creating an orphan
mutation delivery. The normal mutation case must preserve the newer source entity and link graph,
leave the repair command prepared, and reject the stale exact-edge owner call.

## Retained admission boundary

The canonical clean-commit runner is `capture-index-repair-postgres.mjs` at
`scripts/evidence/capture-index-repair-postgres.mjs`. The source-ready packet is locked by:

```text
crates/rustok-index/contracts/evidence/concrete-repair-postgres-execution-contract.json
scripts/evidence/capture-index-repair-postgres.mjs
scripts/verify/verify-index-repair-retained-evidence.mjs
```

The runner requires a clean commit, executes the metadata target followed by both scenario targets,
rejects skips and non-zero results, retains current source hashes, and writes credential-redacted
complete stdout/stderr plus one final pass packet. Until the runner is executed, all three retained
output files must remain absent and the verifier reports execution pending.

## Deliberate limits

This packet does not add or claim:

- a public GraphQL, HTTP, CLI, MCP, or native-admin repair command;
- automatic finding iteration, scanning, scheduling, or repair loops;
- time-derived lease expiry, ownership inference, or automatic takeover;
- lifecycle auto-resolution after repair;
- production execution results, timings, database version metadata, or CI artifacts;
- admission of the scenarios until the repository owner runs and retains them.

## Owner verification

The admitted path is:

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  node scripts/evidence/capture-index-repair-postgres.mjs

node scripts/verify/verify-index-repair-retained-evidence.mjs
node scripts/verify/verify-index-repair-execution-postgres-harness.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

The two scenario targets may still be run directly for diagnosis, but only the capture runner writes
the bounded retained packet and complete redacted logs.

Tests, Node verifiers, formatting, Cargo checks, migrations, PostgreSQL scenarios, workflows, and CI
were not executed by the implementation agent, per maintainer instruction.
