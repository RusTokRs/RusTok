# Forum module defaults are soft: they never restrict a user beyond the common platform norm

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-forum` (`ForumModuleSettings` defaults, `rustok-module.toml`); `apps/next-admin` (Forum settings fields in `module-settings-fields/forum.tsx`)
- Extends: `DECISIONS/2026-10-09-forum-vote-policy-settings.md`
- Supersedes: None
- Superseded by: None

## Context

Most Forum module settings are declared in `rustok-module.toml` and read through `ForumModuleSettings`. The
audit found that only a few of them have runtime effect. Before the remaining settings are enforced, their
defaults must be decided. Enforcing the current defaults would immediately restrict every tenant that has
not set a value, for example by rejecting titles shorter than 5 characters or posts shorter than 10.

The owner asked that every default be soft, and asked to take the defaults from established platforms.

## Decision

A default is soft when it does not restrict a legitimate user beyond the norm that common forum platforms ship
with. A default for a permission-granting flag is the least privileged state. Each changed default below
follows this rule, and the sources are the platform defaults checked on 2026-10-09.

| Setting | Old default | New default | Source of the norm |
| --- | --- | --- | --- |
| `min_topic_title_length` | 5 | 1 | NodeBB `minimumTitleLength` is 0 in `install/data/defaults.json`. Discourse ships 15. The new value rejects only empty titles, which the core already rejects. |
| `max_topic_title_length` | 150 | 255 | NodeBB `maximumTitleLength` is 255. Flarum allows 80 by default. |
| `min_post_body_length` | 10 | 1 | NodeBB `minimumPostLength` is 8 and Discourse ships 20. The new value rejects only empty bodies. |
| `max_post_body_length` | 30000 | 60000 | phpBB default maximum is 60,000 characters. NodeBB ships 32767. |
| `rate_limit_new_topic_seconds` | 60 | 10 | NodeBB `postDelay` is 10 seconds. Discourse ships 15 seconds, phpBB's flood interval is 15 seconds. |
| `rate_limit_new_reply_seconds` | 15 | 5 | Discourse ships 5 seconds between posts. |
| `vote_undo_window_minutes` | 10 | 0 (no limit) | NodeBB `undoTimeout` is 0. |
| `auto_flag_threshold` | 3 | 0 (disabled) | NodeBB `flags:autoFlagOnDownvoteThreshold` is 0. No report pipeline exists yet, so automatic hiding has no basis. |

Defaults that already matched the rule stay as they are: `pre_moderation_enabled` false, `allow_anonymous_reading`
true, `allow_downvotes` true, `allow_self_voting` false, `allow_user_topic_closing` false, `edit_grace_period_minutes` 5,
`max_edit_window_minutes` 0 (unlimited, NodeBB `postEditDuration` 0), `topics_per_page` and `replies_per_page` 20,
`default_topic_sort` `latest_reply`, `use_reactions` false, `auto_close_inactive_days` 0, `show_locked_topics_in_lists`
true.

`allow_self_voting` and `allow_user_topic_closing` default to off. They grant an ability, so the least privileged
value is the soft one. Turning them off does not restrict anyone who already has the default behaviour.

## Invariants

- The defaults in `ForumModuleSettings::default()`, the serde default functions, `rustok-module.toml`, and the admin
  fallback values in `forum.tsx` are the same numbers. A change to one must change all four.
- A default never blocks content that the platform norm would accept.
- A stored settings value overrides the default. A default never overrides a stored value.

## Non-goals

- No new enforcement in this change. The defaults do not change any runtime behaviour by themselves. Length
  enforcement is decided in `DECISIONS/2026-10-09-forum-content-length-enforcement.md`, and rate-limit enforcement in
  `DECISIONS/2026-10-09-forum-posting-rate-limits.md`. The vote undo window and auto-flag remain unenforced and need
  their own decisions.
- No data migration. Existing tenants that saved the admin form keep the explicit values in their settings row.
  Operators who want the new defaults must set them.

## Data, transaction, and concurrency boundary

No new reads or writes. The defaults are pure values.

## Context dimensions

- Tenant: the defaults apply to every tenant that has no stored value for the key.
- Channel, locale, policy, auth: not used.

## Events and projections

No new events and no projections.

## Failure semantics

Unchanged. A missing key falls back to the default through `#[serde(default)]`. An invalid stored value still fails
the settings parse as before.

## Sources of truth and ownership

- `ForumModuleSettings` in `crates/modules/rustok-forum/src/dto/settings.rs` is the typed contract.
- `crates/modules/rustok-forum/rustok-module.toml` is the declared manifest contract.
- `apps/next-admin/.../forum.tsx` only displays and edits the values.

## Migration and cutover

- Tenants with no stored value for a key: the new default applies once the key is enforced.
- Tenants with a stored value: no change.
- Admin users who saved the form before this change keep their saved values. Review them if the old limits were
  saved without intent.

## Alternatives considered

- Keep the old defaults and enforce them. Rejected: they restrict users who never configured anything.
- Set the length minimums to 0. Rejected: a minimum of 0 means no minimum. A minimum of 1 is an explicit floor that
  the admin form can show, and it still accepts any non-empty text.
- Disable the topic and reply rate limits by default. Rejected: every surveyed platform ships a short flood limit,
  so 10 and 5 seconds keep the same protection with the least friction.

## Verification

- Source-level check of the changed values in `settings.rs`, `rustok-module.toml`, and `forum.tsx`.
- The Rust toolchain is not installed here, so no build or test has run. Run `cargo test -p rustok-forum` before merge.

## Consequences

- New tenants and tenants without stored values get the soft defaults when the settings are enforced.
- The admin form shows the same numbers as the manifest.
- Follow-up work: enforce the vote undo window and auto-flag, each with its own decision and tests. Title and body
  length limits and the posting rate limits are enforced by their own decisions.
- A topic title or reply body that has no text, for example only an image, is rejected. The body length check counts
  plain text, and the minimum is 1 character. Authors must add text. This is a known consequence of the minimum,
  not a separate media rule. Image-aware bodies need their own decision.
- Imports keep their existing shape and tag checks. They do not apply the length limits or the posting cooldown. See
  `DECISIONS/2026-10-09-forum-posting-rate-limits.md` and `DECISIONS/2026-10-09-forum-content-length-enforcement.md`.
- `auto_flag_threshold = 0` disables auto-flagging, and no flag pipeline exists yet, so the old value of 3 did not take
  effect either. `vote_undo_window_minutes` is removed by
  `DECISIONS/2026-10-09-forum-remove-vote-undo-window-setting.md`, so its default row no longer applies.
