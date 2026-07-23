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
FFA/FBA status, integration gates, and release evidence. Do not create parallel group
roadmaps, phpFox parity documents, remediation plans, or duplicated task ledgers. Issues
and pull requests are execution records only.

Every change that modifies Groups behavior must update this plan in the same change: task
status, remaining scope, definition of done, verification evidence, and degraded-mode notes.

## Scope

Build phpFox-class social groups as modular micro-social networks while preserving RusToK
ownership boundaries:

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

Groups owns group identity, localized group presentation, memberships, local roles, join
policy, invitations, membership applications, rules/questions, authoritative group-local
membership/access enforcement state, feature bindings, command receipts, domain audit, and
Groups semantic events.

Groups does not own profile presentation, media binaries, forum topics, blog posts, Pages
documents, marketplace listings, products, comments, notification inboxes, search
documents, feed entries, checkout, payment, orders, fulfillment, moderation reports,
moderation cases, moderation policies, immutable moderation decisions,
decision-application scheduling, appeals, or cross-domain moderation audit history.

Optional modules never receive database foreign keys from Groups. Cross-module composition
uses typed identifiers, neutral typed ports, semantic events, and host-owned UI/runtime
composition.

### Moderation compatibility

`rustok-moderation` owns the moderation workflow. Groups remains authoritative for the group
or membership mutation produced by an applicable moderation decision.

Compatibility uses `rustok-moderation-api`; Groups must never depend on moderation entities,
migrations, or services. The neutral source now provides subject/scope types, typed and
versioned decision effects, `ApplyModerationDecisionCommand`,
`ModerationDecisionApplication`, `ModerationSubjectCommandPort`, and the host-composed
adapter/factory registry. `rustok-moderation` retains temporary source-compatible
re-exports.

Groups integration identity is fixed:

- group subject: `module="groups"`, kind `Group`, ID `groups.id`, revision
  `groups.version`;
- membership subject: `module="groups"`, kind `GroupMembership`, ID
  `group_memberships.id`, revision a monotonic `group_memberships.revision`;
- group-local moderation scope: kind `Group`, ID `group_id`;
- membership revision changes whenever role, lifecycle status, or effective enforcement
  state changes.

`group.version` is not a substitute for membership revision because unrelated group writes
would create false stale conflicts. `updated_at` is not a revision contract.

The Groups adapter validates tenant, scope, subject identity/revision, decision hash, typed
effect version and compatibility, role hierarchy, and owner invariants inside the Groups
transaction. A stale revision fails with a stable conflict and never retargets a decision to
the latest membership state.

The moderation admin queue/case/application UI belongs to moderation. Groups FFA may show
current local enforcement state, provenance, and authorized direct actions, but it must not
implement reports, cases, policy snapshots, appeals, or a second moderation queue.

Direct group-local and moderation-driven actions converge on the same Groups owner mutation
path. Whether a direct action also opens a moderation case is host/product policy and must
not create duplicate persistence in Groups.

### Multilingual storage and locale ownership

Language-neutral state belongs to base tables. Canonical localized business copy belongs to
exact-locale translation tables. There is no English, first-row, or module-local fallback.

Membership-application policies use:

- `group_membership_policies` for language-neutral revision/enabled state;
- `group_membership_policy_translations` for ordered bounded questions and rules by exact
  locale;
- `group_membership_policy_revisions` for append-only exact-locale snapshots;
- the submitted application for the exact policy revision, locale, and immutable
  question/rule snapshot seen by the candidate.

Candidate policy reads remain bound to the host-resolved effective locale in
`PortContext.locale`. Management selection is separate:

- `PortContext.locale` remains the host UI/request locale;
- `GroupApplicationPolicyManagementReadPort` carries the selected exact locale;
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
- an effective membership suspension is not active membership for private content, post,
  comment, invitation, application, or provider ACL decisions;
- expiry is evaluated by the Groups owner clock on reads and writes; restoration must not
  depend on a cleanup job;
- provider unavailability fails closed for private content;
- no transport fallback may bypass an owner denial or timeout.

### Commands and concurrency

Writes require deadline plus idempotency key. Owner services repeat authorization and
invariant checks inside the transaction. Successful state, relevant revision/version,
receipt, and audit commit together where declared.

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
legacy policy-save/candidate-submit paths. Their removal/versioned deprecation remains an
API migration gate.

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
  candidate status/cancel/resubmit, manager review/reopen, and stale-form recovery;
