---
id: doc://crates/rustok-forum/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-forum
last_reviewed: 2026-08-08
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

Forum Page Builder contribution metadata, Fly identity, owner-preview and
owner-property source are now source-ready. Canonical `rustok-module.toml` owns
two complementary version-pinned admin contributions: `rustok.forum.widget-catalog`
admits the `forum.topic_list`, `forum.topic_detail` and `forum.reply_stream` blocks
plus owner-schema-reference property editors under `tree + properties`, while
`rustok.forum.widget-preview` admits only their renderer contracts under
`preview`. `rustok-forum-admin/build.rs` consumes the shared platform contribution
normalizer, and the Forum admin package registers real Fly component/block
identities plus a `ContributionAdapter` without reading Forum owner data.

Owner preview data remains Forum-owned end to end. `ForumWidgetPreviewService`
first normalizes configuration through the existing widget contract, then applies
Forum visibility/RBAC and bounded owner reads for all three widget types. The
HTTP owner route `/api/forum/widgets/preview`, SSR-only Forum admin transport and
provider-neutral Page Builder contribution host compose that source into the real
Pages admin route only when Forum is tenant-enabled and its exact manifest
permission is effectively granted. `preview_off` filters the renderer
contribution without removing `tree + properties`.

Owner-backed property editing is also source-ready. Contribution metadata keeps
only `forum_widget_owner_schema_ref_v1` pointers; the Forum admin property
transport verifies the exact generated descriptor, loads the current schema body
from `ForumWidgetContractService::catalog`, and validates candidate configuration
through `ForumWidgetContractService::validate_props`. The provider-neutral Page
Builder property panel supports the current bounded Forum schema subset and
applies only valid owner `normalized_props` through the ordinary Fly
`EditorCommand::Patch` history path. It rechecks selection/schema identity around
asynchronous validation and never writes Forum owner data, tenant identity or actor
identity into the document. Retained runtime/browser evidence and the observed
tenant Wave remain open; no source may treat source-ready host composition as
observed rollout proof.

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
and GraphQL for headless/CSR. The Forum storefront publicly exposes those
generic facade functions as neutral host-extension facts. The storefront host
now composes one exact Forum reaction target at a time: the selected canonical
topic by default, or the one valid explicitly selected reply when the canonical
topic route carries `?reply={reply_id}`. In both cases the host combines only the
Forum identity/current-revision fact with `ReactionSubjectUiRef` and mounts the
separate module-owned `ReactionBar` when Reactions is enabled. It never fans out
revision requests across the visible reply list. Forum owner/storefront packages
still do not depend on the Reactions owner or presentation package. Rust
Playwright source now covers the observable selected-topic and selected-reply
composition markers against maintainer-supplied real storefront URLs; browser
execution is still pending. The current lockfile contains the
`rustok-reactions-storefront` package entry; event-digest generation and retained
owner/provider/schema/runtime/UI evidence remain pending.

Forum now also publishes source-ready `rustok-moderation-api` subject adapter
factories for `forum_topic` and `forum_post` without depending on the Moderation
owner crate. FORUM-19 adds dedicated tenant-scoped Forum moderation subject
revision clocks for topic/reply core, content, lifecycle and local enforcement
state instead of reusing the narrower Reactions/content revision. The bounded
application source reuses the shared Outbox owner-operation receipt ledger,
fences the active subject plus that revision clock, accepts only trusted
service/system application callers and now supports `NoDomainMutation` for both
subject kinds, permanent topic lock, exact `SetVisibility(Hidden)`, exact
`SetVisibility(Removed)` and exact `RejectPublication` for Forum replies.
Approved-to-hidden and approved-to-rejected use the established
topic/category/author public counter accounting and every changed lifecycle
mutation writes the canonical `ForumReplyStatusChanged` event; already-hidden
and already-rejected are no-ops. Removed reuses the complete
`ReplyService::remove_in_tx` owner path for accepted-solution cleanup,
soft-delete/tombstone capture, public/solution accounting and the canonical
deleted status event/projection in the same fenced receipt transaction.
`SetVisibility(Unpublished)` remains fail-closed and distinct from
`RejectPublication`; it is not approximated as `ReplyStatus::Rejected`.

