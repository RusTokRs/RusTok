# Forum removes module settings that no runtime code reads

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-forum` (`dto/settings.rs`, `rustok-module.toml`, `README.md`), `apps/next-admin` (`module-settings-fields/forum.tsx`, which drops the subscription toggle and the anonymous-reading toggle, neither of which was read)
- Extends: `DECISIONS/2026-10-09-forum-soft-default-settings.md`, `DECISIONS/2026-10-09-forum-remove-edit-grace-period-setting.md`
- Supersedes: None
- Superseded by: None

## Context

- `ForumModuleSettings` declares several fields that no runtime code reads. The admin form and the manifest still
  present some of them as working controls. The README lists them as unread.
- Five `submodule_*` flags say that a feature is enabled. Search and moderation are separate modules (`rustok-search`,
  `rustok-moderation`), and their own enablement controls them. Topic subscriptions are implemented inside the forum
  (`services/subscription`), and nothing reads the subscription flag, so it controls nothing. No code path reads the
  attachment or mention flags, although attachment relations exist in the forum.
- `hot_topic_threshold_replies` and `hot_topic_threshold_views` describe a "hot" badge that does not exist.
- `auto_close_inactive_days` describes automatic closing that does not exist.
- `auto_flag_threshold` describes an automatic hide after user flags. That policy belongs to `rustok-moderation`, not to
  the forum settings.

## Decision

1. Remove these fields from `ForumModuleSettings`, from their default functions, from the `Default` entries, and from
   `rustok-module.toml`:
   - `submodule_subscriptions_enabled`, `submodule_moderation_enabled`, `submodule_attachments_enabled`,
     `submodule_mentions_enabled`, `submodule_search_enabled`;
   - `hot_topic_threshold_replies`, `hot_topic_threshold_views`;
   - `auto_close_inactive_days`;
   - `auto_flag_threshold`.
2. Remove the admin form's "Subscription Levels Submodule" toggle and the two `submodule_*` keys from the form's state,
   defaults, and parser. Remove the "Anonymous Reading" toggle and the `allow_anonymous_reading` key from the form in the
   same way. The form's `updateFields` merges into the stored JSON, so keys the form no longer names keep their stored
   values.
3. Keep these unread fields declared for their own decisions: `default_topic_sort`, `allow_user_topic_closing`, and
   `allow_anonymous_reading`. Each one needs a wiring decision, because it changes a read or write path. They stay
   listed as unread in the README until then. Only the anonymous-reading toggle leaves the admin form, because it was
   the one unread key that a form presented as a working control.
4. A feature that a removed key described returns only through a new decision and a new setting. Automatic flag hiding
   belongs to `rustok-moderation`, and a future auto-close or hot-topic feature starts from its own decision.

## Invariants

- No removed key is declared in `ForumModuleSettings` or in the manifest after the change.
- No runtime path changes. None of the removed keys was read.
- The admin form sends only keys that the struct declares.

## Non-goals

- Wiring `default_topic_sort`, `allow_user_topic_closing`, or `allow_anonymous_reading`.
- Implementing attachments, mentions, a hot-topic badge, or automatic closing.
- Changing the search, moderation, or subscription modules.

## Sources of truth and ownership

- `crates/modules/rustok-forum/src/dto/settings.rs` is the source of the struct, and `rustok-module.toml` is the manifest
  entry. Both lose the same keys in one change.
- The per-module enablement of search, moderation, and subscriptions stays with those modules.

## Data, transaction, and concurrency boundary

No database change. No write-path change.

## Context dimensions

- Tenant: a stored key is ignored after the change for every tenant.
- Actor, locale, channel, policy, auth: not used by any removed key.

## Events and projections

No events and no projections.

## Failure semantics

- Old settings JSON that contains a removed key still parses. `ForumModuleSettings` has no `deny_unknown_fields`, so
  serde ignores the key. A save that serialises the struct drops it.
- The admin form reads missing keys as their defaults and no longer writes the two submodule keys.

## Migration and cutover

- No data migration. Stored rows keep the keys, and the values are ignored.
- The admin form no longer shows the subscription toggle or the anonymous-reading toggle. Stored values stay in the
  JSON and have no effect, as they had before.

## Alternatives considered

- Keep the keys and mark them unread in the README. Rejected for the removed keys: the admin form presented some of them
  as working controls, and that is the misleading state the audit found. For the three kept keys, the form now has no
  control, so no operator is shown an effect that does not happen.
- Wire each key now. Rejected here: `default_topic_sort` needs a cursor decision for each sort, and the other two need
  separate read-path decisions. Doing them in one change would hide the risk of each.
- Move `auto_flag_threshold` to `rustok-moderation` in the same change. Rejected: that is a moderation policy change with
  its own owner and tests.

## Verification

- Source-level check: the removed keys are absent from `settings.rs`, `rustok-module.toml`, and `forum.tsx`.
- A key-by-key comparison of the manifest and the struct finds no difference after this change.
- The Rust toolchain is not installed in this workspace, so no build or test has run. Run `cargo test -p rustok-forum`
  and the admin type check before merge.

## Consequences

- Nine declared settings and three admin controls are gone, and the README's unread list is shorter.
- Three unread settings remain, each waiting for its own decision.
