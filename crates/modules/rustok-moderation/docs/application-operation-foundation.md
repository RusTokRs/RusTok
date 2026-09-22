# Moderation decision application operation foundation

Status: **bounded source-ready slice / maintainer execution pending**

## Scope

This slice gives the Moderation owner one durable current operation per immutable typed decision. It establishes crash-safe intent, bounded due discovery, lease/CAS ownership and terminal evidence recording. Follow-up source now adds the bounded one-attempt dispatcher, shared runtime scheduling and atomic case/application audit lifecycle over the same owner primitives. Bounded operator recovery remains separate owner work.

It does **not** move domain enforcement into Moderation. Domain modules still apply their own state through `rustok-moderation-api` adapters, and the host-composed adapter registry remains the only cross-domain dispatch boundary.

## Atomic decision intent

`decide_case_replay_safe` executes inside the Moderation command receipt transaction. A successful decision commits the following together:

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

The later application-audit lifecycle requires no additional schema migration: it reuses existing `moderation_cases` lifecycle fields and the existing `moderation_events` owner audit ledger.

## Operation identity

Each operation is keyed by immutable `decision_id` and stores the owner facts needed to fence dispatch:

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
- `rejected` — a terminal neutral domain/contract failure was classified;
- `operator_review` — automated progress stopped on invariant/corrupt owner input and requires bounded operator recovery.

Terminal state is never inferred from a timeout, missing adapter or lease loss.

Operation state is coupled to Moderation case/audit state in the same owner transaction:

- first successful claim: case `decided -> applying_decision` plus start/attempt audit facts;
- retry/reclaim: case remains `applying_decision` without another case revision;
- retryable: case remains `applying_decision` plus retry audit;
- applied: case `applying_decision -> closed`, `closed_at`, active-key release and applied/closed audits;
- rejected/operator-review: case `applying_decision -> escalated` with matching application/escalated audits and active identity retained.

## Lease and retry boundary

`ModerationService` exposes bounded owner primitives:

- `get_application_operation`;
- `list_due_application_operations`;
- `claim_application_operation`;
- `mark_application_retryable`;
- `mark_application_rejected`;
- `mark_application_operator_review`;
- `mark_application_applied`;
- `dispatch_application_operation_once` as the bounded one-attempt orchestration primitive.

A claim uses a CAS update and creates a fresh UUID `lease_token`. It increments `attempt_count` and sets a bounded lease expiry. Due discovery and claim share the same predicate: pending/retryable rows whose `next_attempt_at` is due, plus applying rows whose lease expired. An expired lease is therefore reclaimable after worker crash.

The claim CAS now runs in the same owner transaction as the case/audit start boundary. An unrelated case lifecycle state causes the claim transaction to fail closed rather than leaving an operation lease detached from case state.

Every completion/error transition requires the exact unexpired lease token. A stale worker cannot complete an operation after another worker has reclaimed it, even if both use the same human-readable lease owner name. Operation, case and audit finalization then commit or roll back together.

The one-attempt dispatcher supplies deterministic bounded retry backoff after classifying the neutral adapter error: 5, 10, 20, 40, 80, 160 seconds and then a 300-second cap. Shared background scheduling is source-ready through the existing `rustok_runtime::ModuleWorkScheduler`; no Moderation-specific polling loop was added.

## One-attempt dispatch

`dispatch_application_operation_once` claims one due operation, reconstructs `ApplyModerationDecisionCommand` from immutable Moderation decision/effect/case facts, validates exact decision hash and reviewed subject identity, looks up only the matching `(subject_module, subject_kind)` adapter and invokes `apply_moderation_decision`.

The domain call uses service actor `rustok-moderation`, a 30-second deadline and the **decision UUID as the idempotency key**. Attempt identity lives only in the correlation ID/lease token. This preserves lost-response replay: a retry reaches the same domain owner receipt instead of creating a second domain mutation.

A missing exact adapter is retryable. A retryable `PortError` is scheduled using bounded backoff. A non-retryable `Conflict` or `InvariantViolation` becomes `operator_review`; other non-retryable neutral port errors become `rejected`. Deterministic corruption while rebuilding the immutable command also becomes `operator_review`. Owner database/audit failure after claim is returned to the caller and leaves the durable lease/reclaim path in control instead of forging a domain outcome.

See `docs/application-dispatch-once.md` for the exact source contract.

## Applied evidence

`mark_application_applied` accepts only a `ModerationDecisionApplication` matching the durable operation:

- same decision UUID;
- same subject module/kind/UUID;
- `application.subject.revision` equals the exact reviewed revision;
- `applied_revision` is not older than the reviewed revision.

Only after that validation and a matching live lease does the owner transaction move the operation to `applied`, persist `applied_revision` / `applied_at`, close the case, release the completed active-case identity and append matching owner audit facts.

## Internal owner audit boundary

The lifecycle follow-up reuses the existing `moderation_events` table; no new event/audit schema is added. Source-ready internal event types are `case_application_started`, `application_attempt_claimed`, `application_retry_scheduled`, `application_applied`, `application_rejected`, `application_operator_review`, `case_closed` and `case_escalated`.

These rows are Moderation internal audit facts. They are not silently promoted into a typed `rustok-events` cross-domain contract. Any future public semantic event family must be separately versioned and admitted.

## Explicitly not claimed

The combined foundation + dispatcher + shared scheduler + audit lifecycle still does not provide:

- operator retry/requeue/re-review commands/UI;
- automatic replacement or rewriting of immutable decisions after escalation;
- a typed public/cross-domain Moderation application event family;
- retained SQLite/PostgreSQL migration, lease race, lifecycle atomicity, crash/lost-response, scheduler or runtime evidence.

Missing/unavailable adapters remain retryable and never imply applied. Validation/stale/unsupported outcomes never become success.

## Ownership and Reactions boundary

`rustok-moderation` owns this application operation, case lifecycle and internal audit because they are cross-domain decision orchestration state. Domain owners do not copy them. Forum continues to own only its topic/reply lifecycle, dedicated moderation subject revision and domain receipt/effects.

This work is unrelated to Reactions. It adds no reaction catalog, actor state, aggregates, commands, transport or presentation code, and it does not alter the existing `rustok-reactions` / `rustok-reactions-storefront` ownership boundary.

## Maintainer verification handoff

Suggested checks, intentionally not run while preparing these source slices:

```bash
node scripts/verify/verify-moderation-application-operation.mjs
node scripts/verify/verify-moderation-application-dispatch-once.mjs
node scripts/verify/verify-moderation-application-work-scheduler.mjs
node scripts/verify/verify-moderation-application-audit-lifecycle.mjs
cargo check -p rustok-moderation --all-targets
cargo test -p rustok-moderation
cargo xtask module validate moderation
git diff --check
```

Retain clean/upgraded PostgreSQL and SQLite migration evidence, typed-effect-only backfill evidence, decision+effect+operation+receipt atomicity, duplicate command replay, due ordering/bounds, concurrent claim CAS, first-claim case start, retry/reclaim without duplicate case revision, lease expiry/reclaim, stale-token rejection, retry scheduling/audit atomicity, terminal operation/case/audit atomicity, case revision contention, command reconstruction, exact registry selection, retry/error classification, lost-response replay followed by exactly one case close and applied-evidence mismatch behavior before promotion.

No tests, Cargo commands, Node verifiers, formatting, migrations, database scenarios, workflows or CI were executed while preparing these source slices.