The optional server host source-materializes the neutral Moderation adapter
registry when `mod-moderation` and `ModerationModule` are both selected. A
Moderation-only profile materializes an empty registry; Forum+Moderation
materializes exactly the topic/reply adapters; Forum without Moderation remains
valid. The Moderation owner has source-ready durable application operations,
one-attempt dispatch, shared module-work scheduling, atomic application/case
audit lifecycle and bounded replay-safe operator recovery. Human operator
requeue is limited to `rejected`/`operator_review`, preserves the immutable
decision UUID and returns work to the existing scheduler/dispatcher; `applied`
can never be requeued. Pre-audit terminal rows can be reconciled to Moderation
case state without invoking Forum or fabricating historical lifecycle facts. A
true stale-revision re-review is explicitly a new case/new immutable decision on
a fresh producer-supplied revision, never a retargeted old decision. Retained
scheduler/runtime/recovery/concurrency evidence and authorized admin transport
remain pending.

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
| `FORUM-18` | `in_progress` | Neutral API, optional owner registration/selection, tenant-composite persistence, shared receipts, atomic actor aggregates, semantic reaction events, bounded aggregate reconciliation, Forum topic/reply provider, Blog second producer, host materialization, composition-test source, bounded Reactions GraphQL transport, separate module-owned Reactions storefront controls, dual-path generic visibility-gated Forum topic/reply current-revision transport, bounded selected-topic/selected-reply host UI composition and Rust Playwright browser-evidence source are ready. Retain the browser execution plus event-digest, owner/event/repair/Forum+Blog/GraphQL/UI/runtime evidence and release lockfile verification; Forum votes remain separate and no reaction ownership moves into Forum. |
| `FORUM-19` | `in_progress` | Neutral `forum_topic`/`forum_post` adapter factories, dedicated Forum moderation subject revision clocks, shared receipt/revision fencing, trusted application callers, permanent topic lock, exact reply Hidden/Removed/RejectPublication, optional host materialization, Moderation-owned durable application operations, bounded one-attempt exact adapter dispatch, shared `ModuleWorkScheduler`, atomic application/case audit lifecycle, same-decision human operator requeue for rejected/operator-review, and truthful legacy-terminal reconciliation without domain invocation are source-ready. Applied decisions cannot be requeued; true stale-revision re-review requires a fresh case/new immutable decision. Retain host/scheduler/recovery plus PostgreSQL/SQLite migration/lease/dispatch/lost-response/concurrency and Forum accounting/event/tombstone/solution evidence; keep `Unpublished` distinct/fail-closed. Remaining code/product work is authorized Moderation admin transport/RBAC and explicit fresh-revision re-review flow. Moderation keeps cases, decisions, appeals and audit. |
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
| `FORUM-32` | `in_progress` | Generated Forum Fly blocks/renderers/property contracts, Forum-owned preview service/HTTP/native transport, provider-neutral Pages host composition and owner-backed schema/validation property editing are source-ready. Retained runtime/browser evidence and observed Page Builder Wave evidence remain. |
| `FORUM-33` | `in_progress` | Bounded snapshot-consistent owner counter reconciliation report, strict operator GraphQL admission and baseline platform telemetry are source-ready. Retain SQLite/PostgreSQL execution evidence, add bounded continuation and remaining reconciliation/metrics. Repair remains blocked on dry-run/audit/idempotent job state; CLI integration awaits a synchronized dependency/lock update. |
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
commands. `apps/storefront` owns the bounded cross-module presentation: it routes
Forum rendering through `ForumStorefrontComposition`, checks the tenant's
`reactions` module enablement and consumes only the generic Forum revision
facades. An explicit valid reply selection on a Forum topic route takes
precedence over the topic target, so the host requests at most one current
revision and mounts at most one separate `ReactionBar`. With no valid selected
reply it composes the selected topic through
`fetch_storefront_topic_current_revision`; with a valid selected reply it uses
`fetch_storefront_reply_current_revision` and constructs
`ReactionSubjectUiRef("forum", "reply", ...)` only in the host. The Forum reply
list and `ReplyCard` stay reaction-agnostic and no revision request is fanned out
across visible replies. Disabled Reactions, unavailable producer revision or
invalid subject construction leaves Forum rendering intact without host-owned
Reactions error presentation.

The Rust E2E crate now includes
`tests/e2e-rust/tests/leptos_storefront_forum_reactions.rs`. Maintainer execution
supplies one canonical visible topic URL and the same topic route with one valid
`?reply=<uuid>` selection. The browser harness requires the topic document to
render only `data-storefront-composition="forum-topic-reactions"` and the reply
selection to render only `data-storefront-composition="forum-reply-reactions"`.
It observes the mounted host output only; it does not call Reactions transport or
Forum revision fields directly, seed fixtures or bypass producer authorization.
Source presence does not count as retained browser execution evidence.

Reputation and achievements remain separate shared capabilities consuming
semantic facts. Forum trust remains Forum-owned because it controls Forum
posting policy.

### `FORUM-19`: Moderation

Moderation owns reports, cases, queues, immutable decisions/effects, appeals,
application orchestration/retries and cross-domain audit. Forum publishes
subject adapters and applies validated effects through
`ModerationSubjectCommandPort`, retaining only Forum subject revision/local
enforcement state and bounded shared-receipt provenance.

Forum registers neutral factories for `forum_topic` and `forum_post` while
depending only on `rustok-moderation-api`. The adapter reuses
`rustok-outbox::idempotency` rather than creating a Forum application receipt
table. The Moderation decision UUID must equal the trusted write `PortContext`
idempotency key; shared receipt admission binds the full command and happens
before subject reads so replay does not re-evaluate a changed Forum subject.
Direct user callers are rejected; only service/system orchestration callers may
enter the application port.

