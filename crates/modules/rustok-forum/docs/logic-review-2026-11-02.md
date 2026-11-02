# Forum Logic Review — 2026-11-02

Scope: `crates/modules/rustok-forum` (407 Rust files, ~178k LOC with
admin/storefront packages), its GraphQL/REST transports, the Next.js packages
(`apps/next-frontend/packages/rustok-forum`, `apps/next-admin/packages/forum`),
and the cross-module seams (notifications, reactions, moderation, search,
media, taxonomy). Reviewed against `AGENTS.md`, in particular the six logic
dimensions of §2.1/§4 (preconditions and bounds, branch completeness,
state-transition integrity, side-effect completeness, algorithmic correctness,
concurrency/order of operations), §5 platform-capability reuse, §7 write-side
correctness, and §10 performance.

Method: owner-contract read (`README.md`, `CRATE_API.md`,
`docs/implementation-plan.md`), source inspection of the core write/read
paths (topics, replies, moderation, votes, subscriptions, read state,
notifications, settings), anti-pattern scans, and scoped verification (see
"Verification performed"). This is a review document: it records findings and
recommendations; it does not change behavior. Base commit: `7fd5262`.

Severity scale: **Critical** (data loss / security breach), **High**
(user-visible wrong behavior, security weakening, or contract violation),
**Medium** (wrong behavior in edge cases, drift, or abuse potential),
**Low** (robustness, consistency, performance hygiene).

---

## Summary

The forum core is unusually well defended for a module of this size: tenant
scoping is systematic, most write paths lock in one documented order
(category tree → topic → reply), counters use checked arithmetic and fail
closed, mention/quote relations are append-only with CAS, and transports
pin their audience context instead of falling back. No Critical defect was
found. The problems concentrate in four places:

1. **Business logic living in database triggers** (auto-subscribe), which
   the platform contract forbids and which silently contradicts the Rust
   subscription contract.
2. **Settings and product surface that are declared but not enforced**
   (digest modes, trust/posting-policy evaluator, `allow_user_topic_closing`),
   so the UI promises behavior the write path never delivers.
3. **Guard asymmetries between sibling paths**: single-topic read state
   bypasses the audience checks that bulk read state enforces; topic edits
   lack the status guards reply edits have; moderation audience checks run
   outside the write transaction.
4. **Silent counter clamping and unbounded vote aggregation** on the
   read/write hot paths.

---

## High findings

### H1. Auto-subscribe is implemented as PL/pgSQL/SQLite triggers, contradicting the Rust owner contract

Evidence:
`src/migrations/m20260713_000013_add_forum_subscription_levels/postgres_up/automation.rs`
(functions `forum_auto_subscribe_topic_author`,
`forum_auto_subscribe_reply_participant`, triggers `forum_70_*`; mirrored in
`sqlite_up/automation.rs`).

Why it is a defect:

- `AGENTS.md` §7 forbids complex business rules and cascading side effects
  inside PL/pgSQL triggers; domain rules must live in the Rust owner inside
  the transaction. Subscription creation on topic/reply write is exactly
  such a rule, and it is invisible to `SubscriptionService`, to tests built
  on the Rust services, and to any future non-SQL writer.
- The trigger cannot distinguish an *implicit* row from an *explicit* one.
  `SubscriptionService::clear_*_subscription` "unsubscribes" by writing a
  `Normal` row (see `src/services/subscription/topic.rs`,
  `clear_topic_subscription`), and the trigger's conflict handler upgrades
  any row `WHERE level = 'normal'`. A user who explicitly unsubscribed is
  therefore silently re-subscribed (`tracking`) the next time one of their
  replies transitions to `approved` (e.g. after a hide → approve moderation
  cycle). This is a user-visible consent bug, not a style issue.
- Trigger-created rows are returned by the read path with `explicit: true`
  (`topic_response`/`category_response` in
  `src/services/subscription/helpers.rs`), so the UI shows a deliberate
  subscription the user never made.
- The trigger duplicates the preference defaults already owned by
  `ForumSubscriptionLevel::default_preferences` (`src/subscription.rs`);
  the two definitions can drift independently.
- The trigger fires on every `forum_topics`/`forum_replies` insert,
  including imports, forks, splits and restores, none of which made a
  product decision to subscribe the (possibly system-attributed) author.
