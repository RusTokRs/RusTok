# Forum enforces the topic and reply posting intervals per author

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-forum` (`services/posting_rate.rs`, `services/posting_rate` call sites in topic and reply create, `error.rs`, `controllers/mod.rs`); `apps/next-admin` is not changed in this decision
- Extends: `DECISIONS/2026-10-09-forum-soft-default-settings.md`, `DECISIONS/2026-10-09-forum-content-length-enforcement.md`
- Supersedes: None
- Superseded by: None

## Context

`ForumModuleSettings` declares `rate_limit_new_topic_seconds` (default 10) and `rate_limit_new_reply_seconds`
(default 5). Before this decision, no runtime path read them. The only protection was the global `/api/` limiter,
which is not per author and not per content kind. The posting policy evaluator (`posting_policy_evaluator.rs`)
has window-count rules, but it is not wired into any write path and its rules are not read from module settings.

## Decision

1. Each setting is a flat cooldown between two consecutive creates by the same author in the same tenant:
   - a new topic is rejected while the author's most recent topic in the tenant is younger than
     `rate_limit_new_topic_seconds`;
   - a new reply is rejected while the author's most recent reply in the tenant is younger than
     `rate_limit_new_reply_seconds`.
   Topic and reply cooldowns are independent. A topic does not start the reply cooldown, and a reply does not start
   the topic cooldown.
2. A value of `0` disables that cooldown.
3. A rejected create fails with `ForumError::RateLimited { retry_after_seconds }`. The stable code is
   `FORUM_RATE_LIMITED`. REST returns `429 Too Many Requests`. GraphQL returns the stable code, and the message does not
   include the wait time. `retry_after_seconds` is available on the typed error inside the Rust services.
4. The age of the author's last create counts from `created_at` of that row, including rows later moderated, pending,
   or deleted. Deleting a post does not reset the cooldown. This matches the cost of the create.
5. Only user actors (`SecurityActorKind::User`) are rate limited. System and service actors are trusted internal
   writers, including the starter import, which records a user id for attribution through `SecurityContext::system()`.
   A user actor without a user id is rejected with `Forbidden` rather than exempted.
6. Imports, exact translation apply, and moderation writes do not run the check. They do not call the create methods.

## Invariants

- The check runs inside the create transaction, after `lock_category_tree_in_tx`. That lock already serialises every
  Forum create in a tenant, so two concurrent creates by one author cannot both pass the check.
- The check reads the author's latest row with `ORDER BY created_at DESC LIMIT 1` on the tenant and author. The
  timestamp is compared with `Utc::now()`.
- A `created_at` in the future counts as zero elapsed seconds and is rejected for the full interval. Clock skew
  fails closed.
- The actor kind is decided by `SecurityContext::actor_kind`, not by whether `user_id` is set. Only `User` is limited.
  `Public` actors have no create permission, so `enforce_scope(..., Create)` rejects them before the check runs.
  `Service` and `System` actors are never limited, even when they carry a `user_id`. Starter import uses
  `SecurityContext::system()`, so it is exempt. The starter driver (`rustok-starter/drivers/forum.rs`) also runs as
  `System` and is exempt. The driver calls `TopicService::new` and `ReplyService::new` without providers, so its
  length checks use the module defaults, not stored tenant values.
- Authorisation runs before the cooldown check, so a request that fails authorisation does not start or extend a
  cooldown.

## Non-goals

- No window-count limits (for example, N topics per hour). The posting policy evaluator owns that shape, and it is
  not wired yet.
- No per-category or per-channel limits.
- No trust-level exemptions. Staff bypass is not implemented, so every author is subject to the same interval.
- No rate limit for topic bumps or edits. `edit_grace_period_minutes` and `max_edit_window_minutes` are separate
  settings and are not enforced here.
- No admin form fields. The Forum admin form does not expose the rate-limit settings yet; they are set through the
  settings JSON.

## Data, transaction, and concurrency boundary

- One indexed read per create (latest author row in the tenant) inside the write transaction. No new writes and no
  new locks.
- The limit values come from the non-transactional settings read done before the transaction (see the content-length
  decision). A settings change that commits during a create applies to the next create.

## Context dimensions

- Tenant: the interval is per tenant. The author lookup is scoped to the tenant.
- Author: the interval is per `security.user_id`.
- Locale, channel, policy, auth: not used. Authorisation runs before the rate check, so a forbidden request does not
  start a cooldown.

## Events and projections

No new events and no projections.

## Failure semantics

- A rejected create writes nothing. The transaction is rolled back, and the error carries the remaining wait in seconds.
- A database error during the lookup fails the create with the same error as other reads.
- `FORUM_RATE_LIMITED` is not marked retryable. Callers must wait the interval.

## Sources of truth and ownership

- `ForumModuleSettings` in `crates/modules/rustok-forum/src/dto/settings.rs` and `rustok-module.toml` declare the
  intervals.
- `crates/modules/rustok-forum/src/services/posting_rate.rs` is the only place that applies them.

## Migration and cutover

- No data migration. Existing posts count toward the cooldown from their stored `created_at`. An author who posted in
  the last interval before the deployment waits for the remaining time.
- Tenants with a stored value keep it. Tenants without a stored value receive the soft defaults (10 and 5 seconds).

## Alternatives considered

- Count-based windows through `ForumPostingPolicyRules.topic_create_limit`. Rejected for this change: the evaluator
  is not wired, its facts ports are not in the write path, and the module settings do not express count windows.
  When the evaluator is wired, its window limits must be composed with these cooldowns. That composition is a follow-up
  decision.
- A cache or in-memory limiter. Rejected: it would not survive restarts or multiple nodes, and the database row is
  already the source of truth.
- Rate-limit by IP at the `/api/` layer. Already present as the global limiter. It does not express a per-author
  interval.

## Verification

- Unit tests in `services/posting_rate.rs`: no previous post, inside the interval, at the interval, and a future
  timestamp.
- Unit tests in `services/posting_rate.rs` for the actor-kind rule: a `System` actor with a `user_id` is not limited,
  a `User` actor is limited by its id, a `User` actor without an id is rejected, and a zero interval disables the check
  for every actor.
- Integration tests in `tests/posting_limits_sqlite.rs`: topic cooldown per author with a second author allowed, and
  reply cooldown independent of the topic cooldown.
- Every other integration test that creates several posts by one author uses the test-only providers that set both
  cooldowns to `0`: `tests/support/posting_cooldown.rs` (integration tests) and
  `services/engagement_mode.rs::providers_without_posting_cooldown` (crate unit tests). The helper is wired only into
  tests that are not about the cooldown. `tests/votes.rs` keeps the stored settings and adds `0` cooldowns on top, so
  its vote summaries read the same flags as `VoteService`. `tests/posting_limits_sqlite.rs` sets the cooldowns to `0`
  in its settings JSON.
- The Rust toolchain is not installed in this workspace, so no test has run. Run `cargo test -p rustok-forum` before
  merge.

## Consequences

- Topic and reply creates now return `FORUM_RATE_LIMITED` (HTTP 429) inside the cooldown. Clients that post in quick
  succession will see this error for the first time.
- The two rate-limit settings have runtime effect.
- The posting policy evaluator remains unwired. Composing its window rules with these cooldowns is follow-up work.
- Imports (`import_write`, `import_tombstone_write`) do not run the cooldown and do not enforce the content-length
  limits. Their existing shape, locale, timestamp, and tag validation is unchanged. This exception is deliberate: an
  import batch replays historical records, and the cooldown would reject it.
- The cooldown tests in `tests/posting_limits_sqlite.rs` construct the services without the helper, so they run against
  the module defaults. The helper is not used for them.
