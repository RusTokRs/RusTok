# Forum vote policy settings are enforced on every internal vote write

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-forum` (`VoteService`, `ForumSettingsProviders`, `ForumModuleSettings`); `rustok-tenant` (static module settings rows read through `StaticModuleSettingsTransactionReader`)
- Extends: `DECISIONS/2026-10-09-forum-vote-subscription-write-audience.md`
- Supersedes: None
- Superseded by: None

## Context

`rustok-module.toml` and the Forum admin settings form declare two vote flags:

- `allow_downvotes` (default `true`): allow downvotes on topics and replies, with "upvote-only" as the
  stated use case.
- `allow_self_voting` (default `false`): allow users to vote on their own topics or replies.

At the base commit neither flag was read at runtime. Downvotes were always accepted. Self-votes were
always refused, which was the rule set by the vote and subscription write-audience decision. Tenants that
set either flag saw no effect, so the admin control promised behaviour that the runtime did not provide.

## Decision

`VoteService` reads `ForumModuleSettings` inside the same transaction as the vote write, using the
settings reader that already decides between internal votes and Reactions. Two rules follow from the
values it reads:

1. When `allow_downvotes` is `false`, a write with value `-1` on a topic or a reply returns
   `ForumError::Validation`. Writes with value `1` and clears are unaffected. An existing `-1` vote remains
   in the score until its author changes or clears it.
2. When `allow_self_voting` is `false` (the default), an author who votes on their own topic or reply
   returns `ForumError::Forbidden`, as before. When it is `true`, the author vote is recorded.

The defaults equal the behaviour the code already had, so a tenant without an explicit value sees no change.
The same settings helper now backs both the Reactions mode check and the vote policy, so the module settings
are parsed in one place.

## Sources of truth and ownership

- `ForumModuleSettings` (`crates/modules/rustok-forum/src/dto/settings.rs`) is the typed contract for the flags.
- The tenant's stored `forum` module settings row is the source of truth. It is read through the
  `StaticModuleSettingsTransactionReader` supplied by the host.
- `VoteService` is the only owner of the vote write rules. The transports only map errors.

## Invariants

- The downvote rule and the self-vote rule are evaluated on the server for every internal vote write. The UI
  is not a source of enforcement.
- The vote policy is read in the write transaction. A disabled Forum module, a missing settings row, or a
  missing transactional reader yields the default settings.
- Clearing a vote is never blocked by these flags.
- The audience gate of the parent topic (F2) still runs before the policy check, so a restricted topic
  never reveals whether downvotes are enabled.
- Invalid `ForumModuleSettings` JSON fails the write with the same validation error the Reactions check
  already returns. This adds no new failure mode for vote writes.

## Non-goals

- `vote_undo_window_minutes` is removed by `DECISIONS/2026-10-09-forum-remove-vote-undo-window-setting.md`. A vote
  time window needs a new decision on whether it applies to clears, changes, or both.
- Read-side hiding of downvote counts, retroactive removal of existing downvotes, and rate limits for votes.
- Exposing `allow_self_voting` in the admin form. It is manifest-only in this change.

## Data, transaction, and concurrency boundary

The policy is read with `settings_in_tx` inside the vote transaction, after the Reactions check and before
the vote row lock. The settings row is not locked. A settings change that commits concurrently with a vote
write is seen by the next write, which matches the existing Reactions check. No schema change is needed.

## Context dimensions

- Tenant: settings are per tenant. Every read uses the request tenant.
- Channel and locale: not used by this policy.
- Policy: tenant vote policy for the write only. The audience gate remains the access decision.
- Auth: the write requires an authenticated user, as before.

## Events and projections

No new events. The existing topic projection publish in the vote transaction is unchanged.

## Failure semantics

- Static settings unavailable: the write fails with the existing capability failure from
  `map_port_error`. The write fails closed.
- Invalid settings JSON: the write fails with `ForumError::Validation`.
- Downvote rejected: `ForumError::Validation`, which is HTTP 400 and a GraphQL validation error.
- Self vote refused: `ForumError::Forbidden`, which is HTTP 403.

## Migration and cutover

- Tenants with no explicit value: no change.
- Tenants that already stored `allow_downvotes: false`: downvotes start to fail. This takes effect at
  deploy, because the setting had no runtime effect before.
- Tenants that already stored `allow_self_voting: true`: authors can now vote on their own content. This is
  also a change at deploy.

Operators should review stored Forum settings before this change ships. No data migration is needed.

## Alternatives considered

- Remove both flags from the manifest and the admin form. Rejected: upvote-only communities are a real
  configuration, and the admin control already exists.
- Hide the downvote control in the client only. Rejected: the server must enforce the policy.
- Keep self-votes forbidden without a setting. Rejected for this change: the manifest already names a
  tenant opt-in, and the default still keeps the F2 rule.

## Verification

- `crates/modules/rustok-forum/tests/votes.rs`: `downvotes_are_rejected_only_while_the_tenant_disables_them`
  and `self_votes_are_forbidden_by_default_and_allowed_by_the_tenant_setting`. These run against SQLite in
  the same file as the existing vote tests.
- The Rust toolchain is not installed in this environment, so the tests have not been compiled or run.
  They must be run with `cargo test -p rustok-forum --test votes` before merge.

## Consequences

- The F2 invariant "a user can no longer vote on their own content" holds by default. A tenant can opt in
  with `allow_self_voting`.
- `allow_downvotes` now matches what the admin form shows.
- Follow-up work: rate limits for votes, and the remaining unread Forum settings. The undo window is removed.
