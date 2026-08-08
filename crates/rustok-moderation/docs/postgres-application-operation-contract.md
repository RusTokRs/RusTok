# Moderation PostgreSQL application-operation contract evidence

Status: **source-ready / maintainer execution pending**

## Scope

`crates/rustok-moderation/tests/postgres_application_operation_contract.rs` is an opt-in PostgreSQL integration target for the durable application-operation queue/lease invariants in the Moderation implementation plan.

It runs the production Moderation migrations in a unique temporary PostgreSQL schema and covers four database-backed contracts:

1. **bounded ordered due reads** — pending operations are returned by `next_attempt_at` order, future rows stay hidden, a zero requested limit is bounded to one, and the exported production maximum remains 100;
2. **concurrent claim convergence** — two independent PostgreSQL connections race to claim the same pending immutable decision and exactly one wins the owner CAS; storage records one application-attempt event and one first-claim case transition;
3. **expired lease reclaim + stale-token fence** — an expired applying lease becomes due, a new claimant receives a fresh lease token and increments only `attempt_count`, the case remains `applying_decision` without another revision increment, and the old worker cannot complete the reclaimed attempt;
4. **retryable deadline visibility** — a live claimant may schedule a bounded retry, the operation stays absent from due reads before `next_attempt_at`, and becomes visible again once the stored deadline is in the past.

The harness uses direct SQL only as a clock fixture to place `next_attempt_at` or `lease_expires_at` on either side of PostgreSQL `NOW()`. Admission, claim, reclaim, retry scheduling, case transitions, receipts and audit writes all use the real Moderation owner services.

## Database isolation

The target reads `RUSTOK_MODERATION_TEST_DATABASE_URL`, falling back to `DATABASE_URL` only when it is a PostgreSQL URL. Without PostgreSQL it exits successfully with a skip message.

Each invocation creates `rustok_moderation_application_<uuid>`, applies the four real Moderation migrations under that `search_path`, uses separate single-connection SeaORM pools for claim races, and drops the schema with `CASCADE` during cleanup.

## Maintainer commands

Intentionally not run while preparing this slice:

```bash
RUSTOK_MODERATION_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-moderation --test postgres_application_operation_contract -- --nocapture

node scripts/verify/verify-moderation-postgres-application-operation-contract.mjs
```

A broader follow-up is also useful after the focused evidence passes:

```bash
cargo test -p rustok-moderation
```

## What success proves

A passing PostgreSQL run demonstrates the database-specific application queue semantics rather than only Rust/source shape:

- due reads are tenant-scoped, time-gated and deterministically ordered;
- two hosts cannot both acquire the same live attempt;
- first claim advances `decided -> applying_decision` exactly once;
- reclaim after lease expiry creates a new UUID lease and increments the attempt counter without pretending a second case transition occurred;
- an old lease token cannot record retry/reject/apply after another worker reclaimed the attempt;
- retry scheduling clears the lease tuple and does not become due before the stored deadline;
- the canonical Moderation audit ledger records one first-claim lifecycle fact and one winning attempt fact for the concurrent claim.

This target deliberately does not claim adapter invocation, dispatcher error classification, lost-response domain replay, shared scheduler stop behavior, or multi-host domain-call convergence. Those are the next retained runtime/dispatcher evidence slices.

No tests, Cargo commands, Node verifiers, formatting, migrations against a real database, workflows or CI were executed while preparing this file.
