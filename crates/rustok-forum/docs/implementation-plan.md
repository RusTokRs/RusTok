---
id: doc://crates/rustok-forum/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-forum
last_reviewed: 2026-08-06
---

# `rustok-forum` canonical implementation plan

## Canonical-source policy

This file is the single source of truth for Forum product scope, Forum-owned
implementation work, shared-capability integration work, task status, execution
order, and release gates.

The exact pre-correction snapshot is retained at
`docs/archive/implementation-plan-2026-08-06.snapshot` for audit only. The
snapshot is not authoritative even though its preserved bytes contain the old
header and task language. Do not copy task status or ownership from it.

Every pull request that changes a task below must update this plan in the same
pull request when it changes status, remaining scope, ownership, verification,
compatibility, migration, or degraded-mode behavior.

A task is `done` only after its implementation, integration, migration or
backfill, tests, public contracts, documentation, and required runtime evidence
are complete. Source-ready or merged partial slices remain `in_progress`.

## Product model

Forum is an installable domain application composed from platform modules. It
must not recreate common social-platform capabilities inside `rustok-forum`.
The target is comparable to an application-oriented platform such as phpFox:
each module owns one capability and Forum contributes only Forum-specific state,
policy, adapters, semantic events, and UI composition.

The Forum product still includes categories, discussions, Q&A, wiki and
announcement modes, subscriptions, unread state, drafts, bookmarks, mentions,
attachments, reactions, reputation presentation, moderation, search, SEO,
notifications, realtime acceleration, member views, admin, storefront,
observability, and import/export. Product inclusion does not imply Forum storage
ownership.

## Ownership rules

### Forum-owned

`rustok-forum` owns only:

- category hierarchy, category policy, topic/reply lifecycle and revisions;
- localized Forum content and route identity;
- topic kinds, solution semantics, subscriptions and Forum read state;
- Forum drafts and bookmarks unless a proven neutral owner contract replaces
  them;
- Forum attachment relations, usage, order, caption and target revision;
- Forum trust state, Forum posting policy and Forum-local enforcement state;
- Forum subject adapters for reactions and moderation;
- Forum visibility, source authorization, semantic events and projections;
- Forum-specific statistics and activity projections;
- module-owned Forum admin/storefront packages.

Forum tables may reference another owner by typed tenant-scoped identity, but
must not copy that owner's source-of-truth data or read its private tables.

### Shared owners

| Capability | Authoritative owner | Forum responsibility |
| --- | --- | --- |
| Login identity and sessions | `auth/users` | Carry trusted actor identity only. |
| Public handle, display name, biography, locale and profile privacy | `rustok-profiles` | Batch summaries and compose Forum statistics. |
| Avatar/banner and all binary asset lifecycle | `rustok-media` | Store typed media references and attachment policy only. |
| Follow/block relationship facts | `rustok-social-graph` and profile privacy ports | Request exact bounded facts; never read relation tables. |
| Reaction catalog, actor reactions and aggregate reaction counts | planned `rustok-reactions` | Publish Forum subject adapter, authorization and UI. |
| Cross-domain reputation ledger and achievements/badges | planned `rustok-reputation` / achievement capability | Publish semantic facts and display permitted projections. |
| Reports, cases, queues, decisions, appeals and cross-domain audit | `rustok-moderation` through `rustok-moderation-api` | Report subjects and apply validated effects to Forum state. |
| Notification inbox, fan-out, grouping, preferences, digests and deliveries | `rustok-notifications` | Publish source events/providers and authorize current targets. |
| Translation workflow | `rustok-translation` | Publish exact Forum translation targets and apply owner writes. |
| Search storage and retrieval | `rustok-search`; generic materialized indexing in `rustok-index` | Publish visibility-safe Forum projections and repair sources. |
| SEO aggregation and host head composition | `rustok-seo` | Publish canonical Forum targets and structured semantics. |
| Durable event delivery | `rustok-outbox` / `rustok-events` | Commit owner state and semantic events atomically. |
| Realtime transport | shared host/runtime capability | Publish revisions/cursors; reload canonical owner state on reconnect. |
| Import orchestration | shared bounded import framework when available | Provide Forum mapping, validation, receipts and reconciliation. |

