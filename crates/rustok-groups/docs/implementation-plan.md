---
id: doc://crates/rustok-groups/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-groups
  - platform-community
last_reviewed: 2026-07-21
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
- join, request, invitation, ban, ownership transfer, and local-role workflows;
- rules and membership questions;
- owner/admin/moderator/member permissions;
- provider-owned Wall, Forum, Blog, Pages/Wiki, Media, Events, Marketplace, and
  Chat sections;
- visibility-aware search, notifications, moderation, feed, and analytics;
- module-owned admin/storefront FFA packages;
- in-process and remote-ready FBA boundaries with fail-closed privacy.

## Current State

The current owner foundation provides:

- module manifest and build-composition metadata;
- multilingual `groups + group_translations` storage contract;
- PostgreSQL CHECK constraints and SQLite validation triggers for normalized
  `VARCHAR(32)` locale tags, localized presentation shape, and language-agnostic
  `groups.metadata`, `group_memberships.metadata`, and
  `group_feature_bindings.configuration` base JSON;
- exact host-effective-locale reads and locale-scoped catalog/search without a
  module-owned English or arbitrary-row fallback;
- Unicode-scalar title and summary limits rather than UTF-8 byte limits;
- `GroupLocalizationReadPort` and `GroupLocalizationCommandPort` with exact-locale
  list/upsert/delete operations;
- transactional translation mutation plus group-version increment, with deletion
  of the final translation row rejected;
- `group_invitations` and `group_invitation_redemptions` storage with tenant/group
  composite relations, bounded use counts, expiry, revocation, and unique per-user
  redemption;
- `GroupInvitationReadPort` and `GroupInvitationCommandPort` for manager listing,
  create, revoke, and authenticated token acceptance;
- opaque invitation generation with SHA-256-only persistence and create receipts
  that deliberately omit plaintext tokens;
- transactional token acceptance that locks owner rows where supported, activates
  membership, increments member count/version, stores redemption, appends audit,
  and commits a receipt;
- `GroupTargetedInvitationCommandPort` and `GroupTargetedInvitationService` for
  authenticated exact-recipient acceptance by invitation ID without exposing a
  token;
- append-only `group_domain_events` storage and PostgreSQL/SQLite insert triggers
  that append `groups.invitation.targeted_created` in the same owner transaction as
  a targeted invitation insert;
- targeted event payloads containing invitation/group/recipient identifiers only,
  with no plaintext token, digest, profile copy, email, or localized business copy;
- deferred `GroupsNotificationSourceProviderFactory` registration through
  `rustok-notifications-api`;
- a bounded Groups notification source that resolves at most one exact recipient,
  suppresses unavailable invitations, and authorizes only the validated internal
  `/modules/groups?invitation=<uuid>` route;
- group membership and feature-binding storage;
- typed domain enums and DTOs;
- `GroupSummaryReadPort`, `GroupMembershipReadPort`, and
  `GroupAccessReadPort` boundaries;
- service operations for create, localized read/list, join/request, leave, and
  feature binding;
- separate `view_summary` shell access and `view` private-content access;
- discoverable closed-group shells with body and feature-binding redaction for
  viewers without active membership or platform manage authority;
- `GroupGovernanceCommandPort` with role delegation and atomic ownership transfer;
- transactional `group_command_receipts` and immutable `group_audit_entries` for
  governance and invitation commands;
- final merged GraphQL query/mutation roots exposing directory, localization,
  governance, invitation management, token acceptance, and targeted accept-by-ID;
- native Leptos server functions and explicit native/GraphQL facades for
  governance, localization, invitation management, token acceptance, and targeted
  acceptance;
- localized module-owned governance, exact-locale translation, invitation
  management, and storefront acceptance workspaces with framework-neutral command
  preparation;
- storefront auth-session wiring for GraphQL bearer/tenant context without implicit
  transport fallback;
- admin/storefront FFA package structure with host locale and explicit transports;
- FBA registry and source guardrails;
- module-local documentation and platform registry integration.

