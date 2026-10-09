# Forum authors edit their own topics and replies through the Customer role and delete them only when the tenant allows it

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-core` (`rbac.rs`: Customer permissions and scope mapping), `rustok-forum` (`services/rbac.rs`, `services/topic_owner.rs`, `services/reply_owner.rs`, `dto/settings.rs`, `rustok-module.toml`, `README.md`), `apps/next-admin` (`module-settings-fields/forum.tsx`)
- Extends: `DECISIONS/2026-10-09-forum-wire-author-edit-window-and-locked-list-settings.md`, `DECISIONS/2026-10-09-forum-vote-policy-settings.md`
- Supersedes: None
- Superseded by: None

## Context

- The shared RBAC model maps a role and a permission to a scope in `permission_scope_for_set`. `Own` was granted only to
  Customer on orders, and on comments for update and delete. For forum topics and replies, a role holding the update
  permission got `All`, and a role without it got `None`.
- The Customer role had no forum update or delete permission. Authors could create topics and replies but could not
  edit or delete them.
- A permission set that held `forum_topics:update` but no staff permission was inferred as Customer, so it got `All`.
  `enforce_owned_scope` then accepted any topic for it. That is a cross-author write path.
- Forum services already check ownership on every author update and delete (`topic_owner.rs`, `reply_inline.rs`,
  `reply_owner.rs`, `topic_inline.rs`, `quote_command.rs`, and the slug rename). Moderation writes are gated by
  `forum_topics:manage`, `forum_replies:moderate`, or the `Moderate` scope, which Customer does not hold.
- AGENTS.md requires that missing authorization must not silently become permissive. A scope that grants `All` by
  permission name alone does that for authors.

## Decision

1. **Author edit is a role permission with scope `Own`.** The built-in Customer role holds `forum_topics:update`,
   `forum_topics:delete`, `forum_replies:update`, and `forum_replies:delete`. `permission_scope_for_set` returns `Own`
   for Customer on these four actions when the role holds the permission (exact or `manage`). Other Customer actions
   keep their current scope. Staff roles keep `All`, and a role without the permission keeps `None`.
2. **Ownership is checked in the service.** Each update and delete path already calls `enforce_owned_scope` with the
   item's `author_id`. The `Own` scope is accepted only when the caller is the author. Unlisted callers stay refused.
3. **Author deletion is a tenant policy.** The new setting `allow_user_content_deletion` (boolean, default `false`)
   decides whether an author may delete their own topic or reply. The check runs after the ownership check and only
   when the caller's delete scope is `Own`. Moderators and administrators (scope `All`) are not affected. A refused
   author gets `Forbidden`.
4. **Author edit is not a setting.** Whether a role may edit its own content is decided by the role permission. A
   tenant that does not want author edits removes the permission from that role.
5. **Scope of this change.** The same policy does not yet cover topic close or lock (`allow_user_topic_closing`).
   That setting is still unread and stays out of this change. It needs its own decision on the permission it uses.

Default values: the setting is `false`. Authors can edit within the author edit window
(`max_edit_window_minutes`, default `0`, meaning no limit) and cannot delete until a tenant enables the setting.

## Sources of truth and ownership

- The role-to-scope mapping is owned by `rustok-core` (`rbac.rs`, `permission_scope_for_set`). Forum does not
  duplicate it.
- The author ownership check and the deletion policy are owned by `rustok-forum` (`services/rbac.rs`,
  `services/topic_owner.rs`, `services/reply_owner.rs`).
- The `allow_user_content_deletion` value is owned by the Forum module settings contract
  (`dto/settings.rs`, `rustok-module.toml`). The admin form only edits it.

## Invariants

- A Customer holds `Own` on a forum update or delete action only when the role holds that permission. Without it the
  scope is `None`.
- A Customer author reaches an update or delete only through the owner check. `Own` alone never authorizes a write to
  another author's item.
- Every service call site that uses `Update` or `Delete` on `ForumTopics` or `ForumReplies` checks ownership. Call
  sites that use `Manage` or `Moderate` never accept `Own`, because Customer holds neither.
- The author deletion policy applies only to the author path. A caller with scope `All` is never refused by it.
- A missing or unreadable setting never allows author deletion.

## Non-goals

- Topic close and lock by authors (`allow_user_topic_closing`). It is a separate decision.
- Aligning Comments with the permission-gated `Own` rule. Comments keep their current behaviour in this change.
- Any change to blog, pages, or other Customer scopes. Those modules keep staff-only editing.
- A separate setting for author edit. The role permission controls it.

## Data, transaction, and concurrency boundary

- The author policy is read with the non-transactional static reader before the delete transaction. This follows the
  read-before-write pattern used by `reply_inline.rs`. The setting is changed by administrators, so a change that lands
  between the read and the commit is an accepted ordering.
- The delete itself, its locks, its outbox events, and its tombstones are unchanged.
- No schema change and no migration.

## Context dimensions

- Tenant: the setting is per tenant. Each request reads the setting of its own tenant.
- Actor: the user id and the role, through `SecurityContext`. The scope is derived from the role and the permission
  snapshot.
- Locale and channel: no effect.

## Events and projections

- No new domain event. Author deletion emits the existing `ForumTopicStatusChanged` and `ForumReplyStatusChanged`
  events, the same as moderator deletion.
- No projection changes.

## Failure semantics

- A missing reader or an absent setting reads as the default `false`, so author deletion is refused.
- A reader error propagates as a `ForumError`. It is not treated as permission.
- A refused author gets `Forbidden` with the message "Authors cannot delete their own content in this forum".
- Sessions created before the deploy keep their permission snapshot. They gain author edit and delete only after their
  next token or session refresh.

## Migration and cutover

- No migration. The new setting has a default of `false`, and a tenant without a stored value reads `false`.
- The Customer role gains four permissions. Existing Customer sessions keep their snapshot until refresh, as stated
  above.
- A permission set that held `forum_topics:update` without staff permissions changes from `All` to `Own` on refresh.
  This narrows access and needs no data change.
- Rollback: remove the four permissions from the Customer role and restore the previous scope mapping. The setting may
  stay in stored settings unread.

## Alternatives considered

- **Separate `_own` actions** (for example `forum_topics:update_own`). This gives the clearest semantics, but it changes
  the permission contract for API, OAuth scopes, and UI, and needs token migration. Rejected for this change.
- **Keep `All` and check ownership only in the service.** This leaves a role that holds the update permission able to
  reach any item through the scope, which is the cross-author path. Rejected.
- **Author deletion controlled by the role permission alone.** This removes the tenant switch. The product decision is
  that deletion is a separate tenant policy, so the setting is required. Rejected.
- **Author edit controlled by a tenant setting.** Rejected: whether a role may edit is a role decision, so it stays a
  permission.

## Verification

- Unit tests in `rustok-core/src/rbac.rs`: the Customer author scope for the four actions; `None` when the permission is
  missing; a permission-only grant is inferred as Customer and gets `Own`.
- Integration tests in `rustok-forum/tests/rbac.rs`: another customer is refused for update; the author updates own topic
  and reply; author deletion is refused while the setting is off; the author deletes own topic and reply when the setting
  is on; an administrator deletes regardless of the setting.
- Call-site audit (static): all `Update` and `Delete` checks on `ForumTopics` and `ForumReplies` in `rustok-forum/src`
  call `enforce_owned_scope`. Moderation and reconciliation paths use `Manage` or `Moderate`.
- The Rust toolchain is not installed in this workspace, so none of these tests has been compiled or run. Run
  `cargo test -p rustok-core` and `cargo test -p rustok-forum` before merge.

## Consequences

- A permission set that holds only `forum_topics:update` now gets `Own` instead of `All`. It can edit its own topics
  and no one else's.
- Comments keep their existing behaviour, with `Own` granted for update and delete even without the permission. This
  inconsistency is recorded as a follow-up.
- Blog and pages keep staff-only editing.
- The admin form gains one switch, `Allow Author Deletion`.
- Existing tests that expected the Customer role to be refused on its own topic or reply were rewritten.
