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

This file is the single source of truth for the Groups roadmap, implementation
backlog, FFA/FBA status, integration gates, and release evidence. Do not create
parallel group roadmaps, phpFox parity documents, remediation plans, or duplicated
task ledgers. Issues and pull requests are execution records only.

Every change that modifies Groups behavior must update this plan in the same
change: task status, remaining scope, definition of done, verification evidence,
and degraded-mode notes.

## Scope

Build phpFox-class social groups as modular micro-social networks while preserving
RusToK ownership boundaries:

- public, closed, and secret groups;
- categories, stable handles, localized presentation, media references, and SEO;
- open join, application, invitation, ban, ownership transfer, and local-role
  workflows;
- localized group rules and membership questions;
- owner/admin/moderator/member permissions;
- provider-owned Wall, Forum, Blog, Pages/Wiki, Media, Events, Marketplace, and Chat
  sections;
- visibility-aware search, notifications, moderation, feed, and analytics;
- module-owned admin/storefront FFA packages;
- in-process and remote-ready FBA boundaries with fail-closed privacy.

## Status vocabulary

- `planned`: contract or implementation is not yet source-complete.
- `in_progress`: useful source exists, but one or more required runtime, parity,
  concurrency, security, accessibility, migration, or degraded-mode gates remain
  open.
- `done`: implementation and every declared gate have executable evidence.
- `blocked`: an external owner capability is required before work can continue.

Source presence alone never promotes FFA or FBA readiness to `done`.

## Architectural invariants

### Ownership

Groups owns group identity, localized group presentation, memberships, local roles,
join policy, invitations, membership applications, rules/questions, bans, feature
bindings, command receipts, audit, and Groups semantic events.

Groups does not own profile presentation, media binaries, forum topics, blog posts,
Pages documents, marketplace listings, products, comments, notification inboxes,
search documents, feed entries, checkout, payment, orders, or fulfillment.

Optional modules never receive database foreign keys from Groups. Cross-module
composition uses typed identifiers, typed ports, semantic events, and host-owned UI
composition.

### Multilingual storage

Language-neutral state belongs to base tables. Canonical localized business copy
belongs to exact-locale translation tables. The host resolves the effective locale;
Groups normalizes and selects only that row. There is no English, first-row, or
module-local fallback.

Membership-application policies follow the same rule:

- `group_membership_policies` stores language-neutral revision/enabled state;
- `group_membership_policy_translations` stores ordered bounded questions and rules
  by exact locale;
- `group_membership_policy_revisions` stores append-only exact-locale snapshots of
  successful policy writes;
- a submitted application stores the policy revision, locale, and immutable
  question/rule snapshot seen by the candidate.

The visual policy editor in this source slice is intentionally bound to the
host-resolved effective locale. An explicit multi-locale management picker requires a
future read contract carrying the selected locale rather than mutating only the UI
field.

### Privacy

- public groups expose their localized shell and enabled public features;
- closed groups expose a localized summary shell but gate body, members, features,
  and provider content behind active membership or platform authority;
- secret groups are undisclosed to non-members and cannot be reached through public
  discovery or the membership-application flow;
- provider unavailability fails closed for private content;
- no transport fallback may bypass an owner denial or timeout.

### Commands

Writes require deadline plus idempotency key. Owner services repeat authorization
and invariant checks inside the transaction. Successful command state, group
version, receipt, and audit commit together where the contract declares them.

Policy revision capture is performed by a database trigger after the owner-managed
translation INSERT/UPDATE, so the current policy write and append-only history row
commit or roll back together. The history table rejects UPDATE and DELETE.

The current admin editor performs a non-atomic stale preflight by rereading the
current policy revision before save. This reduces accidental overwrites but is not an
atomic expected-revision guarantee. A future command contract must compare an
expected revision while holding the owner transaction lock.

## Current implementation state

The following source exists:

- Groups module manifest, migrations, RBAC, module registration, admin/storefront
  package registration, and generated host composition;
- tenant-scoped groups, exact-locale translations, memberships, local roles,
  feature bindings, receipts, and immutable audit;
- public/closed/secret discovery and private-content access split;
- group creation, join/leave, feature bindings, exact-locale management, role
  delegation, and ownership transfer;
- bounded token invitations, revocation, token acceptance, targeted accept-by-ID,
  one-time plaintext delivery, SHA-256-only storage, redemption, and membership
  activation;