This is a functional foundation, not full phpFox feature parity. Targeted invitation
source events and source-provider registration are present, but the Notifications
owner does not yet provide proven end-to-end ingestion, inbox fan-out, preferences,
delivery channels, retry, or cleanup. Recipient picker UX, invitation cleanup,
confirmation workflows, audit history UI, accessibility, and executable parity,
concurrency, security, migration, and recovery evidence remain.

## FFA/FBA status

- FFA status: `in_progress`.
- FBA status: `in_progress`.
- Structural shape: `core_transport_ui`.
- Admin evidence: framework-neutral governance/localization/invitation command
  preparation, selected native/GraphQL transport, host locale, localized forms,
  one-time token display, and a composed Leptos root are present.
- Storefront evidence: framework-neutral token and targeted-ID command preparation,
  host-authenticated native/GraphQL transports, host locale, query clearing, EN/RU
  states, and thin Leptos binding are present.
- Backend evidence: typed read/write/localization/invitation/targeted-invitation
  ports, request context, stable errors, owner services, language-agnostic DB
  constraints/triggers, exact effective-locale selection, locale-scoped
  catalog/search, closed-shell/private-content action separation, localization
  version transactions, invitation digest/expiry/revocation/redemption
  transactions, targeted event append trigger, bounded notification source,
  audit/receipt persistence, final merged GraphQL roots, native server functions,
  and machine-readable registries are present.
- Remaining FBA evidence: runtime provider/consumer order, source factory
  materialization, targeted invitation notification runtime, fallback execution,
  PostgreSQL/SQLite migration execution, missing-translation behavior,
  localization receipts/replay, invitation token entropy/storage inspection,
  concurrent create/revoke/token-accept/targeted-accept and use-count exhaustion,
  native/GraphQL parity, receipt first-write race recovery, Notifications
  ingestion/fan-out/preferences/disabled-module/retry/recovery, closed/secret
  leakage, governance concurrency/replay, and remote-adapter smoke.
- Last verified at (UTC): not executed in these changes.
- Owner: `rustok-groups`.

No status is promoted to `phase_b_ready`, `parity_verified`, `boundary_ready`, or
`transport_verified` until the documented commands and runtime evidence execute.

## Architecture invariants

1. Groups owns group policy and relations, not foreign module content.
2. No optional domain module reads or writes another module's tables.
3. Group access is re-checked by every authoritative content owner.
4. Secret groups fail closed in search, SEO, notifications, feed, and direct reads;
   unauthorized direct reads return not-found semantics.
5. Base rows contain no localized title/body copies.
6. Locale writes are explicit and do not mutate fallback locales.
7. Local group roles never create platform-global authority.
8. One active owner is preserved transactionally.
9. A disabled/unavailable feature does not take down the group shell.
10. Binary media remains Media-owned; Groups stores typed UUID references only.
11. UI packages consume owner public transports and never import another module's
    UI internals.
12. Native and GraphQL paths call the same domain service and do not duplicate
    business rules.
13. Cache, realtime, feed, notifications, and search are accelerators/consumers,
    never correctness authorities.
14. Governance writes require deadlines and idempotency keys; successful state,
    command receipt, and immutable audit entry commit in one transaction.
15. Closed-group discoverability exposes only the localized shell; it never grants
    body, feature-binding, member-list, or provider-owned content access.
16. Transport selection is explicit and never retries through another transport
    after an owner command error.
17. Governance/localization/invitation UI prepares transport-neutral commands in
    `core` and never reimplements local-role, ownership, fallback, token, target, or
    redemption policy.
18. The host/runtime resolves locale preference and fallback before invoking Groups;
    Groups requires the exact normalized effective-locale row and never privileges
    English or an arbitrary first stored translation.
19. Catalog/search matching and returned presentation use the same effective locale.
20. `groups.metadata`, `group_memberships.metadata`, and
    `group_feature_bindings.configuration` remain language-agnostic. Reserved
    top-level presentation fields cannot become shadow translation storage, while
    nested technical provider-schema fields remain valid non-copy configuration.