### Non-duplication gates

The following are forbidden without a platform ADR and an ownership migration:

- `forum_member_profiles`, copied display names, copied avatars or a new member
  module;
- Forum-owned media uploads, storage keys, delivery URLs, quarantine or deletion
  lifecycle;
- Forum-owned report/case/appeal queues or a second cross-domain moderation
  audit ledger;
- Forum-owned notification inbox, preferences, grouping, digests or delivery
  attempts;
- a Forum-specific reaction catalog once `rustok-reactions` is composed;
- a Forum-only reputation ledger or badge catalog intended for reuse by other
  modules;
- transport-local visibility, profile, reaction or moderation policy;
- direct reads of another module's persistence tables.

Optional shared modules must have explicit unavailable/degraded behavior. Their
absence must not make an otherwise valid Forum owner command fail unless that
command explicitly requires the capability, such as assigning a Media asset.

## Current verified baseline

The current source already provides categories, localized topics/replies,
typed lifecycle, revisions and tombstones, bounded reads, subscription levels,
accepted solutions, topic tags, Forum statistics, transactional events,
visibility-aware Search/SEO providers, shared rich text, module-owned admin and
storefront packages, and Page Builder consumer contracts.

The repository also already contains the relevant shared owners: Profiles,
Media, Social Graph, Moderation, Notifications, Translation, SEO, Search/Index,
Outbox/Events, Taxonomy, Workflow, Comments, Groups and Channel. New Forum work
must integrate those boundaries instead of cloning their data models.

## Program ledger

