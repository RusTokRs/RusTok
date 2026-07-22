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
FFA/FBA status, integration gates, and release evidence. Do not create parallel group
roadmaps, phpFox parity documents, remediation plans, or duplicated task ledgers. Issues
and pull requests are execution records only.

Every change that modifies Groups behavior must update this plan in the same change:
task status, remaining scope, definition of done, verification evidence, and degraded-
mode notes.

## Scope

Build phpFox-class social groups as modular micro-social networks while preserving
RusToK ownership boundaries:

- public, closed, and secret groups;
- categories, stable handles, localized presentation, media references, and SEO;
- open join, application, invitation, group-local enforcement, ownership transfer, and
  local-role workflows;
- localized group rules and membership questions;
- owner/admin/moderator/member permissions;
- provider-owned Wall, Forum, Blog, Pages/Wiki, Media, Events, Marketplace, and Chat
  sections;
- visibility-aware search, notifications, moderation compatibility, feed, and analytics;
- module-owned admin/storefront FFA packages;
- in-process and remote-ready FBA boundaries with fail-closed privacy.

## Status vocabulary

- `planned`: contract or implementation is not yet source-complete.
- `in_progress`: useful source exists, but one or more required runtime, parity,
  concurrency, security, accessibility, migration, or degraded-mode gates remain open.
- `done`: implementation and every declared gate have executable evidence.
- `blocked`: an external owner capability is required before work can continue safely.

Source presence alone never promotes FFA or FBA readiness to `done`.

## Architectural invariants

### Ownership

Groups owns group identity, localized group presentation, memberships, local roles,
join policy, invitations, membership applications, rules/questions, authoritative group-
local membership/access enforcement state, feature bindings, command receipts, domain
audit, and Groups semantic events.

Groups does not own profile presentation, media binaries, forum topics, blog posts,
Pages documents, marketplace listings, products, comments, notification inboxes, search
documents, feed entries, checkout, payment, orders, fulfillment, moderation reports,
moderation cases, moderation policies, immutable moderation decisions, decision-
application scheduling, appeals, or cross-domain moderation audit history.

Optional modules never receive database foreign keys from Groups. Cross-module
composition uses typed identifiers, neutral typed ports, semantic events, and host-owned
UI/runtime composition.

### Moderation compatibility

`rustok-moderation` is the owner of the moderation workflow. Groups remains the owner of
the group and membership mutation produced by an applicable moderation decision.

Compatibility must use a neutral `rustok-moderation-api` rather than making Groups depend
on the persistence owner crate. The neutral API must own the subject/scope types, typed
and versioned decision effects, `ApplyModerationDecisionCommand`,
`ModerationDecisionApplication`, `ModerationSubjectCommandPort`, and the host-composed
subject-adapter registry.

Groups integration identity is fixed as follows:

- group subject: `module="groups"`, kind `Group`, ID `groups.id`, revision
  `groups.version`;
- membership subject: `module="groups"`, kind `GroupMembership`, ID
  `group_memberships.id`, revision a monotonic `group_memberships.revision`;
- group-local moderation scope: kind `Group`, ID `group_id`;
- membership subject revision changes whenever role, lifecycle status, or effective
  enforcement state changes.

`group.version` is not a substitute for membership revision because unrelated group
writes would create false stale conflicts. `updated_at` is not a revision contract.

The Groups adapter validates tenant, scope, subject identity, subject revision, decision
hash, typed effect version, effect compatibility, role hierarchy, and owner invariants
inside the Groups transaction. A stale revision fails with a stable conflict and never
retargets the decision to the latest membership state.

The moderation admin queue and case UI belong to the moderation module. Groups FFA may
show current group-local enforcement state, provenance, and authorized direct domain
actions, but it must not implement reports, cases, policy snapshots, appeals, or a second
moderation queue.

Direct group-local actions and moderation-driven actions converge on the same Groups
owner mutation path. Whether an authorized direct action also opens a moderation case is
host/product policy; it must not create duplicate persistence in Groups.

