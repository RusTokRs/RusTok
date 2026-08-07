# Moderation decision application operation foundation

Status: **bounded source-ready slice / maintainer execution pending**

## Scope

This slice gives the Moderation owner one durable current operation per immutable typed decision. It establishes crash-safe intent, bounded due discovery, lease/CAS ownership and terminal evidence recording before any background worker or domain-adapter dispatch is added.

It does **not** move domain enforcement into Moderation. Domain modules still apply their own state through `rustok-moderation-api` adapters, and the host-composed adapter registry remains the only cross-domain dispatch boundary.

## Atomic decision intent

`decide_case_replay_safe` already executes inside the Moderation command receipt transaction. A successful decision now commits the following together:

- the case transition to `decided` and its CAS revision;
- the immutable `moderation_decisions` row;
- the typed `moderation_decision_effects` row;
- one `moderation_application_operations` row in `pending` state;
- the `case_decided` owner event, including `application_status = pending`;
- the completed moderation command receipt.

If any insert/event/receipt step fails, the transaction rolls back and no durable decision is left without application intent.

## Upgrade/backfill

Migration `m20260807_000004_create_moderation_application_operations` creates the operation table after decision effects.

Upgrade backfill selects only decisions that already have a `moderation_decision_effects` row. Those typed decisions become `pending`, preserving their existing decision hash and reviewed subject identity. Historical decisions with `effect: None` are intentionally not backfilled and remain non-dispatchable until truthful re-review or migration supplies an explicit typed effect.

## Operation identity

Each operation is keyed by immutable `decision_id` and stores the owner facts needed to fence future dispatch:

- tenant and case identity;
- lowercase SHA-256 `decision_hash`;
- exact subject module/kind/UUID;
- exact reviewed subject revision.

The table has a tenant-composite foreign key to `(tenant_id, decision_id)` in `moderation_decisions`. `case_id` is an immutable audit/lookup snapshot; enqueue also checks it against the decision and case before insertion.

## Lifecycle

Source states are:

- `pending` — durable intent is ready when `next_attempt_at` is due;
- `applying` — one worker owns a bounded lease;
- `retryable` — a future retry is scheduled explicitly;
- `applied` — exact domain `ModerationDecisionApplication` evidence was accepted;
- `rejected` — a terminal domain/contract failure was classified by the future dispatcher;
- `operator_review` — automated progress stopped and requires bounded operator recovery.

Terminal state is never inferred from a timeout, missing adapter or lease loss.

## Lease and retry boundary

`ModerationService` now exposes bounded owner primitives:

- `get_application_operation`;
- `list_due_application_operations`;
- `claim_application_operation`;
- `mark_application_retryable`;
- `mark_application_rejected`;
- `mark_application_operator_review`;
- `mark_application_applied`.

A claim uses a CAS update and creates a fresh UUID `lease_token`. It increments `attempt_count` and sets a bounded lease expiry. Due discovery and claim share the same predicate: pending/retryable rows whose `next_attempt_at` is due, plus applying rows whose lease expired. An expired lease is therefore reclaimable after worker crash.

Every completion/error transition requires the exact unexpired lease token. A stale worker cannot complete an operation after another worker has reclaimed it, even if both use the same human-readable lease owner name.

No automatic backoff algorithm is embedded in this foundation. The future dispatcher supplies an explicit bounded retry delay after classifying the adapter error.

## Applied evidence

`mark_application_applied` accepts only a `ModerationDecisionApplication` matching the durable operation:

- same decision UUID;
- same subject module/kind/UUID;
- `application.subject.revision` equals the exact reviewed revision;
- `applied_revision` is not older than the reviewed revision.

Only after that validation and a matching live lease does the operation move to `applied` and persist `applied_revision` / `applied_at`.

The owner still needs the next dispatch slice to reconstruct `ApplyModerationDecisionCommand` from immutable decision/effect/case facts, invoke the host-materialized adapter, classify `PortError`, and handle lost responses by checking the durable operation and relying on the domain adapter receipt replay.

## Explicitly not claimed

This slice does not provide:

- a background application worker or scheduler loop;
- adapter lookup or `apply_moderation_decision` invocation;
- automatic retry/backoff/error classification;
- case lifecycle transitions from `decided` through applying/closed/escalated;
- application lifecycle outbox/audit events beyond the existing `case_decided` pending-intent marker;
- operator recovery commands/UI;
- cross-tenant worker enumeration;
- retained SQLite/PostgreSQL migration, lease race, crash/lost-response or runtime evidence.

Missing/unavailable adapters must remain retryable in the next slice; they must never be interpreted as applied. Validation/stale/unsupported outcomes must never be silently retried as success.

## Ownership and Reactions boundary

`rustok-moderation` owns this application operation because it is cross-domain decision orchestration state. Domain owners do not copy it. Forum continues to own only its topic/reply lifecycle, dedicated moderation subject revision and domain receipt/effects.

This work is unrelated to Reactions. It adds no reaction catalog, actor state, aggregates, commands, transport or presentation code, and it does not alter the existing `rustok-reactions` / `rustok-reactions-storefront` ownership boundary.

## Maintainer verification handoff

Suggested checks, intentionally not run while preparing this source slice:

```bash
node scripts/verify/verify-moderation-application-operation.mjs
cargo check -p rustok-moderation --all-targets
cargo test -p rustok-moderation
cargo xtask module validate moderation
git diff --check
```

Retain clean/upgraded PostgreSQL and SQLite migration evidence, typed-effect-only backfill evidence, decision+effect+operation+receipt atomicity, duplicate command replay, due ordering/bounds, concurrent claim CAS, lease expiry/reclaim, stale-token rejection, retry scheduling, terminal transitions and applied-evidence mismatch behavior before promotion.

No tests, Cargo commands, Node verifiers, formatting, migrations, database scenarios, workflows or CI were executed while preparing this slice.
