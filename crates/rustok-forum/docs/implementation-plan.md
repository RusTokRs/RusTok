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
order and release gates.

The exact pre-correction snapshot remains at
`docs/archive/implementation-plan-2026-08-06.snapshot` for audit only. It is not
authoritative. Do not copy ownership or task status from it.

Every pull request that changes a task below must update this plan when it
changes status, remaining scope, ownership, verification, compatibility,
migration or degraded-mode behavior. A task is `done` only after implementation,
integration, migration/backfill, tests, public contracts, documentation and
required runtime evidence are complete. Source-ready slices remain
`in_progress`.

## Product model

Forum is an installable domain application composed from platform modules. It
must not recreate common social-platform capabilities inside `rustok-forum`.
Each module owns one capability and Forum contributes only Forum-specific state,
policy, adapters, semantic events and UI composition.

Product inclusion does not imply Forum persistence ownership. Forum may present
profiles, media, reactions, reputation, moderation, notifications, search, SEO,
translation and realtime behavior while consuming those owners through public
contracts.

## Ownership rules

### Forum-owned

`rustok-forum` owns only:

- category hierarchy and policy;
- localized topic/reply content, lifecycle, revisions and route identity;
- topic kinds, accepted-solution semantics, subscriptions and Forum read state;
- Forum drafts and bookmarks unless a proven neutral owner replaces them;
- Forum attachment relations, usage, order, caption and source revision;
- Forum trust, posting policy and Forum-local enforcement state;
- Forum subject adapters for reactions and moderation;
- Forum visibility, source authorization, semantic events and projections;
- Forum statistics/activity projections and module-owned admin/storefront UI.

Forum tables may reference another owner by typed tenant-scoped identity, but
must not copy that owner's source-of-truth data or read its private tables.

### Shared owners

| Capability | Authoritative owner | Forum responsibility |
| --- | --- | --- |
| Login identity and sessions | `auth/users` | Carry trusted actor identity only. |
| Public handle, display name, biography, locale and profile privacy | `rustok-profiles` | Batch summaries and compose Forum statistics. |
| Avatar/banner and all binary asset lifecycle | `rustok-media` | Store typed media references and attachment policy only. |
| Follow/block relationship facts | `rustok-social-graph` and profile privacy ports | Request exact bounded facts; never read relation tables. |
| Reaction catalog, actor reactions and aggregate reaction counts | `rustok-reactions` | Publish Forum subject adapter, authorization, events and UI composition. |
| Cross-domain reputation ledger and achievements/badges | planned `rustok-reputation` / achievement capability | Publish semantic facts and display permitted projections. |
| Reports, cases, queues, decisions, appeals and cross-domain audit | `rustok-moderation` through `rustok-moderation-api` | Report subjects and apply validated effects to Forum state. |
| Notification inbox, fan-out, grouping, preferences, digests and deliveries | `rustok-notifications` | Publish source events/providers and authorize current targets. |
| Translation workflow | `rustok-translation` | Publish exact Forum translation targets and apply owner writes. |
| Search storage and retrieval | `rustok-search`; generic indexing in `rustok-index` | Publish visibility-safe projections and repair sources. |
| SEO aggregation and host head composition | `rustok-seo` | Publish canonical Forum targets and structured semantics. |
| Durable event delivery | `rustok-outbox` / `rustok-events` | Commit Forum state and semantic events atomically. |
| Realtime transport | shared host/runtime capability | Publish revisions/cursors and reload canonical owner state. |
| Import orchestration | shared bounded import framework when available | Provide Forum mapping, validation, receipts and reconciliation. |

### Non-duplication gates

Without a platform ADR and ownership migration, the following are forbidden:

- copied Profile identity, avatar or a second member directory;
- Forum-owned uploads, storage keys, delivery URLs, quarantine or Media deletion;
- Forum-owned report/case/appeal queues or a second cross-domain moderation log;
- Forum-owned notification inbox, preferences, grouping, digests or delivery;
- a Forum-specific reaction catalog after `rustok-reactions` composition;
- a reusable Forum-only reputation ledger or universal badge catalog;
- transport-local visibility/reaction/moderation policy;
- direct reads of another module's persistence tables.

Optional capabilities must expose explicit unavailable/degraded behavior. Their
absence must not break unrelated Forum owner commands.

## Current verified baseline

Forum already provides categories, localized topics/replies, lifecycle,
revisions/tombstones, bounded reads, subscription levels, accepted solutions,
tags, Forum statistics, transactional events, visibility-aware Search/SEO,
shared rich text, module-owned admin/storefront packages and Page Builder
contracts.

