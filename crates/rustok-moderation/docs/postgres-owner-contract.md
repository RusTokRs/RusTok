# Moderation PostgreSQL owner contract evidence

Status: **source-ready / maintainer execution pending**

## Scope

`crates/rustok-moderation/tests/postgres_owner_contract.rs` is an opt-in PostgreSQL integration target for the owner invariants that were previously covered only by SQLite/source guards.

It exercises the production Moderation migrations in a unique temporary PostgreSQL schema and covers three plan-level contracts:

1. **active-case convergence** — two concurrent `open_case_replay_safe` calls for the same tenant/scope/subject revision/queue/policy but different reports converge on one active case through the PostgreSQL active deduplication constraint; both reports end attached to that case;
2. **typed decision/effect/application atomicity** — `decide_case_replay_safe` persists the immutable decision, typed `moderation_decision_effects` row and one `pending` `moderation_application_operations` row with matching decision hash/case/reviewed revision;
3. **revision CAS contention** — two concurrent assignment commands with the same expected revision yield exactly one winner and one `ModerationError::RevisionConflict`, and storage advances by exactly one revision to the winning moderator.

The harness does not create domain tables and does not introduce cross-domain foreign keys. It runs the four real `rustok-moderation` migrations directly in its isolated schema.

## Database isolation

The target reads `RUSTOK_MODERATION_TEST_DATABASE_URL`, falling back to `DATABASE_URL` only when that value is a PostgreSQL URL. If no PostgreSQL URL is available, the test exits successfully after printing a skip message.

For each invocation it creates a schema named `rustok_moderation_contract_<uuid>`, scopes every test connection with `SET search_path`, runs Moderation migrations there, and drops the schema with `CASCADE` during cleanup. This keeps the evidence isolated from application tables and from parallel test invocations.

## Maintainer commands

Intentionally not run while preparing this source slice:

```bash
RUSTOK_MODERATION_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-moderation --test postgres_owner_contract -- --nocapture

node scripts/verify/verify-moderation-postgres-owner-contract.mjs
```

Running the broader owner suite afterwards is also useful:

```bash
cargo test -p rustok-moderation
```

## What success proves

A passing PostgreSQL run proves the database-specific behavior used by the owner source rather than only Rust-level response shapes:

- `ON CONFLICT DO NOTHING` active-case admission leaves one active case under concurrent opens;
- both report links and report states converge on that case;
- typed effect schema/version/payload is present for the decision;
- the pending application row is created with the same decision hash, case and exact reviewed revision and has zero attempts;
- assignment compare-and-set does not allow two writers to win the same case revision.

It deliberately does not claim dispatcher lease/reclaim, operator recovery, legacy reconciliation or domain-adapter evidence; those remain covered by their dedicated slices/handoffs.

No tests, Cargo commands, Node verifiers, formatting, migrations against a real database, workflows or CI were executed while preparing this file.