- The revision trigger pair (`forum_validate_subscription_revision`)
  enforces `revision = old + 1` in SQL while the Rust side is the writer
  expected to own that invariant; see also M4.

Recommendation: delete the `forum_70_*` triggers and implement
auto-subscribe in the topic/reply owner transactions, gated by the same
subscription policy row, writing rows marked implicit (reuse the existing
`explicit: false` response semantics instead of inserting a fake explicit
row). Never upgrade an existing explicit row — including a `Normal` row
written by an explicit clear — without a stored "cleared" marker. Add a
SQLite + PostgreSQL test proving unsubscribe → reply moderation cycle does
not resurrect a subscription.

### H2. Digest subscription modes are sold but never delivered; `last_notified_at` is dead state

Evidence: `ForumDigestMode::{Daily, Weekly}` in `src/subscription.rs` are
accepted by the subscription DTOs and persisted, but
`src/notification_source.rs` fan-out filters only on `notify_*` flags and
`level != Muted` (e.g. `reply_topic_subscription_page`,
`reply_category_subscription_page`); it never reads `digest_mode`, and no
Rust code ever updates `last_notified_at` (only `Set(None)` on insert in the
subscription services).

Impact: a user who selects Daily/Weekly digest either receives immediate
notifications anyway (if `notify_replies = true`) or none; the setting has
no honest semantics. Per `AGENTS.md` §14 this is fake completion unless
documented as a limitation — the README's "Reply notifications are
delivered" section does not disclose it.

Recommendation: either (a) implement digest batching in the notifications
owner (forum only emits facts; batching must not be re-implemented inside
the forum), driving it from `digest_mode` + `last_notified_at`, or (b)
remove `Daily`/`Weekly` from the DTO, admin form and contracts under the
zero-legacy rule until (a) exists. Do not leave the enum published.

### H3. Trust/posting-policy engine is public API but disconnected from every write path

Evidence: `ForumPostingPolicyEvaluator`, the fact ports
(`posting_policy_*.rs`, `user_trust*.rs`, FORUM-26 in
`docs/implementation-plan.md`) are exported from `src/lib.rs`, but no
topic/reply create path evaluates them; the README already admits "The
posting policy evaluator (`posting_policy_evaluator.rs`) is not yet wired
into the write path, so its window rules are not enforced."
`allow_user_topic_closing` (`src/dto/settings.rs`) is likewise declared and
unwired.

Impact: tenant admins can configure trust thresholds, posting windows and
author topic-closing that have zero effect — the worst failure mode for a
policy system, because operators believe a control exists. Per `AGENTS.md`
§16, exported-but-unwired capability also inflates the public contract
other modules may start depending on.

Recommendation: wire the evaluator into topic/reply create inside the
existing transaction (facts are already transaction-friendly), or remove
the evaluator, the fact ports and the setting from the public contract
until they are. If wiring is deferred, record it under "Known Limitations"
in `README.md` with the FORUM-26 task as owner, and hide the dead settings
from the admin form (same treatment `allow_anonymous_reading` already got).

### H4. Single-topic read state bypasses the audience contract that bulk read state enforces

Evidence: GraphQL `forumTopicReadState` / `markForumTopicRead`
(`src/graphql/read_state.rs`) and the REST equivalents
(`src/controllers/read_state.rs`) call `ForumTopicReadStateService`
directly. Its `get_topic_read_state` only calls `ensure_topic_exists`, and
`mark_topic_read` locks and marks any existing topic. The bulk operations
in the same files correctly go through the visibility-scoped service
(`mark_category_read_with_audience_context`,
`mark_all_read_with_audience_context`,
`ForumVisibilityScopedReadStateService`), and the storefront mutations use
`mark_topic_read_current_audience_visible`.

Impact: (1) existence oracle — a caller can distinguish "hidden topic
exists" from "topic does not exist" for audience-restricted and `Pending`
topics; (2) a user can mark topics read that they cannot see, polluting
their unread feed (bulk `mark_scope_read` in
`src/services/read_tracking.rs` also includes other users' `Pending`
topics, since it only excludes `Archived`). This is precisely the
asymmetry the vote/subscription write ADR
(`DECISIONS/2026-10-09-forum-vote-subscription-write-audience.md`) was
written to eliminate for writes.

