---
id: doc://crates/rustok-translation/docs/implementation-plan.md
kind: module_plan
language: en
status: in_progress
last_reviewed: 2026-07-28
---

# Translation implementation plan

## Scope

Build the optional, admin-only translation control plane described by the
central plan without taking ownership of domain-localized data or runtime locale
selection.

## Current State

- The platform module, manifest, RBAC resources, migration source, and
  distribution feature are present.
- The module persists tenant-scoped inventory identities and provider cursor
  checkpoints without storing source or translated text.
- `TranslationInventoryService` consumes only the neutral provider registry,
  checks Translation workflow permission, delegates owner authorization to the
  provider, validates bounded requests, rejects cross-provider identities and
  non-advancing or missing cursors, collapses duplicate identities, and
  advances checkpoints with optimistic revision protection.
- Integration evidence covers cursor replay, tenant-isolated inventory and
  checkpoints, invalid bounds, provider outage, missing cursors, and
  cross-provider identity rejection without partial persistence. Bounded
  full-rescan drains the owner cursor, replaces one provider projection
  atomically, and rolls back if that checkpoint advances during listing.
- The first manual-workflow persistence slice creates tenant-scoped jobs,
  immutable owner-provider source snapshots, proposal/approval tables, and
  application-receipt tables. `TranslationWorkflowService` exposes idempotent
  job creation and item admission plus owner-validated proposal save, submit,
  and approval transitions. Request hashes, item/job revision CAS, persisted QA
  issues, current-proposal checks, and translator/reviewer separation guard the
  implemented flow.
- Approved proposals now apply through a durable intent state machine. The
  exact owner patch is persisted before invocation; same-key retries are bound
  to the original actor and request; retryable unknown outcomes remain
  `applying`; owner conflicts become terminal `conflict`; and `applied` is
  committed only together with a validated stable owner receipt.
- Apply attempts use expiring owner-execution leases. An operator holding both
  Translation Manage and Publish can recover an unknown outcome through a
  separately idempotent, actor-bound command with a mandatory reason and
  expected-attempt guard. Recovery is audited before owner invocation, cannot
  steal an unexpired lease, and reuses the original owner mutation key.
- Item assignment and unassignment are actor-bound idempotent commands with
  explicit expected revisions and append-only audit rows. Assigned drafts can
  be saved or submitted only by the assignee or a Translation manager.
- Job cancellation atomically cancels remaining mutable items, clears their
  assignments, preserves applied/excluded items, stores the operator reason,
  and rejects jobs with an unresolved owner apply.
- Job creation/cancellation, assignment changes, proposal submission/approval,
  apply request/completion/failure, and privileged recovery publish sealed,
  content-free `TranslationWorkflowEvent` contracts through the Core outbox in
  the same transaction as the corresponding workflow state.
- A non-empty job completes automatically only after all items become applied,
  excluded, or cancelled. Blocked items can return to their current approved
  proposal only through an actor-bound, idempotent, audited retry; stale and
  conflict items remain rebase-required.
- `translation_job_progress` is a content-free, transactionally maintained
  projection of item states, assignments, required/optional units,
  approved/applied units, completed resources, and character workload.
  `TranslationProgressService` provides tenant-isolated reads and a
  Manage-authorized deterministic rebuild that verifies source/proposal
  digests and owner receipt evidence.
- Media is the first owner provider with durable change-cursor repair.
- Provider-level exact-locale coverage/lag aggregation, policies, memory,
  glossaries, interchange, transports, UI, and AI integration are not
  implemented yet.

## FFA/FBA status

- FFA status: `not_started`
- FBA status: `in_progress`
- Structural shape: `core_no_ui`
- Evidence:
  - module-owned core and neutral provider dependency are separated;
  - no transport or UI has been published;
  - admin UI is planned for Leptos and Next with native/GraphQL parity.
- Last verified at (UTC): 2026-07-28
- Owner: Translation module maintainers

## Milestones

1. Complete Media multi-replica evidence for the implemented inventory replay,
   tenant isolation, stale-checkpoint conflict, provider outage, and
   full-rescan recovery contracts.
2. Add provider-level exact-locale coverage and projection-lag aggregation.
3. Publish operator recovery, assignment, cancellation, retry, and progress through
   native/GraphQL transport parity.
4. Add translation memory and versioned glossaries with separate permissions.
5. Add bounded owner-aware import/export.
6. Publish module-owned GraphQL and native server-function adapters.
7. Add Leptos and Next admin surfaces in parity.
8. Integrate `rustok-ai-translation` only after manual apply and structured AI
   execution foundations pass their gates.

## Verification

- `cargo check -p rustok-translation`
- `cargo test -p rustok-translation`
- `cargo xtask module validate translation`
- `cargo xtask validate-manifest`
- `cargo check -p rustok-server --lib --no-default-features --features mod-translation`
- `node scripts/verify/verify-translation-surface-registry.mjs`

## Update Rules

- Never query or mutate owner tables from this module.
- Never count runtime fallback as an exact translation.
- Never let AI output apply owner data without deterministic validation and
  review.
- Keep the local FFA/FBA block and central readiness board synchronized when
  transport or UI appears.
- Keep `rustok-translation-targets` dependency-neutral even if its directory is
  physically colocated with this module later.
