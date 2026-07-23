---
id: doc://crates/rustok-forum/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-forum
  - rustok-notifications-program
last_reviewed: 2026-07-22
---

# `rustok-forum` canonical implementation plan

## Canonical-source policy

This file is the single source of truth for the forum product roadmap, the
forum-owned implementation backlog, and the forum integration requirements for
the future shared notifications module.

Do not create another forum roadmap, remediation plan, backlog document, or
task-number list. Other documents may describe stable contracts or completed
architecture, but they must link here instead of copying task status or future
work.

The former external NodeBB/notifications remediation draft has been absorbed
into this file and is not authoritative. GitHub issues and pull requests are
execution records; they do not replace this plan.

Every pull request that changes a task below must update, in the same pull
request:

1. the task status in the program ledger;
2. the task's remaining scope and definition of done;
3. verification commands or evidence paths;
4. compatibility, migration, and degraded-mode notes when relevant.

A task may be marked `done` only when implementation, migration/backfill,
tests, module documentation, public contracts, and required runtime evidence
are all present. A merged partial slice remains `in_progress`.

## Current state

The Forum module has an implemented core domain, transport boundary, and
module-owned UI packages. The verified capability baseline and the remaining
product work are tracked in this plan's program ledger; every unfinished item
remains explicitly marked with its current status and completion evidence.

The neutral `rustok-api::richtext` contract and executable
`rustok-content::richtext` profiles are now available. Forum topic/reply
storage and transports remain on the legacy body/format path until the
atomic cutover; do not add new `rt_json`/Markdown aliases,
`content_json` fields, or a Forum-local renderer.

## Verification

Run `cargo xtask module validate forum` for the module contract and use the
task-specific commands and evidence paths recorded in the program ledger for
any changed Forum capability.

Run `npm run verify:forum:admin-boundary`
(`scripts/verify/verify-forum-admin-boundary.mjs`) after an admin-surface or
transport-boundary change. This is the fast guardrail for the module-owned
admin core/transport/UI split.

Run `npm run verify:forum:storefront-boundary`
(`scripts/verify/verify-forum-storefront-boundary.mjs`) after a storefront
surface or transport-boundary change. This is the fast guardrail for the
module-owned storefront core/transport/UI split and its GraphQL read adapter.

## Status vocabulary

| Status | Meaning |
| --- | --- |
| `done` | The current scope is implemented and verified. |
| `in_progress` | Some required behavior is merged, but the definition of done is not complete. |
| `planned` | Approved work with an explicit scope and dependencies. |
| `blocked` | Approved work waiting on a named dependency. |
| `deferred` | Intentionally excluded from the current release target. |

## Execution rules

- One task per pull request unless a task card explicitly permits several
  mechanical PRs.
- Read `AGENTS.md`, module authoring/architecture docs, the event-flow contract,
  and this file before editing.
- Keep code, comments, migrations, tests, ADRs, and repository documentation in
  English.
- `rustok-forum` owns categories, topic/reply lifecycle, subscriptions, read
  tracking, moderation, reports, forum trust/reputation, forum attachments, and
  forum projections.
- Authentication credentials and sessions remain in auth/users.
- Public member identity remains in `rustok-profiles`; do not create a
  `member` module.
- Binary objects and image lifecycle remain in `rustok-media`; forum tables
  store typed media references, never arbitrary asset URLs.
- The future `rustok-notifications` module owns inbox state, preferences,
  fan-out, grouping, digests, and channel delivery attempts.
- `rustok-email` remains an email provider. Notifications decides who, what,
  when, and channel; email performs delivery.
- Forum commands never require notifications to be enabled. Forum commits
  owner state and semantic events; optional consumers process those events.
- Owner state and its outbox event must commit in one database transaction.
- Durable consumers use inbox/idempotency state. Redis pub/sub, SSE, and
  WebSocket delivery are accelerators, not correctness mechanisms.
- Every tenant-owned relation uses tenant-scoped predicates and
  tenant-composite database integrity.
- Do not use unbounded JSON, pagination, tags, mentions, attachment counts,
  subscriber fan-out, or bulk moderation.
- Do not swallow locale, media, indexing, notification, or persistence errors
  as empty/default values.
- Do not hard-delete user-visible categories, topics, or replies from normal
  product flows. Purge is a separate retention operation.
- Database triggers are invariant guards. Domain workflow belongs in explicit
  owner services.
- Do not weaken or delete tests to make a task pass.
- Do not hand-edit rollout evidence that is required to come from an executable
  runtime run.

## Product target

The target is a NodeBB-class, multi-tenant, multilingual forum bounded context
with:

- hierarchical localized categories;
- discussions, Q&A, wiki/announcement modes, revisions, safe deletion, and
  attachments;
- subscriptions, unread state, drafts, bookmarks, mentions, reactions,
  reputation, badges, and member-facing forum profiles;
- reports, moderation queues, restrictions, anti-spam, audit, and trust levels;
- visibility-aware search, SEO, notifications, and realtime acceleration;
- module-owned admin/storefront packages;
- optional-capability degraded profiles that never turn a disabled integration
  into a forum outage.

## Ownership and integration architecture

```text
users/auth
  -> identity, credentials, sessions

rustok-profiles
  -> public handle, display name, biography, avatar/banner media references,
     preferred locale and profile privacy

rustok-media
  -> upload, storage, descriptors, quarantine and asset lifecycle

rustok-forum
  -> category tree, topics, replies, revisions, subscriptions, read state,
     drafts, bookmarks, reports, moderation, restrictions, forum trust,
     forum reactions/reputation, attachment relations and semantic events

rustok-notifications
  -> inbox, unread/read/seen/archive, preferences, recipient fan-out, grouping,
     digests and delivery attempts

rustok-email / push / SMS adapters
  -> channel-specific delivery

rustok-outbox
  -> durable owner-event transport and consumer inbox

rustok-index / rustok-search
  -> visibility-aware forum projections and retrieval

rustok-cache
  -> acceleration only; never the sole authority for permission, notification,
     subscription, or unread correctness
```

## Current verified capability baseline

The module already owns and exposes:

- categories, localized category translations and parent relations;
- localized topics and replies;
- typed topic/reply lifecycle state;
- pin, lock, close and archive moderation;
- pending/approved/rejected/hidden reply moderation;
- `-1/+1` voting;
- category/topic subscriptions with watching/tracking/normal/muted levels;
- accepted solutions;
- topic tags backed by `rustok-taxonomy`;
- forum user statistics;
- channel-aware visibility and SEO target providers;
- transactional forum events through the outbox;
- topic/reply revision history and tombstones;
- bounded cursor read models for categories, topics, and replies;
- module-owned admin/storefront FFA packages;
- Page Builder consumer contracts and fallback profiles.

Existing capability is not proof of full product completion. The release gates
at the end of this file remain authoritative.

## Program ledger