The optional server host materializes the neutral subject-adapter registry only
when `mod-moderation` selects the owner and `ModerationModule` is present in the
supplied `ModuleRegistry`. Materialization happens after `HostRuntimeContext`
exists and after Forum host facts are composed. Moderation without Forum produces
a valid empty registry; Forum+Moderation materializes `forum/forum_topic` and
`forum/forum_post`; Forum without Moderation remains available with
unmaterialized neutral factories. Missing owner, factory build, duplicate-key and
factory-key mismatch failures remain startup errors.

The server's existing generic module-work bootstrap owns background task
lifecycle. `ModerationModule` contributes one
`rustok_runtime::ModuleWorkRegistration` for worker slug
`moderation_decision_application`; no Forum-specific switch and no bespoke
Moderation `tokio::spawn`/polling interval are introduced. The bootstrap starts
registered module work only in runtime modes that run background workers and
uses the deployment `StopHandle` to stop future claims while allowing already
claimed work to finish.

The Moderation owner persists one durable `moderation_application_operations`
row per typed immutable decision. New decision + typed effect + pending operation
+ `case_decided` event + Moderation command receipt share one owner transaction.
Upgrade backfill creates pending operations only for decisions that already have
`moderation_decision_effects`; historical `effect: None` decisions remain
non-dispatchable. The operation snapshots decision hash plus exact reviewed
subject identity and supports bounded due reads, fresh UUID lease-token CAS claim,
expired-lease reclaim, explicit retry scheduling, rejected/operator-review
terminal states and exact `ModerationDecisionApplication` evidence before
`applied`. This state belongs exclusively to Moderation; Forum neither stores nor
reads it.

The Moderation one-attempt dispatcher is source-ready. It claims at most one
exact due tenant/decision operation, reconstructs the immutable command from
Moderation decision/effect/case state, validates exact decision hash and reviewed
subject, looks up only the stored module/kind adapter and invokes it as trusted
service `rustok-moderation`. The domain `PortContext.idempotency_key` remains the
immutable decision UUID across all lease attempts; the lease token only varies the
attempt correlation. Missing adapter and retryable neutral errors schedule
bounded retry. Non-retryable `Conflict` (including stale reviewed revision) and
`InvariantViolation`, corrupt immutable command state and mismatched successful
application evidence require operator review. Other non-retryable neutral port
errors are rejected. A successful result becomes applied only after exact
evidence and live-lease validation.

The shared scheduler source discovers at most one earliest-due Moderation
candidate per pass and does not create the durable lease. Its generic
`ModuleWorkItem.lease_token` is envelope identity only. The handler delegates to
`dispatch_application_operation_once`, which repeats the canonical due predicate
and performs the sole authoritative Moderation CAS before any domain adapter is
called. Two hosts may discover the same candidate, but only one can win that CAS;
the loser performs no domain mutation. Generic module-work completion is a no-op
because `moderation_application_operations` remains the sole durable outcome
source.

Moderation operation state, case lifecycle and the existing `moderation_events`
owner audit ledger advance atomically below the dispatcher for claims/finalizers
executed after this source is active. The first winning claim moves a `decided`
case to `applying_decision` and increments its revision; retries/reclaims while
already applying do not bump the case revision again. Retryable outcomes keep the
case applying. Accepted matching application evidence moves the operation to
`applied` and the case to `closed`, sets `closed_at`, releases the active
deduplication key and writes `application_applied` plus `case_closed` in the same
owner transaction. Rejected and operator-review remain distinct application
outcomes but both fail closed at case level by moving to `escalated` with the
matching application audit event plus `case_escalated`. Escalated cases retain
their active identity for future operator recovery/report attachment; only closed
cases release it. These are internal Moderation owner audit events, not a newly
frozen typed cross-domain event family.

If the operation CAS, case revision CAS or audit insert fails, the Moderation
transaction rolls back instead of partially advancing operation/case state. A
crash after a committed claim leaves the case applying until the existing lease
is reclaimed. Lost-response retries still call Forum with the immutable decision
UUID idempotency key, so an already committed Forum mutation replays its shared
receipt before Moderation records application success and closes the case.

Moderation now also owns replay-safe application recovery commands. A human-user
operator may requeue only a `rejected` or `operator_review` operation. The command
binds an idempotency receipt, exact expected case revision and bounded reason,
validates immutable decision/hash/subject/case identity, moves the operation to
`retryable` due now and advances `escalated` (or legacy pre-audit `decided`) to
`applying_decision`. It never requeues `applied` and never calls a Forum adapter
directly; the existing scheduler/dispatcher remains the next execution path with
the same immutable decision UUID domain idempotency key.

Pre-audit terminal application rows can now be reconciled without manufacturing
history. Exact stored terminal truth maps `applied -> closed` and
`rejected|operator_review -> escalated`; an already-consistent case is a no-op.
Applied reconciliation requires stored applied revision/time and closes the case
at the **current reconciliation time**, releasing the active case key. The owner
writes only present-time `application_legacy_terminal_reconciled` and
`case_legacy_terminal_reconciled` audit facts. It does not emit fake historical
application/case lifecycle events and does not invoke Forum/domain mutation.