Recommendation: route single-topic read-state query and mutation through
the same audience-scoped service as the bulk/storefront paths; make
hidden/missing indistinguishable (`TopicNotFound`), and exclude `Pending`
topics from bulk marking except for their author and moderation scope
`All`. Add one transport-level test per surface (GraphQL, REST, native)
proving a hidden topic cannot be probed or marked.

---

## Medium findings

### M1. Moderation audience authorization runs before the transaction (check-then-act)

Evidence: `ModerationService::set_topic_pinned` / `set_topic_locked` /
`moderate_topic_status` / `moderate_reply_status`
(`src/services/moderation_owner.rs:193, 239, 603`) call
`audience.require_topic/require_reply` on `self.db`, then open the write
transaction. Category audience layers can change between the check and the
commit; topic/reply status commands re-check state under the category-tree
lock, but pin/lock do not re-evaluate the audience inside the transaction,
unlike vote and subscription writes, which deliberately evaluate
`topic_write_audience_allows_in_tx` under the same lock.

Recommendation: move the audience decision into the write transaction
after `lock_category_tree_in_tx`, reusing the existing `*_in_tx` helpers,
for all moderation commands (the topic/reply *status* paths included).

### M2. Pin/lock have no lifecycle guard and are not idempotent at the event boundary

Evidence: `set_pinned_in_tx` / `set_locked_in_tx`
(`src/services/topic.rs:357, 371`) accept any non-deleted topic, including
`Archived` and `Pending`; `set_topic_pinned` publishes
`DomainEvent::ForumTopicPinned` unconditionally, even when the flag already
has the requested value, and both commands publish projection invalidations
for no-ops.

Impact: moderators can pin a pre-moderation (`Pending`) or archived topic;
the flag survives archive → restore, so a restored thread can silently
reappear pinned. Duplicate pin events churn notification/projection
consumers. Contrast with the carefully journaled first-publication logic
in the same file — the event contract deserves the same discipline here.

Recommendation: reject pin/lock changes on `Archived` topics (typed
`TopicArchived`), decide explicitly whether `Pending` topics may be
prepared (pinned) before approval and document it; skip event and
projection publication when the value is unchanged.

### M3. Topic/reply votes are allowed on `Pending` topics; vote read summaries have no audience or status gate

Evidence: `lock_active_topic_vote_write_in_tx`
(`src/services/topic_vote_lock.rs`) rejects only `Archived`, so `Pending`
(pre-moderation) and `Closed` topics accept votes; reply votes check the
parent for `Archived` (`src/services/vote.rs`, `set_reply_vote`) but the
summary readers `topic_vote_summaries` / `reply_vote_summaries` are public,
take only `tenant_id` + ids, and return scores for any topic, including
hidden ones, to whichever caller composes them.

Recommendation: reject topic votes unless the topic is `Open` or `Closed`
(product decision: most forums freeze voting on close — pick one and
encode it in the lock helper so topic and reply votes cannot diverge);
keep the summary functions crate-private or document them as
caller-must-authorize, matching how relation reads are treated in
`CRATE_API.md`.

### M4. Subscription revision arithmetic can silently stop advancing, and conflicts are not typed

Evidence: `let next_revision = previous_revision.saturating_add(1);`
(`src/services/subscription/topic.rs:213`, `category.rs:164`) combined with
the database trigger requiring exactly `old + 1`: at `i64::MAX` the Rust
write is rejected by the trigger as a raw `DbErr`, surfacing as
`FORUM_INTERNAL_ERROR` instead of a typed exhaustion error (the attachment
relation owner already models this correctly with
`FORUM_ATTACHMENT_RELATION_REVISION_EXHAUSTED`). CAS conflicts from
`validate_expected_revision` / `ensure_revision_update` are returned as
generic `ForumError::Validation`, indistinguishable from bad input for
REST/GraphQL clients that should retry on 409.

