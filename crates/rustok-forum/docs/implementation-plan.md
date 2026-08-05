---
id: doc://crates/rustok-forum/docs/implementation-plan.md
kind: module_implementation_plan
language: en
status: active
owners:
  - rustok-forum
  - rustok-notifications-program
last_reviewed: 2026-08-05
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

Forum richtext cutover is complete. Topic and reply writes accept one
`rustok_api::RichTextDocument`; storage contains its canonical serialized JSON;
reads expose `RichTextView` plus server-derived plain text. The owner applies
the `discussion` profile through `rustok-content::richtext`. Format selectors,
parallel content fields, alternate authoring modes, and module-local renderers
are forbidden.

Forum admin category/topic operations use one GraphQL transport. The removed
REST fallback, its unused write helpers, and tests that required fallback
behavior are not compatibility contracts; `rustok-forum-admin` has 36 passing
unit tests for the surviving path.

The Next admin Forum surface is module-owned under
`apps/next-admin/packages/forum/src`. It owns Forum navigation, topic/reply
GraphQL helpers, a canonical document reply composer, and the FORUM-21N/V/W/X
topic merge, split, fork and reply-range workflows. The host only registers and mounts the package. React and
Leptos use their shared framed richtext lifecycle adapters; Forum supplies only
the `discussion` profile and host-effective locale.

## Verification

Run `cargo xtask module validate forum` for the module contract and use the
task-specific commands and evidence paths recorded in the program ledger for
any changed Forum capability.

Run `npm run verify:forum:admin-boundary`
(`scripts/verify/verify-forum-admin-boundary.mjs`) after an admin-surface or
transport-boundary change. This is the fast guardrail for the module-owned
admin core/transport/UI split.

Run `npm run verify:blog:forum-ui-ownership`
(`scripts/verify/verify-blog-forum-ui-ownership.mjs`) after changing the Next
admin Forum package or its former Blog ownership boundary.

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
  -> category tree, topics/replies, revisions, subscriptions, read state,
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
| `FORUM-16` | `in_progress` | FORUM-16A-F add tenant-scoped monotonic topic read state, bounded unread projections, resumable category-subtree/all-read owner commands, REST/GraphQL contracts, authenticated visible-topic storefront composition and a source-ready PostgreSQL concurrency/query-plan proof; maintainer execution/output capture and visibility-scoped storefront bulk commands remain. |
| `FORUM-17` | `planned` | Drafts, autosave, bookmarks and optional reminders. |
| `FORUM-18` | `planned` | Atomic votes, reactions, reputation ledger and badges. |
| `FORUM-19` | `planned` | Reports, moderation queue, restrictions and audit. |
| `FORUM-20` | `in_progress` | FORUM-20A-AZ provide inherited and richer category/topic visibility, recipient-aware Forum notification authorization, the Notifications inbox/group owner plane, authenticated storefront ports, native and GraphQL read/open/write transport parity, grouped UI and navigation, exact topic/reply create authorization, topic-local reply narrowing, inherited moderation audiences, and existing solution-route transport composition. FORUM-20BA synchronizes the canonical ledger and owner notes after FORUM-20AV-AZ. Remaining read/search/index/SEO/deep-link migration, visibility-scoped bulk read commands, future moderation transport reuse, scheduled reconciliation/redaction, delivery transports and PostgreSQL cross-consumer evidence remain. |
| `FORUM-21` | `planned` | FORUM-21A-X provide move, merge, split, fork and reply-range owners, manager GraphQL transports, and split/fork/reply-range admin composition; retained owner/transport runtime evidence remains, while localized route identity proceeds under FORUM-24. |
| `FORUM-22` | `planned` | Topic kinds, wiki/announcement/Q&A policies and scheduled lifecycle. |
| `FORUM-23` | `in_progress` | FORUM-23A through FORUM-23A11 harden public-author Search projections and durable privacy invalidation; FORUM-23B1 through FORUM-23B2F4 add exact Forum category, audience, result-eligibility, trusted-channel, author, tag, solved, locale, date and current-channel filtering; FORUM-23B2G1 adds durable Search ingest ordering; FORUM-23B2G2A/A1 add the Forum owner revision ledger and database hardening; FORUM-23B2G2B1/B2 add the bounded owner source, Search checkpoint and repair protocol; FORUM-23B2G2B3A-C add the caused sealed wire event, atomic dual publisher and default-off persistent one-inbox consumer; FORUM-23B2G2B3D0 freezes executable runtime evidence and FORUM-23B2G2B3D1 reconciles this canonical plan. Arbitrary channel/group filtering remains owner-contract blocked, kind waits on FORUM-22, attachment presence waits on FORUM-14, and maintainer PostgreSQL/Iggy plus LINK-FORUM-03 runtime evidence remain. |
| `FORUM-24` | `planned` | FORUM-24A adds deterministic exact-locale topic route identity and an immutable redirect/tombstone ledger; FORUM-24B composes new merge redirects and FORUM-24C composes delete tombstones in their owner transactions. Rename composition, historical backfill, category routes, storefront mounts, hreflang/SEO policy and runtime evidence remain. |
| `FORUM-25` | `planned` | Full content/UI multilingual contract and RTL support. |
| `FORUM-26` | `in_progress` | FORUM-26A-J provide authoritative Forum trust state/facts, posting-policy contracts, evaluation/composition, account-age, topics-read, approved-post and topic/reply create-window facts, plus pre-enforcement author/query-plan hardening. Active flags/moderation history, reputation, edit windows, bump age, policy persistence, owner enforcement, shared rate-limit execution, duplicate hashing, optional scoring, transports, UI and maintainer runtime evidence remain. |
| `FORUM-27` | `planned` | Member directory, forum profile, badges and activity views. |
| `FORUM-28` | `done` | Canonical editor, safe renderer, plain-text projection, Next/Leptos shared adapters, and atomic storage/transport cutover. |
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

- forum-admin GraphQL transport routes category placement through owner
  commands, and the admin boundary verifier rejects generic `position` bypasses;
- tenant-scoped category topic policy defaults to `allows_topics = true` for
  existing categories without a stored policy row;
- REST, GraphQL and OpenAPI and the canonical tree expose the policy;
- PostgreSQL and SQLite serialize policy changes with topic writes and reject
  topic inserts or category reassignment when topic creation is disabled;
- disabling policy preserves existing topics and controls only new placement;
- shared PostgreSQL/SQLite scenarios cover default allow, disable, blocked
  writes, tenant isolation and re-enable.

### Delivered in `FORUM-04F`

- `CategoryService::archive_subtree` and `restore_subtree` serialize lifecycle changes with the tenant category-tree lock;
- default lifecycle rows preserve existing categories as active without backfill;
- archive writes descendants before ancestors and restore removes ancestor lifecycle rows before descendants;
- REST, GraphQL, OpenAPI and the canonical tree expose subtree lifecycle state and owner commands;
- PostgreSQL and SQLite reject active children beneath archived parents, partial restore and new topic placement in archived categories;
- existing topics are preserved and shared PostgreSQL/SQLite scenarios cover archive, restore, tenant isolation and direct-write guards.

### Delivered in `FORUM-04G`

- forum-admin loads the bounded canonical category tree through its single
  GraphQL transport instead of reconstructing hierarchy from a second flat
  list contract;
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
source revision and mentioned user. Mention extraction traverses the validated
`RichTextDocument` tree directly, excludes code blocks and code marks, and has
no format branch. Resolve handles through the profiles contract, cap mentions
per revision, reject abusive mass mentions and make special audiences such as
moderators permission-gated.

Editing uses a revision diff: new mention produces one semantic event, removed
or unchanged mentions do not produce duplicate delivery. Quotes retain the
quoted target and quoted revision so edits do not rewrite history.

### Delivered in `FORUM-12A`

- `extract_forum_mention_candidates` accepts `&RichTextDocument`, caps each
  revision at 32 unique targets, and deduplicates handles with the
  Profiles-owned handle grammar;
- structural extraction ignores `codeBlock` nodes, text carrying a `code` mark,
  and email address `@` tokens;
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
- relation revisions are created only by owner commands after canonical content
  validation; migrations do not manufacture source revisions or relation IDs;
- the crate-private `MentionRelationService` separates profile-dependent
  `prepare` from transaction-only `persist_in_tx`;
- `prepare` resolves handles through `ProfilesReader` and computes a SHA-256
  replay fingerprint over the canonical document, resolved targets and quotes;
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
- no source INSERT seed trigger or migration-owned placeholder identity exists;
  the owner command creates the first canonical relation revision.

### Delivered in `FORUM-12C`

- the sealed `ForumMentionEvent` family publishes
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

### Failure and degraded mode

A source locale has no relation revision until an owner command creates one;
there is no migration seed identity or alternate read path. Existing
topic/reply create/edit DTOs remain source-compatible; separate D1/D2 command
DTOs carry quote relations. Body edits route through D2 and preserve current quotes.
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

**Status:** `in_progress`  
**Priority:** P1  
**Dependencies:** FORUM-07/10

### Scope

Persist monotonic per-user topic read position/revision. Expose unread count,
last-read position and unread filters on bounded topic projections. Add mark
topic/category/all read commands.

Use `GREATEST(existing, incoming)` or equivalent compare-and-set semantics.
Anonymous page views do not create read rows. Cache and realtime updates may
accelerate the badge but database position/revision remains canonical.

### Delivered in `FORUM-16A`

- PostgreSQL and SQLite add `forum_topic_read_states` keyed by tenant, topic and
  user, with a tenant-composite topic foreign key, nonnegative high-water marks
  and database triggers that reject direct position or revision regression;
- `ForumTopicReadStateService` exposes authenticated owner get/mark operations,
  while anonymous reads return an implicit zero state and anonymous marks fail
  without creating persistence;
- mark validates the incoming position against the latest approved reply and the
  incoming revision against the latest topic revision before independently
  advancing both high-water marks through insert-or-no-op and conditional owner
  updates in one transaction;
- stale devices become no-ops instead of moving either cursor backwards, and a
  SQLite owner scenario covers future-bound rejection plus the direct database
  regression guard.

### Delivered in `FORUM-16B`

- `ForumReadModelService::list_topics_with_unread` exposes a separate
  authenticated owner projection without changing the existing public topic
  list contract;
- each bounded cursor page is enriched through one aggregate owner query with
  explicit read-state presence, last-read position/revision, approved-reply
  unread count, topic-revision unread state and a canonical `is_unread` result;
- a missing read-state row keeps a newly discovered topic unread even when it has
  no replies, while an explicit zero row records that an empty topic was opened;
- `unread_only` is applied before cursor pagination and includes unseen topics,
  approved replies after the high-water mark and newer immutable topic revisions;
- hidden, rejected and deleted replies are excluded by the approved-public
  predicate, and the SQLite scenario covers visibility changes plus two-page
  unread cursor behavior without N+1 reply reads.

### Delivered in `FORUM-16C`

- `ForumTopicReadStateService::mark_category_read` marks a root category and its
  current descendant subtree, while `mark_all_read` covers the tenant scope;
- each command processes at most 100 topics in one transaction and returns an
  exact `(snapshot_at, created_at, topic_id)` continuation cursor bound to the
  tenant, authenticated user and command scope;
- the first page fixes the topic-creation snapshot, so topics created later are
  excluded from that operation and picked up by a subsequent idempotent pass;
- category traversal reuses the existing 512-node owner bound and fails closed
  for an oversized, missing or cyclic tenant tree;
- one aggregate query per bounded page resolves the latest approved-reply
  position and latest immutable topic revision before monotonic owner upserts;
- equal non-stale high-water marks refresh the read snapshot so a late moderator
  approval below an already-read position can be acknowledged, while stale lower
  markers cannot regress state or refresh the snapshot;
