---
id: doc://crates/rustok-groups/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-groups
  - platform-community
last_reviewed: 2026-07-23
---

# `rustok-groups` canonical implementation plan

## Canonical-source policy

This file is the single source of truth for the Groups roadmap, implementation backlog,
FFA/FBA status, integration gates, and release evidence. Do not create parallel Groups
roadmaps, phpFox parity documents, remediation plans, or duplicated task ledgers. Issues
and pull requests are execution records only.

Every change that modifies Groups behavior must update this plan in the same change: task
status, remaining scope, definition of done, verification evidence, and degraded-mode notes.
Source presence alone never promotes FFA or FBA readiness to `done`.

## Scope

Build phpFox-class social groups as modular micro-social networks while preserving RusToK
ownership boundaries:

- public, closed, and secret groups;
- categories, stable handles, localized presentation, media references, and SEO;
- open join, application, invitation, group-local enforcement, ownership transfer, and
  local-role workflows;
- localized group rules and membership questions;
- owner/admin/moderator/member permissions;
- provider-owned Wall, Forum, Blog, Pages/Wiki, Media, Events, Marketplace, and Chat;
- visibility-aware search, notifications, moderation compatibility, feed, and analytics;
- module-owned admin/storefront FFA packages;
- in-process and remote-ready FBA boundaries with fail-closed privacy.

## Status vocabulary

- `planned`: contract or implementation is not yet source-complete.
- `in_progress`: useful source exists, but required runtime, parity, concurrency, security,
  accessibility, migration, or degraded-mode evidence remains open.
- `done`: implementation and every declared gate have executable evidence.
- `blocked`: another owner capability is required before safe work can continue.

## Architectural invariants

### Ownership

Groups owns group identity, localized presentation, memberships, local roles, join policy,
invitations, membership applications, rules/questions, authoritative group-local
membership/access enforcement state, feature bindings, command receipts, domain audit, and
Groups semantic events.

Groups does not own profile presentation, media binaries, forum topics, blog posts, Pages
documents, marketplace listings, products, comments, notification inboxes, search documents,
feed entries, checkout, payment, orders, fulfillment, moderation reports, moderation cases,
moderation policies, immutable moderation decisions, decision-application scheduling,
appeals, or cross-domain moderation audit history.

Optional modules never receive database foreign keys from Groups. Cross-module composition
uses typed identifiers, neutral typed ports, semantic events, and host-owned UI/runtime
composition.

### Moderation compatibility

`rustok-moderation` owns reports, cases, decisions, durable application orchestration,
retries, appeals, and cross-domain moderation history. Groups remains authoritative for the
group or membership mutation produced by an applicable decision.

Compatibility uses `rustok-moderation-api`; Groups must never depend on moderation entities,
migrations, or owner services. The neutral API owns subject/scope types, typed and versioned
decision effects, `ApplyModerationDecisionCommand`, `ModerationDecisionApplication`,
`ModerationSubjectCommandPort`, and the host-composed adapter/factory registry.

Groups subject identity is fixed:

- group subject: `module="groups"`, kind `Group`, ID `groups.id`, revision `groups.version`;
- membership subject: `module="groups"`, kind `GroupMembership`, ID
  `group_memberships.id`, revision `group_memberships.revision`;
- group-local scope: kind `Group`, ID `group_id`;
- membership revision changes whenever role, lifecycle status, invitation lifecycle fields,
  or effective enforcement state changes.

`groups.version` is not a membership revision. `updated_at` is not a revision contract. A
future Groups adapter must validate tenant, scope, subject identity/revision, decision hash,
effect version/compatibility, hierarchy, and owner invariants inside the Groups transaction.
A stale revision fails with a stable conflict and is never retargeted to the latest state.

The moderation admin queue/case/application UI belongs to moderation. Groups FFA may show
current local enforcement state, provenance, and authorized direct actions, but it must not
implement reports, cases, policy snapshots, appeals, or a second moderation queue.

### Multilingual storage and locale ownership

Language-neutral state belongs to base tables. Canonical localized business copy belongs to
exact-locale translation tables. There is no English, first-row, or module-local fallback.

Membership-application policies use:

- `group_membership_policies` for language-neutral revision/enabled state;
- `group_membership_policy_translations` for exact-locale ordered questions/rules;
- `group_membership_policy_revisions` for append-only exact-locale snapshots;
- the submitted application for the exact policy revision, locale, and immutable snapshot
  seen by the candidate.

`PortContext.locale` remains host-owned. Management locale selection is a separate typed
request and never mutates or substitutes the host locale.

### Privacy and enforcement

- public groups expose their localized shell and enabled public features;
- closed groups expose a summary shell but gate body, members, features, and provider
  content behind effective active membership or platform authority;