Related: `clear_topic_subscription` reads the current revision *outside*
the transaction and then writes with `expected_revision` set to that stale
value, so a concurrent subscription update makes an operation documented
as a "blind actor-scoped clear" fail with a revision conflict instead of
clearing. Recommendation: check out the revision inside
`write_topic_subscription`'s transaction (it already re-reads the row
there), use checked arithmetic with a typed
`ForumSubscriptionRevisionExhausted`/conflict code, and map subscription
CAS conflicts to a stable retryable conflict code like the relation
owner does.

### M5. Threading invariants: replies can be attached to non-public parents, with no depth bound

Evidence: reply create (`src/services/reply_owner_inline.rs:102-115`)
rejects only a `Deleted` parent. A reply can be created under a `Pending`,
`Rejected`, `Hidden` or `Flagged` parent; when the child is auto-approved
(most categories) it becomes publicly readable while its parent is
redacted by the read path (`redact_non_public_reply_content` in
`src/services/reply.rs`), producing orphan thread fragments in list and
storefront rendering. No maximum nesting depth exists anywhere in the
create path or DTOs, so a user can build arbitrarily deep chains; any
recursive tree assembly in clients becomes a stack/DoS hazard.

Recommendation: require the parent to be `Approved` for public replies
(or inherit the parent's visibility ceiling), and enforce a bounded
nesting depth at create time (count ancestors inside the transaction; the
topic row lock already serializes writers). Add a contract test for each
non-approved parent status.

### M6. Edits bypass re-moderation, and topic edits lack the status guards reply edits have

Evidence: reply update (`src/services/reply_inline.rs`) checks only
`Deleted`; topic update (`src/services/topic_inline.rs`) checks no status
at all before rewriting translations, metadata and tags. In a
pre-moderated tenant/category, an author can post a benign `Approved`
message and later edit it into arbitrary content that stays `Approved`;
an author can also edit an `Archived` (soft-deleted via archive, not
tombstoned) topic whose content then reappears verbatim on restore.
Reply edits additionally lack the optimistic-concurrency claim that topic
edits have (`claim_topic_update_in_tx`), so two concurrent reply edits
are last-write-wins while topic edits conflict — an undocumented
asymmetry. (The reply path does correctly compose mention/quote relation
persistence and the edit window; the gap is status/re-moderation only.)

Recommendation: decide and document the edit policy — the forum-standard
choice is "edits to `Approved` content in a moderated category / tenant
with `pre_moderation_enabled` return the item to `Pending` (with counter
adjustments), and `Archived` topics reject edits with `TopicArchived`".
Give reply updates the same expected-revision claim topics have, and add
counter/state tests mirroring the moderation status-transition tests.

### M7. User-stat counters silently clamp at zero while solution counters fail closed

Evidence: `UserStatsService::adjust_counts_in_tx`
(`src/services/user_stats.rs`) clamps every negative result with
`CASE WHEN ... < 0 THEN 0` / `MAX(..., 0)` for topic, reply and
(non-exact) solution deltas, while
`decrement_solution_count_exact_in_tx` in the same file deliberately
fails closed on inconsistency, and the README states negative
solution-count transitions "no longer silently saturate inconsistent
state". Category counters (`adjust_counters_in_tx`) also fail closed.
User topic/reply counts are therefore the one counter family that can
drift forever without a signal; FORUM-33 reconciliation exists as a
read-only report, but nothing alerts on or repairs the drift this
clamping hides.

Recommendation: make topic/reply decrements exact like the solution path
(or at minimum emit a structured telemetry event on clamp), and schedule
the FORUM-33 reconciliation report so drift is observed rather than
accumulated silently.

### M8. Database errors are classified by substring matching on the driver message

Evidence: `impl From<sea_orm::DbErr> for ForumError`
(`src/error.rs:288-320`) matches on `message.contains("forum topic merge
audience policy conflict")` etc., and manufactures
`TopicMergeAudiencePolicyConflict(Uuid::nil())`, discarding the real
operation id. Any reworded constraint message, driver change or locale
change silently reclassifies a client-correctable conflict as
`FORUM_INTERNAL_ERROR` (500), and the nil UUID misleads clients that log
or retry by operation id.

Recommendation: match on named constraints (PostgreSQL constraint names /
SQLite extended codes) instead of message substrings, and carry the
operation id from the service layer rather than reconstructing it from
the error.

---

## Low findings and hygiene

- **L1. Vote score aggregation loads every vote row into memory.**
  `topic_vote_summaries` / `reply_vote_summaries` (`src/services/vote.rs`)
  fetch all votes for the page's topics and sum in Rust (`score: i32`
  addition can also overflow before any sane vote count is reached only
  in theory, but the IO is real today: one popular topic makes every list
  page scan its full vote history). Use `SUM(value)` / `GROUP BY` in SQL
  and keep the current-user vote lookup separate. (§10 N+1/unbounded rule.)