- SQLite owner scenarios cover subtree pagination, snapshot exclusion, cursor
  scope isolation, tenant-wide replay, bounds, anonymous rejection and late
  approval acknowledgement.

### Delivered in `FORUM-16D`

- additive REST routes expose the authenticated unread cursor page, topic
  read-state get/mark, category-subtree mark and tenant-wide mark-all commands;
- additive GraphQL query and mutation fields expose the same owner contracts
  with tenant-scope rejection, permission checks and opaque cursor passthrough;
- both transports call `ForumReadModelService` and
  `ForumTopicReadStateService` directly, so unread calculation, monotonic
  updates, bounds and cursor validation remain owner policy;
- the legacy offset-based topic endpoints remain unchanged, and the unread/read
  state contracts do not open anonymous or realtime paths;
- OpenAPI and GraphQL SDL contract coverage locks the route and field names plus
  the shared request and response shapes.

### Delivered in `FORUM-16E`

- `ForumReadModelService::summarize_topic_ids` reuses the canonical unread
  aggregate for an authenticated caller-supplied set capped at 100 exact topic
  IDs, so storefront composition does not duplicate reply/revision policy;
- `TopicService::get_storefront_visible_with_locale_fallback` centralizes the
  single-topic open/channel visibility check used before a storefront mutation;
- authenticated GraphQL and native server-function adapters enrich only the
  topic IDs already selected by the storefront-visible owner list, while the
  public category/topic/reply feed remains the anonymous compatibility path;
- authentication or permission absence degrades explicitly to the public feed,
  but network, HTTP, persistence and domain failures remain visible instead of
  becoming synthetic anonymous state;
- the selected visible topic can be marked at the current approved-reply and
  immutable-revision high-water marks after visibility is rechecked, and content
  published after that owner snapshot remains unread;
- the module-owned storefront renders unread badges and a topic mark-read control
  without creating anonymous rows; GraphQL SDL and SQLite owner-composition tests
  lock the visible-topic-only contract and keep storefront bulk mutations closed.

### Delivered in `FORUM-16F`

- `topic_read_state_postgres` is source-ready in the isolated PostgreSQL Forum
  fixture without changing production runtime APIs, migrations or transports;
- two independent PostgreSQL connections mark the same tenant/user/topic with
  separate reply-position and topic-revision advances, requiring durable state
  to converge to the component-wise maximum and the database regression trigger
  to reject a later direct downgrade;
- a production-sized fixture creates 128 topics, 8,192 approved replies and 512
  topic revisions, then validates the canonical owner summary across a bounded
  100-topic page containing reply-unread, revision-unread, read and unseen rows;
- a natural `EXPLAIN (ANALYZE, BUFFERS, COSTS OFF, FORMAT JSON)` proof requires
  exactly 100 aggregate rows, no per-topic `SubPlan` and all four owner relations;
- a separate `enable_seqscan = off` plan proves index capability for read-state,
  reply-position and topic-revision access without claiming a latency threshold
  or requiring the production planner to choose one fixed plan;
- the machine contract, source verifier and proof record remain explicitly
  source-ready; no successful PostgreSQL execution or captured plan output is
  claimed until the maintainer runs `topic_read_state_postgres`.

### Compatibility and degraded mode

No migration or backfill is required: an absent row means position/revision zero
and preserves unseen-topic identity. Existing public, offset-based and storefront
topic reads remain source-compatible; storefront DTOs add only defaulted optional
unread fields. Authenticated requests enrich the existing visibility-filtered
page, while anonymous requests retain the public feed and never create read rows.
Authentication/permission absence is the only personalization fallback; real
transport or owner failures remain explicit. REST and GraphQL pass opaque cursors
and owner DTOs without duplicating unread policy. Category membership is
revalidated on every resumed owner bulk page; a subsequent idempotent pass
converges after a concurrent category move. Storefront category/all-read controls
remain closed because the current owner bulk scope is tenant/category based and
cannot yet narrow to the channel-visible topic set. FORUM-16F adds only
source-ready tests, contracts and documentation; it changes no persistence or
degraded-mode behavior. Cache and realtime accelerators remain optional and
never replace database owner state.

### Remaining scope

- add visibility/channel-scoped category and all-read storefront commands only
  after the shared ACL/visibility policy can produce an exact bounded owner scope;
- record successful maintainer PostgreSQL execution and capture the natural and
  index-capability `EXPLAIN JSON` output for the source-ready proof.

### Definition of done

Concurrent devices cannot move read state backwards, deleted/hidden replies do
not inflate unread counts, and category/all-read commands are resumable and
bounded.

### Verification

```bash
cargo test -p rustok-forum --test topic_read_state_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_unread_projection_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_bulk_read_state_sqlite -- --nocapture
cargo test -p rustok-forum --test read_state_transport_contract -- --nocapture
cargo test -p rustok-forum --test storefront_read_state_contract -- --nocapture
cargo test -p rustok-forum --test storefront_read_state_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_read_state_postgres -- --nocapture --test-threads=1
cargo test -p rustok-forum-storefront
node scripts/verify/verify-forum-read-state-runtime-proof.mjs
cargo xtask module validate forum
npm run verify:forum:admin-boundary
npm run verify:forum:storefront-boundary
```

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

**Status:** `in_progress`  
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

### Delivered in `FORUM-20A`

- `ForumTopicVisibilityScope` and `ForumTopicVisibilityService` centralize the
  current storefront topic rule: an open topic is visible when it has no channel
  restriction or when the exact normalized request channel is present;
- exact caller-selected topic IDs are bounded to 100 before deduplication,
  filtered in one tenant-scoped owner query and returned in first-occurrence
  order without discovering additional topics;
- missing, foreign-tenant, closed and nonmatching-channel targets all resolve as
  absent, so the exact evaluator does not expose an existence oracle;
- the single-topic storefront facade preserves its existing owner read/RBAC
  check before visibility evaluation, while the storefront list rechecks the
  exact selected page and fails closed if the compatibility SQL prefilter drifts
  from the owner scope;
- storefront unread composition continues through the guarded topic facade, so
  it cannot enrich or mark a topic outside the exact current scope;
- `forum-topic-visibility-scope.json`, `topic_visibility_sqlite` and
  `verify-forum-topic-visibility-scope.mjs` lock bounds, tenant isolation,
  order-preserving deduplication, public/exact-channel behavior and the residual
  full-ACL scope;
- this slice adds no migration, transport field, endpoint or UI and does not
  claim category inheritance, roles, groups, trust or explicit allow/deny.

### Delivered in `FORUM-20B`

- `ForumCategoryVisibility` defines the typed effective category audience floor
  currently supported by Forum: `public` and `authenticated`;
- `forum_category_policies.visibility_override` is nullable and stores only the
  narrowing `authenticated` override; PostgreSQL and SQLite reject an explicit
  `public` row, so a descendant cannot broaden an authenticated ancestor through
  direct persistence;
- `ForumCategoryVisibilityPolicyService` resolves the complete tenant category
  hierarchy within the existing 512-node and depth-16 bounds, reports the exact
  category that supplies an inherited authenticated floor, and fails closed for
  missing, foreign, cyclic or over-bound trees;
- the owner `set` command is guarded by `forum_categories:manage`; requesting
  `public` clears a local override only beneath an effective public parent, while
  requesting `authenticated` narrows the category and all descendants;
- the existing `allows_topics` policy shares the row without overwriting the new
  visibility field, and existing categories require no backfill because absence
  remains the public root default;
- `forum-category-visibility-policy.json`,
  `category_visibility_policy_sqlite` and
  `verify-forum-category-visibility-policy.mjs` lock inheritance, tenant
  isolation, monotonic narrowing and database enforcement;
- the static audit also repairs the already-merged FORUM-20A storefront drift
  error to construct the typed `rustok_core::Error` required by
  `ForumError::Internal`;
- this slice adds no transport, storefront, topic/reply read composition, role,
  trust, group or explicit allow/deny behavior.

### Delivered in `FORUM-20C`

- storefront topic exact and paginated reads compose the inherited category
  floor with the existing open/exact-channel policy through the Forum owner;
- public viewers see public categories only, while authenticated viewers may see
  both public and inherited authenticated categories without page/count drift;
- category exclusions are applied before storefront count and pagination and the
  exact selected page is rechecked by the bounded visibility evaluator;
- `topic_authenticated_visibility_sqlite` and the cumulative storefront contract
  preserve non-oracular missing/denied behavior and tenant-scoped channel checks.

### Delivered in `FORUM-20D`

- exact topic reads authorize, evaluate the inherited category floor and only
  then hydrate localized content; denied public targets resolve as
  `TopicNotFound`;
- ordinary topic pages exclude inherited authenticated categories before count
  and pagination, while authenticated owner reads preserve existing behavior;
- exact reply reads evaluate their parent topic before body/vote enrichment and
  denied public targets resolve as `ReplyNotFound`;
- reply pages reject a denied parent as `TopicNotFound` before reply count and
  pagination;
- `forum-owner-read-visibility.json`,
  `topic_reply_owner_visibility_sqlite` and
  `verify-forum-owner-read-visibility.mjs` lock the owner-read order and residual
  scope.

### Delivered in `FORUM-20E`

- exact category reads authorize and evaluate the inherited category floor
  before localization/subscription hydration; denied public targets resolve as
  `CategoryNotFound`;
- flat category pages exclude inherited authenticated categories before count
  and pagination;
- the bounded canonical category tree removes authenticated subtrees for public
  viewers and recomputes visible `total_nodes`, `max_depth`, `children_count` and
  `has_children` metadata;
- monotonic inheritance guarantees that pruning a hidden node cannot orphan a
  visible descendant, while authenticated tree reads retain the complete bounded
  hierarchy;
- `forum-category-owner-read-visibility.json`,
  `category_owner_visibility_sqlite` and
  `verify-forum-category-owner-read-visibility.mjs` lock exact, page and tree
  composition.

### Delivered in `FORUM-20F`

- `ForumAudienceConstraints` models additional role, minimum-trust,
  channel-member, group-member and explicit user allow/deny selectors after the
  inherited public/authenticated floor; positive selectors form a union and
  explicit deny always wins;
- raw input is capped before normalization at four roles, 32 channel candidates,
  32 group candidates and 100 allow plus 100 deny user IDs; trust is bounded to
  0..100 and channel slugs to 128 characters;
- `ForumAudienceFactsRequest` and `ForumAudienceFacts` form an exact tenant- and
  actor-scoped request/response pair, rejecting cross-identity or unrequested
  trust, channel or group facts;
- `ForumAudienceFactsPort` is a transport-neutral optional owner boundary with
  mandatory read-deadline semantics and no direct Forum dependency on Channel,
  Groups or their private tables;
- `ForumAudienceFactsResolver` binds the port context to the requested tenant and
  user, skips the optional provider when local deny/allow/role facts already
  decide the union, returns empty facts for public or non-user actors, and
  otherwise fails closed with typed capability errors;
- `ForumAudienceEvaluator` validates exact facts and returns an explainable
  decision reason for unrestricted, deny, allow, role, trust, channel, group,
  authentication-required or no-match outcomes;
- `forum-audience-capability-ports.json`, `audience_capability_contract` and
  `verify-forum-audience-capability-ports.mjs` lock bounds, precedence, exact
  subsets, identity/deadline semantics and residual composition work;
- this slice adds no storage, migration, owner-read composition, write policy,
  transport, provider adapter or cross-consumer behavior.

### Delivered in `FORUM-20G`

- five normalized Forum-owned tables persist one optional local category layer
  plus typed role, channel, group and explicit allow/deny relations without JSON;
- every policy and relation is tenant/category scoped through composite foreign
  keys, with typed checks for supported roles, trust `0..100`, canonical channel
  slugs, non-nil identities and allow/deny effects;