### Multilingual storage and locale ownership

Language-neutral state belongs to base tables. Canonical localized business copy belongs
to exact-locale translation tables. There is no English, first-row, or module-local
fallback.

Membership-application policies use:

- `group_membership_policies` for language-neutral revision/enabled state;
- `group_membership_policy_translations` for ordered bounded questions and rules by exact
  locale;
- `group_membership_policy_revisions` for append-only exact-locale snapshots of
  successful policy writes;
- the submitted application for the exact policy revision, locale, and immutable
  question/rule snapshot seen by the candidate.

Candidate policy reads remain bound to the host-resolved effective locale carried in
`PortContext.locale`. Management selection is separate:

- `PortContext.locale` remains the host UI/request locale;
- `GroupApplicationPolicyManagementReadPort` carries the selected exact locale in a typed
  request;
- the owner selects only the matching translation;
- a missing policy returns an empty view without a CAS precondition;
- a missing translation for an existing policy returns an empty view with current policy
  ID/revision and selected locale;
- no management read substitutes another locale or mutates host locale context.

### Privacy and enforcement

- public groups expose their localized shell and enabled public features;
- closed groups expose a localized summary shell but gate body, members, features, and
  provider content behind effective active membership or platform authority;
- secret groups are undisclosed to non-members and cannot be reached through public
  discovery or the membership-application flow;
- an effective membership suspension is not an active membership for private-content,
  post, comment, invitation, application, or provider ACL decisions;
- expiry is evaluated by the Groups owner clock on reads and writes; access restoration
  must not depend on a background cleanup job;
- provider unavailability fails closed for private content;
- no transport fallback may bypass an owner denial or timeout.

### Commands and concurrency

Writes require deadline plus idempotency key. Owner services repeat authorization and
invariant checks inside the transaction. Successful state, relevant revision/version,
receipt, and audit commit together where the contract declares them.

Interactive policy save and candidate submit use `GroupApplicationCasCommandPort` with
policy ID, revision, and exact locale. Existing applications lock application then group;
first submit locks group and repeats the candidate lookup before writes. Receipt replay
occurs before policy precondition re-evaluation.

Candidate cancellation and manager reopen use `GroupApplicationLifecycleCommandPort`.
They replay receipts before locks, lock application then group, and commit application,
membership, group version, audit, and receipt atomically. Manager authorization occurs
before status disclosure.

Single and bounded bulk application review use the focused review owner path. Bulk review
accepts 1..50 unique application IDs, requires confirmation, derives order-independent
per-item idempotency keys, and returns one result per item; it is not one cross-item
transaction.

The older unconditional methods on `GroupApplicationCommandPort` remain public only for
Rust source compatibility. Final GraphQL and module-owned FFA do not expose or use the
legacy policy-save/candidate-submit paths. Their removal/versioned deprecation remains a
separate API migration gate.

## Current implementation state

The following source exists:

- module manifest, migrations, RBAC, registration, admin/storefront package registration,
  and generated host composition;
- tenant-scoped groups, translations, memberships, local roles, feature bindings,
  receipts, immutable audit, public/closed/secret access, join/leave, role delegation,
  and ownership transfer;
- bounded token invitations, revocation, token and targeted acceptance, SHA-256-only
  storage, redemption, membership activation, targeted source events, and a neutral
  Notifications source provider;
- localized membership-application policies, exact-locale management, append-only policy
  revisions, immutable candidate snapshots, answer/rule validation, CAS save/submit,
  candidate status/cancel/resubmit, manager review/reopen, and stale-form recovery;
- focused `GroupApplicationReviewCommandPort` and final no-bypass GraphQL/native
  composition;
- bounded partial-result bulk application review with owner, GraphQL, native/GraphQL FFA
  adapters, 50-row selection, confirmation, per-item results, EN/RU/ARIA copy, formal FBA
  registry, and static guard;
- explicit native/GraphQL transport selection with no implicit fallback;
- EN/RU copy, live module contracts, and focused source guards.

