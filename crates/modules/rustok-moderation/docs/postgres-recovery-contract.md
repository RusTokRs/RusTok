# Moderation PostgreSQL recovery contract evidence

Status: **source-ready / maintainer execution pending**

## Scope

`crates/rustok-moderation/tests/postgres_recovery_contract.rs` is an opt-in PostgreSQL integration target for the operator-recovery and legacy-terminal reconciliation invariants in the Moderation plan.

It uses the same isolated-schema pattern as the owner-contract PostgreSQL target and covers four database-backed scenarios:

1. a real pending application is claimed, rejected, explicitly requeued by a human operator, replayed through the same Moderation command receipt, rejects a changed request under that same receipt key, and becomes claimable again with `attempt_count = 2`;
2. a real applied application closes its case and remains impossible to requeue;
3. a truthful legacy pre-audit `rejected` application row with a still-`decided` case reconciles to `escalated`, then a new reconciliation request with the current case revision is a no-op;
4. a truthful legacy pre-audit `applied` row with stored applied revision/time reconciles to `closed`, sets present-time `closed_at`, and releases `active_deduplication_key`.

The harness runs the four production Moderation migrations in a unique temporary PostgreSQL schema. No Forum/domain table or adapter is present, so legacy reconciliation cannot accidentally depend on or invoke domain enforcement.

## Legacy fixture boundary

Normal pending/rejected/applied states in the first two scenarios are created only through public Moderation services (`claim_application_operation`, `mark_application_rejected`, and `mark_application_applied`).

The two legacy scenarios deliberately mutate only `moderation_application_operations` with SQL while leaving the freshly decided case unchanged. This recreates the exact upgrade condition that cannot be reached through the current atomic finalizers: a terminal application row whose case still reflects pre-audit lifecycle state. Reconciliation itself always runs through `operator_reconcile_legacy_application_replay_safe`.

The seeded legacy shapes remain constrained by the production table contract:

- terminal rows have no lease tuple;
- rejected rows have no applied evidence;
- applied rows have `applied_revision >= subject_revision` and a non-null `applied_at`.

## Maintainer commands

Intentionally not run while preparing this slice:

```bash
RUSTOK_MODERATION_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-moderation --test postgres_recovery_contract -- --nocapture

node scripts/verify/verify-moderation-postgres-recovery-contract.mjs
```

## What success proves

A passing PostgreSQL run demonstrates that the recovery source works against the production PostgreSQL constraints and transactional lifecycle rather than only against SQLite/source inspection:

- rejected/operator intervention can move the same immutable decision back to `retryable` without changing domain idempotency identity;
- the command receipt replays identical operator input and conflicts on changed input;
- the next owner claim advances only the application attempt count and does not duplicate the case revision already advanced by requeue;
- `applied` remains terminal and cannot re-enter the scheduler;
- legacy rejected/applied rows align only the Moderation case according to persisted terminal operation truth;
- already-aligned legacy reconciliation is a no-op;
- applied legacy reconciliation closes at reconciliation time and releases active-case deduplication identity.

This target does not claim adapter invocation, scheduler stop behavior, multi-host dispatcher convergence, or UI evidence; those remain separate retained-evidence slices.

No tests, Cargo commands, Node verifiers, formatting, migrations against a real database, workflows or CI were executed while preparing this file.