- focused review and bounded partial-result bulk review with owner, GraphQL, native/GraphQL
  FFA adapters, 50-row selection, confirmation, per-item results, EN/RU/ARIA copy, formal
  FBA registry, and static guard;
- explicit native/GraphQL transport selection with no implicit fallback;
- neutral moderation subject/scope/effect/application contracts and sealed adapter/factory
  registry in `rustok-moderation-api`; Groups implementation has not started;
- EN/RU copy, live module contracts, and focused source guards.

Evidence still open and must not be inferred:

- compilation and executed unit/integration tests;
- PostgreSQL and SQLite migration execution/rollback evidence;
- native/GraphQL result, stable-code, locale-catalog, and error parity;
- policy history, CAS, lifecycle, bulk-review, replay, lock-order, contention, security,
  retry, recovery, and accessibility execution;
- removal/versioned deprecation of legacy unconditional Rust methods;
- profile-backed candidate summaries and application lifecycle events/Notifications;
- Groups membership revision, enforcement persistence/read-path conversion, direct
  commands, and moderation adapter;
- durable moderation application orchestration and composed adapter materialization;
- fail-closed remote-provider and disabled-module runtime evidence.

## Program ledger

| ID | Status | Scope | Remaining gate |
|---|---|---|---|
| GROUPS-00 | in_progress | ADR, ownership map, phpFox parity map, FFA/FBA contracts | executable architecture review |
| GROUPS-01 | in_progress | module skeleton, manifest, RBAC, migrations, host composition | build/module-validation evidence |
| GROUPS-02 | in_progress | identity, localized presentation, visibility, join policy, features, receipts/audit/events | lifecycle/runtime/concurrency evidence |
| GROUPS-03 | in_progress | memberships, join/leave, local roles, ownership transfer | local enforcement and concurrency completion |
| GROUPS-04 | in_progress | summary, membership, access, localization, invitation, application, CAS/lifecycle/review/bulk/governance ports | provider/consumer/fallback runtime matrix |
| GROUPS-05 | in_progress | GraphQL/native transports, discovery, invitation acceptance/delivery | runtime parity and Notifications evidence |
| GROUPS-06 | in_progress | localized application policy, CAS, lifecycle, focused/bulk review, admin/storefront UX | legacy API migration, profiles/events, parity/concurrency/accessibility |
| GROUPS-07 | planned | membership revision, effective enforcement state, suspension/expiry/reason, direct commands, neutral moderation adapter | Groups owner migration/read-path and moderation application runtime |
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

Owner tables are `group_membership_policies`, exact-locale
`group_membership_policy_translations`, append-only
`group_membership_policy_revisions`, and one current
`group_membership_applications` row per tenant/group/user.

Published boundaries include application read, policy-history/management reads,
exact-candidate lifecycle read, CAS/lifecycle commands, focused review, and bounded bulk
review. Legacy unconditional policy-save/candidate-submit methods are compatibility-only.

Core invariants:

- candidate policy reads require exact host-resolved locale;
- management authorization and locale selection are owner-validated;
- policy writes compare ID/revision/locale under locking before any state write;
- stale forms return `groups.application_policy_changed` without owner mutation;
- snapshots preserve exact policy identity, questions/rules, answers, acknowledgements;
- cancel is exact-candidate pending-only; reopen authorizes before status disclosure;
- review authorizes before pending-status disclosure;
- bulk review requires 1..50 unique IDs and confirmation, validates envelope before item
  writes, uses one transaction/audit/receipt per item, preserves request order, and returns
  partial per-item results;
- native and GraphQL bulk paths use the same 30-second deadline and no fallback.

Admin policy/history/review/reopen/bulk workspaces, explicit transports, storefront
lifecycle/stale recovery, and EN/RU copy exist at source level. Runtime parity, focus,
keyboard, screen-reader, concurrency, replay, and recovery evidence remains open.

Remaining GROUPS-06 work:

- remove/version-deprecate legacy unconditional Rust methods after consumers migrate;
- Profiles-backed candidate summaries without copied profile state;
- submitted/reviewed/cancelled/reopened semantic events and optional Notifications consumer;
- richer filtering, pagination, pickers, audit/receipt history;
- locale translation deletion/lifecycle policy if required;
- executed parity, replay, race, lock-order, migration, security, retry, recovery, and
  accessibility evidence.

## GROUPS-07 group enforcement and moderation compatibility contract

### Dependency gate

The neutral prerequisite is source-complete:

- `rustok-moderation-api` exists without owner persistence dependencies;
- typed/versioned decision effects include `SuspendSubject { effective_until }`;
- adapter/factory registries are keyed by validated `(module, subject_kind)` identity;
- `rustok-moderation` temporarily re-exports moved contracts;
- new moderation decisions bind typed effects into request identity, immutable hash, and
  owner persistence; historical decisions remain truthful `effect: None`.

`GROUPS-07` is no longer blocked by contract extraction. It remains `planned` until Groups
adds membership revision and converts every status-only ban check to effective owner-state
evaluation. End-to-end moderation-driven application additionally depends on durable
moderation application orchestration and host registry materialization.

### Groups-owned enforcement state

Add monotonic `revision` to `group_memberships`. Every role, lifecycle, or effective
enforcement mutation increments it atomically.

Add a tenant-scoped current enforcement record containing at least:

- membership/group/user identity;
- enforcement revision/state;
- reason code;
- effective start and optional expiry;
- source kind (`direct_local` or `moderation_decision`);
- optional moderation decision ID/hash;
- actor identity and created/updated/revoked timestamps;
- bounded pre-enforcement lifecycle/restoration state.

This is domain enforcement state, not a copied moderation case. Do not persist reports,
case notes, policy snapshots, appeal state, queue assignment, or arbitrary moderation JSON.

`group_memberships.status = banned` is legacy compatibility state and is insufficient for
expiring enforcement. Before temporary suspension ships, every join/access/invitation/
application/provider-ACL path must evaluate effective enforcement and owner clock. Expiry
restores eligibility without requiring cleanup; lazy normalization may repair legacy
projection.

### Groups owner ports

Plan focused boundaries:

- `GroupMembershipEnforcementReadPort` for authorized current state and effective subject
  evaluation;
- `GroupMembershipEnforcementCommandPort` for single direct suspend/revoke commands;
- one internal owner mutation path shared by direct actions and the moderation adapter;
- a Groups `ModerationSubjectCommandPort` implementation from `rustok-moderation-api`.

Direct commands remain Groups domain commands with deadline/idempotency, hierarchy checks,
domain receipt/audit, membership revision, and group version. They do not create moderation
cases inside Groups.

### Subject adapter mapping

Initial supported mapping:

- `GroupMembership` + typed `SuspendSubject { effective_until }` maps to the Groups
  suspension owner command;
- identical decision ID/hash replays before subject reads; changed hash conflicts;
- expected membership revision is checked in the transaction;
- unknown effect version, incompatible effect, invalid scope, or unsupported kind fails
  without mutation;
- group-level `Group` decisions require a separately declared effect matrix;
- `AccountSanctionRecommended` is not applied by Groups;
- moderation-only no-mutation/escalation outcomes are not dispatched as Groups mutations.

The returned `ModerationDecisionApplication` must match decision ID, subject identity, and
resulting membership revision. Moderation records applied evidence only after valid adapter
success.

### Authorization and hierarchy

For direct local enforcement:

- platform `groups:manage` may act across local hierarchy;
- owner may suspend non-owner memberships;
- admin may suspend moderator/member, not owner/admin peers unless explicitly allowed;
- moderator may suspend members only;
- self-suspension and owner suspension are rejected;
- authorization repeats after membership/group locks;
- stale role or subject revision fails atomically.

Moderation-driven application uses a host-composed service actor/capability, not local
membership authority. Groups still validates subject/scope/effect and immutable decision
provenance.

### Access, lifecycle, locking, and bulk ownership

An effective suspension:

- removes active-member treatment for private content and provider ACLs;
- denies join, application submit/reopen, invitation acceptance, post/comment/invite, and
  local moderation/settings actions;
- preserves public summary behavior unless explicit product policy says otherwise;
- never reveals secret-group existence to unauthorized suspended actors;
- preserves membership/application/audit history;
- decrements member count exactly once if previously active and restores only through an
  explicit valid lifecycle policy.

Revocation/expiry defines the resulting lifecycle state explicitly rather than always
restoring `active`.

Single commands replay receipt first, then lock membership, group, and enforcement state in
one declared order. Membership revision, enforcement state, group version/member count,
audit, semantic event, and receipt commit atomically.

Cross-domain bulk moderation belongs to `rustok-moderation`, which owns cases, decisions,
application jobs, retry, and per-subject outcomes. The Groups adapter remains single-subject.
A future direct Groups bulk action is a separate bounded domain command and must not
duplicate moderation jobs.

### UI ownership

- moderation admin FFA owns reports, queue, cases, decisions, application/retry, appeals;
- Groups admin FFA owns current local enforcement state, expiry/provenance, and authorized
  direct suspend/revoke controls;