A true stale-revision re-review remains a new review, not a recovery mutation.
The reviewed revision is immutable decision identity, so Moderation must use a
new case and new immutable decision built from a freshly authorized producer
revision. The old escalated case/decision remains historical truth; no recovery
path may retarget it.

The existing Reactions/current-revision clock is intentionally not reused for
Moderation because it does not advance on every lifecycle/enforcement mutation.
Migration `m20260807_000027_add_forum_moderation_subject_revisions` introduces
`forum_topic_moderation_subject_revisions` and
`forum_reply_moderation_subject_revisions`: tenant-scoped Forum-owned current
revision rows, backfilled for existing subjects and maintained by PostgreSQL and
SQLite triggers over topic/reply core, content, lifecycle and local enforcement
changes. They are not case/decision/audit/application-attempt storage and are not
Reactions state.

The adapter fences both the exact active Forum subject and its dedicated
moderation revision row before comparing `ModerationSubjectRef.revision`.
PostgreSQL uses a serializable owner transaction plus row locks; SQLite reserves
the writer through the dedicated revision row without issuing a no-op update on
the subject. A stale decision fails without mutation and is never retargeted.
Successful domain mutation and completed shared receipt commit atomically.
Non-retryable failures may be stored as terminal shared receipts, while retryable
storage/serialization failures leave the processing lease reclaimable rather
than freezing transient failure into replay.

The bounded effect set now supports `NoDomainMutation` for topic/reply, permanent
topic lock through `TopicService::set_locked_in_tx`, exact reply
`SetVisibility { state: Hidden }`, exact reply
`SetVisibility { state: Removed }`, and exact reply `RejectPublication`.
A real lock/hide/reject/removal mutation must advance the dedicated moderation
subject revision in the same transaction and the adapter returns that
post-application value as `ModerationDecisionApplication.applied_revision`; a
true no-op retains the reviewed revision. Missing or non-advancing revision state
fails as an invariant rather than guessed success.

Reply `Hidden` maps only to the established `ReplyStatus::Hidden` lifecycle, and
`RejectPublication` maps only to the established Forum moderator rejection
action with target `ReplyStatus::Rejected`. Both use the same bounded non-public
status owner primitives: an already-target reply is a no-op, each changed
transition must pass the existing state machine, and an Approved source reply
decrements topic/category/author public reply accounting. Other valid
non-public-to-non-public transitions change lifecycle state without public
counter deltas. Every changed hide/rejection writes the canonical
`ForumReplyStatusChanged` root event, already a Forum Search full-scope source,
and category projection invalidation is emitted when public counters changed.
Status, counters/statistics, event/projection, moderation revision and shared
receipt therefore commit or roll back together.

Neutral `SetVisibility { state: Unpublished }` is a different Moderation effect
from `RejectPublication`. Forum has an exact existing rejection lifecycle but no
separate exact unpublished lifecycle state, so `Unpublished` remains fail-closed
and must not be mapped to `ReplyStatus::Rejected`.

Reply `Removed` is not a status-only mapping. Direct delete and Moderation removal
share `ReplyService::remove_in_tx`, which claims the active reply, validates the
existing transition to `ReplyStatus::Deleted`, removes an accepted-solution
relation when present, performs the established `status = deleted` plus
`deleted_at` soft delete so delete revisions/tombstone history are captured, and
applies the existing public-reply and solution-stat accounting. The adapter then
writes the same canonical `ForumReplyStatusChanged` fact with `new_status =
deleted` and category projection invalidation when public counters changed.
Soft-delete/tombstone, solution cleanup, accounting, event/projection, moderation
revision and completed receipt commit or roll back together. Receipt replay
happens before subject reads; a fresh attempt against an already removed subject
is unavailable rather than re-applied.

Temporary locks fail closed because Forum does not yet own expiry-safe moderation
enforcement state. The remaining Moderation effect catalog stays pending. The
remaining FORUM-19 orchestration/product gap is authorized Moderation admin
transport/RBAC for the source-ready recovery commands and an explicit
fresh-producer-revision -> new case -> new immutable decision re-review flow. It
must not move recovery state into Forum or bypass the existing shared-scheduler +
one-attempt dispatch path.

## `FORUM-32` — Page Builder and widget evolution

**Status:** `in_progress`

Forum contribution discovery, Fly contract registration, owner-backed preview
and owner-backed property editing are source-ready. Canonical
`rustok-module.toml` owns the exact three widget identities and splits
authoring/property admission from preview admission: `rustok.forum.widget-catalog`
requires `tree + properties`, while `rustok.forum.widget-preview` requires only
`preview`. This preserves property access when the provider is in `preview_off`
degraded mode.

Forum admin build generation uses the shared platform normalizer and exports the
version-pinned manifest plus stable component ids. The Forum Fly adapter registers
component/block identities and resolves renderer/property-editor contracts but
never imports Forum owner services or persistence. The Fly document stores only
versioned widget configuration in `props`.