The following evidence remains open and must not be inferred:

- compilation and executed unit/integration tests;
- PostgreSQL and SQLite migration execution/rollback evidence;
- native/GraphQL result, stable-code, locale-catalog, and error parity;
- policy history, CAS, lifecycle, bulk-review, replay, lock-order, contention, security,
  retry, recovery, and accessibility execution;
- removal/versioned deprecation of legacy unconditional Rust methods;
- profile-backed candidate summaries and application lifecycle events/Notifications
  consumer behavior;
- neutral moderation API, Groups membership revision, enforcement persistence, and
  moderation adapter implementation/evidence;
- fail-closed remote-provider and disabled-module runtime evidence.

## Program ledger

| ID | Status | Scope | Remaining gate |
|---|---|---|---|
| GROUPS-00 | in_progress | ADR, ownership map, phpFox parity map, FFA/FBA contracts | executable architecture review |
| GROUPS-01 | in_progress | module skeleton, manifest, RBAC, migrations, host composition | build/module-validation evidence |
| GROUPS-02 | in_progress | group identity, localized presentation, visibility, join policy, feature bindings, receipts/audit, targeted source events | lifecycle/runtime/concurrency evidence |
| GROUPS-03 | in_progress | memberships, join/leave, local roles, ownership transfer | local enforcement and concurrency completion |
| GROUPS-04 | in_progress | summary, membership, access, localization, invitation, application, policy-history/management, CAS/lifecycle/review/bulk-review, governance ports | provider/consumer/fallback runtime matrix |
| GROUPS-05 | in_progress | GraphQL/native transports, storefront discovery, invitation acceptance/delivery source | runtime parity and Notifications consumer evidence |
| GROUPS-06 | in_progress | localized application policy, CAS, lifecycle, focused and bounded bulk review, visual admin/storefront UX | legacy Rust-port migration, profiles/events, runtime parity/concurrency/accessibility evidence |
| GROUPS-07 | blocked | group-local enforcement state, suspension/expiry/reason, direct owner commands, neutral moderation subject adapter | `rustok-moderation-api`, typed decision effect, subject registry, and membership revision contract |
| GROUPS-08 | planned | dynamic feature-provider registry and group navigation | registry/runtime degradation evidence |
| GROUPS-09 | planned | Forum group spaces and ACL inheritance | Forum owner integration evidence |
| GROUPS-10 | planned | Blog and Pages/Wiki group contexts | owner integration and privacy evidence |
| GROUPS-11 | planned | Marketplace/Store seller context and listing composition | seller/checkout boundary evidence |
| GROUPS-12 | planned | Media gallery, avatar/cover, Events and Chat providers | provider lifecycle/degradation evidence |
| GROUPS-13 | in_progress | notifications, search/SEO, neutral moderation compatibility, profiles/media integration | consumer runtime, neutral API, and privacy evidence |
| GROUPS-14 | in_progress | storefront/admin UX, localization, loading/error/success states | profile/group pickers, enforcement state UX, accessibility, parity |
| GROUPS-15 | planned | group feed/wall aggregation without ownership leakage | feed owner contract and ranking evidence |
| GROUPS-16 | planned | analytics and operator observability | privacy-safe metrics/evidence |
| GROUPS-17 | planned | import/export, retention, deletion, tenant lifecycle | compliance/recovery evidence |
| GROUPS-18 | planned | remote adapter profile and degraded modes | remote provider/fallback/recovery evidence |
| GROUPS-19 | in_progress | release verification matrix and evidence registry | all open evidence keys resolved |

## GROUPS-06 membership-application contract

### Owner state and ports

Owner tables are `group_membership_policies`, exact-locale
`group_membership_policy_translations`, append-only
`group_membership_policy_revisions`, and one current
`group_membership_applications` row per tenant/group/user.

Published boundaries include application read, policy-history read, policy-management
read, exact-candidate lifecycle read, CAS command, lifecycle command, focused review
command, and bounded bulk-review command ports.