| Task | Status | Current result and remaining deliverable |
| --- | --- | --- |
| `FORUM-00` | `done` | PostgreSQL/SQLite runtime baseline. |
| `FORUM-01` | `done` | Tenant-composite integrity and locale width. |
| `FORUM-02` | `done` | Typed topic/reply lifecycle and revision fields. |
| `FORUM-03` | `done` | Atomic category writes and translations. |
| `FORUM-04` | `done` | Bounded tree, placement, policy, subtree lifecycle and admin DnD. |
| `FORUM-05` | `done` | Serialized publication-aware counters. |
| `FORUM-06` | `done` | Locked topic and moderation publication semantics. |
| `FORUM-07` | `done` | Monotonic reply positions. |
| `FORUM-08` | `done` | Revisions, tombstones and owner soft-delete paths. |
| `FORUM-09` | `done` | Versioned Forum event catalog and journal. |
| `FORUM-10` | `done` | Bounded cursor read models. |
| `FORUM-11` | `done` | Subscription levels and participation policy. |
| `FORUM-12` | `in_progress` | Mention/quote owner relations and notification source exist. Maintainer runtime execution, profile/block privacy, moderator audience expansion and final Notifications policy/evidence remain. |
| `FORUM-13` | `in_progress` | Category presentation and optional Media read policy exist. Persist only typed `cover_media_id`; Media owns upload, lifecycle and descriptors. Owner command, transports, UI and runtime evidence remain. |
| `FORUM-14` | `planned` | Add Forum attachment relations and target policy over Media-owned upload sessions/assets. Do not implement upload or asset lifecycle in Forum. |
| `FORUM-15` | `in_progress` | Profiles already supplies Forum through `ProfilesReader` and batched summaries. Complete member-card composition, privacy/block behavior, Forum-stat enrichment and no-N+1 runtime evidence. |
| `FORUM-16` | `in_progress` | Monotonic read state, unread projections, bounded bulk owners and transports exist. Visibility-scoped storefront bulk commands and maintainer PostgreSQL evidence remain. |
| `FORUM-17` | `planned` | Forum drafts/bookmarks with optional Notifications reminders and Media session references; no Forum timers or asset lifecycle. |
| `FORUM-18` | `planned` | Keep atomic Forum vote hardening in Forum. Move reusable reactions to `rustok-reactions`; move cross-domain reputation and achievements to shared owners. Forum provides subject authorization, events and UI composition. |
| `FORUM-19` | `planned` | Integrate `rustok-moderation-api`: Forum subject adapter, effect application and Forum-local restrictions. Moderation owns reports, cases, queues, decisions, appeals and cross-domain audit. |
| `FORUM-20` | `in_progress` | Rich visibility, create/moderate audiences, recipient-aware Notifications and inbox UI are largely source-complete. Remaining read/search/index/SEO/deep-link migration, scoped bulk reads, scheduled reconciliation/redaction, deliveries and PostgreSQL evidence remain. |
| `FORUM-21` | `in_progress` | A-X provide move, merge, split, fork and range owners/transports/UI. Retained owner/transport runtime evidence remains; localized canonical history is completed under FORUM-24. |
| `FORUM-22` | `planned` | Forum-owned topic kinds, Q&A/wiki/announcement policy and scheduled lifecycle. Polls use a neutral capability when available. |
| `FORUM-23` | `in_progress` | Visibility-safe Search projection/filtering, durable ordering, owner revisions and repair protocol exist. Maintainer PostgreSQL/Iggy and cross-module runtime evidence remain; kind/attachment filters wait on owners. |
| `FORUM-24` | `in_progress` | A-S plus the storefront descriptor correction provide topic/category routes, aliases/tombstones, transports, shared storefront mounts, SEO/hreflang, Search canonical URLs and source-ready evidence. Execute SQLite registered-host, mounted HTTP, browser navigation, PostgreSQL reindex and deployment reindex evidence. |
| `FORUM-25` | `planned` | Forum translation target/provider integration and complete multilingual/RTL UI. Translation workflow remains shared. |
| `FORUM-26` | `in_progress` | Forum trust and posting-policy facts exist. Add Moderation facts, shared Reputation facts, policy persistence/enforcement, shared rate-limit execution, duplicate detection, transports/UI and runtime evidence. |
| `FORUM-27` | `planned` | Compose the Profiles-owned public profile/directory with Forum stats, activity and permitted shared reputation/achievement views. No duplicate Forum profile store. |
| `FORUM-28` | `done` | Shared canonical editor, renderer, projections and Next/Leptos adapters. |
| `FORUM-29` | `planned` | Shared realtime transport integration with Forum cursors/revisions and canonical reload. |
| `FORUM-30` | `planned` | Complete Forum-owned admin product by composing owner transports and shared Moderation/Media/Reaction components. |
| `FORUM-31` | `planned` | Complete Forum storefront product by composing Profiles, Media, Reactions, Notifications, Search and Forum owners. |
| `FORUM-32` | `in_progress` | Widget contracts exist; richer widgets and observed Page Builder evidence remain. |
| `FORUM-33` | `planned` | Forum-local metrics and reconciliation providers integrated with platform observability/job owners. No second generic telemetry system. |
| `FORUM-34` | `planned` | Forum import/export adapter and NodeBB mapping over a bounded shared import runner when available. |
| `NOTIFY-00` | `in_progress` | Neutral API, optional runtime composition and Forum providers exist; executable distribution evidence remains. |
| `NOTIFY-01` | `in_progress` | Owner persistence and source inbox exist; final commands, global migrations, retention and reconciliation remain. |
| `NOTIFY-02` | `planned` | Preferences, quiet hours and digests. |
| `NOTIFY-03` | `in_progress` | Durable source acceptance and candidate materialization exist; policy, final rows, delivery and PostgreSQL lease evidence remain. |
| `NOTIFY-04` | `planned` | Owner inbox/read APIs; existing source slices must be reconciled into this task. |
| `NOTIFY-05` | `planned` | Delivery provider SPI. |
| `NOTIFY-06` | `planned` | Localized semantic templates. |
| `NOTIFY-07` | `planned` | Privacy, blocking and target-open authorization. |
| `NOTIFY-08` | `planned` | Notification UI ownership and parity evidence. |
| `NOTIFY-09` | `planned` | FBA and degraded profiles. |
| `LINK-FORUM-01` | `planned` | Forum-to-Notifications executable proof. |
| `LINK-FORUM-02` | `planned` | Profiles/Media/Forum executable proof. |
| `LINK-FORUM-03` | `planned` | Forum/Search/Index ordering and visibility proof. |
| `LINK-FORUM-04` | `planned` | Required/optional capability profiles and startup validation. |
| `LINK-FORUM-05` | `planned` | Waiver-free production release gate. |