- append-only `groups.invitation.targeted_created` owner events and a neutral
  Notifications source provider resolving one exact recipient;
- owner-owned membership-application policy, exact-locale questions/rules, candidate
  submission snapshot, required answer/rule validation, pending listing, and
  approve/reject review;
- append-only membership policy revision storage, manager-only history port, native
  and GraphQL history adapters, and a localized visual policy editor;
- editor controls for add/remove/reorder questions and rules, bounds preparation,
  loading/history states, and a disclosed non-atomic stale preflight;
- Rust ports, final merged GraphQL roots, native Leptos server functions, explicit
  native/GraphQL transport selection, admin review UI, and storefront application UI;
- EN/RU copy, FBA registry, live README contracts, and focused static guards.

The following evidence remains open and must not be inferred:

- compilation and executed unit/integration tests;
- PostgreSQL and SQLite migration execution/rollback evidence;
- native/GraphQL result and error parity;
- policy history backfill, trigger atomicity, append-only rejection, and pagination
  execution;
- atomic expected-revision enforcement for policy save and candidate submission;
- idempotency replay, concurrent submit/review/policy-write, and lock-order evidence;
- bulk-review limits, confirmation, partial-failure, and audit evidence;
- accessibility and keyboard/screen-reader execution;
- Notifications consumer ingestion/fan-out/retry/recovery evidence;
- fail-closed remote-provider and disabled-module runtime evidence.

## Program ledger

| ID | Status | Scope | Remaining gate |
|---|---|---|---|
| GROUPS-00 | in_progress | ADR, ownership map, phpFox parity map, FFA/FBA contracts | executable architecture review |
| GROUPS-01 | in_progress | module skeleton, manifest, RBAC, migrations, host composition | build/module-validation evidence |
| GROUPS-02 | in_progress | group identity, localized presentation, visibility, join policy, feature bindings, receipts/audit, targeted source events | lifecycle/runtime/concurrency evidence |
| GROUPS-03 | in_progress | memberships, join/leave, local roles, ownership transfer | request/bans/concurrency completion |
| GROUPS-04 | in_progress | summary, membership, access, localization, invitation, application, policy-history, governance ports | provider/consumer/fallback runtime matrix |
| GROUPS-05 | in_progress | GraphQL/native transports, storefront discovery, invitation acceptance/delivery source | runtime parity and Notifications consumer evidence |
| GROUPS-06 | in_progress | localized policy, ordered questions/rules, application snapshots, submit/review, append-only revision history, visual editor, stale preflight | atomic revision guard, cancellation, bulk safety, profiles/events, parity, concurrency, accessibility |
| GROUPS-07 | planned | bans, suspension, expiry, reason, bulk moderation, moderation adapter | all implementation/evidence |
| GROUPS-08 | planned | dynamic feature-provider registry and group navigation | registry/runtime degradation evidence |
| GROUPS-09 | planned | Forum group spaces and ACL inheritance | Forum owner integration evidence |
| GROUPS-10 | planned | Blog and Pages/Wiki group contexts | owner integration and privacy evidence |
| GROUPS-11 | planned | Marketplace/Store seller context and listing composition | seller/checkout boundary evidence |
| GROUPS-12 | planned | Media gallery, avatar/cover, Events and Chat providers | provider lifecycle/degradation evidence |
| GROUPS-13 | in_progress | notifications, search/SEO, moderation, profiles/media integration | consumer runtime and privacy evidence |
| GROUPS-14 | in_progress | storefront/admin UX, localization, loading/error/success states | pickers, confirmation, accessibility, parity |
| GROUPS-15 | planned | group feed/wall aggregation without ownership leakage | feed owner contract and ranking evidence |
| GROUPS-16 | planned | analytics and operator observability | privacy-safe metrics/evidence |
| GROUPS-17 | planned | import/export, retention, deletion, tenant lifecycle | compliance/recovery evidence |
| GROUPS-18 | planned | remote adapter profile and degraded modes | remote provider/fallback/recovery evidence |
| GROUPS-19 | in_progress | release verification matrix and evidence registry | all open evidence keys resolved |

## GROUPS-06 membership-application contract

### Owner tables

- `group_membership_policies` — one current language-neutral policy per group with a
  monotonic revision and enabled flag;
