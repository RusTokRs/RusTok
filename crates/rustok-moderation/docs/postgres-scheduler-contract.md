# Moderation PostgreSQL scheduler runtime contract

Status: **source-ready / maintainer execution pending**

## Scope

`crates/rustok-moderation/tests/postgres_scheduler_contract.rs` is an opt-in PostgreSQL integration target for the shared `ModuleWorkScheduler` boundary used by Moderation decision application.

Unlike direct dispatcher tests, this target registers Moderation through the same public composition path used by the host:

1. `ModerationModule::register_runtime_extensions` publishes `ModuleWorkRegistrations`;
2. a `HostRuntimeContext` carries the materialized `ModerationSubjectAdapterRegistry`;
3. `ModuleWorkRegistrations::register_all` installs the Moderation source/handler into a real `ModuleWorkScheduler`;
4. scheduler `run_once` / `run_until_stopped` drive the private Moderation work adapter, whose authoritative durable claim remains the existing application-operation CAS.

The test runs the production Moderation migrations in an isolated PostgreSQL schema and covers three runtime contracts.

## Multi-host convergence

Two independently constructed `ModuleWorkScheduler` instances share the same PostgreSQL queue and adapter registry, reproducing two runtime hosts discovering the same due decision.

Depending on scheduling, one or both generic scheduler envelopes may discover the read-only candidate, but exactly one authoritative Moderation claim may invoke the domain adapter. The retained assertions require:

- exactly one adapter call;
- application terminal state `applied` with `attempt_count = 1`;
- case terminal state `closed`;
- exactly one `application_attempt_claimed` event;
- exactly one `case_application_started` event;
- exactly one `case_closed` event.

This demonstrates that read-only generic discovery does not become an alternative lease authority.

## Graceful stop

The scheduler receives a deployment stop signal already set to `true` while a due Moderation application remains pending. `run_until_stopped` must return without starting new work.

The target verifies the adapter was never called, operation remains `pending` with zero attempts and no lease, and the case remains exactly `decided` at its previous revision.

This covers the runtime promise that stop prevents future claims rather than cancelling or rewriting durable owner state.

## Crash / lease recovery

The test uses the real `ModerationService::claim_application_operation` once to model a host that acquired the authoritative owner lease and then crashed before reaching the domain adapter. That first claim advances the case from `decided` to `applying_decision` and records the one legitimate start transition.

Only the clock is manipulated directly: PostgreSQL `lease_expires_at` is moved into the past. A newly composed scheduler must discover the expired `applying` row, reclaim it through the normal dispatcher, invoke the adapter once and finish the application.

The retained assertions require:

- final operation `applied` with `attempt_count = 2`;
- final case `closed`;
- exactly two application-attempt claim events;
- still exactly one `case_application_started` event;
- exactly one `case_closed` event;
- final case revision equals the first-claim revision plus only the canonical close revision, proving reclaim itself did not fabricate a second `applying_decision` transition.

## Adapter boundary

The test adapter is intentionally neutral and side-effect-free: it returns valid `NoDomainMutation` evidence with `applied_revision == reviewed revision`, which is allowed by the owner evidence contract. It exists only to count scheduler-driven domain-port invocations. Real Forum producer receipt replay is covered separately by `forum-lost-response-postgres-contract.md`.

## Database isolation and maintainer commands

The target reads `RUSTOK_MODERATION_TEST_DATABASE_URL`, falling back to a PostgreSQL `DATABASE_URL`. Without PostgreSQL it exits successfully with a skip message. Every run creates `rustok_moderation_scheduler_<uuid>`, applies the real Moderation migrations under that search path and drops the schema with `CASCADE` afterwards.

Intentionally not run while preparing this slice:

```bash
RUSTOK_MODERATION_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-moderation --test postgres_scheduler_contract -- --nocapture

node scripts/verify/verify-moderation-postgres-scheduler-contract.mjs
```

No tests, Cargo commands, Node verifiers, formatting, real database migrations, workflows or CI were executed while preparing this file.