| Task | Status | Current result or nearest deliverable |
| --- | --- | --- |
| `FORUM-00` | `done` | PostgreSQL/SQLite runtime baseline and regression profiles. |
| `FORUM-01` | `done` | Tenant-composite forum relation integrity and platform locale width. |
| `FORUM-02` | `done` | Typed topic/reply lifecycle, tombstone and revision fields. |
| `FORUM-03` | `done` | Atomic category owner writes and translation persistence. |
| `FORUM-04` | `done` | FORUM-04A-G provide the bounded tree, atomic placement, write guards, topic policy, subtree lifecycle and canonical-tree admin drag-and-drop; maintainer verification passed. |
| `FORUM-05` | `done` | Publication-aware serialized counters with database safety guards. |
| `FORUM-06` | `done` | Locked-topic and pending/publication semantics are explicit owner workflows. |
| `FORUM-07` | `done` | Monotonic per-topic reply positions and uniqueness constraints. |
| `FORUM-08` | `done` | Revisions, tombstones, owner soft-delete workflows and raw lifecycle service retirement; PR #1867 and maintainer verification complete. |
| `FORUM-09` | `done` | Forum-owned versioned event catalog and journal, merged through PR #1732. |
| `FORUM-10` | `done` | Bounded cursor read models and capped compatibility reads, PRs #1734/#1735. |
| `FORUM-11` | `done` | Subscription levels and participation policy, PR #1736; verification repairs in #1737. |
| `FORUM-12` | `in_progress` | FORUM-12A-D2 deliver bounded extraction, immutable relations, owner writes, mention events, reads and quote commands; source-ready PostgreSQL proof plus the `forum.mention.user_added` provider and bounded candidate fan-out now exist. Maintainer runtime execution, profile/block privacy, moderator audience expansion, final notification creation/open authorization and retention purge remain. |
| `FORUM-13` | `in_progress` | Verified FORUM-13A/B add bounded presentation policy and explicit optional Media capability behavior; Media quarantine/deletion state, persistence, transport composition, runtime evidence and UI remain. |
| `FORUM-14` | `planned` | Topic/reply attachment relations and upload-session lifecycle. |
| `FORUM-15` | `planned` | Profile/member summary and avatar integration. |
| `FORUM-16` | `planned` | Durable read tracking and unread projections. |
| `FORUM-17` | `planned` | Drafts, autosave, bookmarks and optional reminders. |
| `FORUM-18` | `planned` | Atomic votes, reactions, reputation ledger and badges. |
| `FORUM-19` | `planned` | Reports, moderation queue, restrictions and audit. |
| `FORUM-20` | `planned` | Category/topic ACL and group/channel inheritance. |
| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |
| `FORUM-22` | `planned` | Topic kinds, wiki/announcement/Q&A policies and scheduled lifecycle. |
| `FORUM-23` | `planned` | Visibility-aware index/search projections. |
| `FORUM-24` | `planned` | Localized routes, canonical URLs and aliases. |
| `FORUM-25` | `planned` | Full content/UI multilingual contract and RTL support. |
| `FORUM-26` | `planned` | Anti-spam, bounded posting policy and trust levels. |
| `FORUM-27` | `planned` | Member directory, forum profile, badges and activity views. |
| `FORUM-28` | `planned` | Editor, safe renderer and renderer-version rebuilds. |
| `FORUM-29` | `planned` | Realtime acceleration with cursor/revision reconciliation. |
| `FORUM-30` | `planned` | Complete module-owned admin product. |
| `FORUM-31` | `planned` | Complete NodeBB-class storefront product. |
| `FORUM-32` | `in_progress` | Widget contract exists; richer widgets and observed Page Builder evidence remain. |
| `FORUM-33` | `planned` | Analytics, observability and reconciliation operations. |
| `FORUM-34` | `planned` | Import/export and resumable NodeBB migration toolkit. |
| `NOTIFY-00` | `in_progress` | NOTIFY-00A/B deliver the neutral API, optional owner/runtime composition, module-owned packages and real Forum providers. Maintainer verification and complete executable distribution/global migration evidence remain. |
| `NOTIFY-01` | `in_progress` | NOTIFY-01A creates bounded owner persistence; NOTIFY-01B adds the durable source-event inbox and transactional fan-out owner service. Final notification commands, global migration promotion, retention and reconciliation remain. |
| `NOTIFY-02` | `planned` | Preferences, quiet hours and digest scheduling. |
| `NOTIFY-03` | `in_progress` | NOTIFY-03A durably accepts source events, materializes bounded provider descriptors and persists leased cursor pages as idempotent pending candidates. Outbox relay wiring, preference/privacy processing, final notifications, deliveries and PostgreSQL lease evidence remain. |
| `NOTIFY-04` | `planned` | In-app inbox and unread/read mutation APIs. |
| `NOTIFY-05` | `planned` | Email/push/SMS delivery-provider SPI. |
| `NOTIFY-06` | `planned` | Localized semantic templates and recipient locale selection. |
| `NOTIFY-07` | `planned` | Privacy, visibility, blocking and target-open authorization. |
| `NOTIFY-08` | `planned` | Notification admin/storefront UI packages. |
| `NOTIFY-09` | `planned` | FBA contracts and optional-module compatibility profiles. |
| `LINK-FORUM-01` | `planned` | Forum-to-notifications end-to-end proof. |
| `LINK-FORUM-02` | `planned` | Profiles/media/forum end-to-end proof. |
| `LINK-FORUM-03` | `planned` | Forum/index/search ordering and visibility proof. |
| `LINK-FORUM-04` | `planned` | Required/optional capability profiles and startup validation. |
| `LINK-FORUM-05` | `planned` | Production release gate and waiver-free evidence. |

## Completed foundation: `FORUM-00` through `FORUM-11`

The completed foundation must not be reimplemented under new names.

### Delivered invariants

- runtime baseline with PostgreSQL and SQLite regression profiles;
- tenant-composite foreign keys and locale storage compatible with platform
  `LocaleTag`;
- typed lifecycle values and database checks;
- atomic category write/translation transactions;
- category parent and cycle protection;
- publication-aware topic/category/user counters;
- typed locked-topic rejection;
- pending replies that do not become public until moderation approval;
- monotonic reply positions;
- revision history and explicit owner tombstone commands;
- versioned forum event journal;
- bounded cursor read models;
- watching/tracking/normal/muted subscriptions and participation policy.

### Historical execution references

- FORUM-00..08 audit hardening: PRs #1704/#1705.
- Explicit owner lifecycle: PRs #1707/#1709.
- Forum event catalog: PR #1732.
- Cursor read models: PRs #1734/#1735.
- Subscription levels: PR #1736; follow-up verification/format repairs: PR #1737.
- Raw lifecycle service retirement: PR #1867.

These references are audit history only. The current code and this plan define
the present contract.

## Execution order

### Wave A — close remaining foundation gaps

1. keep `FORUM-32` static contracts green while observed evidence is blocked on
   Page Builder/pages readiness.

`FORUM-04` and the residual `FORUM-08` cleanup are complete and maintainer
verified; they are no longer active execution items.

### Wave B — notifications foundation and identity/media integration

1. finish `NOTIFY-00` maintainer verification and executable composition evidence;
2. complete `NOTIFY-01` final notification persistence, global migration and retention commands;
3. complete `NOTIFY-03` production outbox intake plus pending-candidate processing;
4. `NOTIFY-07` profile/block privacy and recipient-specific open authorization;
5. finish `FORUM-13` after the Media lifecycle-state contract is available;
6. `FORUM-14`;
7. `FORUM-15`;
8. `LINK-FORUM-02`.

### Wave C — participation product

1. record successful PostgreSQL execution for `FORUM-12`, then finish
   visibility/privacy, moderator-audience and final notification evidence under
   `NOTIFY-03/07`;
2. `FORUM-16`;
3. `FORUM-17`;
4. `FORUM-18`;
5. `NOTIFY-02`;
6. `NOTIFY-04`;
7. `NOTIFY-05`;
8. `NOTIFY-06`;
9. `LINK-FORUM-01`.

### Wave D — moderation, visibility and retrieval

1. `FORUM-19`;
2. `FORUM-20`;
3. `FORUM-23`;
4. `LINK-FORUM-03`;
5. `FORUM-26`;
6. `FORUM-33`.

### Wave E — advanced discussion and presentation

1. `FORUM-21`;
2. `FORUM-22`;
3. `FORUM-24`;
4. `FORUM-25`;
5. `FORUM-27`;
6. `FORUM-28`;
7. `FORUM-29`;
8. `FORUM-30`;
9. `FORUM-31`;
10. `FORUM-32`;
11. `NOTIFY-08`;
12. `NOTIFY-09`.

### Wave F — migration and release

1. `FORUM-34`;
2. `LINK-FORUM-04`;
3. `LINK-FORUM-05`.

Independent UI slices may run in parallel only after the owner contracts they
consume are stable.

# Forum task cards

## `FORUM-04` — complete the category tree

**Status:** `done`  
**Priority:** P0  
**Dependencies:** completed FORUM-01/03/10

### Delivered in `FORUM-04A`

- `CategoryService::tree` reconstructs the complete tenant hierarchy through one
  owner call bounded to 512 nodes and depth 16;
- `GET /api/forum/categories/tree` and the OpenAPI contract expose nested nodes
  with `parent_id`, `depth`, direct child metadata, stable `(position, id)`
  sibling order and localized breadcrumbs;
- the read fails closed for an oversized, over-depth, disconnected, cyclic or
  foreign-parent hierarchy instead of returning a partial tree;
- PostgreSQL and SQLite integration tests cover nesting, deterministic order,
  locale fallback, breadcrumbs, tenant isolation and the read bounds;
- the flat cursor projection remains a separate bounded compatibility/read use
  case.

### Delivered in `FORUM-04B` and `FORUM-04C`

- tenant-serialized `CategoryService::move_category` and `reorder_siblings`
  normalize complete source/destination sibling groups atomically;
- REST, GraphQL and OpenAPI expose owner commands guarded by
  `forum_categories:manage`;
- move/reorder rejects self/descendant cycles, missing or cross-tenant parents,
  incomplete sibling sets, duplicate IDs, oversized trees and depth overflow;
- PostgreSQL and SQLite enforce zero-based depth 16 at the database write
  boundary, including internal direct writes;