21. Localization management targets one exact normalized locale and never copies a
    fallback translation into another row.
22. Translation upsert/delete and group-version increment commit atomically; the last
    translation row cannot be deleted.
23. Localization command idempotency keys are required, but parity/ready status stays
    blocked until durable receipts and replay/concurrency evidence exist.
24. Invitation plaintext tokens are returned once and never stored in invitation,
    audit, receipt, or semantic event rows; only SHA-256 digests are persisted.
25. Targeted invitations are single-use; shareable links have bounded expiry and at
    most 100 uses.
26. Invalid, expired, exhausted, revoked, and wrong-target token reads use one
    unavailable contract.
27. Invitation acceptance, unique redemption, membership activation, member count,
    group version, audit, and receipt commit in one owner transaction.
28. Groups does not synchronously deliver invitation notifications. Notifications is
    an optional committed-event consumer.
29. Only targeted invitation creation emits a notification source event; shareable
    links never emit one because plaintext cannot be reconstructed safely.
30. The targeted event is appended by the database in the same transaction as the
    invitation row and is immutable after commit.
31. Targeted event payloads contain IDs only and never token, digest, profile,
    contact, or localized presentation fields.
32. Targeted invitation audience resolution returns zero or one exact recipient.
33. Target opening is authorized again against current invitation/group state and
    exact recipient identity; stale or wrong-recipient targets are unavailable.
34. Targeted accept-by-ID is authenticated, idempotent, exact-recipient only, and
    returns not-found semantics for unavailable or mismatched invitations.
35. Notification inbox, preferences, channels, retries, and delivery receipts remain
    Notifications-owned and cannot block Groups invitation creation.

## Program ledger