- secret groups remain undisclosed to unauthorized users;
- effective suspension is not active membership for private content, posting, comments,
  invitation, application, management, or provider ACL decisions;
- expiry is evaluated by the Groups owner clock on reads and writes;
- restoration must not depend on a cleanup job;
- provider unavailability fails closed for private content;
- no transport fallback may bypass an owner denial or timeout.

### Commands and concurrency

Writes require deadline plus idempotency key. Owner services repeat authorization and
invariant checks inside the transaction. State, revision/version, receipt, audit, and
semantic event commit together where declared.

Application policy save and candidate submit use CAS. Candidate cancellation, reopen,
single review, and bounded bulk review retain their declared replay, authorization, and
lock-order contracts. Legacy unconditional Rust application methods remain compatibility-
only and are not exposed by final GraphQL or module-owned FFA.

## Current implementation state

Source exists for:

- module manifest, migrations, RBAC, registration, admin/storefront package registration,
  and generated host composition;
- tenant-scoped groups, translations, memberships, local roles, feature bindings, receipts,
  immutable audit, public/closed/secret access, join/leave, role delegation, and ownership
  transfer;
- bounded token invitations, revocation, token/targeted acceptance, SHA-256-only storage,
  redemption, membership activation, targeted source events, and a neutral Notifications
  source provider;
- localized membership-application policies, exact-locale management, append-only policy
  revisions, immutable candidate snapshots, answer/rule validation, CAS save/submit,
  candidate cancel/resubmit, manager review/reopen, and stale-form recovery;
- focused review and bounded partial-result bulk review with owner, GraphQL, native/GraphQL
  FFA adapters, 50-row selection, confirmation, per-item results, EN/RU/ARIA copy, formal
  FBA registry, and source guard;
- explicit native/GraphQL transport selection with no implicit fallback;
- neutral moderation contracts and sealed adapter/factory registry;
- membership revision and read-only enforcement projection/resolver are source-complete:
  `group_memberships.revision`, bounded `group_membership_enforcements`, database revision
  guards, `GroupMembershipEnforcementReadPort`, and Groups-owner clock evaluation exist;
- current effective-state evaluation distinguishes missing, active, inactive, suspended,
  legacy banned, future, expired, and revoked enforcement without cleanup-worker dependence.

Evidence still open and must not be inferred:

- compilation and executed unit/integration tests;
- PostgreSQL and SQLite migration execution/rollback evidence;
- runtime evidence for membership revision triggers and enforcement owner-clock evaluation;
- native/GraphQL result, stable-code, locale-catalog, and error parity;
- policy history, CAS, lifecycle, bulk-review, replay, lock-order, contention, security,
  retry, recovery, and accessibility execution;
- profile-backed candidate summaries and application lifecycle events/Notifications;
- status-only access-path conversion remains open for core access, invitations,
  applications, provider ACLs, and member-count behavior;
- direct suspend/revoke commands, shared owner mutation path, moderation adapter, and durable
  moderation application orchestration remain open;
- fail-closed remote-provider and disabled-module runtime evidence.

## Program ledger

| ID | Status | Scope | Remaining gate |
|---|---|---|---|
| GROUPS-00 | in_progress | ADR, ownership map, phpFox parity map, FFA/FBA contracts | executable architecture review |
| GROUPS-01 | in_progress | module skeleton, manifest, RBAC, migrations, host composition | build/module-validation evidence |
| GROUPS-02 | in_progress | identity, localization, visibility, join policy, features, receipts/audit/events | lifecycle/runtime/concurrency evidence |
| GROUPS-03 | in_progress | memberships, join/leave, local roles, ownership transfer | enforcement integration and concurrency |
| GROUPS-04 | in_progress | summary, membership, enforcement read, access, localization, invitation, application, governance ports | provider/consumer/fallback runtime matrix |
| GROUPS-05 | in_progress | GraphQL/native transports, discovery, invitation acceptance/delivery | runtime parity and Notifications evidence |
| GROUPS-06 | in_progress | localized application policy, CAS, lifecycle, focused/bulk review, admin/storefront UX | legacy API migration, profiles/events, parity/concurrency/accessibility |
| GROUPS-07 | in_progress | membership revision and read-only enforcement projection/resolver are source-complete; direct commands and adapter remain open | status-only access integration, command/runtime/moderation application evidence |
| GROUPS-08 | planned | dynamic feature-provider registry and navigation | registry/runtime degradation evidence |
| GROUPS-09 | planned | Forum group spaces and ACL inheritance | Forum owner integration evidence |
| GROUPS-10 | planned | Blog and Pages/Wiki group contexts | owner integration/privacy evidence |
| GROUPS-11 | planned | Marketplace/Store seller context and listing composition | seller/checkout evidence |
| GROUPS-12 | planned | Media gallery, avatar/cover, Events and Chat providers | provider lifecycle/degradation evidence |
| GROUPS-13 | in_progress | notifications, search/SEO, neutral moderation compatibility, profiles/media | consumer runtime, adapter, and privacy evidence |
| GROUPS-14 | in_progress | storefront/admin UX and localization | pickers, enforcement UX, accessibility, parity |
| GROUPS-15 | planned | feed/wall aggregation without ownership leakage | feed owner/ranking evidence |
| GROUPS-16 | planned | analytics and operator observability | privacy-safe metrics/evidence |
| GROUPS-17 | planned | import/export, retention, deletion, tenant lifecycle | compliance/recovery evidence |
| GROUPS-18 | planned | remote adapter profile and degraded modes | remote provider/fallback/recovery evidence |
| GROUPS-19 | in_progress | release verification matrix/evidence registry | all open evidence keys resolved |

