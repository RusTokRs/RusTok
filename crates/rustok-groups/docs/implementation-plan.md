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

This file is the single source of truth for the Groups product roadmap,
implementation backlog, FFA/FBA status, integration gates, and release evidence.
Do not create parallel group roadmaps, phpFox parity documents, remediation plans,
or duplicated task ledgers. Issues and pull requests are execution records only.

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
  governance commands;
- a merged GraphQL mutation root exposing role delegation and ownership transfer;
- native Leptos server functions and a selected native/GraphQL admin transport
  facade for the same governance commands;
- admin/storefront FFA package structure with host locale and explicit transports;
- FBA registry and source guardrail;
- module-local documentation and platform registry integration.

This is a functional foundation, not full phpFox feature parity. Governance
commands are available through typed Rust, GraphQL, and native server-function
surfaces, while the operator UI workspace and executable parity evidence remain.

## FFA/FBA status

- FFA status: `in_progress`.
- FBA status: `in_progress`.
- Structural shape: `core_transport_ui`.
- Admin evidence: framework-neutral core, selected native/GraphQL transport,
  host locale, thin Leptos binding, and governance command facade are present.
- Storefront evidence: framework-neutral core, selected native/GraphQL transport,
  host locale, and thin Leptos binding are present.
- Backend evidence: typed read/write ports, request context, stable errors, owner
  services, closed-shell/private-content action separation, governance
  audit/receipt persistence, merged GraphQL root, native server functions, and
  machine-readable registry are present.
- Remaining FBA evidence: runtime provider/consumer order, fallback execution,
  closed/secret leakage evidence, native/GraphQL governance parity execution,
  governance concurrency/replay races, retry/recovery, and remote-adapter smoke.
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
10. Owner state and semantic outbox event will commit atomically when the event
    slice is promoted.
11. Binary media remains Media-owned; Groups stores typed UUID references only.
12. UI packages consume owner public transports and never import another module's
    UI internals.
13. Native and GraphQL paths call the same domain service and do not duplicate
    business rules.
14. Cache, realtime, feed, notifications, and search are accelerators/consumers,
    never correctness authorities.
15. Governance writes require deadlines and idempotency keys; successful state,
    command receipt, and immutable audit entry commit in one transaction.
16. Closed-group discoverability exposes only the localized shell; it never grants
    body, feature-binding, member-list, or provider-owned content access.
17. Governance transport selection is explicit and never retries through another
    transport after an owner command error.

## Program ledger