## Corrected task boundaries

### `FORUM-13` and `FORUM-14`: Media integration

Media owns upload preparation/completion, blob storage, MIME, dimensions,
renditions, quarantine, deletion, delivery descriptors, shared-reference
lifecycle and reconciliation. Forum validates an owner descriptor and stores
only a tenant-scoped typed relation to the Media asset plus Forum usage/order,
caption and source revision.

A Media-disabled deployment keeps text-only Forum behavior. A command that
explicitly assigns an asset fails with typed capability unavailable. Reads may
omit optional presentation only for the documented disabled profile; provider
failures are not converted to absence.

### `FORUM-15` and `FORUM-27`: Profiles composition

Profiles is the canonical public member identity and directory owner. Forum
adds only bounded Forum statistics, trust, permitted activity and references to
shared reputation/achievement projections. Author lists use batched
`ProfilesReader` calls or a revisioned projection with reconciliation. Forum
never stores copied handle/display-name/avatar source data.

### `FORUM-18`: votes, reactions, reputation and achievements

Existing Forum votes remain Forum semantics and must be made atomic and
reconcilable. Reusable reactions become a separate platform capability with a
neutral subject identity, bounded catalog, actor uniqueness, idempotent writes,
aggregate projections and subject authorization adapters.

Forum supplies subject existence/current revision/visibility/reaction-policy
checks for topic and reply targets. The Reactions owner never reads Forum
tables. Forum UI consumes reaction ports and owner aggregates.

Reputation and achievements are separate from reactions. They consume semantic
facts from Forum, Blog, Comments, Groups, Profiles, Commerce and other modules.
Forum does not create a reusable reputation ledger or universal badge catalog.
Forum trust remains Forum-owned because it controls Forum posting policy.

### `FORUM-19`: Moderation integration

`rustok-moderation` owns reports, deduplicated cases, assignment queues,
immutable decisions/effects, appeals, application operations, receipts and
cross-domain audit. Forum publishes subject references and a
`ModerationSubjectCommandPort` adapter that applies validated effects to Forum
owner state.

Forum may store current Forum enforcement state required by its read/write
policy, such as topic visibility, lock state, premoderation and scoped posting
restrictions. It stores bounded decision provenance and an owner receipt, not
copied case notes, queue state or policy snapshots.

### `FORUM-26`: trust and anti-spam

Forum owns Forum trust and versioned posting decisions. Authoritative active
flag/moderation history comes from Moderation; reputation comes from the shared
Reputation owner; account and profile facts come from their owners; distributed
rate limiting is a shared execution capability. Missing required facts fail
closed with typed retryability. Optional scoring never becomes correctness.

### `FORUM-30` and `FORUM-31`: product UI

Module-owned Forum packages own navigation and Forum workflows. Shared modules
own their reusable widgets and transports. The Forum admin/storefront composes
Profiles cards, Media pickers/descriptors, Moderation case links, Reaction
controls, Notification inbox/navigation, Search, Translation and SEO through
public contracts. Hosts register and mount packages; they do not absorb policy.

## Execution order

### Track 1 — ownership-safe shared capabilities

1. Introduce the neutral `rustok-reactions` API and owner module as a separate
   platform task, without changing existing Forum vote behavior.
2. Add Forum topic/reply reaction subject adapters and bounded UI composition.
3. Introduce shared reputation/achievement owners only after at least two real
   producer modules agree on the neutral fact and projection contracts.
4. Integrate Forum with `rustok-moderation-api`; do not add Forum case queues.