- generic category metadata updates reject `position`, so transports cannot
  bypass owner placement commands;
- shared PostgreSQL/SQLite scenarios cover reorder, cross-parent move, sibling
  normalization, cycle/foreign-parent rejection, write-depth rejection and
  tenant isolation.

### Delivered in `FORUM-04D` and `FORUM-04E`

- forum-admin GraphQL/REST adapters route category placement through owner
  commands, and the admin boundary verifier rejects generic `position` bypasses;
- tenant-scoped category topic policy defaults to `allows_topics = true` for
  existing categories without a stored policy row;
- REST, GraphQL, OpenAPI and the canonical tree expose the policy;
- PostgreSQL and SQLite serialize policy changes with topic writes and reject
  topic inserts or category reassignment when topic creation is disabled;
- disabling policy preserves existing topics and controls only new placement;
- shared PostgreSQL/SQLite scenarios cover default allow, disable, blocked
  writes, tenant isolation and re-enable.

### Delivered in `FORUM-04F`

- `CategoryService::archive_subtree` and `restore_subtree` serialize lifecycle changes with the tenant category-tree lock;
- compatibility-default lifecycle rows preserve existing categories as active without backfill;
- archive writes descendants before ancestors and restore removes ancestor lifecycle rows before descendants;
- REST, GraphQL, OpenAPI and the canonical tree expose subtree lifecycle state and owner commands;
- PostgreSQL and SQLite reject active children beneath archived parents, partial restore and new topic placement in archived categories;
- existing topics are preserved and shared PostgreSQL/SQLite scenarios cover archive, restore, tenant isolation and direct-write guards.

### Delivered in `FORUM-04G`

- forum-admin loads the bounded canonical category tree through GraphQL-first transport with REST fallback instead of reconstructing hierarchy from the flat compatibility list;
- the tree is flattened in deterministic preorder with `parent_id`, `depth`, `position`, topic policy and archive state retained for rendering and drop planning;
- interactive drag-and-drop supports moving before a sibling, nesting inside a category and moving to the end of the root set;
- pure drop planning rejects no-op, self/subtree cycles and active moves beneath archived categories before transport execution;
- every accepted drop calls the existing owner `move_category` command and refreshes the canonical tree; generic category update is never used;
- the forum-admin boundary verifier and fixtures reject flat hierarchy reads and DnD placement bypasses.

### Verification result

Maintainer verification of the commands below passed on 2026-07-21. No
remaining `FORUM-04` implementation scope is open.

### Definition of done

- concurrent moves cannot create cycles or duplicate sibling order;
- PostgreSQL and SQLite tests cover move, reorder, max depth, topic policy,
  archive/restore and two tenants with colliding identity fixtures;
- category deletion still fails closed for non-empty trees.

### Verification

```bash
cargo test -p rustok-forum category_tree
cargo test -p rustok-forum --test category_commands_sqlite -- --nocapture
cargo test -p rustok-forum --test category_commands_postgres -- --nocapture --test-threads=1
cargo test -p rustok-forum --test category_policy_sqlite -- --nocapture
cargo test -p rustok-forum --test category_policy_postgres -- --nocapture --test-threads=1
cargo test -p rustok-forum --test category_lifecycle_sqlite -- --nocapture
cargo test -p rustok-forum --test category_lifecycle_postgres -- --nocapture --test-threads=1
cargo test -p rustok-forum --test runtime_regression_baseline
cargo xtask module validate forum
npm run verify:forum:admin-boundary
```

## `FORUM-08` compatibility cleanup — retire raw lifecycle services

**Status:** `done` under completed `FORUM-08`  
**Priority:** P1  
**Dependencies:** all downstream call sites use root owner services

### Delivered

- direct workspace consumers use root `TopicService` and `ReplyService` facades;
- raw topic/reply persistence and owner implementation modules are crate-private;
- database triggers remain invariant protection;
- `scripts/verify/verify-forum-owner-boundary.mjs` rejects new direct imports;
- implementation was merged through PR #1867 and maintainer verification passed.

### Definition of done

Workspace consumers compile through the root owner services and no public
contract exposes persistence services.

## `FORUM-12` — mentions, quotes and recipient projection

**Status:** `in_progress`  
**Priority:** P1  
**Dependencies:** FORUM-08/09, profiles read contract; NOTIFY-03/07 for delivery and privacy integration

### Scope

Create forum-owned mention and quote relations keyed by tenant, source target,
source revision and mentioned user. The implemented parser currently handles
Markdown and `rt_json_v1`; that is legacy state, not the target authoring
contract. During the atomic richtext cutover it must traverse the validated
`RichTextDocument` tree directly, continue to exclude code blocks/code marks,
and remove format branching. Resolve handles through the profiles contract,
cap mentions per revision, reject abusive mass mentions and make special
audiences such as moderators permission-gated.

Editing uses a revision diff: new mention produces one semantic event, removed
or unchanged mentions do not produce duplicate delivery. Quotes retain the
quoted target and quoted revision so edits do not rewrite history.

### Delivered in `FORUM-12A`

- `extract_forum_mention_candidates` supports only canonical Markdown and
  `rt_json_v1`, caps each revision at 32 unique targets and deduplicates handles
  with the Profiles-owned handle grammar;
- Markdown extraction ignores fenced code, inline code, escaped text and email
  address `@` tokens;
- `rt_json_v1` extraction runs after the canonical `rustok-core` sanitizer and
  ignores `code_block` nodes and text carrying a `code` mark;
- `@moderators` is a typed special audience and fails unless the caller supplies
  explicit moderation policy;
- `resolve_forum_mentions` uses tenant-scoped `ProfilesReader` lookup and accepts
  only active public or authenticated profiles;
- missing, hidden, blocked, private, followers-only, foreign-tenant and
  mismatched targets fail with the same safe
  `FORUM_MENTION_TARGET_UNAVAILABLE` class, avoiding a profile-existence oracle;
- `ForumRevisionIdentity` and `ForumQuoteReference` preserve source and quoted
  revision identity rather than relying on display text;
- `diff_forum_mentions` deterministically separates added, removed and unchanged
  targets; only added targets become `ForumMentionEventCandidate` values;
- replaying the same source revision with changed targets fails closed, while an
  identical replay produces no added candidates;
- a source verifier rejects notification/event delivery, profile internals and
  premature Forum persistence in this contract slice.

### Delivered in `FORUM-12B1`

- PostgreSQL and SQLite create `forum_relation_revisions`,
  `forum_user_mentions`, `forum_audience_mentions` and `forum_quotes`;
- relation revision IDs are globally unique and immutable, while every child row
  repeats the complete tenant/source/locale/revision identity for database
  validation and deterministic owner reads;
- database guards validate source translation/body identity, quoted tenant/kind/
  target identity and reject direct updates to revisions or child rows;
- migration backfills one `legacy` relation revision for every existing topic
  translation and reply body without parsing historical copy or querying
  Profiles-owned tables;
- the crate-private `MentionRelationService` separates profile-dependent
  `prepare` from transaction-only `persist_in_tx`;
- `prepare` resolves handles through `ProfilesReader` and computes a SHA-256
  replay fingerprint over canonical body, format, resolved targets and quotes;
- `persist_in_tx` locks the source stream, re-reads the persisted body in the
  same transaction, rejects prepared/body mismatch and atomically appends the
  revision plus all mention/quote rows;
- an identical latest fingerprint must also match the persisted relation
  snapshot before replay returns the same revision with no added targets;
- quote lookup failure, foreign tenant, kind mismatch and target mismatch share
  `FORUM_QUOTE_TARGET_UNAVAILABLE`, avoiding a quote-existence oracle;
- a SQLite owner scenario covers first write, identical replay, edit diff, quote
  binding, cross-tenant rejection and direct immutable-row enforcement;
- a source verifier rejects notification/event publication, profile internals
  and public exposure of the persistence service.

### Delivered in `FORUM-12B2`

- active topic and reply create/edit owner commands prepare relation projections
  before opening their transaction;
- canonical topic translations and reply bodies are written before
  `persist_in_tx`, while relation revisions, counters, lifecycle events and the
  source command commit atomically;
- public topic/reply facades remain the only command entrypoints and transports
  cannot invoke the persistence seam;
- source INSERT seed triggers remain compatible during rollout and active owner
  writes append the canonical projection after the seed identity.

### Delivered in `FORUM-12C`

- the sealed v1 `ForumMentionEvent` family publishes
  `forum.mention.user_added` and `forum.mention.audience_added` with source
  revision and target identity only;