- direct channel/group/user inserts are bounded to the FORUM-20F limits and
  policy and relation updates are rejected; owner replacement uses delete plus
  insert;
- `ForumCategoryAudiencePolicyService` exposes managed policy inspection and
  atomically replaces a local layer under `forum_categories:manage` and the
  tenant category-tree lock;
- an empty constraint set deletes only the local layer and restores inheritance;
- effective policy is an ordered root-to-target conjunction of every non-empty
  local layer, preserving the local union/deny semantics while making child
  broadening impossible;
- storage reads remain bounded to the 512-node/depth-16 hierarchy and fail closed
  for cycles, foreign categories, orphan relations, empty stored layers or
  oversized direct writes;
- `forum-category-audience-policy.json`, `category_audience_policy_sqlite` and
  `verify-forum-category-audience-policy.mjs` lock normalized persistence,
  inheritance, atomic replacement, tenant isolation and direct database guards;
- this slice adds no topic narrowing, category/topic/reply read composition,
  provider adapter, write-audience transport or cross-consumer behavior.

### Delivered in `FORUM-20H` through `FORUM-20Q`

- `FORUM-20H` adds normalized topic-local audience narrowing and owner replacement
  commands that compose after the inherited category layers and cannot broaden them;
- `FORUM-20I` migrates Forum notification description, audience, and target-open checks
  to the canonical topic visibility owner instead of notification-local predicates;
- `FORUM-20J` composes exact richer topic visibility from the route/channel rule,
  inherited category layers, optional topic narrowing, and bounded owner facts;
- `FORUM-20K` applies that richer exact decision to Forum notification source reads and
  reauthorization while retaining public fail-closed degraded behavior;
- `FORUM-20L` and `FORUM-20M` define and publish the exact recipient-context capability,
  backed by authoritative Users/RBAC owners and preserving deadline and trace context;
- `FORUM-20N` authorizes each notification target for the exact current recipient;
- `FORUM-20O` re-resolves mentioned recipients before descriptor/audience publication;
- `FORUM-20P` filters topic-subscription audience pages by current recipient visibility
  while preserving sparse advancing cursors;
- `FORUM-20Q` publishes the Groups-owned exact-membership facts adapter. Trust and
  channel-membership adapters were delivered later through FORUM-26 and FORUM-20AT.

### Delivered in `FORUM-20R` through `FORUM-20AF`

- `FORUM-20R/20S` add non-oracular exact inbox open authorization with current recipient
  privacy before source target authorization;
- `FORUM-20T` through `FORUM-20AA` add bounded authorized listing, exact state commands,
  reconciliation, mark-unread, exact unread count, and resumable mark-all read/unread/archive;
- `FORUM-20AB` adds bounded explicit selected-ID state commands;
- `FORUM-20AC` adds authorized exact-group listing over opaque group keys;
- `FORUM-20AD` makes group keys durable through a bounded owner migration and backfill;
- `FORUM-20AE` adds bounded group summaries with exact stored counts and an authorized
  latest-item projection;
- `FORUM-20AF` adds bounded exact-group mark-read, mark-unread, and archive commands.

### Delivered in `FORUM-20AG` through `FORUM-20AL`

- `FORUM-20AG` exposes a transport-neutral authenticated-user storefront inbox port with
  context-derived tenant/recipient identity and read/write admission semantics;
- `FORUM-20AH` adds native Leptos server-function adapters without owner identity fields;
- `FORUM-20AI` replaces the storefront placeholder with bounded grouped inbox rendering,
  paging, stale-response guards, allowed-only navigation, and authoritative refreshes;
- `FORUM-20AJ` mounts the localized Notifications action and exact unread badge through a
  generic manifest-driven storefront header slot;
- `FORUM-20AK` adds GraphQL parity for unread count and grouped summary/item reads while
  reusing the owner storefront port and host-composed policy/source runtime;
- `FORUM-20AL` adds GraphQL parity for fresh open authorization. Missing, foreign,
  suppressed, and no-longer-openable rows remain indistinguishable `UNAVAILABLE`, and
  only an owner-authorized route can produce `ALLOWED`.

### Delivered in `FORUM-20AM`

- synchronize this canonical ledger, the Notifications owner-local plan, owner README,
  and live contract through `FORUM-20AL` without rewriting unrelated task content;
- record the historical `FORUM-20H` through `FORUM-20AL` execution chain as source-ready
  and unvalidated rather than promoting the overall task to `done`;
- add `forum-notification-plan-sync.json` and
  `verify-forum-notification-plan-sync.mjs` so future owner/storefront work cannot silently
  leave the four authoritative documents at different milestones;
- update the latest `FORUM-20AL` handoff contract to point at this synchronization task.

### Delivered in `FORUM-20AN`

- add the module-owned GraphQL mutation `notificationInboxApplyGroupState` for bounded
  exact-group `MARK_READ`, `MARK_UNREAD`, and `ARCHIVE` commands;
- derive tenant and recipient identity from the authenticated human-user context, require
  module admission before command validation, and carry a five-second deadline plus one
  bounded caller idempotency key through `NotificationInboxStorefrontPort`;
- return only `scanned`, `changed`, `next_cursor`, and `has_more`, preserving owner state,
  timestamp, terminal-archive, pagination, and non-oracular invariants;
- select native writes for SSR/hydrate and GraphQL writes for CSR/headless without fallback,
  while keeping the existing UI call site and authoritative post-command refresh behavior.

### Delivered in `FORUM-20AO`

- expose one current storefront transport-context resolver that reads the reactive auth
  session token and tenant signals without placing owner identity in request DTOs;
- key the grouped bootstrap resource by both the existing manual refresh nonce and the
  current transport context, so sign-in, sign-out, token refresh, and tenant changes refetch;
- pass one resolved context snapshot through exact unread-count and first bounded summary-page
  reads instead of re-resolving credentials between owner calls;
- clear prior mutation feedback when the auth scope changes while preserving explicit
  post-command refresh, compile-profile transport selection, and no-fallback behavior.

### Delivered in `FORUM-20AP`

- materialize `forum.topic.created` descriptors for active topics that are already non-public
  when the host publishes the exact notification recipient-context capability;
- keep public-only descriptor creation when that capability is absent, preserving the existing
  fail-closed optional-module profile;
- expose only topic/category identifiers in the descriptor and defer all recipient authority to
  the bounded subscription audience recheck;
- preserve author exclusion, raw subscription cursor progress, current category/topic audience
  evaluation, non-oracular inactive targets, and later target-open authorization.

### Delivered in `FORUM-20AQ`

- add five normalized Forum-owned tables for a category-local topic-create audience layer plus
  typed role, channel, group, and explicit allow/deny relations;
- keep topic-create policy separate from category/topic visibility while inheriting every
  non-empty root-to-category layer as a conjunction;
- expose managed inspection and atomic replacement under `forum_categories:manage`, with an
  empty constraint set clearing only the local layer and restoring inheritance;
- enforce tenant/category composite ownership, raw relation bounds, immutable stored rows, and
  PostgreSQL/SQLite parity without changing `TopicService::create` or any transport.

### Delivered in `FORUM-20AR`

- enforce `forum_topics:create` before loading the bounded inherited category topic-create
  policy and require every root-to-category layer to allow the caller;
- keep unrestricted categories and locally decidable role/explicit-user layers independent of
  optional owner facts while preserving explicit-deny precedence;
- require exact tenant/user `PortContext` plus the optional facts capability only when trust,
  Channel, or Groups selectors remain unresolved, and fail closed when either is absent;
- gate every public `TopicService` create path before topic, relation, counter, user-stat, or
  event writes and publish context-aware owner seams without changing GraphQL or REST DTOs.

### Delivered in `FORUM-20AS`

- compose both legacy and inline-quote GraphQL topic-create mutations through one manifest-backed
  runtime wrapper and the existing context-aware owner methods;
- compose both REST topic-create handlers through `HostRuntimeContext`, using only authenticated
  tenant/user identity plus the middleware-resolved locale and route channel;
- attach read deadline, permission claims, and a bounded correlation id before any optional owner
  facts call, rejecting mismatched request tenant or actor before provider access;
- consume the existing feature-guarded Groups facts publication for both transports while keeping
  provider absence fail closed and adding no topic-create DTO, migration, or Forum-to-Groups dependency.

### Delivered in `FORUM-20AT`

- publish one host-owned composite `SharedForumAudienceFactsPort` for every Forum build while
  preserving the historical `FORUM-20Q` Groups adapter as an optional owner-backed fallback;
- accept a Channel match only from the exact normalized requested `PortContext.channel`, then
  confirm that slug through `ChannelReadPort` as active and tenant-matching;
- never list or probe unrequested channels, and let an exact current-Channel match decide the
  positive-selector union before optional Groups facts are consulted;
- keep Channel-only topic-create policy operational when Groups is not compiled, while requested
  Groups facts remain typed retryable unavailable when no delivered selector decides;
- add inline host tests, a machine-readable contract and a static verifier without migrations,
  public DTO changes, or a new Forum-to-Channel dependency.

### Delivered in `FORUM-20AU`

- add five normalized Forum-owned tables for a category-local reply-create audience layer plus
  typed role, channel, group, and explicit allow/deny relations;
- keep reply-create policy separate from category/topic visibility and topic-create eligibility
  while inheriting every non-empty root-to-category layer as a conjunction;
- expose managed inspection and atomic replacement under `forum_categories:manage`, with an
  empty constraint set clearing only the local layer and restoring inheritance;
- enforce tenant/category composite ownership, typed trust/role/effect values, raw relation
  bounds, immutable stored rows, and PostgreSQL/SQLite parity;
- leave `ReplyService::create`, REST, GraphQL and transport DTOs unchanged; command-time
  authorization remains a separate follow-up slice.

### Delivered in `FORUM-20AV`

- require `forum_replies:create`, resolve the exact tenant-scoped topic category, and evaluate
  every inherited category reply-create layer before reply-body preparation or owner writes;
- preserve explicit-deny precedence and locally decidable role/explicit-user decisions without
  optional facts calls;
- require exact tenant/user context and the host audience-facts capability only for unresolved
  trust, Channel, or Groups selectors;
- route both public reply create facades through the owner gate before reply, relation, counter,
  user-stat, journal, or event writes without changing DTOs.

### Delivered in `FORUM-20AW`

- compose both legacy and inline-quote GraphQL reply-create mutations through exact authenticated
  read-only `PortContext` values;
- compose both REST reply-create handlers from tenant, auth and middleware request context;
- forward claims, locale, resolved route channel, five-second deadline and bounded correlation
  identity without accepting owner identity from request DTOs;
- reuse the same optional host-published `SharedForumAudienceFactsPort` already used by topic
  creation, preserving fail-closed unresolved external selectors.

### Delivered in `FORUM-20AX`

- persist an optional normalized topic-local reply-create audience layer independently from topic
  visibility and inherited category reply-create storage;
- expose managed get/replace commands under `forum_topics:manage`, with empty constraints clearing
  only the topic layer;
- evaluate every category layer followed by the optional topic layer so topics may narrow but
  never broaden inherited eligibility;
- preserve existing owner and transport paths while PostgreSQL/SQLite enforce tenant ownership,
  typed values, immutable rows and bounded direct relations.

### Delivered in `FORUM-20AY`

- persist normalized inherited category moderation audience layers independently from visibility,
  topic-create and reply-create policy;
- expose managed get/replace commands and evaluate every inherited layer before topic/reply
  moderation transactions;
- preserve the exact topic-author solution path while requiring non-author moderators to hold
  scope and satisfy the moderation audience;
- publish context-aware moderation owner methods and PostgreSQL/SQLite ownership, immutability,
  bounds and advisory-lock guards.

### Delivered in `FORUM-20AZ`

- compose existing GraphQL and REST mark/clear-solution routes through one exact authenticated
  moderation context and the same host-published audience-facts port;