- **L2. Read-path page size is only capped at DTO deserialization.**
  `fetch_reply_page` applies `per_page.max(1)` with no upper bound; the
  100-item cap lives in serde (`MAX_FORUM_READ_LIMIT`). Any future Rust
  caller of the owner list methods can request an unbounded page. Move
  the bound into the service, as the topic cursor path already does.
- **L3. Category counters leak restricted-content volume.**
  `topic_count` / `reply_count` are global aggregates that include
  audience-restricted discussions (documented in the README; the Next.js
  UI labels this, the Leptos surfaces should be checked for the same
  label). Prefer exposing the viewer-scoped count from the audience list
  owner wherever a number is presented as "topics you can open".
- **L4. First-publication detection parses its own event journal with
  hand-written per-backend SQL** (`was_published_before_in_tx`,
  `src/services/moderation_owner.rs`), duplicating event-type strings and
  payload keys (`payload->>'new_status'`) in Postgres and SQLite dialects.
  It is correct today, but a payload rename breaks it silently (a second
  `ForumTopicCreated` would be emitted). Derive it from the same typed
  event contract the publisher uses, or add a journal-backed contract test
  to the existing moderation suites.
- **L5. `ForumTopicCreated` on first publication picks the primary
  translation by `ORDER BY created_at ASC`** (moderation owner) rather
  than by the topic's recorded creation locale; with same-timestamp
  translations (imports) the emitted locale is nondeterministic. Persist
  the creation locale on the topic row or the journal entry instead.
- **L6. Settings failures are all-or-nothing.**
  `parse_forum_settings` rejects the whole settings document on one
  invalid field (`src/services/engagement_mode.rs`), so one bad saved
  value breaks every forum write for the tenant with a generic validation
  error. Validate at settings-write time (admin transport) and keep the
  read path total; surface *which* key is invalid.
- **L7. Posting cooldown compares against the author's last post of any
  status** (`src/services/posting_rate.rs`), including replies a
  moderator later rejected/deleted, and reads "latest" without a
  documented supporting index on `(tenant_id, author_id, created_at)`.
  Acceptable anti-spam semantics, but document the intent and verify the
  index exists (EXPLAIN evidence is already listed as pending for the Top
  indexes).
- **L8. Feed `Top` order ranks by approved reply count, not votes**
  (documented in `CRATE_API.md`). The name invites a votes-based
  expectation in both admin UIs; rename in UI copy ("Most discussed") or
  rank by the canonical vote score once L1 makes scores cheap.
- **L9. Verifier estate is mostly manual.** Only 13 of ~252
  `scripts/verify/verify-forum-*.mjs` gates are wired into CI (per the
  README's own accounting), and the README notes two workflows still
  reference a deleted source file and a non-existent cargo test target.
  Retire or repair those two workflows (CI-owner decision) and wire the
  boundary guards that protect H1–H4 class regressions
  (owner-boundary, subscription, read-state) into the forum CI path.
- **L10. Solution marking skips the author's own permission scope.**
  The GraphQL mutation requires only an authenticated context
  (`src/graphql/mutation.rs`, `mark_forum_topic_solution`), and the
  service's author path (`is_exact_topic_author`,
  `src/services/moderation_owner.rs:917`) checks identity but not that
  the author still holds any forum permission, unlike every other
  author self-service path, which goes through `enforce_owned_scope`.
  A user whose forum role was revoked keeps solution control over their
  old topics. Require `forum_topics:update` (Own) on the author path.

---

## Documented limitations confirmed (not new defects)

