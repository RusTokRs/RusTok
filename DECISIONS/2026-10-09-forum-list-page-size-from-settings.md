# Forum list page size defaults to the tenant setting on transports

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-forum` (REST `controllers/topics.rs`, `controllers/replies.rs`; GraphQL `graphql/query_runtime.rs`, `graphql/storefront_audience_topics.rs`, `graphql/runtime_data.rs`; `services/engagement_mode.rs`); `apps/next-admin` (Forum settings fields, unchanged values)
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

## Invariants

- The default of `topics_per_page` and `replies_per_page` remains 20, so tenants without a stored value see no change.
- An explicit `per_page` always wins over the setting.
- A missing settings reader yields `ForumModuleSettings::default()`, which is the same 20.
- No transport other than the list endpoints above reads the setting. `get_reply` and `redirect_merged_topic` use the
  filter only for `locale` and are not list endpoints.

## Non-goals

- No change to the service signatures and no `Option` in `ListTopicsFilter` or `ListRepliesFilter`.
- No change to the default `default_topic_sort`, `hot_topic_threshold_*`, or the other unread settings. They stay
  declared-but-unused and need separate decisions.
- The storefront native adapter in `crates/modules/rustok-forum/storefront` and the SEO sitemap runtime do not receive
  settings providers in this change. Their topic reads keep default settings until the host wires providers into those
  runtimes. This gap is recorded in the implementation report.

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
