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

Every change that modifies Groups behavior updates this plan in the same change:
task status, remaining scope, definition of done, verification evidence, and
degraded-mode notes.

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

## Current state

The owner foundation currently provides:

- module manifest, workspace/server/distribution composition, permissions, and
  central navigation registration;
- multilingual `groups + group_translations` storage with normalized `VARCHAR(32)`
  locale contracts and language-agnostic base JSON;
- exact host-effective-locale reads and locale-scoped catalog/search without an
  English or arbitrary-row fallback;
- Unicode-scalar title and summary limits;
- `GroupLocalizationReadPort` and `GroupLocalizationCommandPort` with exact-locale
  list/upsert/delete operations and transactional group-version increments;
- public, closed, and secret visibility plus open, request, and invite-only joins;
- discoverable closed shells with private body and feature-binding redaction;
- group membership, local roles, feature bindings, command receipts, and immutable
  audit entries;
- `GroupSummaryReadPort`, `GroupMembershipReadPort`, `GroupAccessReadPort`,
  `GroupGovernanceCommandPort`, and localization ports;
- role delegation and atomic ownership transfer through Rust, GraphQL, native
  server functions, and localized admin UI;
- `group_invitations` and `group_invitation_redemptions` with tenant-composite
  relations, bounded use counts, expiry, revocation, and unique per-user redemption;
- `GroupInvitationReadPort` and `GroupInvitationCommandPort` for manager listing,
  create, revoke, and authenticated acceptance;
- opaque invitation generation with SHA-256-only persistence and create receipts
  that deliberately omit plaintext tokens;
- transactional invitation acceptance that locks owner rows where supported,
  activates membership, increments member count/version, stores redemption,
  appends audit, and commits a receipt;
- final merged GraphQL roots for directory, localization, governance, and invitations;
- module-owned admin invitation management with native/GraphQL facades and one-time
  plaintext token display;
- module-owned storefront invitation acceptance with framework-neutral command
  preparation, explicit native/GraphQL transport, authenticated owner-service
  execution, localized EN/RU UI, password-style token input, and query removal on
  submission;
- admin/storefront `core -> transport -> ui/leptos` FFA structure;
- FBA registry and source guardrails.

This remains a functional foundation rather than complete phpFox parity. Event-driven
invitation delivery, recipient picker, cleanup policy, confirmation UX, audit history,
accessibility completion, executable parity/concurrency/security evidence, and recovery
remain open.

## FFA/FBA status

- FFA status: `in_progress`.
- FBA status: `in_progress`.
- Structural shape: `core_transport_ui`.
- Admin evidence: framework-neutral governance/localization/invitation preparation,
  selected native/GraphQL transport, host locale, localized forms, one-time token
  display, and composed Leptos roots are present.
- Storefront evidence: framework-neutral directory and invitation acceptance command
  preparation, selected native/GraphQL transport, host locale, localized acceptance
  states, query-token clearing, and thin Leptos bindings are present.
- Backend evidence: typed ports, request context, stable errors, owner services,
  multilingual DB constraints, invitation digest/expiry/revocation/redemption
  transactions, audit/receipt persistence, and merged GraphQL roots are present.
- Remaining FBA evidence: runtime provider/consumer order, fallback execution,
  PostgreSQL/SQLite migration execution, localization receipts/replay, invitation
  token entropy/storage inspection, concurrent create/revoke/accept and use-count
  exhaustion, invitation acceptance transport parity, receipt first-write race
  recovery, delivery event/Notifications integration, privacy leakage, governance
  concurrency/replay, retry/recovery, and remote-adapter smoke.
- Last verified at (UTC): not executed in these changes.
- Owner: `rustok-groups`.

No status is promoted to `phase_b_ready`, `parity_verified`, `boundary_ready`, or
`transport_verified` until the documented commands and runtime evidence execute.

## Architecture invariants

