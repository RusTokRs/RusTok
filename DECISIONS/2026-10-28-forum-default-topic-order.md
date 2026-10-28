# Forum topic list default ordering uses the tenant setting

- Date: 2026-10-28
- Decision status: Accepted
- Implementation status: In progress (source-ready; runtime unverified)
- Owner: `rustok-forum` (`dto/settings.rs`, `services/engagement_mode.rs`, REST and GraphQL list transports)
- Extends: [Forum list page-size defaults](./2026-10-09-forum-list-page-size-from-settings.md)
- Supersedes: the unresolved `default_topic_sort` slice of [remove unwired settings](./2026-10-09-forum-remove-unwired-module-settings.md)

## Contract

The canonical list filter supports only `ForumTopicListOrder::Latest`
(latest-reply keyset ordering) and `ForumTopicListOrder::Top`
(the existing top-score keyset ordering).

The tenant settings owner reads `default_topic_sort` through the existing
static settings port. It accepts `latest_reply` (default, maps to `Latest`)
and `top` (maps to `Top`). The canonical module manifest exposes exactly
those two choices. Its previous `created_at`, `votes` and `views` values
were never implemented as list orders; stored values from those options
are not silently remapped or destroyed and instead fail with a typed
validation error until the operator selects a supported mode.

REST `GET /api/forum/topics` treats omitted `order` as an optional query
parameter. GraphQL `forumTopics`, `forumStorefrontAudienceTopics`, and
`forumStorefrontUnreadTopics` apply the same tenant default when their
`order` argument is omitted. Explicit caller ordering always wins,
including `LATEST` when the tenant default is `top`.

Transport adapters resolve omitted inputs; `ListTopicsFilter.order` remains
a required typed enum. The owner keyset ordering, cursor encoding, audience
authorization and tenant/channel/locale rules are not reimplemented. Changing
the configured sort between pages requires restarting pagination, since a
cursor is already bound to its original ordering.

No new migration, table, event, outbox or projection is introduced. Missing
reader uses the canonical settings defaults, while a settings-port failure
propagates. No compatibility or alternate naming keys are added.

`allow_user_topic_closing` stays separate: author close/reopen permissions
change write authorization and require a separate decision.

## Verification

- Rust unit regression for default, top, invalid and case-sensitive values.
- Maintainer checks: `cargo check --locked -p rustok-forum --lib`,
  `cargo test --locked -p rustok-forum --lib`, scoped REST/GraphQL
  integration tests and a configured-top cursor mismatch test.
- No runtime checks were executed in the authoring environment.
