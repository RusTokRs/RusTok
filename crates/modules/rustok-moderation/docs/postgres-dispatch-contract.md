# Moderation PostgreSQL dispatcher contract evidence

Status: **source-ready / maintainer execution pending**

## Scope

`crates/rustok-moderation/tests/postgres_dispatch_contract.rs` is an opt-in PostgreSQL integration target for the one-attempt Moderation dispatcher boundary.

The test uses real Moderation migrations, cases, immutable typed decisions, application operations, leases, finalizers and owner audit storage. A small neutral `ModerationSubjectCommandPort` test double is used only at the domain-port boundary so the harness can deterministically observe routing/context and return controlled outcomes without inventing domain tables.

It covers four contracts:

1. **multi-host CAS + exact adapter routing** — two independent PostgreSQL-backed `dispatch_application_operation_once` calls race the same decision; exactly one claims it and calls the exact `forum/forum_post` adapter once, while a registered `forum/forum_topic` adapter is never used as fallback;
2. **missing exact adapter is retryable** — a registry containing only the wrong subject kind cannot receive the command; the operation becomes `retryable` with `moderation.application_adapter_missing` and the case remains `applying_decision`;
3. **retry identity across attempts** — a controlled retryable adapter failure followed by success receives the same immutable decision UUID as `PortContext.idempotency_key` and causation ID on both attempts, while the lease-derived correlation IDs differ; command reconstruction is identical across attempts;
4. **fail-closed outcome classification** — non-retryable `Conflict` becomes `operator_review`, ordinary validation failure becomes `rejected`, and a nominally successful adapter response with mismatched subject evidence becomes `operator_review` under `moderation.application_evidence_invalid`.

## What this does and does not prove

The PostgreSQL portion proves the Moderation owner/dispatcher side: CAS claim convergence, immutable command reconstruction, exact registry lookup, service actor/deadline/idempotency context, finalizer classification, case lifecycle and owner audit writes.

The adapter double is **not** a replacement for a domain receipt implementation. The retry scenario proves that Moderation sends the same decision UUID idempotency identity on every attempt, but it does not claim that Forum or another producer has persisted/replayed that receipt. A real Forum lost-response integration harness remains separate follow-up evidence.

## Database isolation

The target reads `RUSTOK_MODERATION_TEST_DATABASE_URL`, falling back to a PostgreSQL `DATABASE_URL`. Without PostgreSQL it exits successfully with a skip message.

Each invocation creates `rustok_moderation_dispatch_<uuid>`, runs the four production Moderation migrations under that `search_path`, uses separate single-connection pools for the multi-host race, and drops the schema with `CASCADE` during cleanup.

Direct SQL is used only once to move a retryable operation's `next_attempt_at` into the past before its second dispatcher attempt. All durable state transitions remain owner-service calls.

## Maintainer commands

Intentionally not run while preparing this slice:

```bash
RUSTOK_MODERATION_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-moderation --test postgres_dispatch_contract -- --nocapture

node scripts/verify/verify-moderation-postgres-dispatch-contract.mjs
```

## What success proves

A passing run demonstrates that:

- authoritative PostgreSQL claim CAS prevents duplicate domain-port invocation across two hosts;
- adapter selection is exact on `(subject_module, subject_kind)` and has no kind fallback;
- reconstructed command decision ID, exact subject and immutable decision hash match owner truth;
- the domain-port actor is the Moderation service, deadline is bounded, and idempotency/causation identity is the immutable decision UUID rather than the attempt lease;
- retryable failures remain retryable, conflict-like failures escalate to operator review, deterministic validation failures reject, and invalid success evidence cannot be recorded as applied;
- successful dispatch closes the case and records one winning application attempt/close lifecycle.

This target does not yet prove a real producer receipt replay after a lost response, shared scheduler graceful stop/crash recovery, or a real Forum adapter under concurrent dispatcher hosts.

No tests, Cargo commands, Node verifiers, formatting, migrations against a real database, workflows or CI were executed while preparing this file.
