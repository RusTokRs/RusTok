# Moderation one-attempt decision application dispatcher

Status: **bounded source-ready slice / maintainer execution pending**

## Scope

This slice adds the first executable owner-side dispatch primitive over the durable `moderation_application_operations` foundation.

`ModerationService::dispatch_application_operation_once` handles **at most one** exact tenant/decision operation. It does not start a background loop, enumerate tenants, own host scheduling, expose operator UI or directly implement case/audit persistence. The owner primitives it calls now atomically couple operation outcomes to Moderation case state and the existing internal `moderation_events` audit ledger.

The caller supplies the host-materialized `ModerationSubjectAdapterRegistry`. The Moderation owner remains responsible for orchestration/case/audit state; the selected domain adapter remains responsible for the domain mutation and its own receipt/audit transaction.

The follow-up scheduler composition reuses this primitive through the shared `rustok_runtime::ModuleWorkScheduler`. That registration does not change any dispatcher ownership described below: candidate discovery is read-only and this one-attempt CAS remains the sole durable claim before a domain adapter call.

## Attempt lifecycle

The dispatcher first calls the existing CAS `claim_application_operation` with the default 60-second owner lease. A non-due/already-owned/terminal operation returns `None` and no adapter is called.

A successful claim has a fresh UUID lease token and incremented attempt count. The owner claim primitive now performs that operation CAS in the same transaction as the Moderation case/audit start boundary: the first claim moves `decided -> applying_decision` and writes `case_application_started`; every successful claim writes `application_attempt_claimed`. Retry/reclaim while the case is already applying does not increment the case revision again.

Every eventual owner transition still passes through the existing live-token and unexpired-lease predicates. If another worker reclaims an expired attempt, the stale worker cannot record `applied`, `retryable`, `rejected` or `operator_review`.

Owner-storage/database failure after claim is returned to the caller instead of being rewritten as a domain outcome. The applying lease then expires naturally and becomes reclaimable. This avoids persisting a false domain rejection when the Moderation owner itself is temporarily unavailable.

## Immutable command reconstruction

Before calling any domain adapter, the dispatcher reconstructs `ApplyModerationDecisionCommand` from Moderation-owned persisted facts:

- the claimed operation supplies exact tenant/decision/case identity and reviewed subject module/kind/UUID/revision;
- `get_decision` must return the same case ID, decision hash and subject revision;
- `get_case` must return the exact same reviewed subject identity;
- the decision must still have a typed effect;
- the stored effect must validate for the immutable decision kind.

A deterministic missing/corrupt decision, case, effect, hash or subject relationship is not dispatched. With a live lease it moves to `operator_review` under `moderation.application_command_invalid`.

Legacy decisions without typed effects remain non-dispatchable, consistent with the operation migration/backfill contract.

## Exact adapter lookup

The dispatcher looks up exactly:

```text
registry.get(operation.subject.module, operation.subject.kind)
```

There is no fallback adapter and no kind/module substitution. A missing materialized adapter is treated as retryable (`moderation.application_adapter_missing`), never as success or permanent rejection.

This preserves optional module composition: a temporarily unavailable producer/owner can recover without losing the durable Moderation intent.

## Domain call context and lost-response replay

The adapter call uses a trusted service `PortContext`:

- tenant = the operation tenant;
- actor = service `rustok-moderation`;
- locale = `und` because orchestration carries no presentation locale semantics;
- correlation ID = decision UUID + current lease token, so attempts are distinguishable;
- causation ID = immutable decision UUID;
- **idempotency key = immutable decision UUID**;
- deadline = 30 seconds.

The domain idempotency key intentionally does not contain the attempt/lease token. A lost-response retry must reach the same domain owner receipt identity. Forum therefore replays its existing shared Outbox receipt before subject reads instead of applying the mutation again.

The default Moderation operation lease is 60 seconds, leaving a bounded margin beyond the 30-second adapter deadline. If an adapter ignores/overruns that deadline and the lease expires, the existing stale-token CAS still prevents the old attempt from recording an outcome.

## Error classification and backoff

The one-attempt dispatcher classifies only the returned neutral `PortError` contract:

- `error.retryable == true` -> `retryable`;
- non-retryable `Conflict` or `InvariantViolation` -> `operator_review`;
- all other non-retryable neutral port errors -> `rejected`;
- missing exact adapter -> `retryable`.

A stale reviewed revision is a Forum `Conflict`; routing it to `operator_review` preserves the existing requirement for explicit re-review/new decision rather than treating stale content as an ordinary rejected application.

Retry delay uses a deterministic bounded exponential schedule based on the **post-claim** attempt count:

```text
5s, 10s, 20s, 40s, 80s, 160s, then capped at 300s
```