- `group_membership_policy_translations` — exact-locale ordered questions/rules;
- `group_membership_policy_revisions` — append-only `(tenant, policy, revision,
  locale)` snapshots containing enabled state, ordered questions/rules, actor, and
  timestamp;
- `group_membership_applications` — one current tenant/group/user application with
  policy identity, revision, locale, immutable policy snapshot, answers,
  acknowledgements, status, and review metadata.

### Owner ports

- `GroupApplicationReadPort::read_group_application_policy`;
- `GroupApplicationReadPort::list_group_membership_applications`;
- `GroupApplicationPolicyHistoryReadPort::list_group_application_policy_revisions`;
- `GroupApplicationCommandPort::upsert_group_application_policy`;
- `GroupApplicationCommandPort::submit_group_membership_application`;
- `GroupApplicationCommandPort::review_group_membership_application`.

The history read port uses the same active owner/admin/moderator or platform-manage
authorization boundary as application review. Candidates cannot enumerate policy
history.

### Policy invariants

- policy management requires active owner/admin or platform `groups:manage`;
- questions and rules use stable normalized keys;
- each policy contains at most 20 questions and 20 rules;
- prompts, help text, rule titles/bodies, and answer limits are bounded by Unicode
  scalar count;
- policy reads require the host-resolved exact locale and never select another row;
- policy upsert increments policy revision and group version atomically with receipt
  and audit;
- policy revision capture occurs in the same database transaction through PostgreSQL
  or SQLite triggers;
- current policy rows remain mutable owner state while revision rows are append-only;
- history ordering is revision descending and locale ascending within a tenant/group.

### Submission invariants

- only active `request` join-policy groups accept applications;
- secret groups return not-found semantics;
- the actor must be an authenticated, non-banned, non-active candidate;
- required answers must be non-empty and within the per-question limit;
- unknown answer keys and unknown rule acknowledgements are rejected;
- every required rule key must be acknowledged;
- a pending or already-approved application cannot be submitted again;
- rejected applications may be resubmitted and receive a fresh snapshot;
- application snapshot, pending membership, group version, audit, and receipt commit
  in one owner transaction.

### Review invariants

- listing/review requires active owner/admin/moderator or platform authority;
- only a pending application may be reviewed;
- approve moves membership to `active`, records `joined_at`, increments member count,
  and marks the application approved;
- reject moves membership to `left`, records `left_at`, and marks the application
  rejected;
- review note is optional and bounded to 2,000 characters;
- application, membership, group version, audit, and receipt commit together;
- application and group rows use exclusive locks where supported.

### FFA surfaces

- admin framework-neutral policy/history/review models and preparation core;
- admin native and GraphQL policy/history/list/review adapters through one facade;
- localized visual policy editor for adding/removing/reordering questions and rules;
- editor history list with revision, locale, actor, timestamp, enabled state, and item
  counts;
- host-resolved locale rendered read-only in the current editor;
- non-atomic stale preflight that blocks the save UX when the loaded revision differs;
- localized pending review workspace displaying candidate, policy revision/locale,
  answers, and acknowledged rules;
- storefront request-group links using `apply=<group_uuid>`;
- storefront framework-neutral dynamic-form validation;
- native and GraphQL policy/submit adapters through one facade;
- localized question/rule form with loading, validation, error, and success states;
- `apply` query removal only after successful submission;
- no implicit native/GraphQL fallback.

### GROUPS-06 remaining work

- atomic expected-revision enforcement inside policy upsert and candidate submit
  transactions, including a stable conflict code;
- explicit candidate stale-policy reload UX when the policy changes between render and
  submit;
- explicit multi-locale admin picker backed by an owner read contract carrying the
  selected exact locale;
- candidate cancellation and manager reopen/resubmit policy;
- bulk review with bounded selection, confirmation, per-item results, and audit;
- profile-backed candidate summaries through `ProfilesReader` without copying profile
  state;
- application submitted/reviewed semantic events and optional Notifications consumer;
- operator filtering, pagination controls, pickers, and audit/receipt history;
- keyboard, focus, validation association, and screen-reader evidence;
- executed parity, replay, concurrency, lock-order, migration, security, and recovery
  evidence.

## Other open Groups contracts

Localization remains `in_progress`: localization idempotent receipts/replay,
last-translation delete rejection execution, localization idempotency replay, and
native/GraphQL concurrency evidence remain open.