Owner preview is explicit instead of reusing an incomplete generic topic
transport. `ForumWidgetPreviewService` first runs the existing widget contract
normalizer, applies Forum visibility/RBAC and executes bounded owner reads. Topic
list `activity/newest/top`, `include_pinned`, category filtering and pagination
are applied before result materialization; topic detail uses the existing exact
owner facade; reply-stream moderator mode requires `forum_replies:moderate` and
excludes deleted tombstones. `/api/forum/widgets/preview` and the Forum admin
server-function adapter expose this owner source without moving tenant/actor
selection into widget props.

The generic Page Builder admin package owns only provider-neutral extension ports:
external manifest, Fly registry installer, async preview, and owner property
schema/validation. `apps/admin` composes the Forum extension on the real Pages
route only when Forum is tenant-enabled. Contribution RBAC is resolved
server-side through `has_effective_permission`, so `manage -> read` semantics
remain identical to the platform permission model and the browser never receives
an unrelated permission snapshot. Selected-component owner preview remains
explicit Refresh-only and bounded to a 16 KiB JSON summary; owner data is never
persisted into Fly.

Property schema and validation also remain Forum-owned. The generated property
editor descriptor carries only `forum_widget_owner_schema_ref_v1`; the Forum admin
transport requires an exact descriptor match before loading the schema body from
`ForumWidgetContractService::catalog`, and candidate props are normalized only by
`ForumWidgetContractService::validate_props`. The generic property panel accepts
only the bounded current schema subset, rejects unsupported/additional fields,
rechecks the selected component around async validation, and executes
`ComponentPatch::set_field("props", normalized_props)` only for a valid object
response. Raw browser form state and Forum topic/reply/category data are never
persisted as a second owner authority.

Observed browser/runtime and tenant Wave evidence remain open. Page Builder stays
optional; Forum routes and owner state must remain available when builder,
properties or preview capabilities are disabled.

Replace the synthetic Wave packet with an observed tenant control-plane run only
after the `pages` reference-consumer gate. The observed run must retain the
existing all-on/publish-off/preview-off/builder-off profiles and correlate
`builder_write -> forum_publish -> storefront_read`; source-ready host composition
is not observed rollout evidence.

Verification cursors include:

```bash
node scripts/verify/verify-forum-page-builder-contribution-metadata.mjs
npm run verify:page-builder:consumer:forum
npm run verify:forum:wave-evidence-freshness
cargo xtask module validate forum
```

These commands remain maintainer-run in this source slice.

## `FORUM-33` — analytics, observability and reconciliation

**Status:** `in_progress`

FORUM-33A now provides a read-only `ForumCounterReconciliationService` and the
operator GraphQL field `forumCounterReconciliationReport(limit: Int)`. Tenant
identity comes only from the trusted request `TenantContext`; the transport
rejects auth/tenant mismatch and requires both effective
`forum_categories:manage` and `forum_topics:manage` permissions.

The first owner report reconciles three existing publication-accounting
invariants without mutating owner state: `forum_topics.reply_count` against
approved replies, `forum_categories.topic_count` against current topic rows, and
`forum_categories.reply_count` against approved replies across category topics.
It executes two bounded aggregate queries inside one database snapshot.
PostgreSQL uses `REPEATABLE READ READ ONLY`; SQLite keeps both reads in one
transaction. The default per-shape limit is 100 and the hard cap is 500, with
`has_more_topics`/`has_more_categories` signalling that continuation is required.

The service reuses the platform module-entrypoint/span/error metrics rather than
adding duplicate Forum metric families. Source-ready reporting does not claim
runtime observability evidence.

FORUM-33 remains `in_progress`. Retain clean/drift/snapshot SQLite and PostgreSQL
execution evidence; add bounded continuation beyond the first page; reconcile
accepted solutions, subscriptions, mentions, attachments and permitted
shared-owner projections; and add only non-duplicative operational metrics for
moderation, notification/search lag, unread/activity, locale fallback and spam
outcomes. Any write repair remains blocked until it has explicit operator RBAC,
dry-run behavior, durable audit, idempotent job/receipt state and bounded
recovery. A platform CLI adapter must be added only with its synchronized
workspace dependency and `Cargo.lock` update.

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
7. Bounded Reactions GraphQL transport, separate module-owned Reactions storefront controls, dual-path generic visibility-gated Forum topic/reply current-revision transport, bounded selected-topic/selected-reply neutral host composition and Rust Playwright browser-evidence harness are source-ready. Execute and retain the browser/runtime evidence without adding Reactions functionality to Forum.
8. Introduce Reputation/Achievements only after at least two producers agree.
9. Forum `rustok-moderation-api` topic/reply factories, dedicated Forum moderation subject revision clocks, shared receipt/revision fencing, permanent topic lock, exact reply Hidden/Removed/RejectPublication application, optional server host registry materialization, Moderation-owned durable application operations, bounded one-attempt exact adapter dispatch, shared `ModuleWorkScheduler`, atomic application/case audit lifecycle, rejected/operator-review same-decision requeue and no-domain legacy-terminal reconciliation are source-ready. Retain selected-owner/missing-owner, scheduler/stop/multi-host convergence, recovery receipt/CAS/no-adapter behavior, operation migration/lease/dispatch/lost-response and Forum mutation/replay/concurrency evidence. Keep Unpublished distinct; the next product milestone is authorized Moderation admin transport/RBAC plus explicit fresh-revision new-case/new-decision re-review, never Forum case queues/audit.

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

