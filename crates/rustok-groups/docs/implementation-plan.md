---
id: doc://crates/rustok-groups/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-groups
  - platform-community
last_reviewed: 2026-07-22
---

# `rustok-groups` canonical implementation plan

## Canonical-source policy

This file is the single source of truth for the Groups roadmap, implementation backlog,
FFA/FBA status, integration gates, and release evidence. Issues and pull requests are
execution records only. Source presence never counts as runtime evidence.

## Scope

Groups provides phpFox-class social communities while preserving RusToK owner-module,
multilingual, FFA/FBA, tenant-isolation, and fail-closed privacy boundaries. Scope
includes public/closed/secret groups, localized identity, memberships, applications,
invitations, roles, bans, provider-owned feature sections, integrations, and
module-owned admin/storefront packages.

## Status vocabulary

- `planned`: contract or implementation is not source-complete.
- `in_progress`: useful source exists, but runtime, parity, concurrency, security,
  accessibility, migration, or degraded-mode gates remain open.
- `done`: implementation and every declared gate have executable evidence.
- `blocked`: an external owner capability is required.

## Architectural invariants

Groups owns group identity, exact-locale presentation, memberships, local roles, join
policy, invitations, membership applications, questions/rules, bans, feature bindings,
receipts, audit, and Groups semantic events. It does not own profiles, media binaries,
provider content, comments, notification inboxes, search documents, feed entries,
checkout, payments, orders, or fulfillment. Optional modules use typed identifiers,
ports, semantic events, and host composition, never Groups foreign keys.

Language-neutral state belongs to base tables. Localized business copy belongs to
exact-locale rows selected from the host-resolved locale. There is no English,
first-row, or module-local fallback. Current policy rows remain mutable owner state,
while `group_membership_policy_revisions` stores append-only exact-locale history.

Writes require deadline plus idempotency key. Owner services repeat authorization and
invariants inside transactions. Successful state, group version, receipt, and audit
commit together where declared.

Policy save and candidate submit use `GroupApplicationCasCommandPort` with
`policy_id`, `revision`, and exact `locale`. The group row is locked and the
precondition is checked before owner-state writes. Stale input returns
`groups.application_policy_changed`. For identical committed commands, receipt replay
is checked before the precondition is re-evaluated.

Candidate cancellation and manager reopen use `GroupApplicationLifecycleCommandPort`.
Both check receipt replay first, lock application then group, and commit application,
membership, group version, audit, and receipt atomically. Legacy unconditional Rust
save/submit methods remain compatibility-only. The final GraphQL root does not expose
those legacy unconditional application mutations, and module-owned FFA does not use
them.

## Current implementation state

Source includes group identity/privacy/localization, memberships and governance,
invitations and targeted delivery source, exact-locale application policies,
append-only policy history, CAS save/submit, approve/reject review, and application
lifecycle:

- `GroupApplicationLifecycleReadPort` returns only the exact candidate's current
  tenant/group application;
- candidate cancellation changes only `pending` to `cancelled`, moves membership to
  `left`, and preserves the submitted snapshot;
- manager reopen changes only `rejected` or `cancelled` to `pending`, restores pending
  membership, and preserves policy identity, snapshot, answers, and acknowledgements;
- fresh resubmit from `rejected` or `cancelled` remains a separate current-policy CAS
  command and replaces the snapshot only after successful submission;
- final `graphql_application_cas` roots compose CAS, review, lifecycle query, cancel,
  and reopen without exposing legacy unconditional application mutations;
- the visual policy editor captures loaded policy identity and uses owner CAS;
- admin FFA supports status filtering and reopen controls;
- storefront FFA shows current status, permits pending cancellation, blocks approved
  duplicate submission, and permits fresh rejected/cancelled resubmit;
- cancellation preserves `apply=<group_uuid>`; successful fresh submit clears it;
- native and GraphQL paths remain explicitly selected and never fall back.

The invitation acceptance/delivery source remains source-complete only at its declared
boundary. Targeted invitation delivery remains `in_progress`; runtime parity and
Notifications consumer evidence remain open.

