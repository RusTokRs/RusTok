# Forum list page size defaults to the tenant setting on transports

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: Implemented
- Owners: `rustok-forum` (REST `controllers/topics.rs`, `controllers/replies.rs`; GraphQL `graphql/query_runtime.rs`, `graphql/storefront_audience_topics.rs`, `graphql/runtime_data.rs`; `services/engagement_mode.rs`; `graphql/storefront_read_state.rs`); `apps/next-admin` (Forum settings fields, unchanged values); consumers: `crates/modules/rustok-forum/storefront` transports and `apps/next-frontend/packages/rustok-forum` (Addendum)
- Extends: `DECISIONS/2026-10-09-forum-soft-default-settings.md`
- Supersedes: None
- Superseded by: None

## Context

`ForumModuleSettings` declares `topics_per_page` and `replies_per_page`, but no runtime path read them. Every list
endpoint used a transport constant of 20 when a client omitted `per_page`. The tenant setting therefore had no effect.

The service filters `ListTopicsFilter.per_page` and `ListRepliesFilter.per_page` are `u64` and already receive a
bounded value. A service-level `Option` would have changed about 40 literal construction sites and would not
distinguish "omitted" from "explicit 20" in every caller. The transport layer is the place where the request
shape is known.

## Decision

1. When a REST or GraphQL list request omits `per_page`, the transport resolves the default from the tenant's
   settings: `topics_per_page` for topic lists, `replies_per_page` for reply lists. The value is bounded to
   `1..=MAX_FORUM_READ_LIMIT` (100) by `bounded_forum_read_limit`.
2. An explicit `per_page` keeps its meaning: it is bounded to `1..=100` and is not replaced by the setting.
3. REST list handlers receive their query through `ListTopicsQuery` and `ListRepliesQuery`, whose `per_page` is
   `Option<u64>`. GraphQL arguments already were `Option<i32>`.
4. Service filters stay `u64`. Services never read the transport default.
5. The storefront audience list and the storefront unread composition now receive the same settings providers as the
   owner list services. Without this, `show_locked_topics_in_lists` and the page size did not apply on the public path.
6. Consumer transports do not send their own page size for these reads. An override in a consumer defeats items 1-5,
   so the storefront and Next.js forum surfaces omit `perPage`/`limit` and let the owner resolve the setting (Addendum).

## Invariants

- The default of `topics_per_page` and `replies_per_page` remains 20, so tenants without a stored value see no change.
- An explicit `per_page` always wins over the setting.
- A missing settings reader yields `ForumModuleSettings::default()`, which is the same 20.
- Only the list endpoints above, the storefront unread read-state query, and the storefront widget reads resolve a
  page size from the setting. `get_reply` and `redirect_merged_topic` use the filter only for `locale` and are not
  list endpoints.

## Non-goals

- No change to the service signatures and no `Option` in `ListTopicsFilter` or `ListRepliesFilter`.
- No change to the default `default_topic_sort`, `hot_topic_threshold_*`, or the other unread settings. They stay
  declared-but-unused and need separate decisions.
- The storefront native adapter in `crates/modules/rustok-forum/storefront` and the SEO sitemap runtime did not receive
  settings providers in this change; their topic reads kept default settings until providers reached those runtimes. The
  storefront adapter and the module-owned consumer transports were wired on 2026-10-11 (see Addendum). The SEO sitemap
  runtime stays on its own batch size: sitemap discovery pages through topics in bulk and must not be throttled by a
  UI page-size setting.

## Data, transaction, and concurrency boundary

Each list request reads the tenant's Forum settings once through the existing non-transactional reader, before the
list query. No writes, no transactions, and no new locks.

## Context dimensions

- Tenant: the setting is per tenant. The request tenant resolved by `TenantContext` or `resolve_tenant_scope` is the
  only tenant read.
- Channel, locale: unchanged. Channel filtering and locale fallback are not affected by the page size.
- Policy, auth: unchanged. The permission checks run before the settings are read.

## Events and projections

No events and no projections.

## Failure semantics

- If the settings reader fails, the request fails with the mapped Forum error, the same way the audience and
  visibility reads fail today. The request does not fall back silently to 20.
- An invalid stored value fails the settings parse as before.

## Sources of truth and ownership

- `ForumModuleSettings` in `rustok-forum` owns `topics_per_page` and `replies_per_page`.
- The tenant's stored settings row, read through the module settings port, is the source of the value.
- The transport owns the query shape. The service owns the bounded filter.

## Migration and cutover

No data migration. The change is in behaviour only for tenants that stored a non-default `topics_per_page` or
`replies_per_page`. Those tenants now receive that page size when a client omits `per_page`. Clients that send
`per_page` explicitly are unaffected. Clients that relied on the 20 default while the tenant had stored another value
receive the stored value instead. This is the intended effect of the setting.

## Alternatives considered

- Option A: make `per_page` an `Option<u64>` in the service filters and resolve the default inside the services.
  Rejected for now. It changes about 40 construction sites, and it still needs the same settings reads.
- Keep the constant 20 and remove the settings. Rejected. The settings are part of the declared module contract, and
  the admin form already edits them.

## Verification

Scoped checks for this change, once a Rust toolchain is available:

- `cargo check -p rustok-forum`
- `cargo test -p rustok-forum --test topic_owner_audience_read_sqlite --test storefront_read_state_sqlite`
- `python scripts/verify/verify-remediation-gate.py --files <changed paths>`

In the current sandbox `cargo` is not installed and the Rust crate registry is not reachable, so none of these
ran. They are recorded as not executed.

## Consequences

- Operators who set `topics_per_page` see the effect on clients that omit `per_page`.
- The REST OpenAPI parameter for `per_page` stays a query parameter. Its schema changes from a required-with-default
  integer to an optional integer.
- The public storefront path now honours `show_locked_topics_in_lists`, which it did not before.

## Addendum (2026-10-11): consumers stop overriding the setting

A forum audit found items 1-5 only half effective: the owner honoured `topics_per_page` / `replies_per_page` on every
transport, but each consumer sent an explicit page size, so a configured value was overridden in practice.

1. `crates/modules/rustok-forum/storefront/src/transport/graphql_adapter.rs` (browser GraphQL) declares `per_page` and
   `limit` as `Option<i32>` and sends `None` for topic, reply, and unread pages; the browser no longer invents 12 or 20.
2. The server transport in the same package (`transport/native_server_adapter.rs`) resolves both page sizes through
   `ForumSettingsProviders` taken from the shared static/transaction module-settings readers - the same composition the
   forum composer uses - and passes them to the owner read services.
3. `apps/next-frontend/packages/rustok-forum/src/api/forum.ts` dropped its `?? 50` and `?? 100` fallbacks so the GraphQL
   arguments are omitted. `loadForumWidgetTopics` and `loadForumWidgetReplies` keep the explicit `ForumWidgetLimit`
   because that preview surface documents a cap for statically rendered pages.
4. `graphql/storefront_read_state.rs` resolves an omitted `limit` from `topics_per_page` instead of a transport-local
   `DEFAULT_STOREFRONT_UNREAD_LIMIT`, which was removed together with its unreachable default branch.
5. Category lists keep the fixed 12 in the Leptos storefront: the owner exposes no category page-size setting, so there
   is nothing to resolve there.

The default of both settings is 20 and the unread query default was 20, so a tenant without stored values sees identical
page sizes. Only a tenant that configured another value changes behavior - which is the point of the setting.
