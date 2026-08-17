# M6 concrete repair retained evidence admission

Status: `source_complete_owner_execution_pending`.

## Purpose

This slice locks the maintainer-run admission boundary for the source-ready concrete repair
PostgreSQL harness. It does not execute the database scenarios and does not claim retained runtime
evidence.

The admission tooling makes one successful owner run reproducible and reviewable by binding it to:

- one clean Git commit;
- the exact environment, recovery, and concrete execution commands;
- the current hashes of every retained test, production adapter, and repair migration source;
- bounded PostgreSQL server and toolchain metadata;
- all required test case names and their asserted behavior;
- complete retained stdout and stderr after credential-bearing PostgreSQL URL redaction;
- one terminal `pass` packet written only after every command succeeds.

## Locked contract

The immutable source contract is:

```text
crates/rustok-index/contracts/evidence/concrete-repair-postgres-execution-contract.json
```

It allowlists three commands:

1. `drift_repair_postgres_environment_test` for PostgreSQL server metadata;
2. `drift_repair_recovery_postgres_test` for migration, reservation, recovery, and trigger behavior;
3. `drift_repair_concrete_execution_postgres_test` for concrete crash, retry, recovery-race,
   commitment-change, and ordinary-mutation serialization behavior.

The environment target does not emit the connection URL, host, database name, username, or password.

The contract remains `runtime_execution_pending`. A missing runtime packet and missing logs are the
expected repository state before the owner executes the capture runner.

## Capture runner

The only admitted capture path is:

```text
scripts/evidence/capture-index-repair-postgres.mjs
```

The runner requires `RUSTOK_INDEX_TEST_DATABASE_URL`, with PostgreSQL `DATABASE_URL` as a fallback.
It rejects non-PostgreSQL URLs, a dirty working tree, a changing HEAD, source drift during execution,
skipped database targets, missing test-case success lines, duplicate or missing PostgreSQL metadata,
and any non-zero command result.

The runner never persists the database URL, username, password, or connection string. It retains only
the environment variable name used and a bounded URL class such as `loopback`, `private_ipv4`,
`dns_name`, or `unix_socket`.

Before writing logs it replaces the exact database URL, any PostgreSQL URL-shaped token, and password
assignments. It then rejects the retained text if a PostgreSQL URL or unredacted password assignment
remains.

## Retained outputs

A successful run writes one atomic logical set:

```text
crates/rustok-index/contracts/evidence/concrete-repair-postgres-execution.json
crates/rustok-index/contracts/evidence/concrete-repair-postgres-execution.stdout.log
crates/rustok-index/contracts/evidence/concrete-repair-postgres-execution.stderr.log
```

The JSON packet retains:

- exact commit SHA and clean-tree/source-stability facts;
- start and completion timestamps;
- PostgreSQL server version and numeric version;
- Cargo and Rust compiler versions;
- the exact command allowlist and zero exit status for every command;
- current SHA-256 hashes for every source file named by the contract;
- SHA-256 and byte counts for both complete redacted logs;
- all four required test cases with the locked assertion list;
- `status = postgres_runtime_executed` and `final_status = pass`.

The runtime packet is written last. A partial set is rejected by the verifier.

## Admission verifier

The verifier is:

```text
scripts/verify/verify-index-repair-retained-evidence.mjs
```

Before owner execution it verifies the locked contract, runner boundaries, environment metadata test,
and required scenario test functions, then reports execution as pending.

After owner execution it additionally requires:

- the packet and both logs to exist together;
- exact packet fields and bounded metadata;
- current source hashes matching the retained run;
- all allowlisted commands with status `0`;
- all required cases with result `pass`;
- retained log hashes and byte counts matching the files;
- no PostgreSQL URL or unredacted password assignment in any retained output.

## Owner execution

Run from the exact clean commit intended for admission:

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  node scripts/evidence/capture-index-repair-postgres.mjs

node scripts/verify/verify-index-repair-retained-evidence.mjs
node scripts/verify/verify-index-repair-execution-postgres-harness.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

After a successful capture, review the redacted logs and packet before committing them. Do not hand edit the runtime packet or either retained log.

## Deliberate limits

This slice does not add or claim:

- execution of the PostgreSQL targets;
- a retained runtime packet, logs, timings, or database result;
- public GraphQL, HTTP, CLI, MCP, or native-admin repair transport;
- automatic finding iteration, scheduling, repair loops, or lifecycle auto-resolution;
- time-derived lease expiry or automatic owner inference;
- CI execution or workflow admission.

Tests, Node verifiers, formatting, Cargo checks, migrations, PostgreSQL scenarios, workflows, and CI
were not executed by the implementation agent, per maintainer instruction.