- derive tenant and actor only from authenticated transport context and forward claims, locale,
  route channel, five-second deadline and bounded correlation identity;
- preserve owner-side topic-author versus moderator authorization and keep the gate before
  solution, counter, user-stat, journal and outbox writes;
- add no new moderation endpoint; owner methods without an existing public route remain
  transport-neutral and future routes must reuse the context-aware boundary.

### Delivered in `FORUM-20BA`

- synchronize this canonical ledger through `FORUM-20AZ` and remove reply-create enforcement,
  topic-local narrowing, category moderation audience, and existing solution-route composition
  from remaining scope;
- correct historical owner-note text that still described delivered Forum trust facts as
  unavailable or blocked while preserving that AV-AZ did not create trust state themselves;
- add `forum-audience-plan-sync.json`, `forum-20ba-audience-plan-sync.md`, and
  `verify-forum-audience-plan-sync.mjs` to lock the ledger, five owner notes, five contracts,
  CRATE_API boundary and latest AZ handoff together;
- change no runtime behavior, migration, dependency or public contract.

### Compatibility and degraded mode

The nullable public/authenticated category floor and normalized category/topic layers require no
destructive backfill. Existing content and categories/topics without richer create or moderation
layers retain their previous behavior. Locally decidable role and explicit-user decisions do not
require optional facts providers. Exact trust, Channel, or Groups selectors use the shared
host-published facts capability and fail closed only when a still-required owner fact is absent.
Forum trust is authoritative and host-composed through `ForumUserTrustAudienceFactsPort`;
`forum_user_stats` activity counters are never treated as trust.

FORUM-20AV-AZ add no create/moderation DTO identity fields. GraphQL and REST derive tenant and
actor from authenticated context, and topic/reply/moderation owners validate exact context before
target lookup or mutation. Categories without reply-create layers and topics without local
narrowing preserve historical reply behavior. Existing solution routes are context-composed;
no new approve/reject/hide or pin/lock/status transport is claimed. Context-free owner methods
remain available for direct locally decidable consumers, while future public moderation routes
must use context-aware methods. FORUM-20BA is documentation-only.

Forum producer commands remain independent from Notifications availability. The Notifications
module stays default-off, derives storefront tenant and recipient identity from authenticated
context, and exposes explicit unavailable/degraded states rather than shadow inbox data. Native
and GraphQL storefront reads and writes select exactly one compile-profile transport path with no
cross-path fallback. Both group-state paths delegate to the same owner port; the GraphQL path
cannot select another tenant or recipient and must supply write deadline and idempotency semantics.

### Remaining scope

- migrate remaining Forum reads plus search/index, SEO, and deep-link authorization to the same
  exact richer audience decision;
- add visibility-scoped category/all-read commands over an exact bounded policy scope;
- require any future public moderation transports to reuse the delivered context-aware owner
  methods instead of implementing transport-local policy;
- add tenant-wide scheduled reconciliation, payload redaction, channel enqueue/transports, and
  delivery-time target authorization;
- add PostgreSQL concurrency, lease/contention, inheritance, and cross-consumer runtime evidence
  before promoting `FORUM-20` to `done`.

### Definition of done

Cross-tenant, blocked, private and channel-restricted content is consistently
absent from reads, search, SEO and notifications, including replay and cache
profiles.

### Verification

```bash
cargo test -p rustok-forum --test topic_visibility_sqlite -- --nocapture
cargo test -p rustok-forum --test category_visibility_policy_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_authenticated_visibility_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_reply_owner_visibility_sqlite -- --nocapture
cargo test -p rustok-forum --test category_owner_visibility_sqlite -- --nocapture
cargo test -p rustok-forum --test audience_capability_contract -- --nocapture
cargo test -p rustok-forum --test category_audience_policy_sqlite -- --nocapture
cargo test -p rustok-forum --test category_reply_create_audience_policy_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_reply_create_audience_policy_sqlite -- --nocapture
cargo test -p rustok-forum --test moderation_audience_policy_sqlite -- --nocapture
cargo test -p rustok-forum reply_create_transport -- --nocapture
cargo test -p rustok-forum graphql::runtime_data -- --nocapture
cargo test -p rustok-forum moderation_transport -- --nocapture
node scripts/verify/verify-forum-topic-visibility-scope.mjs
node scripts/verify/verify-forum-category-visibility-policy.mjs
node scripts/verify/verify-forum-owner-read-visibility.mjs
node scripts/verify/verify-forum-category-owner-read-visibility.mjs
node scripts/verify/verify-forum-audience-capability-ports.mjs
node scripts/verify/verify-forum-category-audience-policy.mjs
node scripts/verify/verify-forum-topic-audience-policy.mjs
node scripts/verify/verify-forum-topic-audience-visibility.mjs
node scripts/verify/verify-forum-notification-visibility-composition.mjs
node scripts/verify/verify-forum-notification-recipient-context.mjs
node scripts/verify/verify-forum-notification-recipient-host-runtime.mjs
node scripts/verify/verify-forum-notification-recipient-target-open.mjs
node scripts/verify/verify-forum-notification-recipient-mention-audience.mjs
node scripts/verify/verify-forum-notification-topic-subscription-audience.mjs
node scripts/verify/verify-forum-audience-group-facts-host-runtime.mjs
cargo test -p rustok-server --features mod-forum forum_audience_facts -- --nocapture
cargo test -p rustok-server --features mod-forum,mod-groups forum_audience_facts -- --nocapture
node scripts/verify/verify-forum-audience-channel-facts-host-runtime.mjs
node scripts/verify/verify-forum-notification-inbox-storefront-port.mjs
node scripts/verify/verify-forum-notification-inbox-native-storefront-adapter.mjs
node scripts/verify/verify-forum-notification-inbox-grouped-storefront-ui.mjs
node scripts/verify/verify-forum-notification-navigation-badge.mjs
node scripts/verify/verify-forum-notification-inbox-grouped-graphql.mjs
node scripts/verify/verify-forum-notification-inbox-open-graphql.mjs
node scripts/verify/verify-forum-notification-inbox-group-state-graphql.mjs
node scripts/verify/verify-forum-notification-inbox-auth-reactive-bootstrap.mjs
node scripts/verify/verify-forum-notification-topic-descriptor-materialization.mjs
node scripts/verify/verify-forum-category-topic-create-audience-policy.mjs
cargo test -p rustok-forum --test topic_create_audience_enforcement_sqlite -- --nocapture
node scripts/verify/verify-forum-topic-create-audience-enforcement.mjs
cargo test -p rustok-forum topic_create_transport -- --nocapture
node scripts/verify/verify-forum-topic-create-audience-transport-composition.mjs
node scripts/verify/verify-forum-category-reply-create-audience-policy.mjs
node scripts/verify/verify-forum-reply-create-audience-enforcement.mjs
node scripts/verify/verify-forum-reply-create-audience-transport-composition.mjs
node scripts/verify/verify-forum-topic-reply-create-audience-policy.mjs
node scripts/verify/verify-forum-moderation-audience-policy.mjs
node scripts/verify/verify-forum-moderation-audience-transport-composition.mjs
node scripts/verify/verify-forum-notification-plan-sync.mjs
node scripts/verify/verify-forum-audience-plan-sync.mjs
cargo xtask module validate forum
npm run verify:forum:storefront-boundary
```

Tests, Cargo, verifiers and CI were not run while publishing the source-ready
FORUM-20C-AZ implementation slices or the FORUM-20BA documentation synchronization.
`FORUM-20` remains `in_progress` until the remaining owner and cross-consumer paths are
delivered with maintainer runtime evidence.

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

### Delivered through `FORUM-21X`

- FORUM-21A adds the bounded idempotent topic-move owner with checked category
  counters, immutable operation receipt and unchanged topic identity;
- FORUM-21B through FORUM-21G add the bounded merge owner plus independent
  subscription, read-state, tag, topic-vote and topic-local audience
  reconciliation owners;
- FORUM-21H through FORUM-21L add accepted-solution serialization, exact
  solution-statistics transitions, canonical merged-topic identity, the
  authorization-safe permanent GET redirect, manager-only GraphQL composition
  and explicit manager-selected competing-solution resolution with an
  append-only audit ledger;
- FORUM-21M adds checked cross-category merge ownership: source and target keep
  their original category identities, category topic counts remain unchanged,
  and only the exact moved published-reply contribution transfers between
  category aggregates inside the owner transaction;
- `m20260803_000019_allow_cross_category_topic_merge_redirect_edges` replaces
  the historical same-category receipt trigger predicate while preserving the
  archived/locked/zero-reply source tombstone, active target category, unique
  source edge and unchanged receipt schema;
- FORUM-21N adds a module-owned admin topic merge workflow in both Leptos and
  Next-admin over the existing manager GraphQL mutations. Both surfaces retain
  one UUID operation identity across an exact retry, rotate it when the command
  shape changes, require an explicit source/target solution winner only when
  both topics are solved, and display the immutable owner receipt;
- FORUM-21O selects direct authenticated native owner composition for Leptos
  SSR/hydrate while retaining the existing GraphQL mutations for CSR/headless
  parity, with no cross-path fallback and no owner identity in request DTOs;
- FORUM-21P adds the idempotent selected-reply split owner with an immutable
  receipt and `forum.topic.split` event, parent-closed movement, preserved reply
  identity and relations, exact topic-local access cloning and checked counter
  reconciliation;
- FORUM-21Q adds the idempotent reply-branch fork owner with deterministic copied
  reply identities, bounded revision/mention/quote provenance, exact access and
  tag cloning, source immutability and an explicit votes/subscriptions/read-state/
  solution non-copy policy;
- FORUM-21R adds the routed-tenant, manager-only GraphQL split transport. The
  additive `splitForumTopicReplies` field delegates the complete command to
  `ForumTopicSplitService::split_selected_replies` and returns the immutable
  owner receipt without reading split audit tables or duplicating owner policy;
- FORUM-21S adds the idempotent bounded reply-range move owner with deterministic
  append positions, explicit asymmetric parent policy, unchanged reply-owned
  references, exact topic-local access equality and checked solution/counter
  reconciliation;
- FORUM-21T adds the routed-tenant, manager-only GraphQL reply-range transport.
  The additive `moveForumTopicReplyRange` field delegates the complete command
  to `ForumReplyRangeMoveService::move_reply_range` and returns the immutable
  owner receipt without reading move audit tables or duplicating owner policy;
- FORUM-21U adds the routed-tenant, manager-only GraphQL fork transport. The
  additive `forkForumTopicReplyBranch` field delegates the complete command to
  `ForumTopicForkService::fork_reply_branch` and returns the immutable owner
  receipt without reading fork audit/mapping tables or duplicating copy policy;
- FORUM-21V composes the selected-reply split command in the module-owned Leptos
  and Next-admin surfaces. Both retain operation and target-topic UUIDs for an
  exact retry, rotate both when the command shape changes, preflight the visible
  parent-closed selection and display the immutable owner receipt without adding
  movement, access, solution or counter policy;
- FORUM-21W composes the reply-branch fork command in the module-owned Leptos
  and Next-admin surfaces. Both retain operation and target-topic UUIDs for an
  exact retry, rotate both when the source, root or target shape changes, require
  the root to be present in the bounded visible reply page and display the
  immutable owner receipt without discovering descendants or adding copy policy;
- FORUM-21X composes the bounded reply-range move command in the module-owned
  Leptos and Next-admin surfaces. Both retain one operation UUID for an exact
  retry, rotate it when source, target, endpoint or reason changes, accept exact
  canonical owner positions instead of inferring positions from visible row
  order, and display the immutable owner receipt without adding movement policy;
- every ordinary, resolved, same-category and cross-category merge retains the
  exact `forum.topic.merged` schema-version-1 payload so existing post-merge
  reconciliation owners remain compatible.