- host composition may deep-link by typed IDs;
- neither UI imports the other owner's persistence or business component tree.

### Implementation order

1. **Source-complete prerequisite:** neutral moderation API, typed effect persistence/hash,
   sealed registry, and compatibility re-exports.
2. Membership revision migration and owner read-path conversion away from status-only ban
   checks.
3. Current enforcement persistence plus single direct suspend/revoke commands.
4. Join/access/invitation/application/provider ACL and member-count integration.
5. Neutral Groups subject adapter plus durable moderation application integration.
6. Groups state/direct-action FFA and moderation queue/case FFA host composition.
7. Optional direct local bulk command only after single-command runtime evidence.

### GROUPS-07 definition of done

- no Groups dependency on moderation owner crate;
- no moderation direct writes or foreign keys into Groups tables;
- exact group/membership identity and monotonic revision evidence;
- permanent/expiring suspension behavior across every owner access path;
- hierarchy, owner protection, tenant isolation, replay, changed-hash, stale revision,
  expiry, revoke, member-count, and concurrency evidence;
- missing/timeout/retry/lost-response adapter behavior;
- moderation-disabled mode preserves existing Groups enforcement and configured direct
  actions without inventing cases;
- native/GraphQL parity for Groups state/direct actions;
- separate moderation and Groups FFA ownership;
- PostgreSQL/SQLite migration, compatibility, accessibility, and no-fallback evidence.

## Other open Groups contracts

Localization remains `in_progress`: receipt/replay, last-translation delete rejection, and
native/GraphQL concurrency evidence remain open.

Targeted invitation delivery remains `in_progress`: Groups emits
`groups.invitation.targeted_created`, exposes the targeted command port, and registers a
neutral source provider, while Notifications runtime/fan-out/retry/recovery evidence is open.

## Feature-provider integration order

1. `forum.discussions` — Forum-owned space/category, access through Groups ports.
2. `blog.posts` — Blog-owned group-context posts and CommentsThreadPort.
3. `pages.wiki` — Pages-owned documents/Page Builder artifacts with typed group context.
4. `marketplace.store` — Marketplace seller/listing ownership with Commerce checkout/order
   ownership unchanged.
5. `media.gallery`, `events.calendar`, and `chat.room` — provider-owned lifecycle/UI.

Feature binding expresses policy/configuration only. It never transfers persistence
ownership and Groups never embeds another module's business UI directly.

## Degraded modes

- Groups access unavailable: deny private content.
- Candidate exact-locale policy unavailable: form unavailable; never select another locale.
- Management locale catalog unavailable: disable selection/save; do not infer rows.
- Policy CAS conflict: write no owner state and require explicit reload.
- Application lifecycle/bulk transport failure: preserve selected-path error; no fallback.
- Profiles unavailable: show UUID/placeholder; never copy canonical profile fields.
- Notifications unavailable: Groups command succeeds and owner state remains truth.
- Moderation disabled: existing Groups enforcement remains active; cases/appeals and
  moderation-driven application are unavailable; configured direct Groups actions may
  remain available.
- Moderation unavailable after decision: no Groups mutation is inferred; application remains
  pending/retryable in moderation.
- Groups adapter unavailable: moderation must not mark decision applied.
- Unknown effect, legacy `effect: None`, or stale revision: reject without Groups mutation.
- Expired local enforcement: inactive immediately even if legacy status is not normalized.
- Search/index unavailable: owner writes succeed; projections catch up later.

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
node scripts/verify/verify-db-multilingual-contract.mjs
npm run verify:i18n:ui
npm run verify:frontend:host-ffa-contract
```

Additional GROUPS-06 evidence includes exact-locale management, history,
CAS/lifecycle/bulk replay/races, lock order, parity, no-fallback, EN/RU, and accessibility.

Additional GROUPS-07 evidence includes:

- neutral dependency guards and registry duplicate/missing/mismatch behavior;
- typed effect expiry/version/hash and legacy non-dispatch behavior;
- clean/upgraded membership-revision/enforcement migrations;
- permanent/temporary/revoked/expired enforcement across join, access, invitations,
  applications, provider ACLs, and member count;
- direct/moderation-driven replay, changed-hash, stale revision, hierarchy, tenant isolation,
  concurrency, retry, and lost response;
- moderation-enabled/disabled and adapter available/unavailable runtime matrices;
- separate moderation/Groups FFA ownership and accessibility execution.
