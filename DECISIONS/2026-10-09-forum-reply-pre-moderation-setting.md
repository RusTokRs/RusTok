# Forum reply pre-moderation applies the tenant setting alongside the category flag

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-forum` (`services/reply_owner_inline.rs`, `rustok-module.toml`, `README.md`), `apps/next-admin` (`module-settings-fields/forum.tsx`)
- Extends: `DECISIONS/2026-10-09-forum-soft-default-settings.md`
- Supersedes: None
- Superseded by: None

## Context

- `pre_moderation_enabled` is declared in the manifest, the settings DTO, and the admin form. The description says topics
  and replies are held. No runtime code read the setting, so changing it had no effect. It is one of the dead settings
  listed in the Forum README.
- The per-category flag `moderated` already holds every new reply in the category as `Pending`
  (`services/reply_owner_inline.rs`). The category flag does not hold topics.
- `TopicStatus` has `Open`, `Closed`, and `Archived`. It has no pending state, so a held topic would need a new status
  and new visibility rules.

## Decision

1. **The tenant setting holds replies.** A new reply is `Pending` when `category.moderated` or
   `pre_moderation_enabled` is `true`. The setting is read with `module_settings_in_tx` inside the create transaction,
   the same read the content limits use.
2. **The rule applies to every author, including staff.** The category flag has no exemption, so the tenant setting
   adds no exemption either. One rule for both flags is simpler to reason about. A moderator's reply is held and the
   moderator approves it.
3. **Topics are not held.** The manifest and admin description now say so. Holding topics needs a `Pending` topic status
   and its own decision, and it is not part of this change.
   *Superseded in part by [Forum pre-moderation holds new topics](./2026-10-09-forum-topic-pre-moderation.md): the
   tenant setting now holds topics as well. The rest of this decision stands.*

Default value stays `false`.

## Sources of truth and ownership

- The reply status rule is owned by `rustok-forum` (`services/reply_owner_inline.rs`). The category flag and the
  tenant setting are read there; neither is duplicated elsewhere.
- The `pre_moderation_enabled` value is owned by the Forum module settings contract (`dto/settings.rs`,
  `rustok-module.toml`). The admin form only edits it.

## Invariants

- A reply is `Pending` if either flag is set, and `Approved` otherwise, under the same transaction as the insert.
- A held reply is not visible in storefront reads. The existing `PUBLIC_REPLY_STATUSES` filter applies unchanged.
- Approval uses the existing moderation path (`ModerationService::approve_reply`). No new status transition is added.

## Non-goals

- Topic pre-moderation and a `Pending` topic status.
- An exemption for moderators or trusted authors.
- Changes to the category flag or to the moderation queue.

## Data, transaction, and concurrency boundary

- The settings read is inside the create transaction and uses the transactional reader, so the status is decided from the
  same snapshot as the insert.
- No schema change. The `status` column already holds `pending`.

## Context dimensions

- Tenant: the setting is per tenant.
- Category: the category flag is read with the category row, as before.
- Actor: no exemption. Every author is held.

## Events and projections

- The existing reply create event carries the held status. Consumers that filter on `Approved` already skip the reply.
- No new event. No projection change.

## Failure semantics

- A settings read error fails the create. The reply is not inserted with a guessed status.
- A missing reader or an absent value reads as `false`, so the category flag alone decides.

## Migration and cutover

- No migration. Tenants with a stored `pre_moderation_enabled: true` will start holding replies after deploy. That is the
  behaviour the setting always documented, so no data change is needed. Operators should check the setting before deploy.
- Rollback: set the value back to `false`. Replies that are already held stay `Pending` until a moderator acts.

## Alternatives considered

- **Hold topics as well.** Needs a new topic status, visibility rules, and a moderation path for topics. Deferred.
- **Exempt moderators from the tenant setting.** Diverges from the category flag. Deferred until a product decision asks
  for it.
- **Remove the setting and rely on the category flag.** Removes a documented control. Rejected.

## Verification

- Integration tests in `rustok-forum/tests/posting_limits_sqlite.rs`: with the setting on, a new reply is `Pending`, and
  a moderator approval makes it `Approved`; with the setting off, a new reply is `Approved`.
- The Rust toolchain is not installed in this workspace, so the tests have not been compiled or run. Run
  `cargo test -p rustok-forum` before merge.

## Consequences

- Every new reply in a tenant with the setting on waits for a moderator. Authors see their reply as pending, and
  storefront readers do not see it until approval.
- The manifest, admin form, and README now describe what the setting does. Topic pre-moderation is documented as absent.
- The setting leaves the list of dead settings in the Forum README.