| Task | Status | Current result or nearest deliverable |
| --- | --- | --- |
| `GROUPS-00` | `done` | Ownership, naming, FFA/FBA, multilingual, privacy, and integration contracts are documented. |
| `GROUPS-01` | `done` | Module package, manifest, workspace/server/distribution composition, permissions, and central navigation are connected. |
| `GROUPS-02` | `in_progress` | Base schema/service, multilingual constraints, exact locale selection, governance/invitation audit and receipts, and targeted invitation source events exist; general event coverage, archive lifecycle, localization receipts, receipt-race recovery, executed PostgreSQL/SQLite evidence, and fixtures remain. |
| `GROUPS-03` | `in_progress` | Public/closed/secret and open/request/invite-only policies exist; closed shells are discoverable while body/features remain membership-gated, and the complete granular action matrix plus leakage evidence remain. |
| `GROUPS-04` | `in_progress` | Role delegation and atomic ownership transfer have typed Rust, GraphQL, native server-function, and localized admin form surfaces with audit/receipts; confirmation UX, concurrent-owner proof, parity execution, and recovery remain. |
| `GROUPS-05` | `in_progress` | Invitation/redemption schema, bounded SHA-256 token flows, transactional token and targeted-ID acceptance, append-only targeted creation events, exact-recipient notification source, Rust/GraphQL/native/storefront adapters, and localized UI exist; recipient picker, cleanup policy, Notifications ingestion/fan-out/preferences/channels, migration/concurrency/parity/security evidence, and recovery remain. |
| `GROUPS-06` | `planned` | Membership questions, answers, rule acknowledgements, application review, and bulk-safety limits. |
| `GROUPS-07` | `planned` | Bans, temporary restrictions, removal, appeal handoff, and immutable local moderation audit. |
| `GROUPS-08` | `in_progress` | Versioned namespaced feature bindings exist; provider registry, health, settings schemas, and UI contributions remain. |
| `GROUPS-09` | `planned` | Media-owned avatar/cover/gallery references and quarantine/deletion reconciliation. |
| `GROUPS-10` | `planned` | SEO targets, canonical localized routes, aliases, redirects, and secret-group exclusions. |
| `GROUPS-11` | `in_progress` | Final GraphQL roots expose directory/read/create/join/leave/feature/governance/localization/invitation and targeted acceptance operations; REST remains deferred. |
| `GROUPS-12` | `in_progress` | Admin FFA has native/GraphQL directory, governance, localization, and invitation facades plus localized forms and one-time token display; recipient/member pickers, confirmation, categories, moderation, settings, receipt/audit history, and accessibility remain. |
| `GROUPS-13` | `in_progress` | Storefront FFA exposes exact-locale public/closed shells plus token and targeted-ID invitation acceptance with explicit native/GraphQL transport; detail routing, role-aware management, accessibility execution, and parity evidence remain. |
| `GROUPS-14` | `in_progress` | Typed FBA ports and registry cover localization, invitations, targeted acceptance, and the neutral notification-source factory; executable provider/consumer, fallback, parity, replay, concurrency, and degraded-mode evidence remain. |
| `GROUPS-15` | `planned` | Forum group-context provider, ACL inheritance, local category binding, and leakage tests. |
| `GROUPS-16` | `planned` | Blog group-context binding and author/publish policy. |
| `GROUPS-17` | `planned` | Pages/Wiki binding without shadow documents or group-owned blocks. |
| `GROUPS-18` | `planned` | Marketplace seller/listing binding; Commerce remains checkout/order owner. |
| `GROUPS-19` | `in_progress` | Groups registers a bounded targeted-invitation source provider with exact-recipient open authorization and no synchronous dependency; Notifications-owned ingestion, inbox fan-out, preferences, channel delivery, disabled-module proof, retry, and recovery remain. |
| `GROUPS-20` | `planned` | Visibility-aware Index/Search projections, recovery, and secret-group non-disclosure tests. |
| `GROUPS-21` | `planned` | Moderation subject command provider, reports, transfer/closure decisions, and reconciliation. |
| `GROUPS-22` | `planned` | Wall/content owner integration; Groups must not implement a hidden wall table. |
| `GROUPS-23` | `planned` | Feed, reactions, events, chat, reputation, recommendations, and social-graph integrations after owner modules exist. |
| `GROUPS-24` | `planned` | Analytics, observability, SLOs, audit repair, import/export, and resumable phpFox migration toolkit. |
| `GROUPS-25` | `planned` | Production release gate with PostgreSQL concurrency, FFA parity, FBA fallback, E2E, security, and performance evidence. |

## Milestones

### Milestone A — owner foundation

Complete `GROUPS-00` through `GROUPS-04`, including governance/localization
transport, transactional ownership transfer, durable command receipts, broader
semantic events, and PostgreSQL integrity/concurrency tests.

### Milestone B — membership product

Complete invitations, applications, questions, rules, bans, local moderation,
and bounded member management. Targeted source events do not close this milestone
until Notifications consumer behavior, cleanup, recipient picker, and executable
recovery evidence are complete.

### Milestone C — modular group constructor

Complete provider registration and the first owner integrations in this order:
Forum, Blog, Pages/Wiki, Marketplace, Media, Notifications, Index/Search. Each
integration requires required/optional capability profiles and fail-closed privacy.

### Milestone D — social network completion

Integrate dedicated Wall, Feed, Reactions, Events, Chat, Social Graph, and
Reputation owners. Do not implement substitutes inside Groups while those owners
are absent.

### Milestone E — release

Close multilingual routing, accessibility, import/export, phpFox migration,
observability, performance, security, recovery, and waiver-free runtime evidence.

## Verification

Not executed in these implementation changes at the repository owner's request.
The expected verification sequence is:

```bash
cargo fmt --all -- --check
cargo xtask module validate groups
cargo xtask module test groups
cargo check -p rustok-groups --features graphql
cargo check -p rustok-server --features mod-groups
cargo check -p rustok-groups-admin --features ssr
cargo check -p rustok-groups-storefront --features ssr
node scripts/verify/verify-groups-boundary.mjs
node scripts/verify/verify-groups-localization-boundary.mjs
node scripts/verify/verify-groups-invitations-boundary.mjs
node scripts/verify/verify-groups-invitation-acceptance-ui.mjs
node scripts/verify/verify-groups-targeted-invitation-delivery.mjs
node scripts/verify/verify-db-multilingual-contract.mjs
npm run verify:i18n:ui
npm run verify:i18n:contract
npm run verify:frontend:host-ffa-contract
npm run verify:storefront:routes
```

