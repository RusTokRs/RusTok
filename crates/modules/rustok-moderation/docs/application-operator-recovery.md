# Moderation application operator recovery

Status: **owner recovery + authorized GraphQL recovery/re-review transport implemented / production promotion deferred**

## Scope

This slice covers Moderation-owned replay-safe operator recovery over the existing durable application operation and case lifecycle, plus the host-owned authenticated GraphQL transport that enters those owner ports. It also defines the explicit fresh-revision re-review workflow as a composition of the existing replay-safe case commands. It does not add an admin UI, a new queue, a new scheduler, a migration, a replacement decision table, or another domain-application path.

Two owner recovery commands remain source-ready:

- `operator_requeue_application_replay_safe` for an explicit same-decision retry;
- `operator_reconcile_legacy_application_replay_safe` for truthful case-state reconciliation of a pre-audit terminal operation.

Both commands require write semantics, a human `PortActorKind::User` with UUID identity, a positive expected case revision, a bounded non-empty reason, and the existing Moderation command idempotency receipt.

The dedicated `ModerationRecoveryCommandPort` additionally requires the trusted caller permission snapshot to contain `moderation_cases:override` or effective `moderation_cases:manage` authority. Ordinary Forum moderation permissions do not authorize Moderation application recovery or re-review orchestration.

## Authorized GraphQL transport

When `rustok-server` is compiled with `mod-moderation`, the host schema exposes three administrative mutations:

- `requeueModerationApplication(idempotencyKey, decisionId, expectedCaseRevision, reason)`;
- `reconcileLegacyModerationApplication(idempotencyKey, decisionId, expectedCaseRevision, reason)`;
- `createModerationRereview(idempotencyKey, sourceDecisionId, freshSubjectRevision, rereviewReason, decisionKind, reasonCode, effect, policySnapshot)`.

The transport is intentionally host-owned rather than adding a GraphQL dependency to the Moderation owner crate. Before entering recovery/re-review it requires:

1. an authenticated `AuthContext` whose tenant matches the request `TenantContext`;
2. a human-user principal; OAuth service principals fail closed;
3. effective `moderation_cases:override` authority, with `moderation_cases:manage` satisfying that authority through shared permission semantics;
4. the `moderation` tenant module to be enabled through the shared GraphQL module guard;
5. a non-nil UUID root idempotency key.

The transport builds a five-second `PortContext` from trusted tenant/actor/locale/permission facts, preserves the authenticated permission snapshot as port claims, carries the resolved channel when present, and maps owner `PortError` kinds to existing public GraphQL error classes without exposing storage details.

Same-decision requeue and legacy reconciliation call only `ModerationRecoveryCommandPort`; the transport does not bypass the port to call `operator_*` owner methods directly. Re-review composes only the public `ModerationReadPort` and `ModerationCommandPort` operations described below.

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

The next scheduler claim still increments `attempt_count` and invokes the existing one-attempt dispatcher. Recovery never invokes a subject adapter directly. The domain idempotency key remains the immutable decision UUID.

## Legacy terminal reconciliation

`ReconcileLegacyModerationApplicationCommand` is only for terminal `applied`, `rejected`, or `operator_review` rows whose case state may predate the atomic application-audit lifecycle.

Target mapping is fixed:

- `applied -> closed`;
- `rejected -> escalated`;
- `operator_review -> escalated`.

For an `applied` row, stored `applied_revision >= reviewed_revision` and stored `applied_at` are required. Rejected/operator-review rows must not contain applied evidence. All terminal rows must have no lease tuple.

If the case is already in the correct terminal state, reconciliation is an idempotent no-op (`changed = false`). If the case is still `decided` or `applying_decision`, reconciliation advances it with one revision CAS. Closing happens at the **current reconciliation time** and releases `active_deduplication_key`; it does not pretend the case historically closed at the older domain `applied_at` timestamp.

The command writes only present-time reconciliation audit facts:

- `application_legacy_terminal_reconciled`;
- `case_legacy_terminal_reconciled`.

Legacy reconciliation never invokes a domain adapter.

## Fresh-revision re-review

Re-review is not mutation of an old case or decision. `createModerationRereview` creates a **new moderation case and new immutable decision** from an explicit producer/admin-supplied subject revision while preserving the historical source case/decision unchanged.

The source decision is resolved through `ModerationReadPort`, then its source case is read and checked. Re-review is admitted only when:

- the source decision points to that exact case;
- the source decision reviewed the same stored source-case subject revision;
- the source case is currently `escalated`;
- `freshSubjectRevision` is strictly greater than the historical reviewed revision.