## GROUPS-06 membership-application contract

Owner tables are `group_membership_policies`, exact-locale translations, append-only policy
revisions, and one current membership application per tenant/group/user.

Published boundaries include application read, policy-history/management reads,
exact-candidate lifecycle read, CAS/lifecycle commands, focused review, and bounded bulk
review. Core invariants remain:

- candidate reads require exact host-resolved locale;
- manager authorization precedes sensitive status disclosure;
- policy writes compare ID/revision/locale under owner locking before state writes;
- stale forms return `groups.application_policy_changed` without owner mutation;
- snapshots preserve exact policy identity, questions/rules, answers, acknowledgements;
- bulk review accepts 1..50 unique IDs, requires confirmation, uses one owner transaction,
  audit, and receipt per item, preserves request order, and returns partial results;
- native and GraphQL bulk paths use the same deadline and no fallback.

Remaining GROUPS-06 work includes removal/versioned deprecation of legacy methods,
Profiles-backed candidate summaries, lifecycle semantic events, richer management UX, and
executed parity/replay/race/security/accessibility evidence.

## GROUPS-07 group enforcement and moderation compatibility contract

### Source-complete foundation

The neutral prerequisite, membership revision, and read-only enforcement
projection/resolver are source-complete:

- `rustok-moderation-api` is persistence-neutral and owns typed effects/adapter contracts;
- `group_memberships.revision` starts at one and is protected from regression;
- lifecycle/role/invitation-field updates bump revision through database guards while legacy
  command owners migrate to a shared explicit owner path;
- enforcement insert/update/delete bumps membership revision in the same transaction;
- `group_membership_enforcements` stores one bounded current row per membership;
- `GroupMembershipEnforcementReadPort` resolves effective state using the Groups UTC clock;
- future, expired, or revoked enforcement falls back to stored lifecycle state;
- legacy `status=banned` remains fail-closed for re-entry;
- no Groups dependency on moderation owner entities/services exists.

The trigger bridge is transitional compatibility infrastructure, not the final command
architecture. Direct enforcement commands and the moderation adapter must use explicit CAS,
receipts, audit, and a shared owner mutation path in later slices.

### Bounded Groups-owned enforcement state

The current owner row contains only:

- tenant, group, user, and membership identity;
- one supported state (`suspended`);
- reason code and source kind (`direct_local` or `moderation_decision`);
- effective start, optional expiry, optional revocation;
- bounded restoration lifecycle status;
- actor identity;
- optional moderation decision ID/hash provenance;
- enforcement revision and timestamps.

It does not contain reports, case notes, policy snapshots, queue assignment, appeal state,
or arbitrary moderation payload JSON.

### Read boundary

`GroupMembershipEnforcementReadPort` is source-complete. It allows the exact user or an
explicit Groups access/read/manage claim to read a tenant-scoped effective state. The result
contains stored lifecycle status, membership revision, effective status, active-member and
denied-reentry booleans, optional bounded provenance, and evaluation time.

The canonical resolver is owner-internal so existing and future access services can converge
without importing persistence details. It validates stored identity, revision, source,
provenance, expiry, restoration state, and actor fields before returning a decision input.

### Open access-path conversion

Status-only access-path conversion remains open. The next atomic slice must switch every
relevant owner path to the canonical resolver:

- `GroupsService::decide_access_owned` and feature visibility;
- join/rejoin and leave lifecycle semantics;
- invitation management and acceptance;
- membership application read/submit/reopen/review;
- governance authorization;
- provider ACL decisions and member count transitions.

Until that conversion lands, no product command writes enforcement rows and no suspension is
claimed end-to-end. The current table and resolver are a migration/read contract, not a
shipped moderation workflow.

### Planned owner command boundary

The next command slice will add:

- `GroupMembershipEnforcementCommandPort` for one direct suspend/revoke operation;
- receipt-first replay and changed-hash conflict behavior;
- expected membership revision CAS;
- owner/admin/moderator hierarchy and owner protection;
- one shared transaction path for direct actions and the neutral moderation adapter;
- membership revision, enforcement row, member count/group version, audit, event, and receipt
  atomicity;
- explicit restoration semantics on revoke/expiry rather than unconditional activation.

No bulk command is introduced before single-command runtime evidence.

### Planned moderation adapter

Initial mapping remains:

- `GroupMembership` + `SuspendSubject { effective_until }` maps to the shared Groups owner
  suspension command;
- identical decision ID/hash replays before subject reads; changed hash conflicts;
- expected membership revision is checked inside the owner transaction;
- unsupported effect/version/scope/kind fails without mutation;
- group-level effects require a separate declared matrix;
- account sanctions are never applied by Groups;
- moderation records applied evidence only after a matching adapter result.

### GROUPS-07 definition of done

- no Groups dependency on moderation owner crate and no moderation writes/FKs into Groups;
- exact membership identity and monotonic revision evidence;
- permanent/expiring/revoked enforcement across every owner access path;
- hierarchy, owner protection, tenant isolation, replay, changed-hash, stale revision,
  expiry, revoke, member-count, and concurrency evidence;
- missing/timeout/retry/lost-response adapter behavior;
- moderation-disabled mode preserves existing Groups enforcement without inventing cases;
- native/GraphQL parity for state/direct actions;
- separate moderation and Groups FFA ownership;
- PostgreSQL/SQLite migration, compatibility, accessibility, and no-fallback evidence.

## Feature-provider integration order

1. `forum.discussions` — Forum-owned space/category, access through Groups ports.
2. `blog.posts` — Blog-owned group-context posts and CommentsThreadPort.
3. `pages.wiki` — Pages-owned documents/Page Builder artifacts.
4. `marketplace.store` — Marketplace ownership with Commerce checkout/order unchanged.
5. `media.gallery`, `events.calendar`, and `chat.room` — provider-owned lifecycle/UI.

Feature bindings express configuration only. They never transfer persistence ownership and
Groups never embeds another module's business UI directly.

## Degraded modes

- Groups access unavailable: deny private content.
- Enforcement row corrupt or unsupported: return invariant failure; never infer active.
- Expired/revoked enforcement: evaluate immediately from owner clock and stored lifecycle;
  cleanup is optional normalization only.
- Legacy banned membership: deny re-entry until explicitly migrated/reviewed.
- Candidate exact-locale policy unavailable: form unavailable; never choose another locale.
- Policy CAS conflict: write no owner state and require explicit reload.
- Profiles unavailable: show UUID/placeholder; never copy canonical profile fields.
- Notifications unavailable: Groups command succeeds and owner state remains truth.
- Moderation disabled: existing Groups enforcement remains active; moderation-driven
  application is unavailable; configured direct Groups actions may remain available later.
- Moderation unavailable after a decision: no Groups mutation is inferred.
- Groups adapter unavailable: moderation must not mark a decision applied.
- Unknown effect, legacy `effect: None`, or stale revision: reject without Groups mutation.
- Search/index unavailable: owner writes succeed and projections catch up later.

## Verification matrix

Required before affected statuses become `done`:

```bash
cargo xtask module validate groups
cargo check -p rustok-moderation-api
cargo check -p rustok-groups --features graphql
cargo check -p rustok-groups-admin --features ssr
cargo check -p rustok-groups-storefront --features ssr
cargo test -p rustok-groups
node scripts/verify/verify-moderation-api-boundary.mjs
node scripts/verify/verify-groups-boundary.mjs
node scripts/verify/verify-groups-localization-boundary.mjs
node scripts/verify/verify-groups-invitations-boundary.mjs
node scripts/verify/verify-groups-invitation-acceptance-ui.mjs
node scripts/verify/verify-groups-targeted-invitation-delivery.mjs
node scripts/verify/verify-groups-membership-applications.mjs
node scripts/verify/verify-groups-membership-policy-revisions.mjs
node scripts/verify/verify-groups-application-policy-cas.mjs
node scripts/verify/verify-groups-application-lifecycle.mjs
node scripts/verify/verify-groups-application-policy-locales.mjs
node scripts/verify/verify-groups-application-bulk-review.mjs
node scripts/verify/verify-groups-membership-enforcement-read-path.mjs
node scripts/verify/verify-db-multilingual-contract.mjs
npm run verify:i18n:ui
npm run verify:frontend:host-ffa-contract
```

Additional GROUPS-07 evidence remains open for clean/upgraded migration and rollback,
revision trigger behavior, owner-clock expiry/revoke evaluation, complete access integration,
direct/moderation replay and conflicts, concurrency, enabled/disabled runtime matrices,
separate FFA ownership, and accessibility.
