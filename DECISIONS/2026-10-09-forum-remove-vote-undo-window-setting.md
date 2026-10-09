# Forum removes the unread `vote_undo_window_minutes` setting

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-forum` (`dto/settings.rs`, `rustok-module.toml`, `README.md`); `apps/next-admin` is not changed because the Forum admin form does not expose this field
- Extends: `DECISIONS/2026-10-09-forum-vote-policy-settings.md`, `DECISIONS/2026-10-09-forum-soft-default-settings.md`
- Supersedes: None
- Superseded by: None

## Context

`ForumModuleSettings.vote_undo_window_minutes` (default `0`, "0 = unlimited") is declared in `rustok-module.toml` and
`dto/settings.rs`. No runtime code reads it. `VoteService` clears and changes votes without any time check. The
vote-policy decision left the window open: it needs a decision on whether the window applies to clears, changes, or
both. The product owner chose to remove the setting instead of deciding the window semantics now.

## Decision

1. Remove the `vote_undo_window_minutes` field from `ForumModuleSettings`, its default function, its `Default`
   entry, and its entry in `rustok-module.toml`.
2. Votes keep their current behaviour: a vote can be cleared or changed at any time by its author, subject to the
   existing vote policy (`allow_downvotes`, `allow_self_voting`, and the audience gate).
3. Stored settings JSON that still contains the key continues to parse. The struct has no
   `deny_unknown_fields`, and serde ignores the unknown key. No data migration is needed.
4. A future time window for votes needs a new decision. That decision defines its semantics (from the first vote,
   for clears only, or for every change) and adds a new setting. It does not revive this key.

## Invariants

- No settings field is declared for this key after the change.
- The removal does not change any vote result. The setting never had runtime effect.

## Non-goals

- Choosing the undo window semantics. That is deferred to a new decision if the product needs a window.
- Changing other unread settings. They are listed in `crates/modules/rustok-forum/README.md` and are handled in
  separate decisions.

## Data, transaction, and concurrency boundary

No database change. No write-path change. Vote writes keep their transaction boundaries.

## Context dimensions

- Tenant: the stored key is ignored for every tenant.
- Locale, channel, policy, auth: not used.

## Events and projections

No events and no projections.

## Failure semantics

- Old JSON with the key is accepted and the key is ignored. A save that serialises the struct drops it.
- No new validation failure.

## Sources of truth and ownership

- `crates/modules/rustok-forum/src/dto/settings.rs` is the source of the struct.
- `crates/modules/rustok-forum/rustok-module.toml` is the source of the manifest entry. Both must lose the key in the
  same change.

## Migration and cutover

- No data migration. Tenants that stored `vote_undo_window_minutes` keep the row. The value is ignored.
- Clients that read the key from the settings JSON will no longer receive it. The admin form does not show it, so no
  first-party client reads it.

## Alternatives considered

- Keep the key and document it as unread. Rejected: the product does not need the window now, and an unread key
  invites operators to set it expecting an effect.
- Wire the key with the 10-minute value the soft-default decision replaced. Rejected: it would make a restrictive
  behaviour the default for tenants that have no stored value, and the semantics are still open.

## Verification

- Source-level check that the key is absent from `settings.rs` and `rustok-module.toml`.
- The Rust toolchain is not installed in this workspace, so no build or test has run. Run `cargo test -p rustok-forum`
  before merge.

## Consequences

- One unread setting is gone. The forum settings list in the README is shorter by one entry.
- A future vote time window starts from a new decision and a new setting.
