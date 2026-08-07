# Moderation application audit lifecycle

Status: **bounded source-ready slice / maintainer execution pending**

## Scope

This slice makes the existing Moderation application operation, case lifecycle and owner audit ledger advance atomically around the already source-ready one-attempt dispatcher for every transition performed after this source is active.

It does not add a second queue, scheduler, audit table, domain adapter path or cross-domain event family. `moderation_application_operations` remains the durable application state, `moderation_cases` remains the case state machine, and the existing `moderation_events` table remains the Moderation-owned audit ledger.

Operator retry/requeue/re-review commands are deliberately left for the next bounded owner slice. That follow-up must also reconcile pre-audit terminal operation rows truthfully rather than inventing historical lifecycle events.

## Claim and case start

`ModerationService::claim_application_operation` still owns the authoritative due predicate and UUID lease-token CAS. The CAS now runs inside one owner transaction together with the case/audit transition.

After a successful claim:

- a case in `decided` moves to `applying_decision` with one case revision increment;
- an already-`applying_decision` case stays there during retries or expired-lease reclaim and does not receive another case revision bump;
- any unrelated case state fails closed and the transaction rolls the operation claim back;
- `case_application_started` is written only for the first `decided -> applying_decision` transition;
- every successful lease claim writes one `application_attempt_claimed` event with bounded attempt/lease metadata.

The lease token remains operation-attempt state only. It is not written into the domain idempotency key and it does not replace the immutable decision UUID used by `dispatch_application_operation_once`.

## Retryable outcome

A retryable adapter result atomically moves the operation from `applying` to `retryable`, clears the live lease, stores the bounded error and `next_attempt_at`, and writes `application_retry_scheduled`.

The case must still be `applying_decision` and remains in that state. Retry scheduling does not create a new case revision because the case-level meaning has not changed.

Missing adapters and retryable neutral owner/domain failures therefore remain open application work rather than guessed enforcement success.

## Applied outcome

A matching `ModerationDecisionApplication` and live lease atomically:

1. move the operation from `applying` to `applied`;
2. persist exact applied revision/time evidence;
3. move the case from `applying_decision` to `closed` with one revision increment;
4. set `closed_at` and clear `active_deduplication_key`, releasing the completed active-case identity;
5. append `application_applied` on the application aggregate;
6. append `case_closed` on the case aggregate.

If the operation CAS, case CAS or either owner audit insert fails, the owner transaction rolls back. Moderation never records a newly finalized closed case without the matching durable application state and audit facts.

## Rejected and operator-review outcomes

A non-retryable rejected application and an operator-review stop both mean the immutable decision was not proven applied. They therefore fail closed at case level.

Both outcomes atomically move the case from `applying_decision` to `escalated` with one case revision increment while preserving its active deduplication identity for operator work and further reports.

The operation keeps the existing distinct terminal state:

- `rejected` writes `application_rejected`;
- `operator_review` writes `application_operator_review`.

Both also write `case_escalated` with the exact application status and bounded error code. `closed_at` is not set and the active deduplication key is not released.

This distinction remains important: a deterministic domain/contract rejection is not silently rewritten as applied, and a stale revision/invariant/evidence mismatch remains an explicit operator-review condition.

## Upgrade compatibility for pre-audit terminal rows

This source slice intentionally adds no migration and does not fabricate audit history for application operations that were already terminal before the atomic lifecycle source existed.

A legacy row already in `applied`, `rejected` or `operator_review` is no longer due and therefore cannot pass through the new claim/finalizer path. Its associated case may consequently still reflect the pre-audit lifecycle state (for example `decided`) and there may be no truthful `case_application_started` / terminal lifecycle audit pair to backfill.

Those rows are a bounded reconciliation concern, not evidence that the historical events occurred. The next operator-recovery slice must define an explicit owner reconciliation path that:

- inspects exact immutable decision, operation and case identity;
- distinguishes legacy terminal-state reconciliation from a new domain application attempt;
- never re-invokes a domain adapter merely to manufacture lifecycle history;
- never invents historical event timestamps or claims that an unobserved lifecycle transition happened;
- preserves an already accepted `applied` operation as applied while bringing case state forward through an explicit reconciliation fact;
- keeps rejected/operator-review recovery fail-closed and operator-visible;
- remains bounded and tenant/decision scoped.

Until that follow-up exists and is evidenced, upgraded legacy terminal rows are explicitly **not** claimed reconciled by this slice.

## Crash and lost-response behavior

The existing crash/lost-response contract is preserved.

If a process dies after a claim transaction commits, the case stays `applying_decision` and the operation stays `applying` until the lease expires. A later claim increments the operation attempt count, leaves the case revision unchanged, and appends another `application_attempt_claimed` audit fact.

If the domain already committed its owner mutation but Moderation lost the response, a retry still uses the immutable decision UUID as `PortContext.idempotency_key`. The Forum adapter therefore replays its shared owner-operation receipt rather than mutating twice. Moderation then records the replayed application evidence and closes the case atomically.

If Moderation storage or an owner audit insert fails while finalizing a domain response, the operation/case transaction does not partially commit. The live operation lease remains the recovery boundary and can eventually be reclaimed; no fabricated applied/rejected outcome is written.

## Event boundary

This slice uses only the existing internal `moderation_events` audit ledger. It does not claim a new typed `rustok-events` cross-domain contract.

Source-ready event types for transitions executed by this source are:

- `case_application_started`;
- `application_attempt_claimed`;
- `application_retry_scheduled`;
- `application_applied`;
- `application_rejected`;
- `application_operator_review`;
- `case_closed`;
- `case_escalated`.

Application events use `aggregate_kind = application` and `aggregate_id = decision_id`. Case events keep `aggregate_kind = case` and the existing case UUID.

Future public/transactional outbox contracts, if needed by another module, must be versioned separately instead of treating this owner audit table as a public event bus.

## Explicitly not claimed

This slice does not add:

- operator retry/requeue/re-review commands;
- reconciliation of pre-audit terminal application rows and their legacy case state;
- a UI or admin recovery surface;
- automatic creation of a replacement decision after escalation;
- report dismissal/closure policy beyond the existing report state model;
- a new migration or audit/event table;
- a new scheduler loop or host task;
- a typed cross-domain Moderation event family;
- retained SQLite/PostgreSQL, crash, concurrency or scheduler execution evidence.

## Ownership and Reactions boundary

Moderation remains the only owner of reports, cases, immutable decisions, application operations, retries and cross-domain moderation audit. Forum owns only Forum topic/reply state, the dedicated Forum moderation subject revision and the domain-side receipt/effect transaction.

This slice is unrelated to Reactions. Existing `rustok-reactions` remains the reaction catalog/state/command/aggregate/event/repair owner and `rustok-reactions-storefront` remains the reusable presentation owner. No duplicate Forum reactions subsystem is created.

## Maintainer verification handoff

Suggested checks, intentionally not run while preparing this slice:

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

Retained evidence should cover: first claim `decided -> applying_decision`; retry/reclaim without another case revision; claim/event rollback on audit failure; retryable operation + retry event atomicity; applied operation + `closed` case + cleared active key + both audit events atomicity; rejected/operator-review operation + `escalated` case + both audit events atomicity; stale-token finalizer rollback; case revision CAS contention; domain lost-response receipt replay followed by exactly one case close; owner storage failure followed by lease reclaim; PostgreSQL/SQLite parity; and explicit evidence for legacy-terminal reconciliation once the follow-up recovery path exists.

No tests, Cargo commands, Node verifiers, formatting, migrations, database scenarios, workflows or CI were executed while preparing this slice.
