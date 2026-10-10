# Forum enforces the author edit window and hides locked topics from storefront lists

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-forum` (`services/edit_window.rs`, topic and reply update paths in `services/topic_inline.rs` and `services/reply_inline.rs`, storefront list in `services/topic_visibility_list.rs`, `rustok-module.toml`, `README.md`)
- Extends: `DECISIONS/2026-10-09-forum-remove-vote-undo-window-setting.md`, `DECISIONS/2026-10-09-forum-soft-default-settings.md`
- Supersedes: None
- Superseded by: None

## Context

Two unread `ForumModuleSettings` fields have a clear, local meaning and no new substrate:

- `max_edit_window_minutes` (default `0`, unlimited) is declared as the maximum time after posting during which
  authors may edit their post. No runtime path reads it.
- `show_locked_topics_in_lists` (default `true`) controls whether locked topics appear in lists. No runtime path reads it.

The other unread fields need new behaviour (views, reports, scheduling, pre-moderation, closing rights) or a semantic
decision (`edit_grace_period_minutes` needs an edited marker, which the model does not have). They are not part of
this decision.

## Decision

1. **Author edit window.** Topic and reply update commands reject an author update after
   `created_at + max_edit_window_minutes`. The check applies only when `max_edit_window_minutes > 0`, the caller is
   the item's author (the caller's user id equals `author_id`), and the caller does not hold the moderation scope
   `All` for the resource. Moderators and administrators are not limited. The author test uses ownership, not the
   update scope. Staff roles hold `All` and the forum author role holds `Own` (see
   `DECISIONS/2026-10-09-forum-author-self-service-and-deletion.md`); this window rule uses ownership so it holds for
   both. The ownership check in `enforce_owned_scope` is unchanged.
   The check covers every field of the update command (title, body, tags, metadata, quotes), because the command is
   the author's edit surface. It returns `ForumError::Validation` with the message "Forum edit window has closed for this author". The
   response is HTTP 400 with `FORUM_VALIDATION_FAILED`. A validation error is used rather than `Forbidden` because the
   HTTP mapping of `Forbidden` replaces the message with "Permission denied", and the author needs the reason.
2. **Locked topics in storefront lists.** When `show_locked_topics_in_lists` is `false`, the storefront topic list
   filters out topics with `is_locked = true`. The filter applies in the single storefront list query, so the public,
   authenticated storefront, and facade paths share it. Owner and moderator lists are not filtered.
3. The settings are read through `ForumSettingsProviders::module_settings` (non-transactional) at the start of each
   command. Defaults keep the current behaviour: an unlimited window and locked topics shown.

## Invariants

- The window check runs after the owner-scope check and before any write. A rejected update writes nothing.
- A created-at in the future counts as zero elapsed time, so the window is not closed by clock skew.
- The storefront filter does not change the cursor semantics. It removes rows before the keyset page is cut.

## Non-goals

- `edit_grace_period_minutes`. It needs an edited marker, which the model does not have. It stays unread and needs its
  own decision.
- Locked topics in the widget preview (`topic_widget_preview.rs`). The preview is a separate read path and keeps its
  own list rules. It is listed in the README as a gap.
- Closing rights for authors (`allow_user_topic_closing`). That changes the moderation authorisation and needs its own
  decision.
- Archived topics. The storefront list already excludes every status other than `Open`, so the setting does not
  change them.

## Data, transaction, and concurrency boundary

- No schema change. One settings read per update command and one per storefront list call, both outside the write
  transaction.
- A settings change committed during a command applies to the next command.

## Context dimensions

- Tenant: the settings and the window are per tenant.
- Author: the window applies to the caller's own items, identified by `author_id`, to callers without moderation scope `All`.
- Locale, channel: the list filter does not depend on them. The channel filter already applies in the same query.
- Policy and auth: scope decides who is limited. No new policy evaluator is involved.

## Events and projections

No events and no projections.

## Failure semantics

- A closed window returns `FORUM_VALIDATION_FAILED` (HTTP 400) with the reason. The item is unchanged.
- A failed settings read fails the command with the same error as other settings reads.

## Sources of truth and ownership

- `ForumModuleSettings` in `dto/settings.rs` and `rustok-module.toml` declare both fields.
- `services/edit_window.rs` is the only place that applies the window. `services/topic_visibility_list.rs` is the only
  place that applies the locked-topic filter to storefront lists.

## Migration and cutover

- No data migration. Tenants keep stored values. A tenant that stored a window will see it take effect at the next
  author update.
- Tenants with `show_locked_topics_in_lists = false` stored will see locked topics disappear from storefront lists at
  the next request.

## Alternatives considered

- Apply the window to the moderation path too. Rejected: moderators are exempt by design, and the window is an author
  rule.
- Apply the locked-topic filter in the owner list. Rejected: the setting is about what visitors see in lists.
- Implement `edit_grace_period_minutes` with the window. Rejected: it needs an edited marker, which does not exist.

## Verification

- Unit tests in `services/edit_window.rs`: inside the window, after the window, exactly at the limit, a window of `0`,
  a moderator who is also the author, a different user, and a creation time in the future.
- Integration tests in `tests/posting_limits_sqlite.rs`: an author update after the window is rejected and leaves the row
  unchanged; a moderator update after the window succeeds; a window of `0` allows edits; the storefront list hides a
  locked topic when the setting is `false` and shows it when `true`. Creation times are moved into the past with the
  entity model.
- Neither set of tests has been run: the Rust toolchain is not installed in this workspace.
- The Rust toolchain is not installed in this workspace, so no build or test has run. Run `cargo test -p rustok-forum`
  before merge.

## Consequences

- Two settings take effect. Defaults do not change behaviour.
- The README lists the remaining unread settings with the reason each one is not wired.