The legacy unconditional policy-save and candidate-submit methods remain compatibility-
only; final GraphQL and module-owned FFA do not use them.

### Core invariants

- candidate policy reads require the exact host-resolved locale;
- manager catalog/read uses active owner/admin or platform authority;
- policy writes compare ID/revision/locale under owner locking before any state write;
- stale forms return `groups.application_policy_changed` with no membership,
  application, audit, version, or receipt mutation;
- application snapshots preserve exact policy identity, questions/rules, answers, and
  acknowledgements;
- cancel is exact-candidate pending-only and moves membership to `left`;
- reopen is manager-authorized rejected/cancelled-only, authorizes before status
  disclosure, preserves the submitted snapshot, and moves membership to `pending`;
- fresh resubmit is a distinct CAS command and replaces the snapshot only after success;
- review authorizes before pending-status disclosure; approve activates membership and
  increments member count, reject moves membership to `left`;
- review note is bounded to 2,000 characters;
- bounded bulk review requires 1..50 unique IDs and confirmation, validates its envelope
  before item writes, uses one owner transaction/audit/receipt per item, preserves request
  order, and exposes partial per-item results;
- native and GraphQL bulk paths use the same 30-second owner deadline and no fallback.

### FFA state

Admin policy-locale/history/review/reopen and bulk-review workspaces, native and GraphQL
adapters, explicit transport facade, storefront lifecycle/stale recovery, and EN/RU copy
exist at source level. Runtime parity, focus restoration, keyboard, screen-reader,
concurrency, replay, and recovery evidence remains open.

### GROUPS-06 remaining work

- remove or version-deprecate legacy unconditional Rust policy save/submit methods after
  external consumers migrate;
- profile-backed candidate summaries through a Profiles read port without copied state;
- submitted/reviewed/cancelled/reopened semantic events and optional Notifications
  consumer;
- richer operator filtering, pagination controls, group/member pickers, and audit/receipt
  history;
- locale translation deletion/lifecycle policy if required;
- executed parity, replay, stale-race, locale-creation, lifecycle/bulk races, lock-order,
  migration, security, retry, recovery, keyboard, focus, and screen-reader evidence.

## GROUPS-07 group enforcement and moderation compatibility contract

### Dependency gate

Do not implement a Groups adapter against `rustok-moderation` owner entities/services.
Before adapter work:

1. extract `rustok-moderation-api`;
2. add typed/versioned decision effects including suspension expiry;
3. add a host subject-adapter registry keyed by `(module, subject_kind)`;
4. retain owner-crate re-exports only as a temporary compatibility bridge.

Groups local enforcement persistence may be designed in parallel, but `GROUPS-07` remains
`blocked` until the shared identity/effect contract is fixed.

### Groups-owned enforcement state

Add a monotonic `revision` to `group_memberships`. Every role, lifecycle, or effective
enforcement mutation increments it atomically.

Add a tenant-scoped current enforcement record for membership suspension/ban state with
at least:

- membership/group/user identity;
- enforcement revision and state;
- reason code;
- effective start and optional expiry;
- source kind (`direct_local` or `moderation_decision`);
- optional moderation decision ID and decision hash;
- actor identity and created/updated/revoked timestamps.

This is domain enforcement state, not a copy of the moderation case. Do not persist case
notes, reports, policy snapshots, appeal state, queue assignment, or arbitrary moderation
JSON in Groups.

`group_memberships.status = banned` is legacy compatibility state, not sufficient for
expiring enforcement. Before temporary suspension ships, every Groups join/access/
invitation/application/provider-ACL path must evaluate the effective enforcement record
and owner clock rather than status alone. Expired enforcement restores eligibility
without requiring a cleanup worker; lazy normalization may repair legacy projection.

### Groups owner ports

Plan focused boundaries:

- `GroupMembershipEnforcementReadPort` for authorized current-state reads and effective
  subject evaluation;