Profiles, Media, Social Graph, Reactions, Moderation, Notifications,
Translation, SEO, Search/Index, Outbox/Events, Taxonomy, Workflow, Comments,
Groups and Channel are separate platform capabilities. New Forum work must
integrate them instead of cloning their data models.

The Reactions owner now has neutral bounded API contracts, unique source
provider/factory registries, PostgreSQL/SQLite-compatible tenant-composite
persistence, immutable catalog snapshots, shared Outbox command receipts and
atomic actor-state/aggregate updates. Forum publishes a source-ready
`topic`/`reply` provider factory with exact active-state, visibility and current-
revision authorization plus a bounded single-`like` v1 catalog. Optional owner
selection and host materialization are now source-ready in the distribution and
server, after Forum audience and recipient-context providers. Maintainer lockfile
generation and enabled/disabled runtime evidence remain pending. No reaction
event/reconciliation layer, transport or UI exists yet.

## Program ledger

| Task | Status | Current result and remaining deliverable |
| --- | --- | --- |
| `FORUM-00` | `done` | PostgreSQL/SQLite runtime baseline. |
| `FORUM-01` | `done` | Tenant-composite integrity and locale width. |
| `FORUM-02` | `done` | Typed topic/reply lifecycle and revisions. |
| `FORUM-03` | `done` | Atomic category writes and translations. |
| `FORUM-04` | `done` | Tree, placement, policy, subtree lifecycle and admin DnD. |
| `FORUM-05` | `done` | Serialized publication-aware counters. |
| `FORUM-06` | `done` | Locked topic and moderation publication semantics. |
| `FORUM-07` | `done` | Monotonic reply positions. |
| `FORUM-08` | `done` | Revisions, tombstones and owner soft-delete paths. |
| `FORUM-09` | `done` | Versioned Forum event catalog and journal. |
| `FORUM-10` | `done` | Bounded cursor reads. |
| `FORUM-11` | `done` | Subscription levels and participation policy. |
| `FORUM-12` | `in_progress` | Mention/quote relations and notification source exist. Runtime execution, profile/block privacy, moderator audience and final Notifications evidence remain. |
| `FORUM-13` | `in_progress` | Optional Media presentation policy exists. Add typed category-cover owner command, transports, UI and runtime evidence; Media keeps lifecycle ownership. |
| `FORUM-14` | `planned` | Forum attachment relations over Media-owned sessions/assets; no upload or asset lifecycle in Forum. |
| `FORUM-15` | `in_progress` | Profiles supplies `ProfilesReader`. Finish member-card composition, privacy/block behavior, Forum-stat enrichment and no-N+1 evidence. |
| `FORUM-16` | `in_progress` | Read state, unread projections, bounded bulk owners and transports exist. Visibility-scoped storefront bulk commands and PostgreSQL evidence remain. |
| `FORUM-17` | `planned` | Forum drafts/bookmarks with optional Notifications reminders and Media references. |
| `FORUM-18` | `in_progress` | Neutral API, optional owner registration/selection, tenant-composite persistence, shared receipts, atomic actor aggregates, Forum topic/reply provider factory and host materialization are source-ready. Regenerate `Cargo.lock`, retain owner and enabled/disabled composition evidence, then add events/reconciliation, transports/UI and runtime proof; Forum votes remain separate. |
| `FORUM-19` | `planned` | Integrate `rustok-moderation-api` subject/effect adapters and Forum-local restrictions. Moderation owns cases and audit. |
| `FORUM-20` | `in_progress` | Rich visibility and recipient-aware source/inbox slices largely exist. Complete remaining reads, Search/SEO/deep links, reconciliation, delivery and PostgreSQL evidence. |
| `FORUM-21` | `in_progress` | A-X provide move/merge/split/fork/range owners, transports and UI. Retained runtime evidence remains. |
| `FORUM-22` | `planned` | Forum-owned Q&A/wiki/announcement kinds and scheduled lifecycle. |
| `FORUM-23` | `in_progress` | Search projection/filtering, ordering, owner revisions and repair exist. PostgreSQL/Iggy and cross-module evidence remain. |
| `FORUM-24` | `in_progress` | A-S plus descriptor correction provide category/topic routes, transports, mounts, SEO/hreflang and Search URLs. Execute registered-host, HTTP, browser and reindex evidence. |
| `FORUM-25` | `planned` | Forum Translation provider and complete multilingual/RTL UI. |
| `FORUM-26` | `in_progress` | Forum trust/posting facts exist. Add Moderation/Reputation facts, persistence/enforcement, shared rate limits, transports/UI and evidence. |
| `FORUM-27` | `planned` | Compose Profiles directory/profile with Forum stats/activity and permitted reputation/achievements. |
| `FORUM-28` | `done` | Shared editor, renderer, projections and Next/Leptos adapters. |
| `FORUM-29` | `planned` | Shared realtime transport with Forum cursors/revisions and canonical reload. |
| `FORUM-30` | `planned` | Complete Forum admin by composing Forum and shared owners. |
| `FORUM-31` | `planned` | Complete Forum storefront by composing Profiles, Media, Reactions, Notifications and Search. |
| `FORUM-32` | `in_progress` | Widget contracts exist; richer widgets and observed Page Builder evidence remain. |
| `FORUM-33` | `planned` | Forum metrics/reconciliation providers integrated with platform operations. |
| `FORUM-34` | `planned` | Forum import/export adapter and NodeBB mapping over a shared runner. |
| `NOTIFY-00` | `in_progress` | Neutral API/runtime composition and Forum providers exist; executable distribution evidence remains. |
| `NOTIFY-01` | `in_progress` | Persistence/source inbox exist; final commands, migrations, retention and reconciliation remain. |
| `NOTIFY-02` | `planned` | Preferences, quiet hours and digests. |
| `NOTIFY-03` | `in_progress` | Source acceptance/candidate materialization exist; policy, final rows, delivery and lease evidence remain. |
| `NOTIFY-04` | `planned` | Owner inbox/read APIs and reconciliation of existing slices. |
| `NOTIFY-05` | `planned` | Delivery provider SPI. |
| `NOTIFY-06` | `planned` | Localized semantic templates. |
| `NOTIFY-07` | `planned` | Privacy, blocking and target-open authorization. |
| `NOTIFY-08` | `planned` | Notification UI and parity evidence. |
| `NOTIFY-09` | `planned` | FBA/degraded profiles. |
| `LINK-FORUM-01` | `planned` | Forum-to-Notifications proof. |
| `LINK-FORUM-02` | `planned` | Profiles/Media/Forum proof. |
| `LINK-FORUM-03` | `planned` | Forum/Search/Index ordering and visibility proof. |
| `LINK-FORUM-04` | `planned` | Capability profiles and startup validation. |
| `LINK-FORUM-05` | `planned` | Waiver-free production release gate. |