- only the exact persisted added-target diff produces events; replay, removed
  targets and unchanged targets emit nothing;
- the same event UUID is written to the transactional outbox and append-only
  Forum domain-event journal in the owner transaction;
- PostgreSQL and SQLite journal constraints accept the mention event types and
  preserve immutable update/delete guards;
- `ForumRelationReadService` returns latest or exact tenant/source/locale
  snapshots bounded to 32 mention targets and 32 quotes without exposing handle
  snapshots or replay fingerprints;
- invalid or foreign relation identities use
  `FORUM_RELATION_REVISION_UNAVAILABLE` without an existence oracle.

### Delivered in `FORUM-12D1`

- `SetForumQuotesInput` defines an exact source locale and a full replacement
  list of typed target kind, target ID and quoted revision ID references;
- `ForumQuoteCommandService` replaces quotes for an existing topic translation
  or reply body under the corresponding update owner scope;
- raw and unique quote input is bounded to 32, exact duplicates are normalized,
  and an empty list explicitly clears quotes while preserving mentions parsed
  from the unchanged canonical body;
- preparation occurs before transaction start, while immutable relation
  persistence and bounded response materialization complete before commit;
- identical replacement replays the current relation revision and missing,
  cross-tenant or mismatched quote identities use the existing safe failure;
- REST, GraphQL and OpenAPI expose dedicated topic/reply quote replacement
  commands without transport access to `MentionRelationService`.

### Delivered in `FORUM-12D2`

- separate topic/reply command DTOs accept bounded typed quote references without
  changing the existing Rust create/update DTO structs;
- create commands treat omitted quotes as an empty initial set, while update
  commands distinguish omitted preservation, explicit empty clear and explicit
  full replacement;
- legacy facade create/update methods convert to command DTOs, so ordinary body
  edits preserve the latest exact-locale quote set instead of silently clearing
  it;
- omitted preservation records the relation revision used during preparation,
  locks the active source in the owner transaction and returns retryable
  `FORUM_RELATION_REVISION_CONFLICT` if D1 or D2 changed the stream concurrently;
- canonical body persistence, immutable relation projection, mention events,
  outbox/journal rows and existing source counters/events remain one transaction;
- existing REST create/update routes consume command DTOs, while GraphQL keeps
  legacy mutations and adds additive `*WithQuotes` mutations;
- SQLite source coverage proves stale omitted-preserve conflict and explicit
  clear semantics without exposing the persistence seam to transports.

### Delivered with the PostgreSQL proof and `NOTIFY-01B/03A`

- `mention_quote_runtime_postgres` is source-ready for deterministic D1-before-D2
  root-lock ordering, retryable stale-preserve conflict, stale body rollback,
  soft-delete rejection and the notifications-off producer profile;
- the proof record remains explicitly `source_ready`; no successful PostgreSQL
  run is claimed until the maintainer executes it;
- the Forum notification source now supports `forum.mention.user_added` and
  binds each event to the exact immutable `forum_user_mentions` row;
- topic/reply source visibility is rechecked while describing, resolving the
  one candidate and opening the target; pending replies are retryable and
  closed, hidden, deleted or channel-restricted sources fail closed;
- self-mentions are suppressed and only target/revision identity is exposed;
- the Notifications owner can durably accept the source event and persist a
  pending candidate without creating a final notification or delivery attempt;
- `forum.mention.audience_added` remains deferred until a bounded moderator
  directory owner port exists.

### Compatibility and degraded mode

Existing source locales retain their `legacy` seed identity until an active
owner write appends a canonical relation revision. Existing topic/reply
create/edit DTOs remain source-compatible; separate D1/D2 command DTOs carry
quote relations. Legacy body edits route through D2 and preserve current quotes.
Notifications remain an optional downstream consumer and are never called
synchronously from Forum transactions. When Notifications or the Forum source
provider is absent, Forum owner commands and semantic-event commits still
succeed; a durably accepted Notifications source event remains retryable until
its provider is available.

### Remaining scope

FORUM-12 remains `in_progress` until all of the following are delivered:

- record successful maintainer PostgreSQL execution for the concurrent D1/D2,
  deletion and notifications-off source-ready proof;
- apply profile/block privacy and recipient-specific authorization before a
  pending mention candidate becomes a final notification, and recheck before
  target open or delayed delivery under `NOTIFY-03/07`;
- add bounded moderator-audience expansion for `forum.mention.audience_added`;
- prove final notification dedupe, notifications-on delivery/open behavior and
  retention purge/reconciliation without deleting immutable quoted history.

### Definition of done

- mention resolution is tenant/profile scoped and idempotent by source revision;
- the source event contains target identity, not recipient contact data;
- quote commands retain immutable target revision identity, are bounded and
  conflict rather than restore a stale preserved set;
- blocked, private, deleted and unauthorized targets cannot generate or open a
  notification;
- duplicate source events, overlapping audience rules and retries create one
  permitted notification;
- tests cover edit diffs, duplicate handles, code blocks, escaping, caps, quote
  replacement/clear/preserve, replay, expected-revision conflicts and source
  consumer retry/visibility behavior.

### Verification

```bash
cargo test -p rustok-forum --test mention_contract
cargo test -p rustok-forum mention_relation
cargo test -p rustok-forum quote_command
cargo test -p rustok-forum inline_quote
cargo test -p rustok-forum --test mention_quote_runtime_postgres -- --nocapture --test-threads=1
cargo test -p rustok-forum --test notification_source_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_sqlite -- --nocapture
node scripts/verify/verify-forum-mention-contract.mjs
node scripts/verify/verify-forum-mention-contract.test.mjs
node scripts/verify/verify-forum-mention-persistence.mjs
node scripts/verify/verify-forum-mention-persistence.test.mjs
node scripts/verify/verify-forum-mention-integration.mjs
node scripts/verify/verify-forum-mention-events.mjs
node scripts/verify/verify-forum-quote-commands.mjs
node scripts/verify/verify-forum-mention-runtime-proof.mjs
node scripts/verify/verify-notifications-source-fanout.mjs
cargo xtask module validate forum
```

The commands above are the maintainer verification set for FORUM-12. Source and
contract records do not claim executable verification until the maintainer runs
them.

## `FORUM-13` — category icon and image integration

**Status:** `in_progress`  
**Priority:** P1  
**Dependencies:** media read/upload capability

### Scope

Replace ambiguous category icon/image strings with:

```text
icon_key          validated design-system token
cover_media_id    optional media-owned image reference
```

Validate tenant, asset kind, MIME, dimensions, size, quarantine/deletion state
and public delivery policy through a media port. Responses expose a media image
descriptor. Existing color values must be validated design tokens or safe
bounded colors.

### Degraded mode

With media disabled, icon/color behavior remains available, image selection is
hidden, and existing image descriptors degrade to absent without breaking
category reads. A command that attempts to set a media reference fails with a
typed capability-unavailable error.

### Delivered in `FORUM-13A`

- category icon writes normalize to bounded lowercase kebab-case semantic keys
  at the database write boundary; CSS classes, markup, URLs and paths fail closed;
- category colors remain restricted to safe bounded hexadecimal values;
- `CategoryCoverMediaCandidate` is a transport-neutral validation input containing
  only media identity, tenant, MIME, size, dimensions and `MediaImageDescriptor`;
- cover candidate policy rejects foreign tenants, unsupported image MIME, size or
  dimension violations, descriptor mismatch and non-direct-public delivery;
- a verifier rejects Media persistence/storage access and arbitrary category
  image URL/path fields;
- maintainer verification of the `FORUM-13A` commands passed on 2026-07-21.

### Delivered in `FORUM-13B`

- `resolve_category_cover_for_write` resolves Media metadata only through
  `MediaAssetReadPort`, validates the candidate and returns stable
  `FORUM_CATEGORY_COVER_MEDIA_CAPABILITY_UNAVAILABLE` when the optional Media
  owner is not composed;
- `hydrate_category_cover_for_read` degrades to an absent descriptor only in
  the explicit Media-disabled profile;
- not-found, timeout, storage and other Media provider failures remain typed
  `ForumError::CapabilityFailure` values with source code and retryability;
- the category-presentation verifier locks the optional-capability split and
  rejects swallowed provider failures;
- source-level contracts and fixtures were added in this slice; maintainer
  execution of the verification commands remains pending.

### Remaining scope

- Media must publish quarantine and deletion lifecycle state through its owner
  read contract before Forum persists `cover_media_id`;
- add the owner command, persistence, response integration and admin/storefront
  image selection after that state is available;
- compose the Media read provider into actual Forum transport entrypoints and
  capture executable media-disabled/media-enabled evidence after persistence
  exists.