Compilation, tests, migrations, parity, replay, lifecycle race, lock-order,
accessibility, security, retry, and recovery evidence remain open.

## Program ledger

| ID | Status | Scope | Remaining gate |
|---|---|---|---|
| GROUPS-00 | in_progress | architecture, ownership, parity, FFA/FBA contracts | executable review |
| GROUPS-01 | in_progress | skeleton, manifest, RBAC, migrations, host composition | build/module evidence |
| GROUPS-02 | in_progress | identity, localization, privacy, bindings, receipts/audit, events | runtime/concurrency evidence |
| GROUPS-03 | in_progress | memberships, join/leave, roles, ownership transfer | bans/concurrency completion |
| GROUPS-04 | in_progress | typed read/write/CAS/lifecycle/governance ports | provider/consumer matrix |
| GROUPS-05 | in_progress | GraphQL/native and storefront invitation acceptance/delivery source | runtime parity and Notifications consumer evidence |
| GROUPS-06 | in_progress | localized policy, snapshots, CAS, visual policy editor, review, history, candidate cancellation, manager reopen, resubmit UX | legacy migration, bulk safety, profiles/events, parity, concurrency, accessibility |
| GROUPS-07 | planned | bans and moderation | implementation/evidence |
| GROUPS-08 | planned | dynamic feature registry/navigation | runtime degradation evidence |
| GROUPS-09 | planned | Forum spaces and ACL | Forum integration evidence |
| GROUPS-10 | planned | Blog and Pages/Wiki contexts | owner integration evidence |
| GROUPS-11 | planned | Marketplace seller context | checkout boundary evidence |
| GROUPS-12 | planned | Media, Events, Chat providers | provider lifecycle evidence |
| GROUPS-13 | in_progress | notifications, search, moderation, profiles/media | consumer privacy evidence |
| GROUPS-14 | in_progress | storefront/admin UX | pickers, accessibility, parity |
| GROUPS-15 | planned | feed/wall aggregation | feed owner evidence |
| GROUPS-16 | planned | analytics/observability | privacy-safe evidence |
| GROUPS-17 | planned | import/export, retention, deletion | compliance/recovery evidence |
| GROUPS-18 | planned | remote adapters/degraded modes | fallback/recovery evidence |
| GROUPS-19 | in_progress | release verification registry | all open evidence resolved |

## GROUPS-06 owner ports

- `GroupApplicationReadPort::read_group_application_policy`;
- `GroupApplicationReadPort::list_group_membership_applications`;
- `GroupApplicationPolicyHistoryReadPort::list_group_application_policy_revisions`;
- `GroupApplicationLifecycleReadPort::read_my_group_membership_application`;
- `GroupApplicationCasCommandPort::upsert_group_application_policy_if_current`;
- `GroupApplicationCasCommandPort::submit_group_membership_application_if_current`;
- `GroupApplicationLifecycleCommandPort::cancel_group_membership_application`;
- `GroupApplicationLifecycleCommandPort::reopen_group_membership_application`;
- `GroupApplicationCommandPort::review_group_membership_application`.

Candidates cannot enumerate another candidate's application or policy history.
Manager listing/history/review/reopen uses declared owner/admin/moderator or platform
authority.

## Lifecycle invariants

### Candidate cancellation

- only the exact candidate may cancel; a different actor receives not-found semantics;
- only `pending` may be cancelled;
- membership must still be `pending` and not banned;
- membership becomes `left`, `left_at` is recorded, application becomes `cancelled`;
- review metadata is cleared while policy snapshot, answers, and acknowledgements remain;
- audit `group.membership_application_cancelled`, version, and receipt commit together;
- storefront cancellation keeps `apply`, allowing a fresh CAS resubmit.

### Manager reopen

- manager reopen requires active owner/admin/moderator or platform authority;
- only `rejected` or `cancelled` may be reopened;
- group must remain active, non-secret, and `request` join policy;
- membership must be `left`, not banned, and not active;
- membership/application become `pending`; previous review metadata is cleared;
- submitted timestamp, policy identity/revision/locale, snapshot, answers, and
  acknowledgements are preserved;