No jitter or host clock policy is hidden in the domain adapter. The shared module-work scheduler decides only when to ask for the next candidate; it must not bypass `next_attempt_at` / CAS claim semantics or reproduce this classification.

## Success and applied evidence

A successful adapter return is passed to `mark_application_applied`. The existing owner guard verifies:

- matching decision UUID;
- exact reviewed subject module/kind/UUID/revision;
- `applied_revision >= reviewed_revision`;
- exact live, unexpired lease token.

Only then does Moderation record `applied_revision` and `applied_at`. The same owner transaction now also moves the case `applying_decision -> closed`, increments the case revision, sets `closed_at`, clears `active_deduplication_key`, and writes `application_applied` plus `case_closed` to the existing `moderation_events` audit ledger.

If the adapter returned `Ok(...)` but the application evidence does not match the immutable operation, the dispatcher moves the still-live attempt to `operator_review` under `moderation.application_evidence_invalid`; the owner primitive atomically escalates the case and writes matching audit facts rather than letting deterministic bad evidence expire and retry forever.

Database failures, operation disappearance, lease conflicts, case revision conflicts and owner-audit failures while recording success are not rewritten as operator outcomes. The transaction cannot partially advance operation/case/audit state; recovery or lease reclaim resolves the owner failure without fabricating a domain result.

## Owner lifecycle finalization beneath the dispatcher

The dispatcher still delegates all persistence to the existing `claim/mark_*` owner methods. Those methods now provide the case/application audit lifecycle:

- retryable -> operation `retryable`, case stays `applying_decision`, `application_retry_scheduled`;
- applied -> operation `applied`, case `closed`, active case identity released, `application_applied` + `case_closed`;
- rejected -> operation `rejected`, case `escalated`, `application_rejected` + `case_escalated`;
- operator review -> operation `operator_review`, case `escalated`, `application_operator_review` + `case_escalated`.

Rejected and operator-review remain distinct application outcomes even though both fail closed at case level. Escalated cases retain their active deduplication key for later operator recovery/report attachment. `moderation_events` remains an internal owner audit ledger; no typed cross-domain event family is frozen by this source slice.

## Scheduler follow-up boundary

`ModerationModule` publishes one shared module-work registration. Its source returns one read-only earliest-due candidate and its handler calls this dispatcher. The generic `ModuleWorkItem.lease_token` is scheduler-envelope identity only; this dispatcher still creates the authoritative Moderation UUID lease and still uses decision UUID as domain idempotency.

Multi-host duplicate discovery therefore converges on this existing CAS. A host that loses the claim sees `None` and never reaches a domain adapter. Generic module-work completion is a no-op because owner primitives already persist durable operation/case/audit outcomes.

No capability-specific polling loop was added. Host startup, polling cadence and graceful stop remain the existing shared `ModuleWorkScheduler` / deployment `StopHandle` lifecycle.

## Explicitly not claimed

The combined operation + one-attempt + shared-scheduler + audit-lifecycle source does not add:

- operator retry/requeue/re-review commands or UI;
- automatic replacement/rewrite of an immutable decision after escalation;
- a typed public/cross-domain Moderation application event family;
- retained runtime, crash, timeout, lifecycle-atomicity, multi-host, graceful-stop, PostgreSQL or SQLite execution evidence.

Those remain follow-up Moderation-owner work. The next bounded code slice should define operator recovery without weakening this one-attempt CAS/idempotency or the atomic operation/case/audit boundary.

## Ownership and Reactions boundary

Forum still depends only on `rustok-moderation-api`. The dispatcher does not import Forum or any other producer owner crate; it reaches domains only through the neutral registry.

This slice is unrelated to Reactions. `rustok-reactions` remains the sole reaction state/command/aggregate owner and `rustok-reactions-storefront` remains the reusable reaction presentation owner. No reaction catalog, state, aggregate, command, transport or UI is added to Moderation or Forum.

## Maintainer verification handoff

Suggested checks, intentionally not run while preparing this source:

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

Retained evidence should cover: not-due no claim/no adapter call; exact command reconstruction; first-claim case start and retry/reclaim no-extra-revision; missing-adapter retry; retryable timeout/unavailable backoff and retry audit atomicity; non-retryable validation/not-found/forbidden rejection + case escalation; conflict/invariant operator-review + case escalation; stale-revision re-review classification; successful applied evidence + exactly one case close/active-key release; mismatched successful evidence -> operator-review; lost-response replay with the same decision UUID idempotency key; adapter runtime exceeding lease; stale-token finish rejection after reclaim; owner DB/audit failure followed by lease reclaim; shared scheduler candidate selection/multi-host convergence/stop behavior; and PostgreSQL/SQLite operation/case/audit atomicity.

No tests, Cargo commands, Node verifiers, formatting, migrations, database scenarios, workflows or CI were executed while preparing this source.