- `GroupMembershipEnforcementCommandPort` for single direct suspend/revoke commands;
- an internal shared owner command used by direct actions and the neutral moderation
  adapter;
- a Groups implementation of `ModerationSubjectCommandPort` from
  `rustok-moderation-api`.

Direct local commands remain Groups domain commands with deadline/idempotency, hierarchy
checks, domain receipt/audit, membership revision, and group version. They do not create
moderation cases inside Groups.

### Subject adapter mapping

Initial supported moderation application:

- subject kind `GroupMembership` + typed `SuspendSubject { effective_until }` maps to the
  Groups suspension owner command;
- an identical decision ID/hash replays before subject reads;
- changed hash conflicts;
- expected membership revision is checked inside the transaction;
- unsupported kinds/effect versions fail validation without mutation;
- group-level `Group` decisions require a separately declared effect matrix and must not
  be inferred from membership behavior;
- `AccountSanctionRecommended` is not applied by Groups;
- `NoViolation`, escalation, and other moderation-only outcomes should not be dispatched
  as Groups mutations.

The returned `ModerationDecisionApplication` must match decision ID, subject identity,
and the resulting membership revision. Moderation records applied evidence only after
valid adapter success.

### Authorization and hierarchy

For direct local enforcement:

- platform `groups:manage` may act across local hierarchy;
- owner may suspend non-owner memberships;
- admin may suspend moderator/member but not owner/admin peers unless policy explicitly
  grants it;
- moderator may suspend members only;
- self-suspension and owner suspension are rejected;
- authorization is repeated after membership/group locks;
- stale role or subject revision fails atomically.

Moderation-driven application does not trust the moderation actor as a local member. It
uses a service actor/capability granted through host composition, while Groups still
validates subject/scope/effect and immutable decision provenance.

### Access and lifecycle behavior

An effective suspension:

- removes active-member treatment for private group content and member/provider ACLs;
- denies join, application submit/reopen, invitation acceptance, post, comment, invite,
  and local moderation/settings actions;
- preserves public summary behavior for public/closed discovery unless product policy
  explicitly hides it;
- never reveals secret-group existence to an unauthorized suspended actor;
- does not silently delete membership/application/audit history;
- decrements member count exactly once if the membership was active and restores count
  only through an explicit valid reactivation path.

Revocation/expiry must define the resulting membership lifecycle state explicitly rather
than always restoring `active`; the pre-enforcement state or a bounded restoration policy
must be recorded and validated.

### Locking, replay, and bulk ownership

Single enforcement commands replay receipt first, then lock membership, group, and
current enforcement state in one declared order. Membership revision, enforcement state,
group version/member count, domain audit, semantic event, and receipt commit atomically.

Cross-domain bulk moderation belongs to `rustok-moderation`, which owns cases, decisions,
application jobs, retry, and per-subject outcomes. The Groups moderation adapter remains a
single-subject owner boundary. A future direct local Groups bulk action, if required, is a
separate bounded domain command and must not duplicate moderation cases or application
jobs.

### UI ownership

- moderation admin FFA owns report queue, cases, decisions, application/retry, and appeals;
- Groups admin FFA owns current group-local enforcement state, expiry/provenance display,
  and authorized direct suspend/revoke controls;
- host composition may deep-link between a moderation case and the Groups membership
  state using typed IDs;
- neither UI imports the other owner's persistence or business component tree.

### Implementation order

1. neutral moderation API, typed effect, registry, and compatibility re-exports;
2. membership revision migration and owner read-path conversion away from status-only
   ban checks;
3. current enforcement persistence plus single direct suspend/revoke commands;
4. join/access/invitation/application/provider ACL and member-count integration;
5. neutral Groups subject adapter and durable moderation application integration;
6. Groups state/direct-action FFA and moderation queue/case FFA host composition;
7. optional direct local bulk command only after single-command runtime evidence.

### GROUPS-07 definition of done