## Corrected task boundaries

### `FORUM-13`/`FORUM-14`: Media

Media owns upload, blobs, MIME, dimensions, renditions, quarantine, deletion,
delivery and reconciliation. Forum stores only typed tenant-scoped relations,
Forum usage/order/caption and source revision. Text-only Forum remains available
when Media is disabled.

### `FORUM-15`/`FORUM-27`: Profiles

Profiles owns public member identity and directory. Forum adds bounded stats,
trust and activity. Author lists use batched `ProfilesReader` calls or a
revisioned reconciled projection. Forum never stores copied profile source data.

### `FORUM-18`: votes, reactions, reputation and achievements

Existing Forum votes remain Forum semantics and must be hardened independently.
`rustok-reactions` owns reusable reaction catalogs, actor state, shared-receipt
command execution and aggregate projections. Forum owns topic/reply existence,
current revision, visibility and reaction-policy authorization through its
provider.

The Forum provider factory supports `topic` and `reply`, checks tenant/source/
kind, active soft-delete state, approved/open lifecycle and the existing rich
audience visibility service, and returns one bounded single-selection `like`
catalog. Its current revision is `latest captured Forum revision id + 1`, so the
identity advances with captured topic translation/metadata and reply-body edits.
Missing and denied targets share one `Unavailable` result; revision conflict is
returned only after current visibility succeeds. Delegated service/system access
uses the existing exact recipient-context port rather than inventing profile or
authority storage.

Forum registers only the neutral provider factory and depends only on the API
crate through an explicit path. It does not depend on the Reactions owner and
does not add Reactions to Forum module dependencies. The optional `mod-reactions`
feature selects the owner independently in the distribution/server, remains
outside defaults, and materializes the Forum provider only after host audience
and recipient-context facts exist.

The Reactions-disabled Forum composition remains valid: Forum commands and reads
continue without owner storage or a materialized reaction registry. Reactions
without Forum materializes an empty source registry; Forum with Reactions
materializes the `forum` source with `topic` and `reply` kinds.

Reputation and achievements remain separate shared capabilities consuming
semantic facts. Forum trust remains Forum-owned because it controls Forum
posting policy.

### `FORUM-19`: Moderation