1. Groups owns group policy and relations, not foreign module content.
2. Optional domain modules never read or write another module's tables.
3. Every authoritative content owner re-checks group access.
4. Secret groups fail closed in search, SEO, notifications, feed, and direct reads.
5. Base rows contain no localized title/body copies.
6. Locale writes are explicit and do not mutate fallback locales.
7. Local roles never create platform-global authority.
8. One active owner is preserved transactionally.
9. A disabled feature provider does not take down the group shell.
10. Owner state and semantic outbox events will commit atomically when promoted.
11. Binary media remains Media-owned; Groups stores typed references only.
12. UI packages consume public owner transports and never import foreign UI internals.
13. Native and GraphQL paths call the same domain service.
14. Cache, realtime, feed, notifications, and search are never correctness authorities.
15. Governance and invitation writes require deadlines and idempotency keys.
16. Closed-group discovery exposes only a localized shell without private body,
    features, member lists, or provider-owned content.
17. Transport selection is explicit and never retries through another transport.
18. Governance/localization/invitation UI prepares transport-neutral commands in
    `core` and never reimplements owner policy.
19. The host resolves locale preference before invoking Groups.
20. Catalog/search matching and returned presentation use the same effective locale.
21. Base JSON remains language-agnostic and cannot shadow translation storage.
22. Localization management targets one normalized locale.
23. Translation mutation and version increment commit atomically.
24. Localization readiness remains blocked until durable replay/concurrency evidence.
25. Invitation plaintext tokens are returned once and never stored in invitation,
    audit, or receipt rows; only SHA-256 digests persist.
26. Targeted invitations are single-use; shareable links have at most 100 uses.
27. Invalid, expired, exhausted, revoked, and wrong-target tokens share one unavailable
    error contract.
28. Invitation acceptance, redemption, membership activation, member count, version,
    audit, and receipt commit in one owner transaction.
29. Groups does not synchronously deliver invitation notifications.
30. Storefront invitation acceptance uses the `invite` query key only as an input
    handoff, clears it when submission starts, and never renders plaintext as result
    text.
31. Storefront acceptance has no implicit native/GraphQL fallback and preserves the
    same idempotency key for one submitted command.

## Program ledger

| Task | Status | Current result or nearest deliverable |
| --- | --- | --- |
| `GROUPS-00` | `done` | Ownership, naming, FFA/FBA, multilingual, privacy, and integration contracts are documented. |
| `GROUPS-01` | `done` | Module package, manifest, composition, permissions, and central navigation are connected. |
| `GROUPS-02` | `in_progress` | Base schema/service, multilingual constraints, exact locale, audit/receipts exist; semantic events/outbox, archive lifecycle, localization receipts, recovery, executed DB evidence, and fixtures remain. |
| `GROUPS-03` | `in_progress` | Public/closed/secret and open/request/invite-only policies exist; the complete action matrix and leakage evidence remain. |
| `GROUPS-04` | `in_progress` | Role delegation and atomic ownership transfer have typed Rust, GraphQL, native, audit/receipts, and localized admin UI; confirmation, concurrency, parity, and recovery remain. |
| `GROUPS-05` | `in_progress` | Invitation/redemption schema, targeted/shareable tokens, SHA-256-only storage, expiry, revocation, transactional acceptance, receipts/audit, Rust/GraphQL/native management, localized admin UI, and localized storefront invitation acceptance exist; event-driven delivery, recipient picker, cleanup policy, migration/concurrency/parity/security evidence, accessibility review, and recovery remain. |
| `GROUPS-06` | `planned` | Membership questions, answers, rule acknowledgements, application review, and bulk-safety limits. |
| `GROUPS-07` | `planned` | Bans, temporary restrictions, removal, appeal handoff, and immutable local moderation audit. |
| `GROUPS-08` | `in_progress` | Versioned namespaced feature bindings exist; provider registry, health, settings schemas, and UI contributions remain. |
| `GROUPS-09` | `planned` | Media-owned avatar/cover/gallery references and reconciliation. |
| `GROUPS-10` | `planned` | SEO targets, localized routes, aliases, redirects, and secret exclusions. |
| `GROUPS-11` | `in_progress` | Final GraphQL roots expose directory, governance, localization, and invitation operations; REST remains deferred. |
| `GROUPS-12` | `in_progress` | Admin FFA has directory, governance, localization, and invitation facades/forms; pickers, confirmation, categories, moderation, settings, history, and accessibility remain. |
| `GROUPS-13` | `in_progress` | Storefront FFA has visibility-safe catalog and invitation acceptance; detail routing, role-aware management, and accessibility completion remain. |
| `GROUPS-14` | `in_progress` | Typed FBA ports/registry include localization and invitations; executable provider/consumer, fallback, parity, replay, concurrency, and degraded-mode evidence remain. |
| `GROUPS-15` | `planned` | Forum group-context provider, ACL inheritance, category binding, and leakage tests. |
| `GROUPS-16` | `planned` | Blog group-context binding and publish policy. |
| `GROUPS-17` | `planned` | Pages/Wiki binding without shadow documents. |
| `GROUPS-18` | `planned` | Marketplace binding; Commerce remains checkout/order owner. |
| `GROUPS-19` | `planned` | Notifications source provider, bounded recipient resolution, preferences, and disabled-module proof. |
| `GROUPS-20` | `planned` | Visibility-aware Index/Search projections and secret non-disclosure tests. |
| `GROUPS-21` | `planned` | Moderation subject provider, reports, decisions, and reconciliation. |
| `GROUPS-22` | `planned` | Wall/content owner integration without a hidden Groups wall table. |
| `GROUPS-23` | `planned` | Feed, reactions, events, chat, reputation, recommendations, and social graph. |
| `GROUPS-24` | `planned` | Analytics, observability, SLOs, audit repair, import/export, and phpFox migration. |
| `GROUPS-25` | `planned` | Production gate with concurrency, FFA parity, FBA fallback, E2E, security, and performance evidence. |

