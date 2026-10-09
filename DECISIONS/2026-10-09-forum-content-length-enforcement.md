# Forum enforces tenant length limits on topic titles and post bodies at every user write

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-forum` (`services/content_limits.rs`, topic create, topic translation upsert, reply create and update)
- Extends: `DECISIONS/2026-10-09-forum-soft-default-settings.md`
- Supersedes: None
- Superseded by: None

## Context

`ForumModuleSettings` declares `min_topic_title_length`, `max_topic_title_length`, `min_post_body_length`, and
`max_post_body_length`. Before this decision, none of them was read at runtime. The only check on a title was
non-emptiness, and bodies had no length check. The soft-default decision set the defaults to values that the
surveyed platforms accept, and this decision makes the settings effective.

## Decision

1. The four limits are enforced on every user-facing write that carries the content:
   - topic create (`TopicInlineService` create path): title and body;
   - topic translation upsert (`upsert_translation_in_tx`), for each caller-supplied title and body;
   - reply create (`ReplyOwnerInlineService` create path): body;
   - reply update (`ReplyInlineService` update path): body, when `content` is supplied.
2. The metric is fixed:
   - a title is measured after `trim()`, in Unicode scalar values (`chars().count()`);
   - a body is measured on the plain text produced by `richtext::project_discussion`, in Unicode scalar values.
   The `Discussion` rich-text profile has no media nodes, so a body without text has zero length and is rejected
   by a minimum of 1.
3. A maximum of `0` means no maximum. A minimum of `0` means no minimum; the non-empty title check still applies.
4. If a minimum is greater than a non-zero maximum, the write fails with `ForumError::Validation`. A tenant with
   inconsistent limits cannot write content until the limits are corrected. The error is not silently widened.
5. A violation returns `ForumError::Validation` with the limit in the message. REST returns 400, GraphQL returns
   the validation error.
5a. Every writer must receive the tenant settings providers. REST, GraphQL, and the Forum admin native
   reply-create function build the services with `with_settings_providers`. The admin path reads the readers from
   `HostRuntimeContext` and fails the request if they are missing, instead of silently applying the defaults.
   Seed drivers in `rustok-starter` and test fixtures use the default providers; they therefore apply the soft
   defaults, not the tenant's stored values.
6. Only caller-supplied values are checked. A seed title or body copied from an existing locale during a
   translation upsert is not re-checked, and an update that does not change the title or body does not re-check it.

## Exclusions

These paths do not apply the limits. Each one writes content that was accepted elsewhere or is a platform
migration, and rejecting it would lose data:

- Imports: `topic_import.rs`, `reply_owner_import.rs`, `reply_owner_tombstone_import.rs`, `import_write.rs`.
  Imported content is a migration of existing material. Operators who need a cap for imports must enforce it
  in the source export.
- Exact translation apply: `ugc_translation_apply.rs` (`apply_exact_translation_in_tx`). It writes a translation
  of content the author already wrote, verified by expected revisions. Changing the text would break the exact
  translation contract.

## Invariants

- Limits are read from the tenant's stored settings, with defaults applied by `ForumModuleSettings`. The
  same defaults are recorded in the soft-default decision.
- Length checks run before any write and before the body is serialised, so a rejected write leaves no partial row.
- The check uses the same projection as the stored plain text.
- The rich-text profile limits still apply and are the outer bound: `Discussion` allows 100,000 text characters
  and 256 KiB of JSON. A tenant maximum above those values has no effect, because the rich-text validation runs
  first and fails the write with the same `Validation` error.

## Non-goals

- Rate limits are decided in `DECISIONS/2026-10-09-forum-posting-rate-limits.md`. `auto_flag_threshold` is still not
  enforced and needs its own decision. `vote_undo_window_minutes` was removed by
  `DECISIONS/2026-10-09-forum-remove-vote-undo-window-setting.md`.
- No migration. Existing content longer than a newly lowered maximum is kept. It is rejected only when the
  author next supplies that title or body.

## Data, transaction, and concurrency boundary

- Topic create and reply create/update read the limits through `ForumSettingsProviders::module_settings`
  (non-transactional static reader) before their write transaction. A settings change that commits during a write
  applies to the next write, not the current one.
- Topic translation upsert reads the limits through `module_settings_in_tx` inside the transaction that writes
  the translation, so the check and the write see the same settings snapshot.
- Length checks are pure; they add no writes and no locks.

## Context dimensions

- Tenant: the limits are per tenant, read from the tenant's Forum settings.
- Locale: the limits do not vary by locale. Each translation is checked against the same limits.
- Channel, policy, auth: not used. Authorisation is unchanged and runs before the length check.

## Events and projections

No new events and no projections.

## Failure semantics

- A violation fails the whole write with `Validation` before any row is written.
- A settings read error fails the write with the same error as other settings reads.
- Inconsistent limits fail the write with `Validation`.

## Sources of truth and ownership

- `ForumModuleSettings` in `crates/modules/rustok-forum/src/dto/settings.rs` and `rustok-module.toml` declare the
  limits.
- `crates/modules/rustok-forum/src/services/content_limits.rs` is the only place that applies them.

## Migration and cutover

- No data migration. Existing rows are not rewritten.
- Tenants keep the limits they already stored. Tenants with no stored value receive the soft defaults from the
  soft-default decision.
- Operators who lower a maximum should check existing long content before the change, because the next edit to
  that content will be rejected.

## Alternatives considered

- Enforce only the maximum: rejected. The minimum of 1 rejects only empty bodies, and a tenant would otherwise
  have no way to forbid empty posts.
- Count bytes instead of characters: rejected. Characters match what the admin form and the stored plain text
  show, and byte limits would reject non-Latin text sooner than the same visible length.
- Enforce limits on import and exact translation apply: rejected for the reasons in Exclusions.

## Verification

- Unit tests in `services/content_limits.rs`: zero maximum, inclusive bounds, inconsistent limits.
- Integration tests in `tests/posting_limits_sqlite.rs`: topic create (title and body, trimmed title),
  reply create and update, topic update with supplied title or body only, zero maximum, and an inconsistent
  minimum above the maximum.
- Not covered by a test: imports and exact translation apply. Their exclusion is by code path. Neither calls the
  checked methods, which was confirmed by reading the call sites.
- The Rust toolchain is not installed in this workspace, so neither the unit nor the integration tests have run.
  Run `cargo test -p rustok-forum` before merge.

## API surface

- `TopicService::with_settings_providers` and `ReplyService::with_settings_providers` on the public facades are
  now `pub` (they were `pub(crate)`). This is additive: the method previously existed on the inner services and
  is needed by the Forum admin crate and by integration tests.
- `rustok-forum-admin` gains `forum_settings_providers(&HostRuntimeContext)` in its native server support module.

## Consequences

- Forum writes now fail with a validation error when a title or body is outside the tenant's limits.
- The four settings have runtime effect for the first time. The admin form values now matter.
- Import and exact translation apply remain unlimited. Operators who need a cap there must enforce it upstream.
- The Next admin form for the title limits accepts `0` as "no maximum" and its fallback values match the defaults.
  The form does not edit the body limits; they are set through the settings JSON.
