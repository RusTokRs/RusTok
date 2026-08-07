---
id: doc://crates/rustok-forum/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-forum
last_reviewed: 2026-08-07
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
| Reaction catalog, actor reactions and aggregate reaction counts | `rustok-reactions` | Publish Forum subject adapter and authorization; consume permitted semantic facts and compose UI. |
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
persistence, immutable catalog snapshots, shared Outbox command receipts,
atomic actor-state/aggregate updates, sealed semantic reaction events and
bounded aggregate reconciliation. Forum publishes a source-ready `topic`/`reply`
provider factory with exact active-state, visibility and current-revision
authorization plus a bounded single-`like` v1 catalog. Blog now supplies the
second real producer through the same neutral SPI using Blog-owned publication,
channel visibility and owner version, with a Blog+Reactions composition profile
in source. Optional owner selection, host materialization and executable source
evidence are source-ready. A bounded manifest-composed GraphQL read/write
transport over the neutral Reactions ports and the separate module-owned
`rustok-reactions-storefront` reaction controls are source-ready. Forum exposes
generic visibility-gated `forumStorefrontTopicCurrentRevision` and
`forumStorefrontReplyCurrentRevision` owner facts and carries them through a
dual-path storefront transport facade: native server functions for SSR/hydrate
and GraphQL for headless/CSR. The Forum storefront now publicly exposes those
generic facade functions as neutral host-extension facts. The storefront host
uses the selected canonical topic identity plus the Forum topic revision fact to
construct `ReactionSubjectUiRef` and mount the separate `ReactionBar` only when
the Reactions module is enabled. Forum owner/storefront packages still do not
depend on the Reactions owner or presentation package. Reply-level host
composition, maintainer lockfile/event-digest generation and retained
owner/provider/schema/runtime/UI evidence remain pending.

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
| `FORUM-18` | `in_progress` | Neutral API, optional owner registration/selection, tenant-composite persistence, shared receipts, atomic actor aggregates, semantic reaction events, bounded aggregate reconciliation, Forum topic/reply provider, Blog second producer, host materialization, composition-test source, bounded Reactions GraphQL transport, separate module-owned Reactions storefront controls, dual-path generic visibility-gated Forum topic/reply current-revision transport and selected-topic host UI composition are source-ready. Regenerate `Cargo.lock`/event digests, retain owner/event/repair/Forum+Blog/GraphQL/UI/runtime transport evidence, then add bounded reply-level host composition without moving reaction ownership into Forum; Forum votes remain separate. |
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
command execution, aggregate projections, semantic reaction events and bounded
aggregate repair. Forum owns topic/reply existence, current revision, visibility
and reaction-policy authorization through its provider.

The Forum provider factory supports `topic` and `reply`, checks tenant/source/
kind, active soft-delete state, approved/open lifecycle and the existing rich
audience visibility service, and returns one bounded single-selection `like`
catalog. The `topic`/`reply` provider factory current revision is
`latest captured Forum revision id + 1`, so the identity advances with captured
topic translation/metadata and reply-body edits. Missing and denied targets
share one `Unavailable` result; revision conflict is returned only after current
visibility succeeds. Delegated service/system access uses the existing exact
recipient-context port rather than inventing profile or authority storage.

Forum registers only the neutral provider factory and depends only on the API
crate through an explicit path. It does not depend on the Reactions owner and
does not add Reactions to Forum module dependencies. The optional
`mod-reactions` feature selects the owner independently in the
distribution/server, remains outside defaults, and materializes the Forum
provider only after host audience and recipient-context facts exist. This is the
optional owner selection and host materialization boundary.

The Reactions-disabled Forum composition remains valid: Forum commands and reads
continue without owner storage or a materialized reaction registry. Reactions
without Forum or Blog materializes an empty source registry; Forum with Reactions
materializes the `forum` source with `topic` and `reply` kinds. Blog is the
second real producer and materializes the `blog` source with `post` while using
Blog-owned positive version, `published` lifecycle and typed channel visibility.
Forum and Blog both depend only on the neutral API, so no producer implies the
Reactions owner. Executable source evidence exists for these profiles and the
selected-feature/missing-owner failure, but retained execution evidence remains
pending.

A changed Reactions command commits actor state, aggregate deltas, one sealed
`reactions.actor_state.changed` event and its completed owner receipt atomically.
No-op and receipt replay do not publish another fact. Reactions repair is bounded
to one exact stored subject and rebuilds only aggregate projection rows from
valid actor selections under the immutable current catalog. Missing/corrupt
catalog or actor-state corruption blocks repair; the repair never mutates Forum
content, visibility, lifecycle, votes or producer-private state.

The first Reactions transport slice is manifest-composed GraphQL over the
existing neutral read/write ports. Tenant comes only from `TenantContext`;
subject source/kind/UUID/revision remain caller input, while actor identity is
never accepted from the caller. Anonymous reads request public state only,
authenticated human reads may include their actor state, and writes require a
human-user principal. The mutation command UUID is also the owner idempotency
key. Producer visibility, lifecycle, revision and catalog decisions remain in
Forum/Blog providers, and GraphQL does not read producer-private storage.
Positive revisions and aggregate counts are exposed as decimal strings to avoid
GraphQL integer-width truncation. This is source-ready transport foundation, not
a frozen presentation contract; retained schema/runtime evidence remains.