These are already recorded in `README.md` / `CRATE_API.md` and are listed
only so the product backlog owns them consciously: topic move has an owner
service but no transport/UI; topic revision history has services but no
API/UI; attachment relations have no transport/UI; user reports have no
REST transport or storefront action and no report rate limit; dedicated
approve/reject moderation transports do not exist (reopen/archive double
as approve/reject); Leptos storefront composer lacks tags, slug,
attachments, edit and pinned quote relations, and its feed controls only
request `Latest`; topic view counters do not exist; the widget preview
ignores `show_locked_topics_in_lists`; cross-category topic merges strand
source-topic subscriptions/read-states/votes on the tombstone until an
operator reconciliation; historical user-stat drift predating the current
write paths requires operator reconciliation (FORUM-33).

Priorities among these: report rate limiting (abuse vector), the Leptos
composer/edit parity (AGENTS §5 dual-implementation parity), and the
merge-stranded relations (silent data inconsistency for subscribers).

## What is already done well (do not regress)

- Tenant filters on every entity query inspected; composite
  `(tenant_id, id)` identity and DB-level tenant-integrity migrations.
- One lock order (category tree → topic → reply/solution scopes) reused
  by create, moderation, votes, subscriptions and read state; explicit
  comments record *why* at each site.
- Checked counter arithmetic that fails closed for category and topic
  reply counts; append-only relation revisions with in-transaction body
  revalidation for mentions/quotes; identical event UUID in outbox and
  forum journal.
- Audience authorization evaluated inside the write transaction for
  votes, subscriptions and topic/reply creation, with generic denials
  that do not leak existence.
- Cursor (keyset) pagination with context-bound cursors on topic/reply
  lists; no offset paging on hot paths.
- Transports never fall back across native/GraphQL paths on failure.

## Recommended remediation order

1. **P0 — H1, H4**: remove auto-subscribe triggers (consent-visible) and
   close the read-state existence oracle. Both are small, owner-local
   changes with existing in-repo patterns to copy.
2. **P0 — H2/H3 decision**: wire the digest and posting-policy behavior or
   remove it from the published contract; do not ship another release
   with inert controls.
3. **P1 — M1, M2, M3**: move moderation audience checks into the write
   transaction, add lifecycle/idempotency guards to pin/lock, freeze
   voting by topic status.
4. **P1 — M5, M6**: parent-status and re-moderation rules for replies and
   edits; reply optimistic concurrency.
5. **P2 — M4, M7, M8, L1**: typed subscription conflicts and revision
   exhaustion, exact user-stat decrements (plus scheduled FORUM-33
   reconciliation), constraint-based DB error mapping, SQL vote
   aggregation.
6. **P2 — L9**: repair/wire CI verifiers so the fixed boundaries stay
   fixed.

## Verification performed

- `node scripts/verify/verify-forum-admin-boundary.mjs` — pass.
- `node scripts/verify/verify-forum-storefront-boundary.mjs` — pass.
- `npm --prefix apps/next-frontend run test:forum` — 118/119 pass; the one
  failure is `src/shared/lib/graphql.test.mjs`, which fails with
  `ERR_MODULE_NOT_FOUND` because workspace `node_modules` are not
  installed in this environment; unrelated to forum logic.
- Rust checks were **not run**: `cargo` is not available in this
  environment. `cargo check -p rustok-forum`, `cargo test -p
  rustok-forum --lib`, and the SQLite/PostgreSQL integration suites under
  `crates/modules/rustok-forum/tests/` remain for the maintainer/CI, as
  required by `AGENTS.md` §15. Line-level findings above are from source
  inspection and should be confirmed by the scoped test run before the
  corresponding fixes are merged.

---

## Remediation status (2026-11-02, same branch)

Fixes were implemented on the `arena/fa9dd997-rustok` branch per the
remediation order. Rust compilation/tests could not be run (no Rust
toolchain in the review sandbox); Node static verifiers were run. Statuses:

**Fixed in code**
- **H1 (auto-subscribe triggers):** `forum_70_*` triggers removed from
  migration `m20260713_000013` (both backends); new migration
  `m20261102_000040` drops them on existing databases (irreversible
  cutover, documented). New Rust owner
  `services/subscription/auto_subscribe.rs` inserts implicit subscriptions
  with `ON CONFLICT DO NOTHING` inside the topic-create, approved
  reply-create, and moderation-approval transactions, honoring the tenant
  subscription policy (author = policy level, participants = policy level,
  revision 1). An explicit `Normal` (cleared) or `Muted` row now always
  survives. Imports, forks, splits, and restores deliberately do not
  auto-subscribe. Verifier `verify-forum-subscription-levels.mjs` and
  `tests/subscription_levels_contract.rs` updated to the new contract.