The caller cannot supply a replacement module, kind, subject UUID, scope, queue, priority, policy ID or policy version. The workflow copies those facts from the source case and changes only the subject revision. This prevents an administrative re-review request from silently retargeting historical Moderation identity to another domain subject.

The fresh case deliberately attaches no historical report IDs. Existing reports are revision-bound evidence for the old subject state and cannot truthfully be reused as if they were submitted against the new revision.

The caller supplies a new decision kind, reason code, typed/versioned effect and policy snapshot. Effect/kind compatibility is validated before owner commands run, and `decide_case` validates the same contract again before persisting the new immutable decision/effect/pending application operation.

### Replay-safe orchestration

One non-nil root UUID becomes three deterministic owner receipt identities:

- `<root>:rereview:open`;
- `<root>:rereview:assign`;
- `<root>:rereview:decide`.

The workflow therefore composes the existing `open_case -> assign_case -> decide_case` replay-safe owner operations without adding an orchestration table. A caller retry after a lost response re-enters the same owner receipts rather than creating another command identity.

Fresh case metadata contains a bounded `operator_rereview` ownership marker with root idempotency key, source case ID, source decision ID, old subject revision, fresh subject revision and operator reason. Because `open_case` uses active-case deduplication, it can truthfully return an already-existing case for the same fresh scope/subject/queue/policy identity. Before assign/decide, the transport verifies that the returned case carries the exact marker for this root workflow. If another active case already owns that fresh revision, orchestration fails closed instead of adopting or mutating it.

The assigned moderator is the authenticated human operator. `assign_case` and `decide_case` retain their normal expected-revision CAS boundaries, so concurrent edits cannot be silently overwritten.

### Producer revision truth

Moderation still does not invent or fetch a producer's current revision. `freshSubjectRevision` is an explicit input fact supplied by the authorized producer/admin flow. A fabricated, future or already-stale revision cannot be silently retargeted during application: the subject-owner adapter fences the current domain revision and returns a conflict when it does not equal the immutable decision's reviewed revision.

A later neutral producer-read contract may improve operator ergonomics, but it is not required to keep this workflow fail-closed.

## Concurrency and replay

Same-decision recovery commands use the existing `moderation_receipts` ledger. Re-review uses that same ledger indirectly through the existing open/assign/decide owner commands with deterministic per-step keys.

Case revision remains an explicit optimistic CAS boundary. Requeue also CASes the exact prior application terminal status. Re-review cannot adopt a foreign active case because ownership metadata is checked before assignment. Any individual owner command remains transactionally atomic; a retry resumes by replaying completed steps and executing the first unfinished step.

No workflow rewrites the historical source case, historical decision hash, historical reviewed revision or old domain receipt identity.

## Ownership boundaries

Moderation remains the sole owner of reports, cases, immutable decisions, application operations, operator recovery, re-review workflow facts and cross-domain moderation audit.

The server transport owns only authenticated GraphQL adaptation/orchestration. It supplies trusted request facts and composes public Moderation ports; it does not reproduce Moderation persistence, lifecycle or decision-application logic.

Forum is not involved in recovery persistence and is never called for legacy reconciliation. Forum continues to own only its topic/reply state, moderation subject revision and domain-side application receipt/effect transaction.

This slice is unrelated to Reactions. Existing `rustok-reactions` remains the sole reaction catalog/state/command/aggregate/event/repair owner and `rustok-reactions-storefront` remains the reusable presentation owner.

## Explicitly not claimed

This slice does not add:

- mutation or retargeting of an old moderation decision;
- requeue of an already-applied decision;
- automatic producer current-revision lookup from Moderation;
- domain adapter invocation from recovery/reconciliation/re-review creation;
- admin UI;
- public typed recovery/re-review event contracts;
- a migration, orchestration table or new persistence owner;

## Verification and promotion handoff

The recovery/RBAC/GraphQL/re-review implementation is retained by the existing source verifiers plus later repository evidence. PostgreSQL recovery parity landed in #3213; application-operation/dispatcher/lost-response/scheduler evidence landed in #3216, #3217, #3219 and #3221; SQLite/PostgreSQL migration and host-composition evidence landed in #3225/#3226.

The transport remains deliberately UI-agnostic. A module-owned Moderation admin UI is broader product work, not a prerequisite for the completed recovery transport or the bounded FORUM-19 producer integration. Deployment-dependent promotion belongs to the final production-validation phase rather than reopening these implementation slices.