The presentation owner is also separate. `rustok-reactions-storefront` provides
module-owned neutral controls over the Reactions GraphQL transport and has no
Forum/Blog/private-owner dependency. Forum does not copy those controls. Instead,
Forum exposes `forumStorefrontTopicCurrentRevision` and
`forumStorefrontReplyCurrentRevision` as generic Forum owner facts after the
same tenant/channel/audience gates as the corresponding storefront reads. The
Forum storefront facade selects native server functions for SSR/hydrate and
GraphQL owner fields for headless/CSR, preserving FFA transport parity while
returning only positive decimal revisions derived from Forum revision history.
Those generic facade functions are publicly exported for neutral host
extensions; neither adapter creates reaction subjects, catalogs, actor state or
commands. `apps/storefront` now supplies the first bounded host composition: it
routes Forum rendering through `ForumStorefrontComposition`, checks the tenant's
`reactions` module enablement, takes the exact selected topic id from the
canonical Forum route context, consumes `fetch_storefront_topic_current_revision`,
constructs the neutral `ReactionSubjectUiRef` in the host, and mounts the
separate `ReactionBar`. If Reactions is disabled, no topic is selected, the
producer fact is unavailable, or the optional UI request fails, Forum continues
to render without moving reaction state or policy into Forum. Reply-level host
composition remains a later bounded slice.

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
5. Executable composition profiles, sealed semantic reaction events and bounded aggregate reconciliation: source-ready; retain execution, rollback, replay and repair evidence.
6. Second producer and neutral-contract review: Blog `post` source and Blog+Reactions composition profile are source-ready; retain provider/host execution evidence before freezing shared presentation contracts.
7. Bounded Reactions GraphQL transport, separate module-owned Reactions storefront controls, dual-path generic visibility-gated Forum topic/reply current-revision transport and selected-topic neutral host composition are source-ready. Add bounded reply-level host composition without adding Reactions functionality to Forum, then retain runtime/browser evidence.
8. Introduce Reputation/Achievements only after at least two producers agree.
9. Integrate Forum with `rustok-moderation-api`; never add Forum case queues.

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
  required Forum module dependency; the same boundary is now proven by Blog as
  a second producer.
- Selecting `mod-reactions` without registering `ReactionsModule` is a startup
  configuration error, not an implicit empty owner.
- Reactions GraphQL derives tenant and actor scope from trusted request context;
  it does not accept caller-supplied tenant/actor identity or duplicate producer
  authorization in transport code.
- Forum current-revision reads remain generic Forum owner facts. They must not
  construct reaction subjects, import Reactions UI/owner crates or duplicate
  reaction state/commands inside Forum.
- Forum current-revision storefront adapters must preserve native/GraphQL path
  parity and the same visibility/status gates before exposing a revision.
- Cross-module Reactions presentation composition belongs to the storefront
  host. The host may import both producer and Reactions UI packages, but Forum
  owner/storefront packages must remain free of the Reactions owner/UI dependency.
- Reactions semantic event envelope identity is the admitted owner-operation
  UUID; it is not a Forum route/revision/vote identity.
- Reactions bounded reconciliation repairs aggregate projection only and cannot
  mutate Forum-owned subjects or reinterpret Forum votes.

## Required verification

```bash
node scripts/verify/verify-forum-shared-capability-ownership.mjs
node scripts/verify/verify-reactions-foundation.mjs
node scripts/verify/verify-reactions-owner-persistence.mjs
node scripts/verify/verify-forum-reaction-subject-provider.mjs
node scripts/verify/verify-blog-reaction-subject-provider.mjs
node scripts/verify/verify-reactions-host-composition.mjs
node scripts/verify/verify-reactions-composition-profiles.mjs
node scripts/verify/verify-reactions-events-reconciliation.mjs
node scripts/verify/verify-reactions-storefront-ui.mjs
node scripts/verify/verify-forum-storefront-topic-current-revision.mjs
node scripts/verify/verify-forum-storefront-reply-current-revision.mjs
node scripts/verify/verify-forum-storefront-current-revision-transport-parity.mjs
node scripts/verify/verify-forum-topic-reactions-storefront-composition.mjs
cargo test -p rustok-events reactions
cargo test -p rustok-reactions-api
cargo test -p rustok-reactions
cargo test -p rustok-reactions --features graphql graphql
cargo test -p rustok-reactions-storefront
cargo test -p rustok-forum reaction_subject
cargo test -p rustok-blog reaction_subject
cargo check -p rustok-reactions --features graphql --all-targets
cargo check -p rustok-reactions-storefront --all-targets
cargo check -p rustok-distribution --features "mod-forum mod-reactions"
cargo check -p rustok-server --no-default-features --features mod-reactions
cargo check -p rustok-server --no-default-features --features "mod-forum mod-reactions"
cargo test -p rustok-server --no-default-features --features "mod-blog mod-reactions" --test reactions_composition_profiles blog_with_reactions_materializes_post_provider
cargo run -p rustok-events --example event_contract_digests -- --write
cargo xtask module validate forum
npm run verify:forum:admin-boundary
npm run verify:forum:storefront-boundary
git diff --check
```

Tests, lockfile/event-digest generation and runtime evidence are maintainer-run.
Source contracts do not promote runtime status.

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

Regenerate the event-contract digests and `Cargo.lock`, then retain SQLite and
PostgreSQL evidence for changed/no-op/replayed reaction event cardinality,
rollback on event failure, concurrent actor writes, clean/blocked/drift bounded
aggregate reconciliation and repair receipt replay. Retain Blog provider and
Blog+Reactions composition evidence plus manifest-composed Reactions GraphQL
schema/runtime evidence for anonymous/authenticated reads, human-user writes,
tenant mismatch, idempotent replay and stale/denied subjects. Retain the separate
Reactions storefront source/runtime evidence plus Forum topic/reply current-
revision GraphQL/native transport parity evidence and selected-topic host
composition evidence. Then add bounded reply-level host composition from the
generic Forum reply revision facade without adding reaction catalogs, state,
commands or presentation ownership to Forum.