- **H4 (read-state audience bypass):** single-topic get/mark read state now
  goes through `ForumVisibilityScopedReadStateService` with the exact topic
  audience gate in both GraphQL and REST; missing and audience-denied
  topics are indistinguishable (`TopicNotFound`). The unaudited bulk
  `mark_scope_read` no longer creates read state for other users' Pending
  topics.
- **M1 (moderation audience race):** pin/lock/topic-status/reply-status
  now evaluate the moderation audience inside the write transaction under
  the category-tree lock (`require_topic_in_tx` / `require_reply_in_tx`).
  Solution mark/clear intentionally keeps the pre-transaction moderator
  check; its writes remain guarded by the solution scope lock and the
  in-transaction archived check (residual risk accepted, revisit if
  solution writes move under the category lock). Pin/lock additionally
  gained status guards (Archived → `TopicArchived`, Pending →
  `TopicAwaitingModeration`) and idempotent no-ops (no duplicate events).
  The solution author path is no longer identity-only: it requires the
  owned-update scope (`enforce_owned_scope`), so a revoked role loses
  solution control over old topics.
- **Vote status freeze:** votes on Pending topics are rejected
  (`TopicAwaitingModeration`); previously only Archived was rejected.
- **Reply parent/depth rules:** only Approved replies can be parents, and
  nesting is capped (`MAX_FORUM_REPLY_NESTING_DEPTH = 10`) with an
  in-transaction ancestor walk that also terminates on cyclic chains.
- **Edit re-moderation:** content edits of Open topics in pre-moderated
  tenants return the topic to Pending; content edits of Approved replies
  in moderated categories (or pre-moderated tenants) return the reply to
  Pending with the owner's counter/solution effects. Archived topics are
  immutable until restored. State machine gained `Open → Pending` and
  `Approved → Pending` re-moderation transitions.
- **Reply optimistic concurrency:** reply update now fences on the
  pre-transaction `updated_at` (concurrent edit → `TopicUpdateConflict`),
  mirroring the topic claim pattern.
- **Subscription revisions:** typed `SubscriptionRevisionConflict`
  (retryable, HTTP 409) and `SubscriptionRevisionExhausted` replace
  generic validation errors and the `saturating_add(1)` revision bump.
  Blind unsubscribe no longer pins the revision read outside the
  transaction, so a concurrent update cannot fail an unsubscribe.
- **User stats:** negative adjustments fail closed with a guarded update
  (previously `MAX(count + delta, 0)` silently clamped and hid drift),
  matching category/topic counters and the exact solution decrement.
- **H2/H3 (declared-but-unenforced surface):** resolved by
  `DECISIONS/2026-11-02-forum-remove-unwired-closing-and-digest-settings.md`:
  `allow_user_topic_closing` removed (zero-legacy precedent); effective
  Daily/Weekly digest selections rejected at the subscription write
  boundary and stored digest rows honored as "no immediate delivery" in
  the fan-out until the notifications owner ships batching. The posting
  policy evaluator remains a host-composed decision library (no tenant
  rule source exists inside the forum); README keeps stating this.
- **Verifier repair:** `verify-forum-read-state-runtime-proof.mjs` now
  reads `read_model_owner.rs` (was pointing at the retired `read_model.rs`
  and failing on a clean tree); moderation audience verifiers updated to
  the stronger in-transaction contract; stale single-line import marker in
  `verify-forum-topic-audience-visibility.mjs` fixed.

**Downgraded after re-inspection**
- **DB error classification (`error.rs`):** the matched strings are
  first-party trigger/check messages authored by forum migrations
  themselves (stable literals, not vendor-generated text), so
  substring mapping is not the locale/vendor fragility the finding
  assumed. Left as is; revisit if SQLSTATE-based mapping becomes
  available through the SeaORM error API.