### Definition of done

No forum table stores arbitrary asset URLs and a foreign/quarantined asset
cannot be attached.

### Verification

```bash
cargo test -p rustok-forum category_presentation
node scripts/verify/verify-forum-category-presentation.mjs
node scripts/verify/verify-forum-category-presentation.test.mjs
cargo xtask module validate forum
```

## `FORUM-14` — topic and reply attachments

**Status:** `planned`  
**Priority:** P1  
**Dependencies:** FORUM-08, media upload/reference contracts

### Scope

Add tenant-scoped attachment relations for topic/reply target, target revision,
media identity, order, usage and optional localized caption. Supported usage
types are explicit (`inline_image`, `gallery_image`, `file`, `video`, `audio`).

Use temporary upload sessions with expiry so abandoned uploads are reclaimable.
Enforce per-tenant/trust-level limits for count, file size, aggregate size,
MIME and image dimensions. Forum deletion detaches relations; media owns final
asset cleanup and shared-reference checks.

### Definition of done

- no direct media table access;
- no unbounded attachment lists;
- shared media references survive deletion of one post;
- replay and edit revisions do not duplicate relations;
- disabled media behavior is explicit.

## `FORUM-15` — public member summary and avatar integration

**Status:** `planned`  
**Priority:** P1  
**Dependencies:** profiles and media contracts

### Decision

Do not create `rustok-member`.

```text
auth/users       login identity and sessions
rustok-profiles  public member identity and avatar/banner references
rustok-forum     forum-only stats, trust, badges and restrictions
```

### Scope

Expose a batched author/member summary containing user ID, handle, display
name, media descriptor, preferred locale, forum stats and forum badges. Use the
shared UI avatar primitive through forum-specific composition components;
fallback is media image, generated initials, then generic avatar.

Respect profile visibility, blocked relationships, deleted-user tombstones and
media quarantine. Do not copy display name/avatar into forum source-of-truth
rows. An event-driven read projection is allowed only with profile revision and
reconciliation.

### Definition of done

Topic/reply lists render authors without N+1 reads and profile/avatar changes
propagate without stale identity becoming authoritative.

## `FORUM-16` — read tracking and unread state

**Status:** `planned`  
**Priority:** P1  
**Dependencies:** FORUM-07/10

### Scope

Persist monotonic per-user topic read position/revision. Expose unread count,
last-read position and unread filters on bounded topic projections. Add mark
topic/category/all read commands.

Use `GREATEST(existing, incoming)` or equivalent compare-and-set semantics.
Anonymous page views do not create read rows. Cache and realtime updates may
accelerate the badge but database position/revision remains canonical.

### Definition of done

Concurrent devices cannot move read state backwards, deleted/hidden replies do
not inflate unread counts, and category/all-read commands are resumable and
bounded.

## `FORUM-17` — drafts, autosave and bookmarks

**Status:** `planned`  
**Priority:** P1  
**Dependencies:** media upload sessions; NOTIFY-02 for bookmark reminders

### Scope

Add revisioned topic/reply drafts with locale, content format, attachment
session, expiry and one active draft per user/context. Autosave uses expected
revision and idempotency. Add bookmarks for topic/reply targets with optional
private note and reminder time.

### Definition of done

Drafts restore across devices, stale autosaves conflict instead of overwriting,
discard/expiry cleans temporary assets, bookmark target access is revalidated,
and reminders are optional notification jobs rather than forum timers.

## `FORUM-18` — votes, reactions, reputation and badges

**Status:** `planned`  
**Priority:** P1  
**Dependencies:** FORUM-05/09

### Scope

Replace check-then-write voting with tenant-scoped database upsert and
projection counters. Add a bounded, configurable reaction catalog and explicit
one/multiple reaction policy.

Initially keep reputation and badges forum-owned:

```text
forum_reputation_ledger
forum_user_reputation
forum_badges
forum_user_badges
```

The reputation ledger is immutable and idempotent by semantic source event.
Do not create a shared reputation/reactions module until a second real owner
consumer proves a neutral contract.

### Definition of done

Concurrent vote/reaction changes converge, self-vote/trust policy is enforced,
projection drift is reconcilable, and ledger replay cannot double award.

## `FORUM-19` — reports, moderation queue, restrictions and audit

**Status:** `planned`  
**Priority:** P0  
**Dependencies:** FORUM-06/09, RBAC decision contract

### Scope

Add forum-owned reports, immutable moderation actions and scoped member
restrictions. Queue filters include pending content, reports, spam score,
assignment, age and SLA. Restrictions support read-only, posting suspension,
premoderation and category/channel scope with start/expiry/reason/issuer.

Every moderation mutation requires a reason, actor, before/after state, audit
record and owner event in one transaction. Bulk actions are bounded and
idempotent.

### Definition of done

No moderation path bypasses RBAC, restricted members cannot evade scope through
another transport, private reasons are not leaked, and expired restrictions
reconcile automatically.

## `FORUM-20` — ACL and visibility inheritance

**Status:** `planned`  
**Priority:** P0  
**Dependencies:** RBAC, channel/group capability contracts

### Scope

Model typed category visibility and create/reply/moderate audience rules:
public, authenticated, roles, trust level, channel members, group members and
explicit allow/deny. Child categories inherit unless explicitly overridden.
A topic may narrow but cannot broaden parent visibility without a privileged
command.

Forum reads, notifications, search, SEO and deep links must call the same
visibility policy. Do not place ACL policy in arbitrary JSON.

### Definition of done

Cross-tenant, blocked, private and channel-restricted content is consistently
absent from reads, search, SEO and notifications, including replay and cache
profiles.

## `FORUM-21` — move, merge, split and fork topics

**Status:** `planned`  
**Priority:** P1  
**Dependencies:** FORUM-04/08/09/20

### Scope

Implement idempotent owner operations for moving topics, merging threads,
splitting selected replies, forking a reply branch and moving reply ranges.
Preserve revisions, attachments, mentions and audit. Remap reply positions
safely, deduplicate subscriptions, revalidate solutions and ACL, update
category counters and create canonical URL aliases.

### Definition of done

Each operation has an operation ID, reason, transactional state change and
semantic event; retry produces the same result; partial moves are impossible;
source tombstones/redirects are safe.

## `FORUM-22` — topic kinds and scheduled policies

**Status:** `planned`  
**Priority:** P2  
**Dependencies:** FORUM-09/19/20

### Scope

Add explicit topic kinds:

```text
discussion
question
wiki
announcement
poll
```

Q&A solution applies only to questions. Wiki edit policy uses trust/RBAC.
Announcements define reply policy. Add slow mode, bump cooldown, max replies,
auto-close after inactivity and scheduled open/close through durable jobs.

Polls use a typed child model or a later neutral poll capability, never
unbounded topic metadata.

## `FORUM-23` — search/index integration

**Status:** `planned`  
**Priority:** P0  
**Dependencies:** FORUM-09/20, durable index consumer

### Scope

Publish versioned category/topic/reply/member index projections. Index only
published/approved content with safe author summary and visibility metadata.
Consumers use durable inbox and owner revision ordering. Search filters include
category subtree, author, tag, locale, date, solved, kind, channel/group and
attachment presence.

### Degraded mode

When search/index is disabled, bounded SQL title/tag fallback may be used or a
typed search-unavailable result returned. Core forum reads remain available.

### Definition of done

Pending/hidden/private content never leaks, out-of-order events cannot regress
a projection, owner/index revisions reconcile, and deletion/ACL changes remove
documents.

## `FORUM-24` — localized routes, canonical URLs and aliases

**Status:** `planned`  
**Priority:** P1  
**Dependencies:** FORUM-04/21/25, SEO contracts

### Scope

Use localized category paths and stable topic routes with short identity.
Maintain locale-specific slugs, canonical locale routes, old slug aliases,
move/rename redirects and hreflang. Private/pending targets are not published
to SEO. Use schema.org DiscussionForumPosting or QAPage only when semantics
match.

ID routes remain internal compatibility paths, not the primary storefront UX.

## `FORUM-25` — full multilingual and RTL contract

**Status:** `planned`  
**Priority:** P1  
**Dependencies:** platform locale contract

### Scope

Track source locale, translation kind (`original`, `manual`, `machine`),
translation status, translator and publication timestamps for category/topic/
reply content. Missing translation returns explicit fallback provenance, never
a silently empty body. Slugs and moderation may be locale-specific.

UI packages use tenant-enabled locales rather than a hard-coded `en`/`ru`
manifest and support RTL direction, logical CSS properties, editor behavior and
nested quotes. Notification locale is selected from the recipient, not actor.