### Compatibility and degraded mode

FORUM-21M adds one append-only PostgreSQL/SQLite migration that changes only the
canonical receipt trigger predicate. FORUM-21N adds no migration and changes no
owner method, GraphQL schema, REST route, receipt, event, canonical-resolution
lane or reconciliation owner. Existing same-category receipts and merges keep
their previous behavior. The source category remains discoverable from the
archived source topic and is not copied into the semantic event or receipt.

FORUM-21O changes only the Leptos admin transport selection and host composition:
SSR/hydrate use direct authenticated native owner state, CSR/headless retain
GraphQL, and no fallback is introduced. FORUM-21P and FORUM-21Q add append-only
PostgreSQL/SQLite owner receipt and mapping migrations without changing existing
move or merge receipts and events. FORUM-21S adds append-only PostgreSQL/SQLite
owner receipt and mapping state without changing earlier commands. FORUM-21R,
FORUM-21T and FORUM-21U each add one additive GraphQL field and no REST route,
owner method, receipt shape or semantic-event change. FORUM-21V adds Leptos and
Next-admin split composition only. FORUM-21W adds Leptos and Next-admin fork
composition only. FORUM-21X adds Leptos and Next-admin reply-range composition
only: none of the three slices adds a migration, native split/fork/reply-range
transport, owner, GraphQL, receipt or semantic-event change. Direct owner callers remain
source-compatible.

Forum move, merge, split and fork commands remain independent from Notifications,
Search, Page Builder and other optional integrations. Owner state, the Forum
semantic event, receipt, mapping/audit rows and projection invalidations commit
atomically; optional consumers reconcile after commit. A disabled optional
capability cannot turn a valid Forum owner command into a synchronous outage.

### Remaining scope

`FORUM-21` remains `planned` until all of the following are delivered and
maintainer-executed:

- retained SQLite and PostgreSQL execution evidence for the complete move,
  merge, split and fork owner/migration chain, including cross-category
  concurrency, replay and rollback;
- mounted-browser and runtime transport evidence for the native/GraphQL merge
  paths plus the split, fork and reply-range GraphQL fields;
- final canonical localized URL aliases and route tombstones under FORUM-24,
  rather than a parallel FORUM-21 slug authority.

### Definition of done

Each operation has an operation ID, reason, transactional state change and
semantic event; retry produces the same result; partial moves are impossible;
source tombstones/redirects are safe.

### Verification

```bash
node scripts/verify/verify-forum-topic-move-owner.mjs
node scripts/verify/verify-forum-topic-merge-owner.mjs
node scripts/verify/verify-forum-topic-merge-cross-category.mjs
node scripts/verify/verify-forum-topic-merge-solution-policy.mjs
node scripts/verify/verify-forum-topic-merge-solution-resolution.mjs
node scripts/verify/verify-forum-topic-canonical-resolution.mjs
node scripts/verify/verify-forum-topic-http-redirect.mjs
node scripts/verify/verify-forum-topic-merge-graphql-transport.mjs
node scripts/verify/verify-forum-topic-merge-admin-ui.mjs
node scripts/verify/verify-forum-topic-merge-native-admin.mjs
node scripts/verify/verify-forum-topic-split-owner.mjs
node scripts/verify/verify-forum-topic-fork-owner.mjs
node scripts/verify/verify-forum-reply-range-move-owner.mjs
node scripts/verify/verify-forum-topic-split-graphql-transport.mjs
node scripts/verify/verify-forum-topic-split-admin-ui.mjs
node scripts/verify/verify-forum-reply-range-move-graphql-transport.mjs
node scripts/verify/verify-forum-topic-fork-graphql-transport.mjs
node scripts/verify/verify-forum-topic-fork-admin-ui.mjs
node scripts/verify/verify-forum-reply-range-move-admin-ui.mjs
npm run verify:forum:admin-boundary
npm run verify:blog:forum-ui-ownership
cargo test -p rustok-forum --test topic_move_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_merge_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_merge_cross_category_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_merge_solution_resolution_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_canonical_resolution_sqlite -- --nocapture
cargo test -p rustok-forum controllers::topic_redirect::tests -- --nocapture
cargo test -p rustok-forum --test topic_merge_graphql_contract -- --nocapture
cargo test -p rustok-forum graphql::topic_split_mutation::tests -- --nocapture
cargo test -p rustok-forum --test topic_split_graphql_contract -- --nocapture
cargo test -p rustok-forum --test topic_split_sqlite -- --nocapture
cargo test -p rustok-forum graphql::topic_reply_range_move_mutation::tests -- --nocapture
cargo test -p rustok-forum --test reply_range_move_graphql_contract -- --nocapture
cargo test -p rustok-forum --test reply_range_move_sqlite -- --nocapture
cargo test -p rustok-forum graphql::topic_fork_mutation::tests -- --nocapture
cargo test -p rustok-forum --test topic_fork_graphql_contract -- --nocapture
cargo test -p rustok-forum --test topic_fork_sqlite -- --nocapture
cargo test -p rustok-forum-admin topic_merge_model -- --nocapture
cargo test -p rustok-forum-admin topic_split_model -- --nocapture
cargo test -p rustok-forum-admin topic_fork_model -- --nocapture
cargo test -p rustok-forum-admin topic_reply_range_model -- --nocapture
cargo check -p rustok-forum-admin --all-targets
npm --prefix apps/next-admin run typecheck
```

The FORUM-21A through FORUM-21X source and contract records do not claim
successful verifier, SQLite, PostgreSQL, Cargo, formatting, npm, browser,
workflow or CI execution. The canonical task remains `planned` until the
remaining workflows and maintainer evidence are complete.

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

**Status:** `in_progress`  
**Priority:** P0  
**Dependencies:** FORUM-09/20, durable index consumer

### Scope

Publish versioned category/topic/reply/member index projections. Index only
published/approved content with safe author summary and visibility metadata.
Consumers use durable inbox and owner revision ordering. Search filters include
category subtree, author, tag, locale, date, solved, kind, channel/group and
attachment presence.

### Delivered in `FORUM-23A` through `FORUM-23A11`

- public topic and approved-reply Search projections resolve author presentation
  through the Profiles owner and expose only the approved public summary;
- private or unavailable profile presentation removes the embedded public author
  value instead of leaking a raw Forum author identifier;
- durable profile visibility and account-lifecycle invalidation is ordered
  independently from unrelated Forum wall-clock progress and retries on owner
  failure instead of silently serializing stale public author data;
- the A-family contracts and verifiers preserve the owner boundary while leaving
  monotonic Forum projection revisioning and complete Search reconciliation open.

### Delivered in `FORUM-23B1`

- the existing bounded `SearchQuery.category_ids` contract matches exact Forum
  category documents plus topic and approved-reply `facets.category_id` values;
- product category filtering remains active, Forum values remain bound query
  parameters, and Search does not copy Forum hierarchy or visibility policy;
- the slice adds no public Search field, migration, dependency or projection
  shape change and remains exact-category behavior until an owner expands roots.

### Delivered in `FORUM-23B2A`

- `ForumSearchCategoryScopeService` accepts at most ten raw roots before
  deduplication and resolves the canonical tenant tree within 512-node and
  depth-16 bounds;
- the owner requires `forum_categories:list`, reuses the inherited
  public/authenticated visibility floor, excludes archived branches and returns
  missing, foreign, archived or viewer-hidden selected roots as
  `CategoryNotFound`;
- requested-root first occurrence plus owner `(position, id)` child order
  produces deterministic preorder identifiers while overlapping roots are
  emitted once;
- Forum returns already-authorized IDs for the existing internal Search
  `category_ids` field without importing Search or moving Forum tree policy into
  the retrieval owner;
- `forum-search-category-subtree-scope.json`,
  `forum-23b2-category-subtree-scope.md` and
  `verify-forum-search-category-subtree-scope.mjs` lock the source boundary and
  record maintainer execution as pending.

### Delivered in `FORUM-23B2B`

- `ForumSearchCategoryAudienceScopeService` exposes separate public and exact
  authenticated subtree entrypoints while preserving the B2A result shape and
  raw-root/tree/output bounds;
- the authenticated entrypoint binds the exact tenant/user `PortContext` and
  reuses `ForumCategoryAudienceVisibilityService` for inherited role, trust,
  Channel, Groups, and explicit user allow/deny category layers;
- public evaluation never requires optional owner facts, locally decidable
  authenticated rules avoid unnecessary provider calls, and unresolved trust or
  membership selectors fail closed when the shared owner facts capability is
  unavailable;
- archived categories are excluded, a denied ancestor prunes its descendants,
  and missing, foreign, archived, or denied selected roots remain
  non-oracular `CategoryNotFound` results;
- deterministic requested-root and canonical child order is retained while
  overlapping visible roots are emitted once;
- `forum-search-category-audience-scope.json`,
  `forum-23b2b-category-audience-scope.md`, and
  `verify-forum-search-category-audience-scope.mjs` lock the richer category
  audience boundary without executing Search or changing a public transport.

### Delivered in `FORUM-23B2C`

- `StorefrontSearchCategoryScopePort` is a neutral Search-owned optional contract;
  the server is the only adapter that imports both owners and delegates exact
  public or authenticated requests to `ForumSearchCategoryAudienceScopeService`;
- GraphQL `forumStorefrontSearch` and native
  `search/forum-storefront-search` expose one explicit Forum-only execution path,
  while the module-owned storefront selector uses it only for exactly
  `source_modules: ["forum"]` with at least one category root;
- mixed, unspecified, Product, Blog, Content, and Forum-without-category requests
  remain on the existing `storefrontSearch` and exact-category native path, so
  product category semantics are unchanged;
- GraphQL and native transports share one Search execution owner that reuses
  dictionary transforms, tenant-effective presets, ranking, PostgreSQL Search,
  query rules, canonical URLs, analytics, published-only filtering, and the
  existing Search result DTOs;
- tenant identity comes from trusted request context, an explicit tenant override
  is rejected, authenticated Forum scope is selected only with
  `forum_categories:list`, and missing permission uses the public owner decision;
- disabled Forum state, absent owner composition, unresolved required facts,
  denied roots, and owner failures fail closed rather than silently falling back
  to an unsafe exact-category Search;
- `forum-search-storefront-scope.json`,
  `forum-23b2c-storefront-search-scope.md`, and
  `verify-forum-search-storefront-scope.mjs` lock the cross-owner and transport
  boundary while recording execution as pending.

### Delivered in `FORUM-23B2D`

- `StorefrontSearchResultEligibilityPort` is a second neutral Search-owned
  optional contract; the server remains the only adapter importing both Search
  and Forum and publishes the exact Forum owner implementation;
- Search scans the existing Forum-only query from offset zero in bounded 50-row
  pages, rejects a raw result set above 100 rows, and fails closed when the raw
  total changes, a continuation page does not advance or a raw row repeats;
- `ForumSearchResultEligibilityService` batch-loads current approved reply-to-topic
  ownership and reuses `ForumTopicAudienceVisibilityService` for every distinct
  topic, including open state, route channel, inherited category layers,
  topic-local narrowing, roles, Forum trust, Channel, Groups and explicit
  allow/deny;
- missing, stale, closed, denied or non-approved topic/reply candidates are omitted
  without an existence oracle, while missing owner composition, disabled Forum,
  invalid owner subsets and unresolved required facts fail closed;
- Search preserves the raw ranking order of authorized rows and computes visible
  totals, facets, offset and limit before query rules or transport mapping;
- GraphQL and native transports share the same execution owner, while mixed,
  unspecified, Product, Blog, Content and Forum-without-category Search paths
  remain unchanged;