PostgreSQL and SQLite verification must eventually cover:

- apply from zero and incremental migration;
- normalized locale acceptance/rejection across language, region, script, numeric,
  and variant subtags;
- exact host-effective-locale reads and explicit missing-translation behavior;
- locale-scoped catalog/search without cross-language matching;
- Unicode-equivalent title/summary length limits for Latin, Cyrillic, and CJK;
- rejection of top-level localized presentation copies in `groups.metadata`,
  `group_memberships.metadata`, and `group_feature_bindings.configuration`,
  including direct SQL writes, while nested non-copy provider schema remains valid;
- exact-locale translation list/upsert/delete authorization;
- last-translation delete rejection;
- atomic translation mutation plus group-version increment;
- localization idempotency replay, key-payload conflict, concurrent upsert/delete,
  and native/GraphQL result/error parity;
- invitation migration constraints, token-hash uniqueness, and absence of plaintext
  token in invitation, audit, receipt, and event rows;
- targeted single-use and shareable 1..100-use boundaries;
- expiry boundary, wrong-target, revoked, exhausted, and malformed-token parity;
- concurrent token and targeted-ID acceptance at the final use and unique per-user
  redemption;
- atomic membership/member-count/version/audit/receipt rollback on acceptance
  failure;
- create replay returning `token = null` without creating a second invitation;
- manager role authorization and native/GraphQL list/create/revoke parity;
- append-only `group_domain_events` behavior and event rollback when invitation
  creation rolls back;
- targeted event payload inspection proving no plaintext token or digest fields;
- one targeted event per successful targeted invitation and no event for shareable
  links;
- source revision/event identity consistency;
- source factory materialization with and without Notifications enabled;
- bounded audience resolution returning only the exact current target;
- target open denial after revocation, expiry, exhaustion, acceptance, group
  suspension/archive, tenant mismatch, or wrong recipient;
- targeted accept-by-ID replay, concurrency, not-found non-disclosure, and
  native/GraphQL parity;
- targeted invitation notification runtime ingestion, inbox fan-out, preference,
  disabled-module, retry, deduplication, and recovery behavior;
- tenant-composite identity and relation integrity;
- locale uniqueness and host-owned fallback behavior;
- concurrent handle creation;
- concurrent join/leave and member counts;
- owner transfer and last-owner protection;
- governance idempotent replay, key-payload conflicts, and simultaneous first writes;
- audit/receipt rollback when a governance command fails;
- native/GraphQL governance result and error parity;
- governance/localization/invitation UI loading, validation, success, error, replay,
  one-time-token, targeted-route, query-clearing, and confirmation states;
- public/closed catalog visibility, closed shell redaction, and member content access;
- secret-group query/search/SEO/direct-read leakage;
- feature-provider unavailable/read-only/hidden profiles;
- event/outbox/inbox retry, replay, delivery, cleanup, and recovery.

## Update Rules

1. This file remains the only Groups roadmap and task ledger.
2. Code, comments, migrations, fixtures, and repository documentation are English.
3. Every architecture, API, event, migration, UI, FFA/FBA, or integration change
   updates local docs and central registries in the same change.
4. Status promotion requires executable evidence, not source presence.
5. New cross-module dependencies require an explicit owner port/event/provider
   contract and a documented degraded mode.
6. Do not preserve superseded internal paths, compatibility aliases, duplicate
   authorities, or hidden fallback-to-legacy behavior.
7. Do not add group content tables that belong to Wall, Forum, Blog, Pages,
   Marketplace, Media, Events, or Chat.
8. Direct commits to `main` must remain fast-forward and must never overwrite
   parallel module work.