### Track 4 — Forum Page Builder contribution continuation

1. Keep canonical Forum contribution metadata on the shared module-tooling boundary and keep Forum widget persistence/visibility/validation authoritative.
2. Generated Fly component/block/renderer/property identities plus provider-neutral Pages host composition: source-ready.
3. Forum-owned preview service/HTTP/native transport with explicit selected-component Page Builder preview: source-ready.
4. Forum-owned property schema/validation transport plus generic normalized-`props` editor: source-ready.
5. Retain all-on/preview-off/properties-off/Forum-disabled runtime/browser evidence before replacing synthetic Wave evidence with an observed run.

## Compatibility and migration

- Existing Forum votes remain source-compatible. Do not reinterpret them as
  reactions without explicit semantic mapping and migration.
- Forum statistics are projections, not a reputation ledger.
- Forum counter reconciliation is read-only in FORUM-33A. No transport may repair
  owner counters until the write path has explicit RBAC, dry-run, audit,
  idempotent job/receipt state and bounded recovery. Shared-owner reconciliation
  must use public owner contracts and must not read another module's private
  tables.
- Forum moderation state remains authoritative Forum state while Moderation
  decisions are applied idempotently through an adapter.
- Forum depends only on `rustok-moderation-api`, not the Moderation owner crate;
  reports, cases, queues, decisions, appeals and cross-domain audit remain outside
  Forum persistence.
- `mod-moderation` remains optional and is not implied by `mod-forum`. Selecting
  the owner feature without `ModerationModule` is a startup configuration error;
  Forum without the owner remains valid with unmaterialized neutral factories.
- Moderation application-operation persistence, one-attempt dispatch, shared
  scheduler registration, case/application audit lifecycle and operator recovery
  belong only to `rustok-moderation`. Forum must not copy pending/applying/retry/
  lease/applied state, recovery commands, case revisions, owner audit rows,
  reconstruct Moderation decisions, publish its own worker, or invoke the owner
  dispatcher/recovery directly.
- Shared module-work discovery is only a scheduling hint. The existing Moderation
  operation CAS must remain the sole durable claim before a Forum adapter call;
  generic scheduler envelope tokens must never become Forum receipt keys,
  Moderation operation lease tokens or domain idempotency keys.
- Moderation's first successful operation claim may advance its own case from
  `decided` to `applying_decision`; retries/reclaims must not repeatedly bump the
  case revision. This is Moderation owner state and never Forum lifecycle state.
- Only matching accepted application evidence may close the Moderation case and
  release its active deduplication key. Rejected/operator-review outcomes remain
  not-applied and escalate the Moderation case; they must not be reported as
  Forum mutation success.
- Moderation operation/case transitions and internal `moderation_events` audit
  facts must commit atomically for transitions executed by the lifecycle source.
  Forum must not mirror those events into a second moderation audit log or treat
  the internal audit ledger as a public event bus.
- Same-decision operator requeue may only move Moderation `rejected` or
  `operator_review` back to retryable work. `Applied` can never be requeued, and
  Forum is not called by the recovery command itself; any later domain call still
  comes only from the canonical scheduler/dispatcher with the same decision UUID.
- Pre-audit terminal Moderation operations are reconciled by Moderation owner
  recovery, not by replaying a Forum mutation or inventing historical case/audit
  facts. Forum must not participate in or store that reconciliation state.
- A stale reviewed decision must never be retargeted by recovery. A true re-review
  requires a new case and new immutable decision built from a fresh authorized
  producer revision; Forum only supplies its normal subject/revision facts.
- Moderation one-attempt dispatch must keep the immutable decision UUID as the
  domain idempotency key across lease attempts. The lease token is attempt/
  correlation identity only; it must never create a second Forum receipt key.
- Moderation dispatch must select exactly the stored subject module/kind adapter.
  Missing or retryable adapters/errors remain retryable; there is no fallback to
  another producer or subject kind and no guessed applied result.
- Non-retryable Moderation `Conflict`/`InvariantViolation`, including stale
  reviewed revision, and mismatched successful application evidence require
  operator review; they must not be silently retried or collapsed into ordinary
  rejection/applied success.
- Legacy decisions without typed effects remain non-dispatchable; a stale worker
  lease must not bypass the Forum domain receipt or exact reviewed revision.
- FORUM-19 owns only subject-local moderation revision clocks and local enforcement
  state. Those revision rows must not contain reports, decisions, audit payloads,
  application-attempt state or copied Reactions state.
- The moderation subject revision is distinct from the Reactions/content revision,
  timestamps and global Forum event offsets; no integration may silently substitute
  one of those narrower/unrelated clocks.
