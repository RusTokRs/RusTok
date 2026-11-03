# Forum removes the unwired `allow_user_topic_closing` setting and rejects digest selections it cannot deliver

- Date: 2026-11-02
- Decision status: Accepted
- Implementation status: Implemented
- Owners: `rustok-forum` (`dto/settings.rs`, `rustok-module.toml`, `services/subscription`, `notification_source.rs`, `README.md`)
- Extends: `DECISIONS/2026-10-09-forum-remove-unwired-module-settings.md`
- Supersedes: None
- Superseded by: None

## Context

- `allow_user_topic_closing` was declared in `ForumModuleSettings` and in the module manifest, but no runtime
  code read it: topic authors cannot close or lock their own topics through any transport, and the setting
  presented a control that changed nothing. The 2026-10-09 precedent removes settings that no runtime code
  reads instead of keeping them as decoration.
- `ForumDigestMode::{Daily, Weekly}` was accepted and persisted by the subscription owner, and stored rows
  carried the setting through every API surface, but no digest batching exists in the forum or in the
  notifications owner. The notification fan-out ignored the stored mode entirely, so a user who selected
  daily delivery kept receiving immediate notifications — the opposite of the promise the setting makes.
- The platform rule is zero legacy and no fake implementations: unsupported capabilities must fail closed
  or be removed, never simulated.

## Decision

1. Remove `allow_user_topic_closing` from `ForumModuleSettings`, its `Default` entry, and
   `rustok-module.toml`. If author self-close is wanted later, it returns as a complete feature: settings
   flag, owner-authorized close/unlock write path, transports, UI, and tests in one change.
2. Reject an *effective* `Daily`/`Weekly` digest selection at the subscription write boundary with a typed
   validation error ("Forum digest delivery is not available yet; use immediate or disabled"). A muted
   level resolves to `Disabled` and is unaffected. Level defaults never produce a digest mode, so
   implicit/auto subscriptions are unaffected.
3. Honor stored digest rows in the immediate fan-out: topic-reply, category-reply, and category-new-topic
   recipient queries only include subscriptions whose digest mode is `Immediate`. Stored `Daily`/`Weekly`/
   `Disabled` rows receive no immediate delivery, matching the preference the user expressed, until the
   notifications owner ships real digest batching.
4. The posting-policy evaluator (`posting_policy_evaluator.rs`) stays as a host-composed decision library:
   the server composes facts and rules (`apps/server/src/services/forum_posting_policy_facts.rs`), and no
   tenant-side rule source exists inside the forum, so the forum write path cannot enforce it. The README
   keeps stating this boundary explicitly instead of implying enforcement.

## Sources of truth and ownership

- The forum subscription owner (`services/subscription`) is the only writer of subscription preference
  state, including `digest_mode`; the notifications owner is the only future owner of digest batching
  delivery. The forum owns recipient *selection*; notifications own delivery timing.
- Tenant module settings are owned by the platform settings store and parsed by
  `ForumModuleSettings`; the forum manifest (`rustok-module.toml`) is the canonical list of exposed
  settings. A setting that no runtime code reads must not appear in either.
- No derived projections are involved: digest mode is read directly from subscription rows at fan-out
  time.

## Invariants

### Allowed states

- `digest_mode ∈ {Immediate, Disabled}` for every row written after this decision.
- Stored rows with `Daily`/`Weekly` remain readable and keep round-tripping through APIs; they simply
  receive no immediate delivery.
- Previously stored tenant settings JSON that still contains `allow_user_topic_closing` keeps parsing
  (unknown keys are ignored); the key has no representation anymore.

### Forbidden states

- A write path that accepts a setting it will not honor (silent no-op settings, silent immediate
  delivery for digest subscribers).
- A muted subscription resolving to anything other than `Disabled` digest with all notify flags off.

## Non-goals

- Implementing digest batching (notifications owner, separate decision when scheduled).
- Implementing author self-close/self-lock for topics.
- Wiring the posting-policy evaluator into forum write paths (host composition concerns only).

## Data, transaction, and concurrency boundary

- No schema change: `forum_*_subscriptions.digest_mode` and the tenant settings JSON shape are unchanged.
- The digest rejection happens inside `resolve_preferences`, before the subscription write transaction
  opens, so a rejected write persists nothing and emits no event.
- Fan-out filtering is a read-side `WHERE` clause on the same bounded keyset pages; pagination cursors
  and limits are unaffected.

## Context dimensions

- Tenant: all rules are per-tenant (settings and subscription rows are tenant-scoped as before).
- Principal: unchanged — subscription writes still require the authenticated owner; digest rejection is
  not a permission decision.
- Locale, channel, currency, timezone: not applicable (digest *scheduling* would introduce timezone
  semantics; that is exactly why it belongs to the future notifications-owner design).

## Events and projections

- No new events. Rejected digest selections produce no `forum.subscription.changed` event because no
  write occurs. Successful writes keep the existing schema v1 event contract.
- No projection or index changes; notification recipient selection reads subscription rows directly.

## Failure semantics

- New daily/weekly selections fail closed with a typed validation error instead of silently degrading
  to immediate delivery.
- Stored daily/weekly rows degrade to "no immediate delivery" — the conservative reading of the user's
  expressed preference — rather than receiving immediate notifications they opted out of.

## Migration and cutover

- No data migration is required or performed. Zero-legacy applies to the removed setting: the field,
  its default, and the manifest entry are deleted outright rather than deprecated.
- Rollback note: restoring the setting field would make old stored JSON values meaningful again, but
  no data depends on this because the setting never had runtime effect.

## Alternatives considered

- **Implement digest batching inside the forum:** rejected — delivery scheduling, retry, and inbox
  batching are notifications-platform responsibilities; a forum-local scheduler would duplicate them.
- **Normalize daily/weekly to immediate on write:** rejected — silently changes the user's expressed
  preference and hides the missing capability.
- **Keep `allow_user_topic_closing` until the self-close feature lands:** rejected — that is precisely
  the unwired-setting state the 2026-10-09 decision removes; the feature can reintroduce it atomically.

## Verification

- `scripts/verify/verify-forum-subscription-levels.mjs` covers the subscription contract (levels,
  revision CAS, policy, auto-subscribe) and remains green; digest rejection is enforced in
  `services/subscription/helpers.rs::resolve_preferences`, the single resolution point for topic and
  category writes.
- Immediate fan-out filters are visible in `notification_source.rs` (topic replies, category replies,
  category new topics) and covered by `verify-forum-notification-topic-subscription-audience.mjs`.
- Rust integration suites (`subscription_levels_sqlite`, contract tests) require a Rust toolchain and
  remain CI/maintainer evidence; they were not executed in the authoring sandbox.

## Consequences

- Admins no longer see a closing toggle that does nothing; API clients selecting daily/weekly digest get an
  explicit error instead of silent immediate delivery.
- Users who previously stored daily/weekly digest stop receiving immediate notifications. That matches the
  preference they selected; digest batching, when implemented by the notifications owner, must start
  delivering to those stored rows without any data migration.
