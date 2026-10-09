# Forum removes the unread `edit_grace_period_minutes` setting

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-forum` (`dto/settings.rs`, `rustok-module.toml`, `README.md`); `apps/next-admin` is not changed because the Forum admin form does not expose this field
- Extends: `DECISIONS/2026-10-09-forum-wire-author-edit-window-and-locked-list-settings.md`, `DECISIONS/2026-10-09-forum-soft-default-settings.md`
- Supersedes: None
- Superseded by: None

## Context

- `ForumModuleSettings.edit_grace_period_minutes` (default `5`, "edits inside the grace period do not trigger an edited
  indicator") is declared in `rustok-module.toml` and `dto/settings.rs`. No runtime code reads it.
- The window decision (`wire-author-edit-window`) left this key unread. Its semantics need an edited marker, which the
  forum model does not store. Using `updated_at` for that marker is not reliable: a topic's `updated_at` also moves on
  reply activity, and a reply's `updated_at` moves on moderation.
- The soft-default decision set the value to `5`. The value never changed behaviour.

## Decision

1. Remove the `edit_grace_period_minutes` field from `ForumModuleSettings`, its default function, its `Default` entry,
   and its entry in `rustok-module.toml`.
2. The author edit window stays the only edit-time rule. It is `max_edit_window_minutes`, enforced by `edit_window.rs`.
3. An "edited" indicator needs a new decision. That decision adds the stored marker, for example an `edited_at` column,
   and a new setting with its own name. It does not revive this key.

## Invariants

- No settings field is declared for this key after the change.
- Edit results do not change. The setting never had runtime effect.

## Non-goals

- Adding an edited marker or an edited indicator. That needs a schema decision.
- Changing `max_edit_window_minutes` or the author edit rule.

## Sources of truth and ownership

- `crates/modules/rustok-forum/src/dto/settings.rs` is the source of the struct.
- `crates/modules/rustok-forum/rustok-module.toml` is the source of the manifest entry. Both lose the key in the same
  change.

## Data, transaction, and concurrency boundary

No database change. No write-path change. Edit writes keep their transaction boundaries.

## Context dimensions

- Tenant: a stored key is ignored for every tenant.
- Actor, locale, channel, policy: not used by this setting.

## Events and projections

No events and no projections.

## Failure semantics

- Old settings JSON that contains the key still parses. `ForumModuleSettings` has no `deny_unknown_fields`, so serde
  ignores the key. A save that serialises the struct drops it.
- No new validation failure.

## Migration and cutover

- No data migration. Stored rows keep the key, and the value is ignored.
- No first-party client reads the key. The admin form does not show it.

## Alternatives considered

- Keep the key and document it as unread. Rejected: an operator would set it expecting an effect.
- Implement the marker now with `updated_at`. Rejected: `updated_at` moves on activity that is not an edit, so the
  indicator would be wrong.
- Add an `edited_at` column now. Rejected here: it is a schema change and needs its own decision and migration.

## Verification

- Source-level check: the key is absent from `settings.rs` and `rustok-module.toml`, and no code or test refers to
  `edit_grace_period_minutes`.
- The Rust toolchain is not installed in this workspace, so no build or test has run. Run `cargo test -p rustok-forum`
  before merge.

## Consequences

- One more unread setting is gone. The unread-settings list in the README is shorter by one entry.
- An edited indicator starts from a new decision, with a stored marker and a new setting.