## Milestones

### Milestone A — owner foundation

Complete `GROUPS-00` through `GROUPS-04`, including semantic events and PostgreSQL
integrity/concurrency evidence.

### Milestone B — membership product

Complete invitations, applications, questions, rules, bans, local moderation, and
bounded member management.

### Milestone C — modular group constructor

Complete provider registration and integrations in this order: Forum, Blog,
Pages/Wiki, Marketplace, Media, Notifications, Index/Search. Each integration needs
capability profiles and fail-closed privacy.

### Milestone D — social network completion

Integrate dedicated Wall, Feed, Reactions, Events, Chat, Social Graph, and Reputation
owners. Do not implement substitutes inside Groups.

### Milestone E — release

Close multilingual routing, accessibility, import/export, phpFox migration,
observability, performance, security, recovery, and waiver-free runtime evidence.

## Verification

Not executed in the direct-to-`main` implementation changes at the repository
owner's request. The expected sequence is:

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
node scripts/verify/verify-db-multilingual-contract.mjs
npm run verify:i18n:ui
npm run verify:i18n:contract
npm run verify:frontend:host-ffa-contract
npm run verify:storefront:routes
```

PostgreSQL, SQLite, transport, and UI evidence must eventually cover:

- apply from zero and incremental migration;
- normalized locale acceptance/rejection and exact effective-locale reads;
- locale-scoped catalog/search and Unicode-equivalent text limits;
- rejection of localized copies in base JSON;
- translation authorization, final-row deletion denial, atomic versioning, replay,
  concurrency, and native/GraphQL parity;
- invitation token-hash uniqueness and absence of plaintext in invitation, audit,
  receipt, logs, and result rendering;
- targeted/shareable use bounds and expiry boundaries;
- wrong-target, revoked, exhausted, malformed, and unauthenticated behavior parity;
- concurrent acceptance at the final use and unique per-user redemption;
- atomic membership/member-count/version/audit/receipt rollback;
- create replay returning `token = null`;
- manager authorization and list/create/revoke parity;
- storefront invitation acceptance transport parity, idempotent replay, query-token
  clearing, loading/error/success states, and accessibility;
- tenant-composite identity, handle creation, join/leave counts, owner transfer,
  governance replay/concurrency, and rollback;
- public/closed/secret leakage and provider degraded modes;
- event/outbox/inbox retry, replay, delivery, and recovery.

## Update rules

1. This file remains the only Groups roadmap and task ledger.
2. Code, comments, migrations, fixtures, and repository documentation are English.
3. Every architecture, API, event, migration, UI, FFA/FBA, or integration change
   updates local docs and central registries in the same change.
4. Status promotion requires executable evidence, not source presence.
5. Cross-module dependencies require an owner port/event/provider contract and a
   documented degraded mode.
6. Do not preserve superseded internal paths, compatibility aliases, duplicate
   authorities, or hidden fallback behavior.
7. Do not add group content tables owned by Wall, Forum, Blog, Pages, Marketplace,
   Media, Events, or Chat.
8. Direct commits to `main` remain fast-forward and never overwrite parallel work.