- `forum-search-result-eligibility.json`,
  `forum-23b2d-search-result-eligibility.md`, and
  `verify-forum-search-result-eligibility.mjs` lock the owner, bound, transport and
  post-authorization pagination contract while recording execution as pending.

### Delivered in `FORUM-23B2E1`

- `TrustedStorefrontChannel` is a Search-owned neutral authority derived from the
  middleware `RequestContext`; it requires the exact Search tenant and a complete
  channel ID/slug pair or an explicitly unscoped request;
- the public `channel_id` input remains for compatibility but is assertion-only:
  absent input uses trusted context, an exact match is accepted, and malformed or
  mismatched input fails closed instead of selecting another channel;
- ordinary and Forum-only GraphQL/native Search use the same authority, while the
  shared Forum execution owner revalidates the context for future transports;
- admin preview and admin global Search keep their existing operator-selected
  channel behavior;
- `forum-search-trusted-channel-authority.json`,
  `forum-23b2e1-trusted-channel-authority.md`, and
  `verify-forum-search-trusted-channel-authority.mjs` lock the source boundary and
  record maintainer execution as pending.

### Delivered in `FORUM-23B2E2`

- Search projects Product-owned
  `metadata.channel_visibility.allowed_channel_slugs` into the Search-owned
  Product payload without importing Product or Channel policy services;
- absence of the owner visibility object preserves the canonical global Product
  meaning as an empty array, canonical arrays are retained, and malformed explicit
  values remain non-arrays so storefront evaluation fails closed;
- `PgSearchEngine::search_storefront` applies one Product predicate before FTS or
  typo ranking, so result rows, totals, facets and attribute-filtered queries use
  the same trusted channel decision;
- storefront query-rule pins recheck Product payload eligibility and document
  suggestions reuse the SQL predicate; storefront query-text suggestions are
  disabled because aggregate logs cannot be channel-authorized, while
  admin/global query suggestions remain unchanged;
- ordinary and Forum-only GraphQL/native Search preserve the exact
  `TrustedStorefrontChannel` through every bounded page and post-query rule step;
- a Search-owned bounded reconciler discovers tenant Product documents whose
  projection path is absent, and a host background worker runs product-scope rebuilds
  at server startup until no legacy batch remains;
- legacy missing projections are hidden before repair, while malformed explicit
  owner values remain hidden and are not rebuilt forever without an owner fix;
- admin preview/global Search retain their existing non-storefront execution path;
- `forum-search-product-channel-visibility.json`,
  `forum-23b2e2-product-channel-visibility.md`, and
  `verify-forum-search-product-channel-visibility.mjs` lock the projection,
  reconciliation and surface contract while recording execution as pending.

### Delivered in `FORUM-23B2F1`

- the explicit Forum-only GraphQL path accepts an optional author-ID argument and
  an additive native endpoint carries the equivalent list, capped at ten raw UUID
  values without changing neutral `SearchPreviewInput`, `SearchQuery`, or the
  shared storefront filter DTO;
- the existing native `search/forum-storefront-search` endpoint retains its prior
  signature, while author-scoped calls use
  `search/forum-storefront-search-by-authors` and share the same execution owner;
- Search matches only the existing Forum-owned public
  `payload.author.user_id` projection for topics and approved replies; categories,
  non-Forum rows, and missing, denied, redacted, or malformed author summaries do
  not match an active author scope;
- the stable raw Forum candidate snapshot and 100-row bound are resolved before
  author narrowing, so a broad query cannot bypass the existing owner-call cap;
- exact author narrowing runs before topic/reply owner eligibility, visible totals,
  facets, offset, and limit while preserving the original ranking order;
- query-rule pins are disabled while an author filter is active because the pin
  loader has no author argument and must not reintroduce an out-of-scope document;
- ordinary storefront, mixed, Product, admin preview, and admin global Search
  remain unchanged, and existing explicit Forum calls retain their old endpoint;
- `forum-search-author-filter.json`,
  `forum-23b2f1-search-author-filter.md`, and
  `verify-forum-search-author-filter.mjs` lock the public-author source,
  transport, ordering, bounds, and compatibility contract while recording
  execution as pending.

### Delivered in `FORUM-23B2F2`

- the Forum-only GraphQL owner accepts optional bounded `tags` and nullable
  `solved` arguments; an additive GraphQL operation and native endpoint carry
  author/tag/solved filters without changing neutral Search inputs or shared DTOs;
- tag values are trimmed, case-sensitive exact values capped at ten entries and
  64 characters each; every requested tag must occur in the projected list;
- topics match `payload.tags`, approved replies match Forum-projected parent
  `payload.topic_tags`, and legacy replies missing that projection fail closed
  under an active tag scope until reindexed;
- solved topics require either a valid UUID string or explicit null in
  `solution_reply_id`; replies require the exact current projected `is_solution`
  boolean, and malformed tag or solved projections fail closed;
- active author, tag and solved predicates intersect after the stable bounded raw
  snapshot and before exact Forum owner eligibility, visible totals, facets,
  offset and limit while preserving ranking order;
- categories and non-Forum rows do not match any active document filter, and
  query-rule pins remain disabled while any document filter is active;
- existing `ForumStorefrontSearch`, `ForumStorefrontSearchByAuthors`,
  `search/forum-storefront-search`, and
  `search/forum-storefront-search-by-authors` wire contracts remain unchanged;
- `forum-search-tag-solved-filter.json`,
  `forum-23b2f2-search-tag-solved-filter.md`, and
  `verify-forum-search-tag-solved-filter.mjs` lock the owner projection, exact
  semantics, ordering, compatibility and degraded-mode contract while recording
  execution and reindex evidence as pending.

### Delivered in `FORUM-23B2F3`

- every explicit Forum-only GraphQL/native wire operation delegates to one shared
  execution owner that normalizes the requested locale or tenant fallback and uses
  it for PostgreSQL FTS/typo scope, category scope, owner eligibility and a
  post-scan exact result assertion; missing or mismatched locale fails closed;
- locale-only execution retains category, topic and reply results and query-rule
  pins; no multi-locale candidate union is introduced;
- topics and approved replies project Forum-owned creation time as UTC RFC3339
  `payload.published_at`; legacy rows without it fail closed for date windows until
  reindexed;
- optional inclusive `published_from` / `published_to` bounds accept RFC3339, may
  be one-sided, reject reversed ranges, exclude categories and fail closed on
  malformed projected timestamps;
- date narrowing intersects author/tag/solved after the stable bounded raw snapshot
  and before exact Forum owner eligibility, visible totals, facets, offset and limit;
- existing legacy, author-only and B2F2 GraphQL/native wire signatures remain
  unchanged; date windows use additive `ForumStorefrontSearchByDateWindow` and
  `search/forum-storefront-search-by-date-window` transports;
- `forum-search-locale-date-filter.json`,
  `forum-23b2f3-search-locale-date-filter.md`, and
  `verify-forum-search-locale-date-filter.mjs` lock locale, projection, range,
  ordering, compatibility and degraded-mode behavior while execution/reindex
  evidence remains pending.

### Delivered in `FORUM-23B2F4`

- an optional `current_channel_only` filter narrows explicit Forum-only Search to
  topics explicitly assigned to the trusted request channel and approved replies
  inheriting the same parent-topic assignment;
- Forum projects parent-topic channel slugs onto reply documents, and legacy reply
  rows without that projection fail closed until reindexed;
- the filter accepts no caller-selected channel slug, excludes global topics and
  categories, runs before exact Forum owner eligibility/totals/facets/pagination,
  and suppresses query-rule pins while active;
- topic channel updates publish the existing transactional `forum_topic`
  invalidation so topic and parent-derived reply channel projections rebuild
  together;
- existing wire signatures remain unchanged; additive
  `ForumStorefrontSearchByCurrentChannel` and
  `search/forum-storefront-search-by-current-channel` transports share the
  existing execution owner;
- arbitrary channel/group selection remains blocked on a separately authorized
  Forum owner contract, kind filtering waits on `FORUM-22`, and attachment
  presence waits on `FORUM-14`.

### Delivered in `FORUM-23B2G1`

- a Search-owned PostgreSQL migration adds positive unique immutable
  `ingest_sequence` values to Forum projection inbox rows and non-negative sequence
  watermarks;
- existing rows are backfilled deterministically by database arrival time,
  envelope revision timestamp and event identity before the database sequence is
  advanced beyond the retained maximum;
- claim order, retry blocking, due-tenant order and stale watermark comparison use
  only `ingest_sequence`; producer wall-clock timestamps and UUID ordering no longer
  choose execution order;
- `revision_at` and `event_id` remain mandatory envelope-identity and diagnostic
  fields, and author privacy/account-deletion scopes remain unskippable redaction
  barriers;
- event schemas, Forum owner writes, reindex targets, projection rebuilds, retry,
  dead-letter, public transport and storefront query behavior remain unchanged;
- `forum-search-durable-ingest-sequence.json`,
  `forum-23b2g1-search-durable-ingest-sequence.md`, and
  `verify-forum-search-durable-ingest-sequence.mjs` lock migration, ordering,
  compatibility and non-claim boundaries while runtime evidence remains pending.

### Delivered in `FORUM-23B2G2A` and `FORUM-23B2G2A1`

- Forum allocates one positive tenant-scoped monotonic owner revision and appends
  an immutable `(tenant_id, revision, event_id, target_type, target_id)` ledger
  row in the same owner transaction as the canonical legacy Search invalidation;
- the ledger event ID is the exact `index.reindex_requested` root envelope ID and
  is not derived from time, UUID order, Forum journal sequence or Search
  `ingest_sequence`;
- PostgreSQL upgrade preflight requires a contiguous `1..counter` ledger, row
  guards require initial revision `1` and exact `+1` updates, and a deferred
  constraint requires commit-time ledger coverage for every counter advance;
- counter and ledger update/delete/truncate bypasses fail closed while the
  canonical `INSERT ... ON CONFLICT ... revision + 1` allocator remains valid;
- Search delivery, inbox execution order and storefront behavior remain
  unchanged by these Forum-owned migrations.

### Delivered in `FORUM-23B2G2B1` and `FORUM-23B2G2B2`

- `ForumProjectionOwnerRevisionSourcePort` exposes bounded contiguous pages of at
  most 100 owner revisions and bounded tenant-head discovery without exposing
  Forum-private actor, timestamp, target or outbox payload data;
- the server is the only Forum/Search composition point; Search does not query
  Forum owner tables directly and malformed, missing or non-contiguous owner
  pages fail closed;
- Search stores an independent exact `+1` owner checkpoint, scans owner tenants
  with a compare-and-set cursor, and preserves the existing Forum inbox as the
  primary execution lane ordered by `ingest_sequence`;
- pending, processing and retryable inbox work blocks checkpoint advancement;
  missing or dead-letter delivery triggers one current-state tenant rebuild under
  the same advisory lock as ordinary projection execution;
- projection success commits before checkpoint advancement, so checkpoint failure
  repeats safe idempotent repair rather than recording coverage early;
- Forum `owner_revision` and Search `ingest_sequence` remain independent clocks
  and are never compared numerically.

### Delivered in `FORUM-23B2G2B3A` through `FORUM-23B2G2B3C`

- the sealed v1 `forum.search_projection.invalidation_issued` event carries the
  Forum owner revision and exact bounded projection impact;
- causation-aware event APIs let Forum atomically publish that typed event beside
  the mandatory legacy root without changing the accepted event registry or
  using the typed envelope ID as projection identity;
- the legacy root envelope ID is simultaneously the Forum revision-ledger event
  ID, typed envelope `causation_id` and shared `search_projection_inbox.event_id`;
- the default-off persistent consumer requires PostgreSQL and `outbox_iggy`, uses
  consumer group `rustok-search-forum-projection-v1`, and adapts typed delivery
  into the existing Forum inbox, reconciler and projector rather than creating a
  second execution path;