### Track 2 — close already implemented Forum work

1. Execute and retain FORUM-24 SQLite registered-host evidence.
2. Add mounted shared-storefront HTTP response evidence.
3. Add browser navigation evidence for category/topic/reply Search links.
4. Execute the FORUM-24 PostgreSQL reindex harness and production deployment
   reindex evidence.
5. Execute retained FORUM-16, FORUM-21, FORUM-23 and FORUM-26 evidence.

### Track 3 — Profiles and Media integration

1. Finish category cover persistence and transport composition over Media.
2. Add topic/reply attachment relations over Media-owned sessions/assets.
3. Finish batched Profiles member-card and directory composition.
4. Retain LINK-FORUM-02 evidence for privacy, quarantine, deletion and disabled
   profiles.

### Track 4 — Forum-owned product semantics

1. Topic kinds and scheduled policies.
2. Drafts, bookmarks and optional reminder integration.
3. Read-state visibility-scoped bulk completion.
4. Posting-policy persistence/enforcement and Forum trust operations.
5. Full admin/storefront assembly.

### Track 5 — release integration

1. Complete Notifications policy/delivery and LINK-FORUM-01.
2. Complete Search/Index runtime evidence and LINK-FORUM-03.
3. Complete capability profiles, import adapter and production release gate.

Independent UI work may run only after the owner contracts it consumes are
stable.

## Compatibility and migration policy

- Existing Forum votes remain source-compatible while Reactions is introduced.
  Do not reinterpret vote rows as generic reactions without an explicit data
  migration and semantic mapping.
- Existing Forum statistics remain Forum projections; they may be inputs to a
  shared reputation owner but are not a reputation ledger.
- Existing topic/reply moderation state remains authoritative Forum state.
  New Moderation integration records decision provenance and idempotent effect
  application without moving Forum tables to Moderation.
- Existing Profile, Media, Notifications, Search and SEO integrations remain
  additive. No private-table fallback is permitted.
- Optional shared capabilities are default-off or explicitly unavailable until
  composed. Forum commands continue to commit semantic events when optional
  consumers are absent.

## Required verification

Use the task-specific existing commands plus these ownership guards:

```bash
node scripts/verify/verify-forum-shared-capability-ownership.mjs
cargo xtask module validate forum
npm run verify:forum:admin-boundary
npm run verify:forum:storefront-boundary
git diff --check
```

Runtime evidence must be generated by executable scenarios. Source contracts,
static fixtures and documentation do not promote runtime status.

## Release gates

Forum is not production-ready while any of the following is possible:

- cross-tenant owner references or direct private-table reads;
- copied Profile identity or Media lifecycle state in Forum;
- duplicate reaction, reputation, moderation or notification owners;
- partial category/topic/reply mutation or missing owner event;
- pending/private content leaking through reads, Search, SEO, notifications or
  reactions;
- another transport bypassing Forum visibility, trust or moderation effects;
- optional-module absence turning unrelated Forum commands into outages;
- unbounded lists, fan-out, imports or reconciliation;
- runtime claims without retained executable evidence.

## Decisions requiring an ADR to reopen

1. Profiles remains the sole public member identity and directory owner.
2. Media remains the sole binary asset and delivery-lifecycle owner.
3. Moderation remains the cross-domain report/case/decision/appeal/audit owner.
4. Notifications remains the inbox/fan-out/preferences/delivery owner.
5. Reusable reactions and cross-domain reputation/achievements are separate
   shared capabilities, not Forum-owned universal models.
6. Forum trust remains Forum-owned and is not equivalent to reputation.
7. Search, SEO, Translation, Outbox/Events and realtime transports remain shared
   platform capabilities consumed through public contracts.

## Immediate next action

Create the first bounded `rustok-reactions` platform slice: neutral subject and
reaction contracts, strict bounds, typed errors, idempotency identity and a
provider registry. Do not add persistence, Forum table reads, transports or UI
in that first slice. Follow it with a separate Forum topic/reply subject adapter
PR while preserving existing vote behavior.