- Moderation decision application uses the shared Outbox owner-operation receipt
  ledger. Forum must not create a second application receipt table or retarget a
  reviewed decision to a newer subject revision.
- Reply `SetVisibility(Hidden)` must preserve the exact Forum lifecycle, public
  counter/statistics and event/projection semantics; already-hidden must not
  duplicate counters or events.
- Reply `RejectPublication` must preserve the exact established Forum
  `ReplyStatus::Rejected` moderator lifecycle, public counter/statistics and
  event/projection semantics; already-rejected must not duplicate counters or
  events.
- Neutral `SetVisibility(Unpublished)` is distinct from `RejectPublication` and
  must not be mapped to `ReplyStatus::Rejected` without a separate explicit Forum
  lifecycle contract.
- Reply `SetVisibility(Removed)` must reuse `ReplyService::remove_in_tx`; no
  Moderation adapter may bypass the full Forum accepted-solution cleanup,
  soft-delete/tombstone capture, public/solution accounting and canonical
  status-event/projection semantics with a status-only approximation.
- Temporary moderation effects must fail closed until Forum owns explicit
  expiry-safe enforcement state; permanent owner state must not be used as a
  lossy approximation of an expiring restriction.
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
- Selected-reply Reactions composition is bounded to one explicit Forum reply
  query target and must never fan out current-revision lookups across the visible
  reply list; a valid selected reply replaces the topic ReactionBar rather than
  mounting a second ambiguous control.
- Reactions semantic event envelope identity is the admitted owner-operation
  UUID; it is not a Forum route/revision/vote identity.
- Reactions bounded reconciliation repairs aggregate projection only and cannot
  mutate Forum-owned subjects or reinterpret Forum votes.
- Forum Fly component/block/renderer identities and Page Builder host composition
  may expose only versioned configuration plus owner contract references. They
  must not copy Forum topic/reply/category data, schemas, visibility facts or
  authorization into Fly or the generic Page Builder package.
- Forum widget preview must normalize through `ForumWidgetContractService`, use
  Forum owner visibility/RBAC, and accept no caller-supplied tenant or actor in
  widget props. Moderator reply preview must exclude deleted tombstones.
- Page Builder contribution discovery may use server-resolved effective permission
  strings for admission, but every Forum owner transport must independently
  reauthorize the request. A `manage` grant may satisfy an exact `read` manifest
  requirement only through the platform `has_effective_permission` semantics.
- Forum widget property editing is owner-contract driven: schema must be loaded
  from the Forum owner catalog, candidate props must pass Forum owner validation,
  and only owner-normalized Fly `props` may be persisted. It must not create a
  second Forum data authority.

## Required verification

```bash
node scripts/verify/verify-forum-counter-reconciliation-source.mjs
node scripts/verify/verify-forum-page-builder-contribution-metadata.mjs
node scripts/verify/verify-forum-shared-capability-ownership.mjs
node scripts/verify/verify-forum-moderation-subject-adapter.mjs
node scripts/verify/verify-moderation-host-composition.mjs
node scripts/verify/verify-moderation-application-operation.mjs
node scripts/verify/verify-moderation-application-dispatch-once.mjs
node scripts/verify/verify-moderation-application-work-scheduler.mjs
node scripts/verify/verify-moderation-application-audit-lifecycle.mjs
node scripts/verify/verify-moderation-application-operator-recovery.mjs
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
node scripts/verify/verify-forum-reply-reactions-storefront-composition.mjs
node scripts/verify/verify-forum-reactions-storefront-browser-evidence.mjs
cargo test -p rustok-events reactions
cargo test -p rustok-reactions-api
cargo test -p rustok-reactions
cargo test -p rustok-reactions --features graphql graphql
cargo test -p rustok-reactions-storefront
cargo test -p rustok-forum reaction_subject
cargo test -p rustok-forum moderation_subject -- --nocapture
cargo test -p rustok-blog reaction_subject
cargo test -p rustok-moderation
cargo test -p rustok-e2e-rust --test leptos_storefront_forum_reactions -- --nocapture
cargo check -p rustok-reactions --features graphql --all-targets
cargo check -p rustok-reactions-storefront --all-targets
cargo check -p rustok-forum --all-targets
cargo check -p rustok-moderation --all-targets
cargo check -p rustok-distribution --features "mod-forum mod-reactions"
cargo check -p rustok-server --no-default-features --features mod-reactions
cargo check -p rustok-server --no-default-features --features "mod-forum mod-reactions"
cargo test -p rustok-server --no-default-features --features "mod-blog mod-reactions" --test reactions_composition_profiles blog_with_reactions_materializes_post_provider
cargo check -p rustok-server --no-default-features --features mod-moderation
cargo check -p rustok-server --no-default-features --features "mod-forum mod-moderation"
cargo test -p rustok-server --no-default-features --features mod-moderation --test moderation_composition_profiles
cargo test -p rustok-server --no-default-features --features "mod-forum mod-moderation" --test moderation_composition_profiles
cargo run -p rustok-events --example event_contract_digests -- --write
cargo xtask module validate forum
npm run verify:page-builder:consumer:forum
npm run verify:forum:admin-boundary
npm run verify:forum:storefront-boundary
git diff --check
```