- legacy-first and typed-first arrivals collapse on one complete durable identity;
  UUID-only conflicts are semantic poison and cannot enter the projector;
- raw and semantic poison reuse connector-owned durable receipts and deterministic
  DLQ identity, while transient persistence, broker publication or acknowledgement
  failures leave the exact source offset uncommitted;
- the legacy root path remains mandatory during rollout and Search-disabled Forum
  commands retain no synchronous Search dependency.

### Delivered in `FORUM-23B2G2B3D0` and `FORUM-23B2G2B3D1`

- D0 freezes the machine-readable
  `forum_search_versioned_invalidation_runtime_evidence_v1` protocol and a
  fail-closed static guard for the merged owner-ledger, checkpoint, sealed wire,
  dual-publisher and persistent-consumer chain;
- the required executable scenarios cover normal delivery, both duplicate arrival
  orders, acknowledgement failure/restart, raw and semantic poison, missing
  delivery repair, multi-process serialization, deletion/ACL ordering and the
  Search-disabled profile;
- runtime output must be generated at
  `target/forum-search-versioned-invalidation-runtime-evidence.json`; hand editing
  or replacing it with a static fixture is forbidden;
- D1 reconciles this single canonical plan with the complete B2G2 source chain and
  extends the existing guard so stale plan text cannot silently return;
- both slices retain `source_ready_maintainer_execution_pending`; they do not
  claim PostgreSQL, Iggy, DLQ, restart, multi-process or `LINK-FORUM-03` evidence.

### Compatibility and degraded mode

`FORUM-23B2G1` adds the PostgreSQL Search ingest-order migration. G2A/A1 add
PostgreSQL-only Forum owner revision and hardening migrations, and G2B2 adds
PostgreSQL-only Search checkpoint/cursor storage. SQLite remains a
validation-only Forum Search profile. G2B3A-C add a sealed caused event and a
server-owned persistent consumer without changing public Search DTOs or query
shapes; the consumer remains default-off and requires `outbox_iggy`. D0/D1
change only documentation, evidence contract and source guards.

No B2G2 slice replaces the mandatory legacy `index.reindex_requested` event or
adds a second inbox, projector, reconciler, watermark or execution clock. The
legacy and typed representations converge on the same root envelope ID and one
`search_projection_inbox` row. Forum owner commands and transactional events
continue to commit when Search or the typed consumer is disabled; bounded owner
ledger reconciliation repairs delivery gaps after Search is restored.

The B2A-F4 compatibility rules remain unchanged: Product category identifiers
are not expanded through Forum policy; trusted channel selection is assertion
only; active author/tag/solved/date/current-channel filters use only approved
owner projections; legacy missing projections fail closed until reindexed; and
admin/global Search behavior remains unchanged.

### Remaining scope

- add owner-safe arbitrary channel/group filtering only after an exact authorized
  Forum owner contract exists; add kind after `FORUM-22` and attachment presence
  after `FORUM-14`;
- execute and retain every D0 PostgreSQL/Iggy scenario, including duplicate
  arrival orders, restart/acknowledgement, durable poison/DLQ, missing-delivery
  repair, multi-process locking, deletion/ACL ordering and Search-disabled
  continuity on the exact reviewed commit;
- capture maintainer-executed PostgreSQL query/result evidence and complete the
  `LINK-FORUM-03` cross-module runtime proof.

### Definition of done

Pending/hidden/private content never leaks, out-of-order events cannot regress
a projection, owner/index revisions reconcile, and deletion/ACL changes remove
documents.

### Verification

```bash
cargo test -p rustok-forum category_search_scope -- --nocapture
cargo test -p rustok-forum category_search_audience_scope -- --nocapture
cargo test -p rustok-search storefront_category_scope -- --nocapture
cargo test -p rustok-search storefront_result_eligibility -- --nocapture
cargo test -p rustok-search forum_document_filters -- --nocapture
cargo test -p rustok-search storefront_channel_authority -- --nocapture
cargo test -p rustok-search storefront_product_channel_visibility -- --nocapture
cargo test -p rustok-search product_channel_visibility_legacy_projection_is_detected -- --nocapture
cargo test -p rustok-search product_channel_reconciliation -- --nocapture
cargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture
cargo test -p rustok-search category_filter_preserves_product_and_adds_exact_forum_scope -- --nocapture
cargo test -p rustok-search forum_contract_ingress -- --nocapture
cargo test -p rustok-search owner_revision_tests -- --nocapture
cargo test -p rustok-search --test forum_projection_sweeper_contract -- --nocapture
cargo test -p rustok-search-storefront transport::tests::only_explicit_forum_category_scope_selects_owner_path -- --nocapture
cargo test -p rustok-server --features mod-forum forum_search_category_scope -- --nocapture
cargo test -p rustok-server --features mod-forum forum_search_result_eligibility -- --nocapture
node scripts/verify/verify-forum-search-category-subtree-scope.mjs
node scripts/verify/verify-forum-search-category-audience-scope.mjs
node scripts/verify/verify-forum-search-exact-category-filter.mjs
node scripts/verify/verify-forum-search-storefront-scope.mjs
node scripts/verify/verify-forum-search-result-eligibility.mjs
node scripts/verify/verify-forum-search-trusted-channel-authority.mjs
node scripts/verify/verify-forum-search-product-channel-visibility.mjs
node scripts/verify/verify-forum-search-author-filter.mjs
node scripts/verify/verify-forum-search-tag-solved-filter.mjs
node scripts/verify/verify-forum-search-locale-date-filter.mjs
node scripts/verify/verify-forum-search-current-channel-filter.mjs
node scripts/verify/verify-forum-search-durable-ingest-sequence.mjs
node scripts/verify/verify-forum-search-owner-revision-ledger.mjs
node scripts/verify/verify-forum-search-owner-revision-counter-hardening.mjs
node scripts/verify/verify-forum-search-owner-revision-source.mjs
node scripts/verify/verify-forum-search-owner-revision-checkpoint.mjs
node scripts/verify/verify-forum-search-versioned-invalidation-wire.mjs
node scripts/verify/verify-forum-search-versioned-invalidation-causation-api.mjs
node scripts/verify/verify-forum-search-versioned-invalidation-publisher.mjs
node scripts/verify/verify-forum-search-versioned-invalidation-consumer.mjs
node scripts/verify/verify-forum-search-versioned-invalidation-runtime-evidence.mjs
cargo check -p rustok-search --features graphql --all-targets
cargo check -p rustok-search-storefront --features ssr --all-targets
cargo check -p rustok-server --features mod-forum --all-targets
cargo xtask module validate forum
cargo xtask module validate search
```

The `FORUM-23B2A` through `FORUM-23B2G2B3D1` source and contract records do not
claim successful runtime verification. The D0 protocol status remains
`source_ready_maintainer_execution_pending` until the maintainer generates and
retains the executable evidence artifact.

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

### Delivered in FORUM-24A

- `ForumTopicRouteService` owns `/{locale}/forum/t/{short_id}/{slug}` descriptors,
  where `short_id` is the first 48 bits of the topic UUID in lowercase hex and
  the readable slug is not identity;
- current route lookup reads at most two candidates and fails closed on a
  short-identity collision instead of choosing by slug;
- existing bounded merge canonical resolution is reused so an archived merge
  source redirects to the terminal retained topic;
- PostgreSQL and SQLite `forum_topic_route_aliases` provide one append-only
  redirect/gone ledger keyed by tenant, locale, short identity and slug;
- redirects store target topic plus locale and recompute the latest target slug;
- route identity is transport-neutral and does not bypass topic visibility,
  channel, moderation or SEO publication authorization.

Verification sources:

```bash
node scripts/verify/verify-forum-topic-route-identity-owner.mjs
cargo test -p rustok-forum services::topic_route::tests -- --nocapture
cargo test -p rustok-forum --test topic_route_identity_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

No command above was run by the implementation agent, per maintainer request.

### Delivered in FORUM-24B

- `ForumTopicMergeService` delegates localized route history to
  `ForumTopicRouteService::record_merge_redirect_aliases_in_tx` before commit;
- every source translation with a non-empty slug receives one immutable redirect
  keyed by its original locale, short identity and slug;
- target locale selection is deterministic: exact source locale, platform
  fallback locale, then the lexicographically first available target locale;
- redirects store target topic plus locale and continue to recompute the latest
  target slug without changing the merge receipt or `forum.topic.merged` event;
- source topics without routes keep existing merge behavior, while a routed
  source fails closed when the target has no canonical localized route;
- exact merge replay returns the existing receipt and does not duplicate aliases.

Topic rename aliases remain follow-up owner composition. Historical merge receipt
backfill, storefront mounting and retained runtime proof also remain.

Verification sources:

```bash
node scripts/verify/verify-forum-topic-merge-route-alias-owner.mjs
cargo test -p rustok-forum --test topic_merge_route_alias_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

No command above was run by the implementation agent, per maintainer request.

### Delivered in FORUM-24C

- `TopicService::delete` delegates to
  `ForumTopicRouteService::record_delete_tombstones_in_tx` before localized
  cleanup and soft-delete mutation;
- every topic translation with a non-empty slug receives one immutable `gone`
  route with no target topic or locale and the stable reason `Topic deleted`;
- the tombstones commit with the existing delete lifecycle, counters, events and
  projection invalidation without changing public command or event schemas;
- an existing redirect for the same topic and route is preserved, so deleting an
  archived merge source cannot downgrade FORUM-24B canonical history;
- exact existing `gone` rows are idempotent and ownership, target-field or reason
  drift fails closed.

Topic rename aliases, historical backfill, storefront mounting, category routes,
hreflang/SEO policy and retained runtime proof remain.

Verification sources:

```bash
node scripts/verify/verify-forum-topic-delete-route-tombstone-owner.mjs
cargo test -p rustok-forum --test topic_delete_route_tombstone_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_merge_route_alias_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

No command above was run by the implementation agent, per maintainer request.

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

**Status:** `in_progress`  
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

### Delivered in `FORUM-26A` through `FORUM-26J`

- Forum-owned tenant/user trust state, typed trust levels, managed writes and an
  authoritative exact-user trust facts adapter replace activity-counter or role
  inference;
- bounded posting-action, candidate-metric, rule, outcome, evidence and decision
  contracts define explainable local policy evaluation without executing owner
  writes or distributed reservations;
- the facts composer keeps missing owner capabilities explicit and preserves
  exact tenant/user actor identity, deadlines, typed retryability and one unique
  provider per fact kind;
- authoritative server/users account age, Forum topic-reading activity and
  current retained approved-post facts are published without copying owner data
  into policy configuration;
- `FORUM-26I` adds PostgreSQL/SQLite partial author indexes aligned to the exact
  approved-post query, source-ready `EXPLAIN` proofs, index-definition guards and
  the minimal platform-user fixture required by the isolated PostgreSQL Forum
  migration bootstrap;
- `FORUM-26J` publishes authoritative exact-user topic/reply create-window facts
  over all persisted owner rows, keeps soft-deleted and moderated attempts in the
  observed budget, composes both providers in the host, and adds PostgreSQL/
  SQLite author-time indexes plus source-ready `EXPLAIN` proof;
- none of these slices invokes the composer from topic/reply/edit/bump commands,
  reserves shared rate-limit capacity, hashes duplicate content, calls external
  scoring or automatically changes trust state.

### Compatibility and degraded mode

Trust rows, approved-post indexes and create-window indexes are additive.
Existing users without a trust row retain the documented compatibility level;
existing topic/reply rows are indexed automatically with no backfill or owner-
state rewrite. The approved-post query and fact value remain unchanged by its
index migration. Create-window facts count every persisted owner create inside
the exact inclusive observation window, including rows later soft-deleted and
replies in any moderation state, so ordinary deletion or moderation cannot reset
the budget. A hard retention purge removes historical rows and must remain later
than any configured production window unless a durable usage ledger replaces the
snapshot. Missing fact owners continue to return explicit unavailable results,
and optional external or AI scoring can never become a synchronous correctness
dependency. These facts are not a concurrency-safe reservation; rolling back the
index migrations changes only query performance, not fact semantics.

### Remaining scope

- publish authoritative active-flag and moderation-history facts from the
  moderation/report owner without inferring them from reply-status totals;
- publish authoritative reputation facts from the reputation owner and ledger;
- publish edit-window and topic-target bump-age facts only after exact editing
  actor and target identity are represented in the owner history and fact request;
- persist and administer bounded policy configuration with versioned audit;
- enforce the composed decision in topic, reply, edit and bump owner commands;
- reserve, commit and release distributed rate-limit capacity through the shared
  capability without making it Forum persistence;
- add retained duplicate-content fingerprints and optional external/AI scoring;
- add automatic trust promotion/demotion only through explicit explainable owner
  commands and immutable evidence;
- expose bounded admin/storefront/transport surfaces and capture maintainer-
  executed PostgreSQL, SQLite and cross-consumer runtime evidence.

### Definition of done

All posting owner paths evaluate one versioned bounded policy before mutation,
missing required facts fail closed with typed retryability, distributed limits
cannot be bypassed through another transport, duplicate/replay behavior is
idempotent, optional scoring failure does not break correctness, and every trust
change and posting denial is explainable and auditable.

### Verification

```bash
cargo test -p rustok-forum posting_policy
cargo test -p rustok-forum posting_policy_approved_facts -- --nocapture
cargo test -p rustok-forum posting_policy_create_window_facts -- --nocapture
cargo test -p rustok-forum --test approved_posts_index_sqlite -- --nocapture
cargo test -p rustok-forum --test approved_posts_index_postgres -- --nocapture --test-threads=1
cargo test -p rustok-forum --test create_window_facts_index_sqlite -- --nocapture
cargo test -p rustok-forum --test create_window_facts_index_postgres -- --nocapture --test-threads=1
node scripts/verify/verify-forum-posting-policy-facts.mjs
node scripts/verify/verify-forum-topic-reading-posting-facts.mjs
node scripts/verify/verify-forum-approved-posts-posting-facts.mjs
node scripts/verify/verify-forum-approved-posts-index-debt.mjs
node scripts/verify/verify-forum-approved-posts-index-hardening.mjs
node scripts/verify/verify-forum-create-window-posting-facts.mjs
cargo xtask module validate forum
```

The FORUM-26J source and contract records do not claim successful runtime
verification until the maintainer runs the commands above.

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

**Status:** `done`
**Priority:** P0
**Dependencies:** FORUM-12/14/25

### Delivered

- Topic/reply create and update contracts accept one `RichTextDocument`; reads
  expose one `RichTextView` and bounded server-derived plain text.
- The initial PostgreSQL and SQLite schema stores only canonical serialized
  documents. Revision tables preserve the same document and deletion preserves
  content history while lifecycle state carries deletion.
- Mentions walk structural document nodes and relation fingerprints cover the
  canonical serialization. No source-row seed or placeholder relation identity
  exists.
- HTML and plain text come only from `rustok-content::richtext` under the
  `discussion` profile.
- Next Forum authoring uses the shared React frame directly. Leptos Forum
  authoring uses `leptos_ui::RichTextEditorFrame`; its WASM lifecycle is compiled
  only for browser hydration while native SSR uses the shared component API.
- Leptos keeps native `#[server]` as its selected internal path and GraphQL as
  the parallel public/headless contract.
- `cargo test -p rustok-forum --lib` passes 113 tests; native and WASM checks,
  `@rustok/richtext` tests, Next typecheck, and the Forum/Blog ownership verifier
  pass for this cutover.
- SQLite and PostgreSQL soft-delete/revision tests execute against the canonical
  document storage. Both preserve content on deletion, retain edit/delete
  revisions, and reject physical deletion of non-empty categories.

Spoilers, emoji, media, attachments, preview, drafts, and richer keyboard
behavior are future shared extension work. They must extend the same document
and profile contracts rather than introduce another editor or storage format.

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
- Richtext parity evidence: the shared Leptos frame compiles for native and
  `wasm32-unknown-unknown`; Next typecheck passes; both hosts submit the same
  `RichTextDocument` and consume server-owned projections.

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
cargo test -p rustok-forum --test topic_read_state_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_unread_projection_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_bulk_read_state_sqlite -- --nocapture
cargo test -p rustok-forum --test read_state_transport_contract -- --nocapture
cargo test -p rustok-forum --test storefront_read_state_contract -- --nocapture
cargo test -p rustok-forum --test storefront_read_state_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_read_state_postgres -- --nocapture --test-threads=1
cargo test -p rustok-forum --test approved_posts_index_sqlite -- --nocapture
cargo test -p rustok-forum --test approved_posts_index_postgres -- --nocapture --test-threads=1
cargo test -p rustok-forum --test create_window_facts_index_sqlite -- --nocapture
cargo test -p rustok-forum --test create_window_facts_index_postgres -- --nocapture --test-threads=1
cargo test -p rustok-forum --test topic_visibility_sqlite -- --nocapture
cargo test -p rustok-forum --test category_visibility_policy_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_authenticated_visibility_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_reply_owner_visibility_sqlite -- --nocapture
cargo test -p rustok-forum --test category_owner_visibility_sqlite -- --nocapture
cargo test -p rustok-forum --test audience_capability_contract -- --nocapture
cargo test -p rustok-forum --test category_audience_policy_sqlite -- --nocapture
cargo test -p rustok-forum --test category_reply_create_audience_policy_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_reply_create_audience_policy_sqlite -- --nocapture
cargo test -p rustok-forum --test moderation_audience_policy_sqlite -- --nocapture
cargo test -p rustok-forum reply_create_transport -- --nocapture
cargo test -p rustok-forum graphql::runtime_data -- --nocapture
cargo test -p rustok-forum moderation_transport -- --nocapture

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
node scripts/verify/verify-forum-read-state-runtime-proof.mjs
node scripts/verify/verify-forum-approved-posts-index-hardening.mjs
node scripts/verify/verify-forum-create-window-posting-facts.mjs
node scripts/verify/verify-forum-topic-visibility-scope.mjs
node scripts/verify/verify-forum-category-visibility-policy.mjs
node scripts/verify/verify-forum-owner-read-visibility.mjs
node scripts/verify/verify-forum-category-owner-read-visibility.mjs
node scripts/verify/verify-forum-audience-capability-ports.mjs
node scripts/verify/verify-forum-category-audience-policy.mjs
node scripts/verify/verify-forum-category-reply-create-audience-policy.mjs
node scripts/verify/verify-forum-reply-create-audience-enforcement.mjs
node scripts/verify/verify-forum-reply-create-audience-transport-composition.mjs
node scripts/verify/verify-forum-topic-reply-create-audience-policy.mjs
node scripts/verify/verify-forum-moderation-audience-policy.mjs
node scripts/verify/verify-forum-moderation-audience-transport-composition.mjs
node scripts/verify/verify-forum-audience-plan-sync.mjs
cargo test -p rustok-forum category_search_scope -- --nocapture
cargo test -p rustok-forum category_search_audience_scope -- --nocapture
cargo test -p rustok-search storefront_category_scope -- --nocapture
cargo test -p rustok-search storefront_result_eligibility -- --nocapture
cargo test -p rustok-search forum_document_filters -- --nocapture
cargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture
cargo test -p rustok-search forum_contract_ingress -- --nocapture
cargo test -p rustok-search owner_revision_tests -- --nocapture
cargo test -p rustok-search --test forum_projection_sweeper_contract -- --nocapture
cargo test -p rustok-search-storefront transport::tests::only_explicit_forum_category_scope_selects_owner_path -- --nocapture
cargo test -p rustok-server --features mod-forum forum_search_category_scope -- --nocapture
cargo test -p rustok-server --features mod-forum forum_search_result_eligibility -- --nocapture
node scripts/verify/verify-forum-search-exact-category-filter.mjs
node scripts/verify/verify-forum-search-category-subtree-scope.mjs
node scripts/verify/verify-forum-search-category-audience-scope.mjs
node scripts/verify/verify-forum-search-storefront-scope.mjs
node scripts/verify/verify-forum-search-result-eligibility.mjs
node scripts/verify/verify-forum-search-author-filter.mjs
node scripts/verify/verify-forum-search-tag-solved-filter.mjs
node scripts/verify/verify-forum-search-locale-date-filter.mjs
node scripts/verify/verify-forum-search-current-channel-filter.mjs
node scripts/verify/verify-forum-search-durable-ingest-sequence.mjs
node scripts/verify/verify-forum-search-owner-revision-ledger.mjs
node scripts/verify/verify-forum-search-owner-revision-counter-hardening.mjs
node scripts/verify/verify-forum-search-owner-revision-source.mjs
node scripts/verify/verify-forum-search-owner-revision-checkpoint.mjs
node scripts/verify/verify-forum-search-versioned-invalidation-wire.mjs
node scripts/verify/verify-forum-search-versioned-invalidation-causation-api.mjs
node scripts/verify/verify-forum-search-versioned-invalidation-publisher.mjs
node scripts/verify/verify-forum-search-versioned-invalidation-consumer.mjs
node scripts/verify/verify-forum-search-versioned-invalidation-runtime-evidence.mjs
cargo check -p rustok-search --features graphql --all-targets
cargo check -p rustok-search-storefront --features ssr --all-targets
cargo check -p rustok-server --features mod-forum --all-targets
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
10. record `FORUM-16` maintainer PostgreSQL proof and finish visibility-scoped
    storefront bulk composition after the complete `FORUM-20` policy can page an
    exact category or tenant scope;
11. `FORUM-19`: reports/moderation/restrictions;
12. continue `FORUM-20` with remaining exact Forum read authorization, then
    search/index, SEO and deep-link consumers; route any future moderation
    transports through the delivered context-aware owner boundary;
13. continue `FORUM-26` with authoritative active-flag and moderation-history
    fact adapters, keeping every missing owner capability explicitly unavailable;
14. execute `FORUM-23B2G2B3D` and `LINK-FORUM-03` runtime evidence on the
    reconciled owner-revision/one-inbox source, then add owner-safe arbitrary
    channel/group filters when their exact owner contract exists, kind after
    `FORUM-22`, and attachment presence after `FORUM-14`;
15. `LINK-FORUM-01` and the remaining `LINK-FORUM-03` release proof only after
    their owner contracts and executable evidence are stable.

# Decisions that must not be reopened without an ADR

## No separate member module

`rustok-profiles` is the public member identity. Forum owns only forum-specific
stats, trust, badges, restrictions and activity.

## Media ownership

Profiles stores avatar/banner media references. Forum stores category and post
attachment references. `rustok-media` owns files, URLs, MIME, storage,
quarantine and deletion.

## Notifications are optional consumers

Forum always commits semantic events. It does not synchronously call
notifications to complete a command. Disabling notifications hides its UI and
stops deliveries without breaking forum state changes.

## Email is a provider

`rustok-email` performs delivery. The notifications owner controls recipient
resolution, preferences, timing, templates, retries and channel selection.

## No premature shared reactions/reputation/mentions module

Keep these models forum-owned until another real owner consumer demonstrates a
stable neutral contract. Publish semantic events to make later extraction
possible.

# Immediate next action

Process `notification_fanout_items` through explicit preference and profile/block
privacy policy, reauthorize the source for the recipient, and only then create a
final idempotent notification row. Keep channel delivery, moderator audience
expansion, production outbox wiring and maintainer PostgreSQL evidence as
separate bounded follow-up slices; never make Notifications a synchronous Forum
command dependency.