## `FORUM-26` — anti-spam, limits and trust levels

**Status:** `planned`  
**Priority:** P0  
**Dependencies:** FORUM-19, shared rate-limit capability

### Scope

Implement forum-local trust levels and explainable posting policy based on
account age, reading/activity, approved posts, flags, reputation and moderation
history. Bound topics/day, replies/minute, links, mentions, attachments, edits
and bump intervals. Add duplicate-content hashing and optional external/AI spam
scoring.

External/AI scoring is optional and cannot be a synchronous correctness
dependency. Forum owns policy; shared rate limiting owns distributed execution.

## `FORUM-27` — member directory and forum profile

**Status:** `planned`  
**Priority:** P1  
**Dependencies:** FORUM-15/18/20

### Scope

Provide member directory and handle-based forum profile pages with topics,
replies, solutions, badges, reputation history and permitted activity views.
Compose profiles, forum stats/reputation and media descriptors without copying
their source-of-truth data.

Respect private profiles, blocks, deleted-account tombstones and
moderator-only statistics.

## `FORUM-28` — editor, renderer and sanitization

**Status:** `planned`  
**Priority:** P0  
**Dependencies:** FORUM-12/14/25

### Scope

Join the platform's atomic
[Richtext cutover](../../../docs/modules/rich-text-implementation-plan.md).
Forum topic and reply bodies use one `RichTextDocument` with the owner-selected
`discussion` profile. Markdown and `rt_json_v1` remain historical migration
inputs only; BBCode belongs only to an explicit importer. Support owner-aware
quotes and mentions first, then add spoilers, emoji, media, attachments,
preview, drafts, and keyboard behavior only through the shared extension and
server-profile contract.

Use the single `rustok-content::richtext` HTML renderer and plain-text
extractor. Do not persist an independently writable HTML source or implement a
Forum renderer. Cache only a derived projection keyed by canonical document
hash and current renderer identity when evidence requires it.

Migrate active topic/reply rows and both revision tables. Deletion and domain
events must use typed lifecycle state instead of the literal `[deleted]` and
`body_format = 'markdown'`. Next Forum UI/API/navigation must move out of the
Blog package; Leptos uses native `#[server]` with parallel GraphQL and never
retries a failed mutation blindly through REST.

## `FORUM-29` — realtime acceleration

**Status:** `planned`  
**Priority:** P2  
**Dependencies:** FORUM-09/10/16, NOTIFY-04

### Scope

SSE/WebSocket may accelerate published replies, lifecycle changes, reactions
and unread notification counts. Typing/presence is ephemeral and not placed in
outbox.

On reconnect, clients reload the canonical topic revision and reply cursor;
socket sequence alone is never trusted for correctness.

## `FORUM-30` — complete admin product

**Status:** `planned`  
**Priority:** P1  
**Dependencies:** stable owner contracts from FORUM-04/19/20/26/33

### Scope

Module-owned admin pages cover dashboard, category tree, topic/reply management,
pending queue, reports, restrictions, tags, badges/trust, settings, analytics,
reconciliation and links to notification defaults. Category editing includes
localization, tree placement, icon/media, ACL, moderation and topic policy.

The moderation workspace shows content, author/history, reports, assignment,
reason and audit. It uses owner transport facades and preserves the
core/transport/UI boundary.

## `FORUM-31` — complete NodeBB-class storefront

**Status:** `planned`  
**Priority:** P1  
**Dependencies:** FORUM-12 through FORUM-29 as consumed

### Scope

Provide forum home, nested category pages, topic lists/details, composer,
members, bookmarks, drafts, unread/recent/popular, tags and solved questions.
Cards expose author/last poster, counts, unread state, lifecycle indicators,
tags, locale and activity. Topic pages expose breadcrumbs, author summary,
stable reply numbers, quotes/thread links, reactions/votes, solution, history,
attachments, subscription level, bookmark, report and navigation.

Meet keyboard, focus, semantic heading, ARIA, contrast, reduced-motion,
responsive, SSR and hydration requirements. Use shared UI primitives for
avatar, forms, tables and pagination.

## `FORUM-32` — Page Builder and widget evolution

**Status:** `in_progress`  
**Priority:** P2  
**Dependencies:** stable bounded read ports; Page Builder/pages provider readiness

### Remaining scope

Add category tree, latest/popular/unanswered/solved topics, recent replies, top
members, tags and forum-stat widgets through public forum read ports. Preserve
`readonly`, `degraded` and `hidden` fallback profiles.

Replace the synthetic Wave packet with an observed tenant control-plane run
that correlates builder write, forum publication and storefront read after the
`pages` reference-consumer gate. Page Builder stays optional; forum routes must
not depend on provider availability.

### Verification

```bash
npm run verify:page-builder:consumer:forum
npm run verify:forum:wave-evidence-freshness
```

## `FORUM-33` — analytics, observability and reconciliation

**Status:** `planned`  
**Priority:** P1  
**Dependencies:** owner projections and consumers

### Scope

Instrument bounded, label-safe metrics for command latency, moderation age,
approval rate, reports, notification lag, unread counts, active members, search
lag, counter drift, media enrichment failures, locale fallback and spam
outcomes.

Add report/repair operations for category/topic/reply counters, solution state,
subscriptions, mentions, attachments, profile/index projections and
notification fan-out. Repair requires RBAC, dry-run, audit and idempotent job
state.

## `FORUM-34` — import/export and NodeBB migration toolkit

**Status:** `planned`  
**Priority:** P2  
**Dependencies:** stable category/topic/reply/media/profile schemas

### Scope

Provide module-local CLI commands for export, import, NodeBB import,
reconciliation and search rebuild. Imports validate and map users/profiles,
category tree, topics/replies, media, tags, votes/reputation when supported and
URL aliases.

Jobs are dry-runnable, resumable, cursor-based, idempotent and bounded; they do
not load complete exports into memory.

# Shared notifications task cards

`rustok-notifications` and `rustok-notifications-api` now exist. This section
remains the canonical cross-module task/status source until a deliberate
plan-ownership migration is approved. Module-local documentation records stable
owner contracts and execution gates without duplicating this backlog.

## `NOTIFY-00` — create the notifications owner module

**Status:** `in_progress`  
**Priority:** P0 platform  
**Dependencies:** durable outbox/inbox foundation

### Scope

Create `rustok-notifications`, module-owned admin/storefront packages, and a
small neutral notifications API crate only for contracts already needed by
forum, blog, social and commerce.

Notifications owns inbox, preferences, unread/read state, recipient fan-out,
grouping, digests, retention and delivery attempts. It does not own source
subscriptions, SMTP, push vendor SDKs, user identity or source authorization.

Define source-provider registration for semantic event descriptors, bounded
audience resolution and target-open authorization. Producer modules declare an
optional capability and continue to work when notifications is absent.

### Delivered in `NOTIFY-00A`

- `rustok-notifications-api` publishes validated source/type/template/target
  keys, bounded template data, revisioned source-event identity and safe
  root-relative target routes;
- audience pages are capped at 256 unique recipients and all construction and
  deserialization paths enforce the same bounds;
- `NotificationSourceProvider` owns semantic description, cursor-based audience
  resolution and per-recipient target-open authorization with typed retryability;
- `NotificationSourceRegistry` is unique by source slug and is composed through
  `ModuleRuntimeExtensions` without producer dependencies on the owner crate;
- `rustok-notifications` initializes a healthy empty registry and exposes only
  source discovery until owner persistence exists;
- module-owned admin/storefront packages expose explicit foundation/unavailable
  states and never synthesize unread counts or shadow inbox state;
- static verifier fixtures reject direct producer imports of the owner crate,
  arbitrary JSON/persistence in the neutral contract and synthetic unread state.

### Delivered in `NOTIFY-00B`

- the optional owner is registered in module/distribution/server composition but
  remains outside tenant default-enabled settings;
- producer factories are registered before host services exist and materialized
  only after `HostRuntimeContext` contains the executable database;
- factory/provider slug conflicts, identity mismatch and build failures fail
  startup instead of silently removing a source;
- Forum provides executable source contracts for `forum.topic.created` and
  `forum.mention.user_added` while Forum commands remain independent from the
  optional Notifications owner;
- module-owned admin/storefront packages remain explicit foundation/unavailable
  surfaces until inbox APIs exist.

### Remaining scope

- record maintainer execution of the neutral API, runtime composition, provider
  fallback and module-owned package verification sets;
- preserve optional-module startup/degraded behavior while global migration and
  production worker composition are promoted under NOTIFY-01/03;