- audit `group.membership_application_reopened`, version, and receipt commit together;
- later manager review uses the preserved snapshot; fresh candidate resubmit instead
  captures the current CAS policy.

### Other application invariants

- exact-locale policy reads never select fallback rows;
- stale policy writes/submits produce no owner-state mutation;
- required questions/rules are owner-revalidated;
- secret/non-request/banned/active candidates are denied as declared;
- pending/approved cannot be freshly resubmitted;
- approve activates membership and increments member count once;
- reject moves membership to `left`;
- application then group lock order is used by review/cancel/reopen where supported.

## FFA surfaces

- admin selected-transport policy/history/list/review/reopen facade;
- admin status filter for pending, approved, rejected, and cancelled rows;
- reopen controls appear only for rejected/cancelled rows;
- storefront exact-candidate application read plus exact-locale policy load;
- pending status and candidate cancel; approved duplicate-submit blocking;
- rejected/cancelled fresh current-policy CAS form;
- stale form reload clears prior answers and preserves route;
- successful submit clears `apply`; cancellation never clears it;
- no implicit native/GraphQL fallback.

## Remaining GROUPS-06 work

- remove or version-deprecate legacy unconditional Rust save/submit methods;
- explicit multi-locale manager picker and selected-locale owner read;
- bounded bulk review with confirmation, per-item results, and audit;
- ProfilesReader candidate summaries without copied profile state;
- submitted/reviewed/cancelled/reopened semantic events and optional consumers;
- pagination controls, pickers, audit/receipt history;
- keyboard, focus, validation association, and screen-reader evidence;
- executed parity, replay, CAS/lifecycle concurrency, lock-order, migration, security,
  retry, and recovery evidence.

## Degraded modes

- Groups unavailable: deny private content.
- Exact-locale policy unavailable: disable form/editor; never choose another locale.
- CAS conflict: write no state and require explicit reload.
- current-application read unavailable: do not guess status or expose submit controls;
- lifecycle transport failure: preserve selected-path error and route, never fall back;
- Profiles unavailable: use UUID/placeholder without copying canonical data;
- Notifications/search unavailable: owner writes remain authoritative.

## Verification matrix

```bash
cargo xtask module validate groups
cargo check -p rustok-groups --features graphql
cargo check -p rustok-groups-admin --features ssr
cargo check -p rustok-groups-storefront --features ssr
cargo test -p rustok-groups
node scripts/verify/verify-groups-boundary.mjs
node scripts/verify/verify-groups-localization-boundary.mjs
node scripts/verify/verify-groups-invitations-boundary.mjs
node scripts/verify/verify-groups-invitation-acceptance-ui.mjs
node scripts/verify/verify-groups-targeted-invitation-delivery.mjs
node scripts/verify/verify-groups-membership-applications.mjs
node scripts/verify/verify-groups-membership-policy-revisions.mjs
node scripts/verify/verify-groups-application-policy-cas.mjs
node scripts/verify/verify-groups-application-lifecycle.mjs
node scripts/verify/verify-db-multilingual-contract.mjs
npm run verify:i18n:ui
npm run verify:frontend:host-ffa-contract
```

Lifecycle promotion additionally requires exact-candidate isolation, pending-only
cancel, rejected/cancelled-only manager reopen, banned/active denial, snapshot
preservation, fresh CAS resubmit replacement, idempotent replay, concurrent terminal
outcomes, native/GraphQL parity, no fallback, and EN/RU accessibility.

## Evidence state for this change

No build, test, migration, verifier, GraphQL schema, parity, concurrency,
accessibility, security, retry, or recovery command was executed. FFA, FBA,
GROUPS-06, and GROUPS-19 remain `in_progress`; `membership_application_policy_cas` remains `null`; `membership_application_lifecycle` remains `null`.

## Release-ready Groups MVP

Release readiness requires executable evidence for tenant isolation, privacy,
localized identity, membership/invitation/role/ban workflows, provider integration,
native/GraphQL parity, accessibility, semantic integrations, PostgreSQL/SQLite,
replay, concurrency, security, recovery, and no direct cross-module persistence or
implicit fallback.