- **FORUM-33 scheduling / vote histogram SQL aggregation / merge
  operation-id in DbErr mapping:** these remain with their owning
  platform trackers (FORUM-33 already tracks reconciliation execution;
  the histogram path is a diagnostic fallback; the merge conflict keeps
  its typed code with a nil operation id because `From<DbErr>` cannot
  know the operation). No code change in this pass.

**Pre-existing verifier failures (confirmed against clean HEAD)**
- `verify-forum-reply-create-audience-enforcement.mjs` (2) and the
  remaining 2 checks in `verify-forum-topic-audience-visibility.mjs`
  fail identically on unmodified `7fd5262`; they encode a stale
  composition contract and were left for their owning change.

**Verification performed for the fixes**
- `node scripts/verify/verify-forum-subscription-levels.mjs --static-only` — passed.
- `verify-forum-moderation-audience-policy`, `verify-forum-moderation-audience-transport-composition`,
  `verify-forum-read-state-runtime-proof`, `verify-forum-visibility-scoped-bulk-read`,
  `verify-forum-owner-read-visibility`, `verify-forum-notification-topic-subscription-audience`,
  `verify-forum-subscription-reconciliation-source`, `verify-forum-counter-reconciliation-source`,
  `verify-forum-admin-boundary`, `verify-forum-storefront-boundary`, `verify-forum-event-contract`,
  `verify-forum-runtime-baseline`, `verify-forum-owner-boundary`, `verify-forum-read-model` — passed.
- Rust: **not compiled** (no toolchain in sandbox) — `cargo check -p rustok-forum`
  and the forum test suites remain to be run by CI/maintainer.

## Remediation round 2 (2026-11-02)

**Verifier debt closed (all previously red checks now green, without
weakening the contracts):**
- `verify-forum-topic-audience-visibility`: the visibility owner now takes
  the topic row type from the policy owner (`ForumTopicModel` alias in
  `topic_audience.rs`) instead of referencing `crate::entities` directly;
  the verifier's policy-load marker follows the generic-connection
  signature `load_policy_for_topic(connection, tenant_id, topic)`.
- `verify-forum-reply-create-audience-enforcement`: the facade delegates
  the canonical authorization service into the owner transaction
  (stronger than the originally verified pre-check). The verifier now
  requires that delegation, requires `require_in_tx` after the audience
  scope lock and before the first reply write, and forbids raw owners
  from evaluating or loading audience policy themselves.

**Logic round 2:**
- **L1 (vote aggregation):** `topic_vote_summaries` / `reply_vote_summaries`
  now aggregate with `SUM(value) ... GROUP BY` in SQL and fetch the
  current user's votes in a separate bounded query, instead of loading
  every vote row into memory. SQL `SUM` widens to `i64`; the public
  `i32` score saturates via `clamp_vote_score` instead of wrapping.
- **L2 (page bound):** `fetch_reply_page` clamps `per_page` to
  `1..=MAX_FORUM_READ_LIMIT` at the owner boundary, not only at DTO
  deserialization.
- **Solution mark/clear audience:** the moderator audience decision is
  now evaluated inside the write transaction under the category-tree
  lock (`require_topic_in_tx`), matching pin/lock/topic-status/
  reply-status. The residual pre-transaction check noted in round 1 is
  closed; the author path still requires the owned-update scope.
- **L6 (settings all-or-nothing) — deliberately unchanged:** typed
  configuration errors are the platform's endorsed stance (see the
  `default_topic_sort` precedent: invalid saved values need explicit
  operator correction, not silent remapping). Per-field salvage would
  silently ignore admin typos. `parse_forum_settings` keeps failing
  closed with the serde field path in the message.

**Regression tests (round 2):**
`tests/logic_review_remediation_sqlite.rs` covers: cleared/muted
subscriptions surviving later approved replies, daily/weekly digest
rejection, pending parents refusing children plus the nesting depth
cap, approved-reply edits returning to pending in a moderated category,
and pending topics rejecting votes. These tests require a Rust
toolchain and run in CI with the rest of the forum suite.

**Still open:** FORUM-33 execution scheduling (platform tracker),
first-publication journal SQL duplication (L4) and creation-locale
determinism (L5) — both correct today; they need a journal-contract
change owned by the moderation/eventing track.