- do not add inbox/read UI before final owner commands and privacy policy exist.

### Definition of done

Forum works in notifications-off and notifications-on profiles without a
synchronous notification call in forum transactions.

### Verification

```bash
cargo test -p rustok-notifications-api
cargo test -p rustok-notifications
cargo check -p rustok-notifications-admin --all-targets
cargo check -p rustok-notifications-storefront --all-targets
cargo test -p rustok-forum --test notification_source_sqlite -- --nocapture
node scripts/verify/verify-notifications-foundation.mjs
node scripts/verify/verify-notifications-foundation.test.mjs
node scripts/verify/verify-notifications-runtime.mjs
node scripts/verify/verify-notifications-runtime.test.mjs
```

## `NOTIFY-01` — notification persistence

**Status:** `in_progress`  
**Priority:** P0  
**Dependencies:** NOTIFY-00

### Scope

Add tenant/user-scoped notifications, channel deliveries, fan-out jobs/items,
preferences, digest jobs/items and push subscriptions. Use typed status,
channel and priority values, bounded safe payloads, idempotency keys and
tenant-composite integrity.

At minimum, dedupe by tenant, recipient, source event and notification type.
Read implies seen. Provider errors are classified/bounded and secrets or raw
private content are not persisted.

### Delivered in `NOTIFY-01A`

- PostgreSQL/SQLite owner persistence covers notifications, delivery attempts,
  fan-out jobs/items, preferences, digest jobs/items and encrypted push
  subscriptions;
- recipient/user references are tenant-composite and optional actor/fan-out
  notification references are guarded against tenant mismatch;
- typed states, priorities, channels and modes match database `CHECK` values;
- read implies seen, leased work requires owner/expiry, terminal work requires
  completion timestamps, and JSON/cursor/error fields are bounded;
- notification/source and command idempotency keys are tenant-scoped;
- raw contact data, source-private payloads, rendered HTML and plaintext push
  endpoints are excluded.

### Delivered in `NOTIFY-01B`

- `notification_source_inbox` durably accepts one source event identity keyed by
  tenant, source slug and source event ID;
- changed event type or source revision conflicts instead of creating a second
  inbox row;
- typed pending/processing/completed/suppressed/retryable/rejected state stores
  bounded retry/error metadata and recoverable leases;
- provider-independent acceptance prevents optional source absence from losing a
  committed owner event;
- completed source rows retain their descriptor-bound fan-out job link;
- the Notifications module migration source orders the inbox migration after the
  base persistence migration for PostgreSQL and SQLite.

### Remaining scope

- promote the module-local migrations into verified global server migration
  composition;
- implement the policy-approved command that converts a pending candidate into
  one final notification row and optional channel work;
- add explicit retention, reconciliation and repair commands with RBAC, dry-run
  and idempotent job state;
- keep inbox/preferences/digest/delivery transports closed until their owner
  commands are implemented and verified.

### Verification

```bash
cargo test -p rustok-notifications --test persistence_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_sqlite -- --nocapture
NOTIFICATIONS_TEST_DATABASE_URL="$DATABASE_URL" \
  cargo test -p rustok-notifications --test persistence_postgres -- --nocapture --test-threads=1
node scripts/verify/verify-notifications-persistence.mjs
node scripts/verify/verify-notifications-persistence.test.mjs
node scripts/verify/verify-notifications-source-fanout.mjs
cargo xtask module validate notifications
```

## `NOTIFY-02` — preferences, quiet hours and digests

**Status:** `planned`  
**Priority:** P1  
**Dependencies:** NOTIFY-01

### Scope

Resolve mandatory policy, per-type override, source/category override, tenant
default and platform default in a documented order. Support off/instant/digest,
recipient timezone, quiet hours and hourly/daily/weekly digest windows.

Digest rendering rechecks target visibility and deduplicates source items.

## `NOTIFY-03` — durable source consumption and fan-out

**Status:** `in_progress`  
**Priority:** P0  
**Dependencies:** NOTIFY-01, durable consumer inbox

### Scope

Consume owner events idempotently, invoke the registered source provider,
resolve candidate audiences by cursor/batch, apply preferences/privacy/blocks,
create in-app rows and enqueue channel deliveries in bounded transactions.

Large audiences create leased fan-out jobs; never place all recipient IDs in an
event or load them into memory. Deduplicate recipients reached through author,
mention, subscription and category-watcher rules.

### Delivered in `NOTIFY-03A`

- `NotificationFanoutService::enqueue_source_event` durably accepts or replays a
  typed source identity before provider discovery;
- `materialize_source_event` leases the source inbox, classifies provider errors,
  suppresses an unavailable source target and creates/replays one bounded
  descriptor fan-out job;
- `process_fanout_page` leases the job, invokes cursor-based audience resolution
  with a hard maximum of 256, rejects oversized, empty-continuing or stalled
  pages, and persists idempotent pending candidates before cursor advancement;
- expired source/job leases cannot complete work and can be reclaimed;
- descriptor and source identity changes fail closed on replay;
- candidate rows remain `pending` with no final notification ID and no delivery
  attempt, so preference/privacy cannot be bypassed;
- Forum `forum.mention.user_added` produces at most one candidate after exact
  relation and current source-visibility checks; pending replies are retryable,
  self-mentions and unavailable sources are suppressed;
- SQLite scenarios cover source replay/conflict, two-page completion, terminal
  replay, zero final notification rows and Forum mention source behavior;
- the machine contract and source verifier are
  `crates/rustok-notifications/contracts/notifications-source-fanout.json` and
  `scripts/verify/verify-notifications-source-fanout.mjs`.

### Remaining scope

- wire the production outbox relay/consumer runner into
  `enqueue_source_event` with durable claim/retry/DLQ controls;
- process each pending candidate through preference resolution, block/profile
  privacy, recipient-specific source authorization, grouping/dedupe and final
  notification creation;
- enqueue channel deliveries only after policy acceptance and outside provider
  calls from the owner database transaction;
- add bounded moderator-directory expansion for
  `forum.mention.audience_added` through an owner port;
- add PostgreSQL concurrent lease/retry/replay evidence and reconciliation.

### Verification

```bash
cargo test -p rustok-notifications --test fanout_sqlite -- --nocapture
cargo test -p rustok-forum --test notification_source_sqlite -- --nocapture
node scripts/verify/verify-notifications-source-fanout.mjs
cargo xtask module validate notifications
```

## `NOTIFY-04` — in-app inbox API

**Status:** `planned`  
**Priority:** P1  
**Dependencies:** NOTIFY-01/07

### Scope

Expose bounded cursor reads, grouped/unread views, unread counts and
seen/read/unread/mark-all/archive mutations. All operations are tenant/user
scoped. Opening a target calls the source authorization provider; forbidden or
deleted targets become a safe unavailable state without an existence oracle.

## `NOTIFY-05` — channel delivery provider SPI

**Status:** `planned`  
**Priority:** P1  
**Dependencies:** NOTIFY-01

### Scope

Define delivery-provider contracts for email, web/mobile push and optional SMS.
Use owner idempotency keys, retry/backoff, attempt journal, transient/permanent
classification, webhook receipt inbox and provider readiness. Provider calls
never run inside the notification database transaction.

Email addresses and other contact data are resolved from a trusted identity/
contact provider at delivery time, not copied into source events.

## `NOTIFY-06` — localized semantic templates

**Status:** `planned`  
**Priority:** P1  
**Dependencies:** NOTIFY-00/05

### Scope

Producer modules own semantic template catalogs and required variables;
notifications owns template selection/rendering. Resolve locale from user
preference, profile locale, tenant default and platform fallback. Record
fallback outcomes. In-app payloads do not accept arbitrary HTML.

## `NOTIFY-07` — privacy and security

**Status:** `planned`  
**Priority:** P0 security  
**Dependencies:** NOTIFY-03 and source authorization contracts

### Scope

Check tenant, source visibility, channel/group membership, blocks/mutes,
profile/content status and recipient preferences before creation and again
before target open or delayed delivery. Payloads store only safe snapshots and
route descriptors. Deleted/private targets are redacted or archived and unread
counts corrected.

## `NOTIFY-08` — notification UI packages

**Status:** `planned`  
**Priority:** P1  
**Dependencies:** NOTIFY-04/06

### Scope

Storefront: bell, canonical unread badge, grouped inbox, cursor loading, safe
deep links, empty/error/offline state and preferences. Admin: tenant defaults,
template catalog, provider status, attempts, fan-out jobs, DLQ/replay and
metrics without secrets.