| Task | Status | Current result or nearest deliverable |
| --- | --- | --- |
| `GROUPS-00` | `done` | Ownership, naming, FFA/FBA, multilingual, privacy, and integration contracts are documented. |
| `GROUPS-01` | `done` | Module package, manifest, workspace/server/distribution composition, permissions, and central navigation are connected. |
| `GROUPS-02` | `in_progress` | Base schema/service plus governance audit and replay receipts exist; semantic events/outbox, archive lifecycle, receipt-race recovery, and PostgreSQL evidence remain. |
| `GROUPS-03` | `in_progress` | Public/closed/secret and open/request/invite-only policies exist; closed shells are discoverable while body/features remain membership-gated, and the complete granular action matrix plus leakage evidence remain. |
| `GROUPS-04` | `in_progress` | Role delegation and atomic ownership transfer have typed Rust, GraphQL, and native server-function transports with audit/receipts; operator UI, concurrent-owner proof, parity execution, and recovery remain. |
| `GROUPS-05` | `planned` | Invitations, invitation links, expiry, token hashing, revocation, and bounded delivery. |
| `GROUPS-06` | `planned` | Membership questions, answers, rule acknowledgements, application review, and bulk-safety limits. |
| `GROUPS-07` | `planned` | Bans, temporary restrictions, removal, appeal handoff, and immutable local moderation audit. |
| `GROUPS-08` | `in_progress` | Versioned namespaced feature bindings exist; provider registry, health, settings schemas, and UI contributions remain. |
| `GROUPS-09` | `planned` | Media-owned avatar/cover/gallery references and quarantine/deletion reconciliation. |
| `GROUPS-10` | `planned` | SEO targets, canonical localized routes, aliases, redirects, and secret-group exclusions. |
| `GROUPS-11` | `in_progress` | GraphQL list/read/create/join/leave/feature/governance surface exists and read/list inherit closed-shell redaction; REST remains deferred until explicit integration demand. |
| `GROUPS-12` | `in_progress` | Admin FFA has explicit native/GraphQL directory and governance command facades; category, membership, moderation, settings, governance UI, and audit workspaces remain. |
| `GROUPS-13` | `in_progress` | Storefront FFA catalog exposes public and closed shells without private body/features; detail routing, role-aware management, and accessibility remain. |
| `GROUPS-14` | `in_progress` | Typed FBA ports and registry exist across Rust/GraphQL/native surfaces; executable provider/consumer, parity, and degraded-mode evidence remain. |
| `GROUPS-15` | `planned` | Forum group-context provider, ACL inheritance, local category binding, and leakage tests. |
| `GROUPS-16` | `planned` | Blog group-context binding and author/publish policy. |
| `GROUPS-17` | `planned` | Pages/Wiki binding without shadow documents or group-owned blocks. |
| `GROUPS-18` | `planned` | Marketplace seller/listing binding; Commerce remains checkout/order owner. |
| `GROUPS-19` | `planned` | Notifications source provider, bounded recipient resolution, preferences, and disabled-module proof. |
| `GROUPS-20` | `planned` | Visibility-aware Index/Search projections, recovery, and secret-group non-disclosure tests. |
| `GROUPS-21` | `planned` | Moderation subject command provider, reports, transfer/closure decisions, and reconciliation. |
| `GROUPS-22` | `planned` | Wall/content owner integration; Groups must not implement a hidden wall table. |
| `GROUPS-23` | `planned` | Feed, reactions, events, chat, reputation, recommendations, and social-graph integrations after owner modules exist. |
| `GROUPS-24` | `planned` | Analytics, observability, SLOs, audit repair, import/export, and resumable phpFox migration toolkit. |
| `GROUPS-25` | `planned` | Production release gate with PostgreSQL concurrency, FFA parity, FBA fallback, E2E, security, and performance evidence. |

## Milestones

### Milestone A — owner foundation

Complete `GROUPS-00` through `GROUPS-04`, including governance transport,
transactional ownership transfer, command receipts, semantic events, and
PostgreSQL integrity/concurrency tests.

### Milestone B — membership product

Complete invitations, applications, questions, rules, bans, local moderation,
and bounded member management.

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

Not executed in the direct-to-`main` implementation changes at the repository
owner's request. The expected verification sequence is:

```bash
cargo fmt --all -- --check
cargo xtask module validate groups
cargo xtask module test groups
cargo check -p rustok-groups --features graphql
cargo check -p rustok-server --features mod-groups
cargo check -p rustok-groups-admin --features ssr
cargo check -p rustok-groups-storefront --features ssr
node scripts/verify/verify-groups-boundary.mjs
npm run verify:i18n:ui
npm run verify:i18n:contract
npm run verify:frontend:host-ffa-contract
npm run verify:storefront:routes
```

PostgreSQL verification must eventually cover:

- apply from zero and incremental migration;
- tenant-composite identity and relation integrity;
- locale uniqueness and fallback;
- concurrent handle creation;
- concurrent join/leave and member counts;
- owner transfer and last-owner protection;
- idempotent command replay, key-payload conflicts, and simultaneous first writes;
- audit/receipt rollback when a governance command fails;
- native/GraphQL governance result and error parity;
- public/closed catalog visibility, closed shell redaction, and member content access;
- secret-group query/search/SEO/direct-read leakage;
- feature-provider unavailable/read-only/hidden profiles;
- event/outbox/inbox retry, replay, and recovery.

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