- no Groups dependency on the moderation owner crate;
- no moderation direct writes or foreign keys into Groups tables;
- exact group/membership subject identity and monotonic revision evidence;
- permanent and expiring suspension behavior across every owner access path;
- role hierarchy, owner protection, tenant isolation, replay, changed-hash, stale revision,
  expiry, revoke, member-count, and concurrency evidence;
- moderation adapter missing/timeout/retry/lost-response behavior;
- moderation-disabled mode preserves existing Groups enforcement and configured direct
  local actions without inventing cases;
- native/GraphQL result/error parity for Groups state/direct actions;
- moderation queue/case UI and Groups enforcement UI remain separately owned;
- PostgreSQL/SQLite migration and rollback/compatibility evidence;
- accessibility and no-fallback evidence.

## Other open Groups contracts

Localization remains `in_progress`: localization receipt/replay, last-translation delete
rejection execution, and native/GraphQL concurrency evidence remain open.

Targeted invitation delivery remains `in_progress`: Groups emits
`groups.invitation.targeted_created`, exposes the targeted command port, and registers a
neutral source provider, while Notifications runtime/fan-out/retry/recovery evidence is
open.

## Feature-provider integration order

1. `forum.discussions` — Forum-owned space/category, access through Groups ports.
2. `blog.posts` — Blog-owned group-context posts and CommentsThreadPort.
3. `pages.wiki` — Pages-owned documents and Page Builder artifacts with typed group
   context.
4. `marketplace.store` — Marketplace seller/listing ownership with Commerce checkout and
   order ownership unchanged.
5. `media.gallery`, `events.calendar`, and `chat.room` — provider-owned lifecycle and UI
   contributions.

A feature binding expresses policy/configuration only. It never transfers persistence
ownership and Groups never embeds another module's business UI directly.

## Degraded modes

- Groups access provider unavailable: deny private content.
- Candidate exact-locale policy unavailable: application form unavailable; never select
  another locale.
- Management locale catalog unavailable: disable selection/save; do not infer locale
  rows.
- Policy CAS conflict: write no owner state and require explicit reload.
- Application lifecycle/bulk transport failure: preserve selected-path error and never
  retry through another transport.
- Profiles unavailable: show UUID/placeholder; never copy canonical profile fields.
- Notifications unavailable: Groups command succeeds and owner state remains truth.
- Moderation module disabled: existing Groups enforcement remains active; reporting,
  cases, appeals, and moderation-driven application are unavailable; configured direct
  local Groups actions may remain available.
- Moderation unavailable after a decision: no Groups mutation is inferred; application
  remains pending/retryable in moderation.
- Groups adapter/owner unavailable: moderation must not mark a decision applied.
- Unknown moderation effect or stale subject revision: reject without Groups mutation.
- Expired local enforcement: evaluate as inactive immediately even if legacy status
  projection has not been normalized.
- Search/index unavailable: owner writes succeed and projections may catch up later.

## Verification matrix

Required before affected statuses become `done`:

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
node scripts/verify/verify-groups-application-policy-locales.mjs
node scripts/verify/verify-groups-application-bulk-review.mjs
node scripts/verify/verify-db-multilingual-contract.mjs
npm run verify:i18n:ui
npm run verify:frontend:host-ffa-contract
```

Additional `GROUPS-06` evidence includes exact-locale management, policy history,
CAS/lifecycle/bulk replay and races, lock order, parity, no-fallback, EN/RU, and
accessibility execution.

Additional `GROUPS-07` evidence will include:

- neutral API dependency guards and adapter-registry duplicate/missing behavior;
- typed effect expiry/version/hash tests;
- clean and upgraded membership-revision/enforcement migrations;
- permanent/temporary/revoked/expired enforcement across join, access, invitations,
  applications, provider ACLs, and member count;
- direct local and moderation-driven replay, changed-hash, stale-revision, hierarchy,
  tenant-isolation, concurrency, retry, and lost-response behavior;
- moderation-enabled/disabled and Groups-adapter available/unavailable runtime matrices;
- separate moderation and Groups FFA ownership plus accessibility execution.