Realtime badges are accelerators and reconcile with the database count.

## `NOTIFY-09` — FBA and degraded profiles

**Status:** `planned`  
**Priority:** P0 release  
**Dependencies:** NOTIFY-00..08

### Scope

Publish neutral source, inbox, mutation, preference and delivery-provider
contracts. Verify notifications off/on, email off/on, push off, delayed outbox,
consumer retry and source-module disablement. Static fixtures are not accepted
as runtime evidence.

# Cross-module proof tasks

## `LINK-FORUM-01` — forum to notifications

**Status:** `planned`  
**Priority:** release blocker  
**Dependencies:** FORUM-09/11/12/19, NOTIFY-03/07

Prove approved reply, pending moderator alert, mention, new watched topic,
solution and moderation outcomes. Hidden/deleted targets must not deliver.
Duplicate events create one notification, overlapping audiences dedupe, and
notifications-off leaves forum commands successful.

Evidence correlates forum transaction, event ID, outbox row, consumer inbox,
audience resolution, notification row, channel delivery and open authorization.

## `LINK-FORUM-02` — profiles and media

**Status:** `planned`  
**Priority:** release blocker  
**Dependencies:** FORUM-13/14/15

Prove avatar propagation without N+1, fallback for deleted/quarantined media,
private-profile behavior, category cover, attachments, media-disabled profile,
shared references and deleted-user tombstones. Forum must not query owner
private tables.

## `LINK-FORUM-03` — index and search

**Status:** `planned`  
**Priority:** release blocker  
**Dependencies:** FORUM-20/23

Prove publish, translation, moderation approval, move, hide/delete, ACL change,
out-of-order events and search-disabled behavior. Projection revision must
match owner revision and private/channel content must remain excluded.

## `LINK-FORUM-04` — capability profiles

**Status:** `planned`  
**Priority:** release blocker  
**Dependencies:** module manifests and FBA registries

Required dependencies should be limited to genuine owner contracts. Media,
notifications, search/index, Page Builder, channel/group and delivery providers
are optional capabilities with explicit degraded behavior. Verify minimal,
media, notifications, search and full profiles. Missing required capability
disables the module with a clear startup error; missing optional capability
does not cause a 5xx.

## `LINK-FORUM-05` — production release gate

**Status:** `planned`  
**Priority:** release blocker  
**Dependencies:** all required P0 tasks and LINK-FORUM-01..04

Forum is not production-ready while any of the following is possible:

- cross-tenant category, reply, vote, media or subscription relation;
- partial category/topic/reply owner mutation;
- reply to locked/inactive topic;
- pending content changing public counters/search/notifications;
- duplicate reply position or lost counter update;
- hard deletion of discussion history through product UI;
- edit/delete without revision, audit and owner event;
- unauthorized/private notification or unsafe deep link;
- unbounded pagination, mentions, attachments or fan-out;
- unsafe rendered HTML;
- private/pending search or SEO leak;
- silent empty multilingual fallback;
- forum command failure because an optional module is disabled.

Release evidence is waiver-free and generated by executable runtime profiles.

# FFA/FBA and UI boundary state

- FFA status: `in_progress`.
- FBA status: `boundary_ready`.
- Structural shape: `core_transport_ui`.
- Admin/storefront work must preserve module-owned core, transport and UI
  adapters.
- Page Builder consumer contracts and static fallback profiles exist, but
  observed rollout evidence remains open under `FORUM-32`.
- Hosts compose owner-owned packages and do not absorb forum policy.

# Required verification set

Use the subset relevant to each task and record exact results. Release and
cross-module PRs use the complete set.

```bash
cargo test -p rustok-forum
cargo test -p rustok-forum-admin
cargo test -p rustok-forum-storefront

cargo test -p rustok-forum --test runtime_regression_baseline
cargo test -p rustok-forum --test wave_invariants_postgres
cargo test -p rustok-forum --test soft_delete_revision_postgres
cargo test -p rustok-forum --test soft_delete_revision_sqlite
cargo test -p rustok-forum --test owner_lifecycle_sqlite
cargo test -p rustok-forum --test mention_contract
cargo test -p rustok-forum mention_relation
cargo test -p rustok-forum quote_command
cargo test -p rustok-forum inline_quote
cargo test -p rustok-forum --test mention_quote_runtime_postgres -- --nocapture --test-threads=1
cargo test -p rustok-forum --test notification_source_sqlite -- --nocapture

cargo xtask module validate forum
cargo xtask module test forum

npm run verify:forum:admin-boundary
npm run verify:forum:storefront-boundary
npm run verify:page-builder:consumer:forum
npm run verify:forum:wave-evidence-freshness
npm run verify:channel:proof-points
node scripts/verify/verify-forum-mention-contract.mjs
node scripts/verify/verify-forum-mention-contract.test.mjs
node scripts/verify/verify-forum-mention-persistence.mjs
node scripts/verify/verify-forum-mention-persistence.test.mjs
node scripts/verify/verify-forum-mention-integration.mjs
node scripts/verify/verify-forum-mention-events.mjs
node scripts/verify/verify-forum-quote-commands.mjs
node scripts/verify/verify-forum-mention-runtime-proof.mjs
cargo test -p rustok-profiles
npm run verify:media:fba
npm run verify:outbox:fba
npm run verify:rbac:fba
npm run verify:index:fba

cargo test -p rustok-notifications-api
cargo test -p rustok-notifications
cargo test -p rustok-notifications --test persistence_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_sqlite -- --nocapture
cargo check -p rustok-notifications-admin --all-targets
cargo check -p rustok-notifications-storefront --all-targets
node scripts/verify/verify-notifications-foundation.mjs
node scripts/verify/verify-notifications-foundation.test.mjs
node scripts/verify/verify-notifications-runtime.mjs
node scripts/verify/verify-notifications-runtime.test.mjs
node scripts/verify/verify-notifications-persistence.mjs
node scripts/verify/verify-notifications-persistence.test.mjs
node scripts/verify/verify-notifications-source-fanout.mjs

git diff --check
```

Add production outbox-consumer, candidate-policy, PostgreSQL lease and final
notification/open-authorization evidence as those owner slices are implemented.

# PR slicing

The canonical order is by task dependency, not by the old external PR numbers.
Use one task per PR; split only mechanically large migrations/UI surfaces while
keeping each PR independently safe.

Recommended next slices:

1. `NOTIFY-03/07`: process pending candidates through preference, profile/block
   privacy and recipient-specific source authorization before final notification
   creation;
2. `NOTIFY-01`: promote verified global migrations and add retention/
   reconciliation commands;
3. `NOTIFY-03`: wire production outbox intake and PostgreSQL lease/retry evidence;
4. `FORUM-12`: record maintainer PostgreSQL proof, add moderator audience owner
   expansion and final notifications-on evidence;
5. `NOTIFY-05/06`: delivery provider SPI and localized semantic rendering;
6. `FORUM-13`: category media references after Media lifecycle state exists;
7. `FORUM-14`: attachment relations and upload sessions;
8. `FORUM-15`: batched member/avatar projection;
9. `LINK-FORUM-02`: profiles/media runtime proof;
10. `FORUM-16`: read/unread state;
11. `FORUM-19`: reports/moderation/restrictions;
12. `FORUM-20`: ACL and visibility policy;
13. `FORUM-23`: index projections;
14. `LINK-FORUM-01` and `LINK-FORUM-03` only after their owner contracts are
    stable.

# Decisions that must not be reopened without an ADR

## No separate member module

`rustok-profiles` is the public member identity. Forum owns only forum-specific
stats, trust, badges, restrictions and activity.

## Media ownership

Profiles stores avatar/banner media references. Forum stores category and
post attachment references. `rustok-media` owns files, URLs, MIME, storage,
quarantine and deletion.

## Notifications are optional consumers

Forum always commits semantic events. It does not synchronously call
notifications to complete a command. Disabling notifications hides its UI and
stops deliveries without breaking forum state changes.

## Email is a provider

`rustok-email` performs delivery. The notifications owner controls recipient
resolution, preferences, timing, templates, retries and channel selection.

## No premature shared reactions/reputation/mentions module

Keep these models forum-owned until another real owner consumer demonstrates
a stable neutral contract. Publish semantic events to make later extraction
possible.

# Immediate next action

Process `notification_fanout_items` through explicit preference and profile/block
privacy policy, reauthorize the source for the recipient, and only then create a
final idempotent notification row. Keep channel delivery, moderator audience
expansion, production outbox wiring and maintainer PostgreSQL evidence as
separate bounded follow-up slices; never make Notifications a synchronous Forum
command dependency.
