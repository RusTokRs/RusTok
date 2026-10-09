# Forum topic pre-moderation holds new topics with a `Pending` status

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-forum` (`state_machine.rs`, `constants.rs`, `services/topic_inline.rs`, `services/moderation_owner.rs`, `services/pending_visibility.rs`, `services/topic_audience_visibility.rs`, `services/reply.rs`, `services/reply_audience_read.rs`, `services/reply_owner_inline.rs`, `error.rs`, `migrations/m20261009_000038_allow_pending_forum_topic_status.rs`), `rustok-content-orchestration` (`bridge/promote.rs`), `apps/next-admin` (`module-settings-fields/forum.tsx`)
- Extends: `DECISIONS/2026-10-09-forum-reply-pre-moderation-setting.md`
- Supersedes: Item 3 of `DECISIONS/2026-10-09-forum-reply-pre-moderation-setting.md` (topics are not held). The rest of that decision stands.
- Superseded by: None

## Context

- `pre_moderation_enabled` holds new replies (see the reply pre-moderation decision). Its manifest description said that
  topics and replies are held, while the code held replies only. The admin form said topics were not held yet.
- `TopicStatus` has `Open`, `Closed`, and `Archived`. It has no pending state. The database enforces the same list:
  the `chk_forum_topics_status` check (PostgreSQL) and the `forum_topics_status_insert` and `forum_topics_status_update`
  triggers (SQLite), both from `m20260712_000004_enforce_forum_status_lifecycle`. A row with `pending` cannot be written
  until that list changes.
- The per-category flag `moderated` holds replies only. Its documented meaning is the reply rule, and the existing
  forum tests rely on topics in moderated categories being `Open`. Changing it would change those tests and every
  moderated category in production, so it is not changed here.

## Decision

1. **A new status `Pending` for topics.** `TopicStatus::Pending` is stored as `pending`. Migration
   `m20261009_000038_allow_pending_forum_topic_status` adds `pending` to the PostgreSQL check and recreates the SQLite
   triggers with `pending` allowed. The migration's down step refuses to run while any topic has `pending`.
2. **The tenant setting holds topics.** A new topic is `Pending` when `pre_moderation_enabled` is `true`, read with
   `module_settings_in_tx` inside the create transaction. The per-category `moderated` flag does not hold topics.
   Every author is held, including staff, as for replies.
3. **Approval and rejection use the existing transitions.** `Pending` to `Open` is the reopen transition, and
   `Pending` to `Archived` is the archive transition. Both run through `ModerationService`, which requires
   `ForumTopics:Moderate` before the transition. No `approve_topic` or `reject_topic` is added. Every other transition
   from `Pending` is refused by `TopicStatus::validate_transition`.
4. **Visibility.** A pending topic is visible only to its author and to viewers with `ForumTopics:Moderate` at scope
   `All`. The rule is `pending_topic_visible` in `topic_audience_visibility.rs`, which the owner audience read calls.
   Storefront topic reads return only `Open` topics, and storefront search reads only `open` topics through
   `forum_visible_status`.
5. **Replies to a pending topic are refused.** Reply creation in `reply_owner_inline.rs` returns
   `ForumError::TopicAwaitingModeration`, code `FORUM_TOPIC_AWAITING_MODERATION`, which maps to HTTP 409.
6. **Pending content is not promoted.** `rustok-content-orchestration` refuses to promote a pending topic to a Blog
   post, with a validation error.
7. **Topic reads and lists filter pending topics.** `topic_pending_condition` applies to every topic list that does not
   go through the audience path, and `get_with_canonical_resolution_and_locale_fallback` returns `TopicNotFound` for a
   pending topic that the caller may not see. The SEO bulk summary list reads with `SecurityContext::public_read()`
   instead of the system context. The system context holds `ForumTopics:Moderate`, so it listed held topics. The public
   read context also applies the public hidden-category rule, which the storefront already uses.
8. **Reply lists filter pending replies by row.** `reply_pending_condition` applies in `fetch_reply_page` and in every
   reply list. Before this decision, any caller with `ForumReplies:List` could read other authors' pending replies
   through the list path. Now only the reply's author and a moderator see them.

Default value stays `false`.

## Sources of truth and ownership

- The status, its transitions, and the pending visibility rule are owned by `rustok-forum`. The database constraint is
  owned by the forum migrations.
- Approval and rejection go through the forum moderation owner. This decision does not move them to
  `rustok-moderation`. Holding a topic is not a moderation case, and the moderation reports and cases remain in
  `rustok-moderation`.
- The `pre_moderation_enabled` value is owned by the forum module settings contract (`dto/settings.rs`,
  `rustok-module.toml`). The admin form only edits it.

## Invariants

- A topic is `Pending` at insert if and only if the tenant setting is `true` at that moment. The status is set in the
  same transaction as the insert.
- No transition leads into `Pending`. Only creation sets it.
- A `Pending` topic has no reply that a non-moderator can read through an owner or storefront path.
- Only a `Pending` to `Open` or `Pending` to `Archived` transition is possible from `Pending`.
- The author cannot approve their own pending topic. Approval requires `ForumTopics:Moderate`, which customers do not
  hold.

## Non-goals

- A dedicated approve or reject transport for topics. Moderators use the existing reopen and archive transports.
- Holding topics in moderated categories through the category flag. That needs a separate decision and test changes.
- Notifying subscribers when a held topic is approved. Sending a notification at approval is a separate change.
- An exemption for moderators or trusted authors.

## Data, transaction, and concurrency boundary

- The status is decided in the create transaction, from the same transactional settings read as the insert.
- The migration changes a check constraint and triggers. It does not rewrite rows. Existing rows keep their statuses.
- Approval runs in the same transaction as the status update, the status-changed event, and the projection publish, as
  for other topic transitions.

## Context dimensions

- Tenant: the setting is per tenant.
- Category: the category flag is read as before and does not affect topics.
- Actor: the author sees the topic, a moderator sees and approves it, and any other user cannot see it.
- Locale and channel: the status check is independent of locale and channel. Storefront channel filters apply to
  `Open` topics only.

## Events and projections

- `ForumTopicCreated` is still published for a pending topic. The search projection reads each topic through the
  storefront-visible read, which requires an `Open` topic. A pending topic therefore yields no projection document, and
  nothing is indexed until the topic is approved. Approval runs `publish_forum_topic_projection_in_tx`, which indexes it.
- Approval publishes `ForumTopicStatusChanged` with `pending` as the old status and `open` as the new status.

## Failure semantics

- A settings read error fails the create. The topic is not inserted with a guessed status.
- A transition from `Pending` to any other status fails with the state-machine error and changes nothing.
- A reply to a pending topic fails with `FORUM_TOPIC_AWAITING_MODERATION` and inserts nothing.
- If the migration is not applied, creating a pending topic fails at the database check. The migration must run before
  the new code serves writes.

## Migration and cutover

- Migration `m20261009_000038_allow_pending_forum_topic_status` runs after `m20260712_000004`. It is registered in
  `migrations/mod.rs` with its dependency.
- Deploy order: run migrations first, then the new server. Setting the value back to `false` stops new holds. Pending
  topics stay `Pending` until a moderator acts on them. To roll back the migration, approve or reject every pending topic
  first, because the down step refuses while any topic is `pending`.
- Tenants with `pre_moderation_enabled: true` start holding topics after deploy. Operators should check the setting
  before deploy.

## Alternatives considered

- **Hold topics through the category flag.** Rejected for this change. It changes existing moderated categories and
  the forum tests that depend on them, and the flag is documented as a reply rule.
- **`approve_topic` and `reject_topic` transports.** Deferred. The existing reopen and archive transitions already
  enforce the moderation scope.

## Verification

- Unit tests in `state_machine.rs`: `pending_topic_transitions_only_approve_or_reject`, and the extended
  `topic_status_roundtrip`.
- Integration tests in `crates/modules/rustok-forum/tests/posting_limits_sqlite.rs`:
  `pre_moderation_holds_new_topics_until_a_moderator_approves_them` and
  `held_replies_are_listed_only_to_their_author_and_moderators`.
- `update_only_role_edits_its_own_topic_and_never_deletes_it` in `tests/rbac.rs` covers a role that holds only
  `Update`.
- Not yet run. The build and tests were not run in this session, because the toolchain is not available here.
- Static checks made by reading the code: `evaluate_topic` requires `ForumTopics:Moderate` before a reopen or archive,
  and `Customer` does not hold that permission. Each include in `services/mod.rs` shares one module, so the new `use`
  lines were checked for duplicates.
- Gate: `scripts/verify/verify-adrs.mjs` and `scripts/verify/verify-remediation-gate.py` before commit.

## Consequences

- A tenant that turns the setting on gets held topics and replies, with one visibility rule for both.
- Pending topics appear in owner and moderator reads only. The storefront topic and search reads do not show them.
- The SEO bulk summary list now excludes pending topics and restricted categories, because it reads as the public.
- SEO reads of a single topic follow the path, so a held topic gets no public metadata. The public route uses
  `get_public_storefront_visible_with_locale_fallback`, which requires an `Open` topic. The sitemap uses the public
  audience list and also requires `Open`. The system context in `load_target` is used only for the `Authoring` scope,
  which is for editors, and moderators see held topics anyway. The bulk summary re-reads each item with the system
  context after the public list has already excluded held topics. No race exposes held content, because no transition
  leads into `Pending` after creation, so a topic that is listed as public cannot become held between the two reads.
- A held topic does not notify subscribers when it is approved. This is a known gap.
- The reply list path changes for every caller. A non-moderator no longer sees other authors' pending replies.
