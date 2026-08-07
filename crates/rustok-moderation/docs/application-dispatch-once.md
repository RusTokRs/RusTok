# Moderation one-attempt decision application dispatcher

Status: **bounded source-ready slice / maintainer execution pending**

## Scope

This slice adds the first executable owner-side dispatch primitive over the durable `moderation_application_operations` foundation.

`ModerationService::dispatch_application_operation_once` handles **at most one** exact tenant/decision operation. It does not start a background loop, enumerate tenants, own host scheduling, close moderation cases, publish application lifecycle events or expose operator UI.

The caller supplies the host-materialized `ModerationSubjectAdapterRegistry`. The Moderation owner remains responsible for orchestration state; the selected domain adapter remains responsible for the domain mutation and its own receipt/audit transaction.

## Attempt lifecycle

The dispatcher first calls the existing CAS `claim_application_operation` with the default 60-second owner lease. A non-due/already-owned/terminal operation returns `None` and no adapter is called.

A successful claim has a fresh UUID lease token and incremented attempt count. Every eventual owner transition still passes through the existing live-token and unexpired-lease predicates. If another worker reclaims an expired attempt, the stale worker cannot record `applied`, `retryable`, `rejected` or `operator_review`.

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
- non-retryable `InvariantViolation` -> `operator_review`;
- all other non-retryable neutral port errors -> `rejected`;
- missing exact adapter -> `retryable`.

Retry delay uses a deterministic bounded exponential schedule based on the **post-claim** attempt count:

```text
5s, 10s, 20s, 40s, 80s, 160s, then capped at 300s
```

No jitter or host clock policy is hidden in the domain adapter. A future scheduler may decide when to invoke one-attempt dispatch, but it must not bypass `next_attempt_at` / CAS claim semantics.

## Success

A successful adapter return is passed unchanged to `mark_application_applied`. The existing owner guard verifies:

- matching decision UUID;
- exact reviewed subject module/kind/UUID/revision;
- `applied_revision >= reviewed_revision`;
- exact live, unexpired lease token.

Only then does Moderation record `applied_revision` and `applied_at`.

## Explicitly not claimed

This slice does not add:

- a polling/background scheduler loop;
- cross-tenant enumeration;
- automatic process startup wiring;
- case transition from `decided` to `applying_decision`/`closed`/`escalated`;
- application lifecycle outbox/audit events;
- operator retry/requeue/re-review commands or UI;
- retained runtime, crash, timeout, PostgreSQL or SQLite execution evidence.

Those remain follow-up Moderation-owner work. The next bounded source slice should compose a scheduler/runner over due operations and define case/application audit lifecycle without weakening this one-attempt CAS/idempotency boundary.

## Ownership and Reactions boundary

Forum still depends only on `rustok-moderation-api`. The dispatcher does not import Forum or any other producer owner crate; it reaches domains only through the neutral registry.

This slice is unrelated to Reactions. `rustok-reactions` remains the sole reaction state/command/aggregate owner and `rustok-reactions-storefront` remains the reusable reaction presentation owner. No reaction catalog, state, aggregate, command, transport or UI is added to Moderation or Forum.

## Maintainer verification handoff

Suggested checks, intentionally not run while preparing this slice:

```bash
node scripts/verify/verify-moderation-application-operation.mjs
node scripts/verify/verify-moderation-application-dispatch-once.mjs
cargo check -p rustok-moderation --all-targets
cargo test -p rustok-moderation
cargo xtask module validate moderation
git diff --check
```

Retained evidence should cover: not-due no claim/no adapter call; exact command reconstruction; missing-adapter retry; retryable timeout/unavailable backoff; non-retryable rejection; invariant operator-review; successful applied evidence; lost-response replay with the same decision UUID idempotency key; adapter runtime exceeding lease; stale-token finish rejection after reclaim; owner DB failure after claim followed by lease reclaim; and both PostgreSQL/SQLite operation semantics.

No tests, Cargo commands, Node verifiers, formatting, migrations, database scenarios, workflows or CI were executed while preparing this slice.