Targeted invitation delivery remains `in_progress`: the owner emits
`groups.invitation.targeted_created`, exposes `GroupTargetedInvitationCommandPort`,
and registers a neutral source provider, while targeted invitation notification
runtime, fan-out, retry, and recovery evidence remain open.

## Feature-provider integration order

1. `forum.discussions` — group space/category owned by Forum, access checked through
   Groups ports.
2. `blog.posts` — Blog-owned group-context posts and CommentsThreadPort.
3. `pages.wiki` — Pages-owned documents and Page Builder artifacts linked by typed
   group context.
4. `marketplace.store` — Marketplace Seller/Listing ownership with Commerce checkout
   and order ownership unchanged.
5. `media.gallery`, `events.calendar`, and `chat.room` — provider-owned lifecycle and
   UI contributions.

A feature binding expresses policy/configuration only. It never transfers persistence
ownership and Groups never embeds another module's business UI directly.

## Degraded modes

- Groups access provider unavailable: deny private content.
- Application exact-locale policy unavailable: application form/editor is unavailable;
  do not select another locale.
- Policy history unavailable: current owner policy remains authoritative; hide history
  and do not synthesize revisions.
- Native or GraphQL application transport failure: surface the selected-path error;
  never retry through the other path.
- Profiles unavailable: show stable UUID/placeholder, never copy canonical profile
  fields into Groups.
- Notifications unavailable: Groups command succeeds and owner state remains the
  source of truth.
- Search/index unavailable: group/application owner writes succeed; projections may
  catch up asynchronously once their owner integration exists.

## Verification matrix

The following commands are required before any affected status may become `done`:

```bash
cargo xtask module validate groups
cargo check -p rustok-groups --features graphql
cargo check -p rustok-groups-admin --features ssr
cargo check -p rustok-groups-storefront --features ssr
cargo test -p rustok-groups
node scripts/verify/verify-groups-boundary.mjs
node scripts/verify/verify-groups-localization-boundary.mjs
node scripts/verify/verify-groups-invitations-boundary.mjs
node scripts/verify/verify-groups-targeted-invitation-delivery.mjs
node scripts/verify/verify-groups-membership-applications.mjs
node scripts/verify/verify-groups-membership-policy-revisions.mjs
node scripts/verify/verify-db-multilingual-contract.mjs
npm run verify:i18n:ui
npm run verify:frontend:host-ffa-contract
```

Additional executable evidence required for GROUPS-06 promotion:

- PostgreSQL and SQLite migration up/down or documented irreversible-policy evidence;
- policy revision backfill, capture trigger, append-only update/delete rejection, and
  manager authorization;
- policy exact-locale read and missing-locale failure;
- required/optional question and rule validation;
- banned, active-member, secret-group, and non-request-group denial;
- idempotent policy save, submit, and review replay;
- concurrent policy writers with atomic expected-revision conflict;
- concurrent duplicate submit;
- concurrent approve/reject with one terminal outcome;
- approve member-count correctness and no double increment;
- native/GraphQL result and stable error parity;
- selected-transport no-fallback behavior;
- EN/RU editor/form/review rendering and accessibility execution.

## Evidence state for this change

No build, test, migration, verifier, runtime parity, concurrency, accessibility,
security, retry, or recovery command was executed for this source slice. Therefore:

- FFA remains `in_progress`;
- FBA remains `in_progress`;
- GROUPS-06 remains `in_progress`;
- GROUPS-19 remains `in_progress`;
- `membership_application_transport_parity` remains `null`;
- `membership_application_concurrency` remains `null`;
- `membership_application_policy_revision` remains `null`;
- `membership_application_bulk_review` remains `null`.

## Definition of release-ready Groups MVP

The Groups MVP is release-ready only when all of the following have executable
evidence:

- public/closed/secret privacy and tenant isolation;
- localized identity and exact-locale management;
- open join, application, invitation, leave, role, ownership, and ban workflows;
- dynamic provider registry with Forum first and safe degraded modes;
- public/admin storefront FFA with native/GraphQL parity and accessibility;
- semantic integrations for notifications, search/SEO, moderation, profiles/media,
  and audit/observability;
- PostgreSQL/SQLite migrations, replay, concurrency, security, and recovery gates;
- no direct cross-module SQL, no embedded foreign business UI, no implicit transport
  fallback, and no false readiness claims.