Moderation owns reports, cases, queues, decisions/effects, appeals, application
operations, receipts and cross-domain audit. Forum publishes subject references
and applies validated effects through `ModerationSubjectCommandPort`, retaining
only Forum enforcement state and bounded decision provenance.

### `FORUM-30`/`FORUM-31`: UI composition

Forum packages own Forum workflows. Shared modules own reusable profile, media,
moderation, reaction, notification, search, translation and SEO components.
Hosts register/mount packages and do not absorb policy.

## Execution order

### Track 1 — shared capabilities

1. Reactions neutral API/optional module foundation: source-ready, maintainer verification pending.
2. Reactions owner persistence/atomic aggregates: source-ready, lockfile and runtime evidence pending.
3. Forum `topic`/`reply` provider factory and disabled Forum profile: source-ready, maintainer verification pending.
4. Optional distribution/server selection and host materialization after Forum facts: source-ready, maintainer verification pending.
5. Retain enabled/disabled host composition and owner persistence evidence.
6. Add semantic events/reconciliation and a second producer before freezing shared presentation contracts.
7. Introduce Reputation/Achievements only after at least two producers agree.
8. Integrate Forum with `rustok-moderation-api`; never add Forum case queues.

### Track 2 — close existing Forum work

1. Execute FORUM-24 SQLite registered-host evidence.
2. Add mounted shared-storefront HTTP evidence.
3. Add browser navigation evidence for category/topic/reply Search links.
4. Execute PostgreSQL and deployment reindex evidence.
5. Execute retained FORUM-16/21/23/26 evidence.

### Track 3 — Profiles/Media and Forum product

1. Category cover and attachment relations over Media.
2. Batched Profiles member composition.
3. Topic kinds, drafts/bookmarks, read-state bulk completion and trust enforcement.
4. Full admin/storefront assembly and release integrations.

## Compatibility and migration

- Existing Forum votes remain source-compatible. Do not reinterpret them as
  reactions without explicit semantic mapping and migration.
- Forum statistics are projections, not a reputation ledger.
- Forum moderation state remains authoritative Forum state while Moderation
  decisions are applied idempotently through an adapter.
- Profile, Media, Notifications, Reactions, Search and SEO integrations are
  additive; private-table fallbacks are forbidden.
- Reactions remains optional and outside server defaults and `default_enabled`
  until persistence, adapters and degraded profiles are executable and verified.
- Reactions owner tables contain no Forum routes, content, visibility or copied
  profile data.
- Forum's neutral Reactions API dependency does not make the Reactions owner a
  required Forum module dependency.
- Selecting `mod-reactions` without registering `ReactionsModule` is a startup
  configuration error, not an implicit empty owner.

## Required verification

```bash
node scripts/verify/verify-forum-shared-capability-ownership.mjs
node scripts/verify/verify-reactions-foundation.mjs
node scripts/verify/verify-reactions-owner-persistence.mjs
node scripts/verify/verify-forum-reaction-subject-provider.mjs
node scripts/verify/verify-reactions-host-composition.mjs
cargo test -p rustok-reactions-api
cargo test -p rustok-reactions
cargo test -p rustok-forum reaction_subject
cargo check -p rustok-distribution --features "mod-forum mod-reactions"
cargo check -p rustok-server --no-default-features --features "mod-forum mod-reactions"
cargo xtask module validate forum
npm run verify:forum:admin-boundary
npm run verify:forum:storefront-boundary
git diff --check
```

Tests, lockfile generation and runtime evidence are maintainer-run. Source
contracts do not promote runtime status.

## Release gates

Forum is not production-ready while there are cross-tenant references, copied
owner data, duplicate capability owners, partial owner mutations, private data
leaks, transport policy bypasses, optional-capability outages, unbounded work or
runtime claims without retained executable evidence.

## Decisions requiring an ADR to reopen

1. Profiles remains the sole public member identity and directory owner.
2. Media remains the sole binary asset and delivery-lifecycle owner.
3. Moderation remains the cross-domain report/case/decision/appeal/audit owner.
4. Notifications remains the inbox/fan-out/preferences/delivery owner.
5. Reactions and cross-domain reputation/achievements are separate shared owners.
6. Forum trust remains Forum-owned and is not equivalent to reputation.
7. Search, SEO, Translation, Outbox/Events and realtime remain shared.

## Immediate next action

Retain source/runtime evidence for Forum without Reactions, Reactions without
Forum and Forum with Reactions, including the selected-feature/missing-registry
startup failure. Then add semantic reaction events and reconciliation before any
transport or UI slice.
