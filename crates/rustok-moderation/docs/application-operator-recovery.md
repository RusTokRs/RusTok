# Moderation application operator recovery

Status: **bounded source-ready slice / maintainer execution pending**

## Scope

This slice adds Moderation-owned, replay-safe operator commands over the existing durable application operation and case lifecycle. It does not add an admin transport/UI, a new queue, a new scheduler, a new decision type, a migration, or another domain-application path.

Two owner commands are source-ready:

- `operator_requeue_application_replay_safe` for an explicit same-decision retry;
- `operator_reconcile_legacy_application_replay_safe` for truthful case-state reconciliation of a pre-audit terminal operation.

Both commands require write semantics, a human `PortActorKind::User` with UUID identity, a positive expected case revision, a bounded non-empty reason, and the existing Moderation command idempotency receipt.

## Same-decision operator requeue

`RequeueModerationApplicationCommand` may requeue only application operations currently in `rejected` or `operator_review`.

It deliberately cannot requeue `applied`. Once matching application evidence has proven a decision applied, the same immutable decision is not returned to the scheduler.

A successful operator requeue commits in the command receipt transaction:

1. exact tenant/decision/case/subject/hash identity validation;
2. exact expected case revision validation;
3. terminal operation shape validation, including absence of a lease tuple and absence of applied evidence for non-applied terminal states;
4. operation `rejected|operator_review -> retryable` with `next_attempt_at = now`, cleared lease/current error fields, unchanged immutable decision identity and unchanged attempt count;
5. case `escalated -> applying_decision`, or legacy pre-audit `decided -> applying_decision`, with one case revision increment;
6. `application_operator_requeued` and `case_application_requeued` owner audit facts containing operator UUID, reason and previous terminal state/error facts;
7. completion of the Moderation command receipt.

The next scheduler claim still increments `attempt_count` and still invokes the existing one-attempt dispatcher. Recovery never invokes a subject adapter directly.

The domain idempotency key therefore remains the immutable decision UUID. If a domain owner already retained a terminal receipt for that decision, the retry reaches that same receipt instead of creating a new domain mutation identity.

## Legacy terminal reconciliation

`ReconcileLegacyModerationApplicationCommand` is only for terminal `applied`, `rejected`, or `operator_review` rows whose case state may predate the atomic application-audit lifecycle.

The command validates exact immutable decision/case/operation identity and terminal storage shape before changing the case.

Target mapping is fixed:

- `applied -> closed`;
- `rejected -> escalated`;
- `operator_review -> escalated`.

For an `applied` row, stored `applied_revision >= reviewed_revision` and stored `applied_at` are required. Rejected/operator-review rows must not contain applied evidence. All terminal rows must have no lease tuple.

If the case is already in the correct terminal state, reconciliation is an idempotent no-op (`changed = false`). A consistent already-closed applied case must have `closed_at` and no active deduplication key.

If the case is still `decided` or `applying_decision`, reconciliation advances it to the mapped terminal state with one revision CAS. Closing happens at the **current reconciliation time** and releases `active_deduplication_key`; this does not pretend the case historically closed at the domain's older `applied_at` timestamp. Escalation preserves the active case identity.

The command writes only present-time reconciliation audit facts:

- `application_legacy_terminal_reconciled`;
- `case_legacy_terminal_reconciled`.

It does **not** fabricate historical `case_application_started`, `application_applied`, `application_rejected`, `application_operator_review`, `case_closed`, or `case_escalated` facts.

Most importantly, legacy reconciliation never invokes a domain adapter. It trusts only already persisted terminal operation truth after validating its immutable decision/case identity and evidence shape.

## Re-review semantics

Re-review is intentionally **not** implemented as mutation of an old case or decision.

A stale reviewed subject revision is part of the immutable decision identity. Changing that revision would silently retarget a decision that was made against different owner state.

Therefore a true re-review must use a **new moderation case and new immutable decision** built from a freshly authorized producer-supplied subject revision. The old escalated case/decision remains historical truth. This slice adds no automatic producer read, no old-decision rewrite and no hidden replacement decision.

Admin transport/UI for creating that fresh review remains separate work.

## Concurrency and replay

Both recovery commands use the existing `moderation_receipts` command ledger. The request hash binds actor plus command payload, including decision ID, expected case revision and reason. Same-key replay returns the stored recovery response; changed input conflicts.

Case revision is an explicit optimistic CAS boundary. Requeue also CASes the exact prior application terminal status. Any operation/case/audit/receipt failure rolls back the owner transaction.

A second recovery request with another idempotency key cannot silently duplicate the state change: after successful requeue the operation is no longer terminal, and after successful reconciliation the case is already in the target terminal state.

## Ownership boundaries

Moderation remains the sole owner of reports, cases, immutable decisions, application operations, operator recovery and cross-domain moderation audit.

Forum is not involved in recovery persistence and is never called for legacy reconciliation. Forum continues to own only its topic/reply state, moderation subject revision and domain-side application receipt/effect transaction.

This slice is unrelated to Reactions. Existing `rustok-reactions` remains the sole reaction catalog/state/command/aggregate/event/repair owner and `rustok-reactions-storefront` remains the reusable presentation owner. No duplicate Forum reactions subsystem is created.

## Explicitly not claimed

This slice does not add:

- automatic re-review or decision rewriting;
- requeue of an already-applied decision;
- producer current-revision lookup from Moderation;
- domain adapter invocation from recovery;
- admin GraphQL/HTTP/UI recovery transport;
- public typed recovery event contracts;
- a migration or new persistence table;
- retained runtime, PostgreSQL, SQLite or concurrency evidence.

## Maintainer verification handoff

Suggested checks, intentionally not run while preparing this slice:

```bash
node scripts/verify/verify-moderation-application-operator-recovery.mjs
node scripts/verify/verify-moderation-application-audit-lifecycle.mjs
node scripts/verify/verify-moderation-application-dispatch-once.mjs
cargo check -p rustok-moderation --all-targets
cargo test -p rustok-moderation
cargo xtask module validate moderation
git diff --check
```

Retained evidence should cover human-actor enforcement; receipt replay/changed-request conflict; expected case revision contention; rejected/operator-review requeue; applied requeue rejection; requeue from current escalated and legacy decided case state; next scheduler claim after requeue; preservation of immutable decision UUID domain idempotency; terminal identity/evidence corruption fail-closed behavior; applied legacy reconciliation to closed at reconciliation time with active-key release; rejected/operator-review legacy reconciliation to escalated; already-consistent reconciliation no-op; no domain adapter invocation during reconciliation; rollback on audit/receipt failure; and PostgreSQL/SQLite parity.

No tests, Cargo commands, Node verifiers, formatting, migrations, database scenarios, workflows, CI or `git diff --check` were executed while preparing this source slice.