Tests, lockfile/event-digest generation and runtime evidence are maintainer-run.
Source contracts, Moderation adapter/migration/materialization/application-operation/
one-attempt-dispatch/shared-scheduler/application-audit/operator-recovery source,
Forum Page Builder Fly/owner-preview/owner-property host source and browser harness
source do not promote runtime status.

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

For FORUM-32, retain browser/runtime evidence for Forum-enabled Pages block
insertion, owner schema loading, invalid diagnostics, owner-normalized `props`,
undo/redo, explicit owner preview, Forum-disabled composition,
`preview_off`/`properties_off`, hidden-category filtering, moderator reply preview
and effective `manage -> read` contribution admission. Source composition is now
complete for metadata, Fly identity, owner preview and owner properties; do not
add another Forum-local schema/data authority. Replace synthetic Wave evidence
only after the existing Pages reference-consumer execution gate and the Forum
executable packet are accepted.

For FORUM-33, retain SQLite and PostgreSQL execution evidence for a clean counter
snapshot and intentionally drifted `topic.reply_count`, `category.topic_count`
and `category.reply_count` fixtures, including one concurrent-write snapshot
case. Then add independent bounded topic/category continuation so a large tenant
can scan past the first 500 rows without unbounded work. Do not add write repair
until operator RBAC, dry-run, durable audit, idempotent job/receipt state and
bounded recovery are designed together. Add a Forum CLI adapter only with the
synchronized workspace dependency and `Cargo.lock` update.

Regenerate the event-contract digests and retain release verification for the
current `Cargo.lock`, then retain SQLite and PostgreSQL evidence for changed/
no-op/replayed reaction event cardinality, rollback on event failure, concurrent
actor writes, clean/blocked/drift bounded aggregate reconciliation and repair
receipt replay. Retain Blog provider and Blog+Reactions composition evidence plus
manifest-composed Reactions GraphQL schema/runtime evidence for anonymous/
authenticated reads, human-user writes, tenant mismatch, idempotent replay and
stale/denied subjects. Retain the separate Reactions storefront source/runtime
evidence plus Forum topic/reply current-revision GraphQL/native transport parity,
then execute and retain the bounded selected-topic/selected-reply Playwright
browser harness from
`tests/e2e-rust/tests/leptos_storefront_forum_reactions.rs`. Do not add reaction
catalogs, state, commands, aggregate ownership or copied Reactions presentation
to Forum.

For FORUM-19, retain server composition evidence for selected `mod-moderation`
with a registered owner, selected-feature/missing-owner failure,
Moderation-only empty materialization and Forum+Moderation topic/reply adapter
materialization. Retain shared module-work evidence for Moderation registration,
background-worker-disabled no-dispatch, earliest-due selection, two-host
same-candidate CAS convergence, deployment stop/no-new-claim with in-flight
completion and missing-registry registration failure. Retain application audit
lifecycle evidence for first-claim `decided -> applying_decision`, retry/reclaim
without duplicate case revision, retry audit atomicity, applied + closed +
active-key release + audit atomicity, rejected/operator-review + escalated +
audit atomicity, audit-insert rollback, stale-token finalizer rollback and case
revision contention. Retain operator recovery evidence for human-user gate,
command receipt replay/changed-request conflict, expected case revision CAS,
rejected/operator-review same-decision requeue, applied requeue denial, next
scheduler claim with unchanged decision UUID idempotency, applied/rejected/
operator-review legacy reconciliation, already-consistent no-op, current-time
case close semantics, active-key release/preservation and proof that legacy
reconciliation invokes no Forum/domain adapter. Retain clean/upgraded
PostgreSQL/SQLite evidence for `moderation_application_operations`,
typed-effect-only backfill, atomic decision/effect/pending-operation/event/receipt
commit, bounded due ordering, concurrent claim, lease expiry/reclaim, stale-token
rejection, exact immutable command reconstruction, exact adapter selection,
missing-adapter retry, retryable/non-retryable classification, stale-conflict
operator-review, invalid-success-evidence operator-review, decision-UUID
lost-response replay, exactly-one case close and applied-evidence validation.
Also retain the Forum moderation subject revision migration/trigger,
shared-receipt replay/request-conflict, stale revision, trusted caller and
concurrent content/lifecycle evidence plus hide/reject/removal
accounting/event/tombstone/solution semantics. `SetVisibility(Unpublished)` stays
blocked until Forum owns a distinct exact lifecycle, and temporary effects still
require expiry-safe Forum state. The next FORUM-19 product/code milestone is the
authorized Moderation admin transport/RBAC plus an explicit fresh producer
revision -> new case -> new immutable decision re-review workflow. Do not add
Forum-owned case queues, recovery state, audit, scheduler state or a duplicate
worker loop.
