# M7 versioned Product graph source

Status: `source_complete_owner_execution_pending`

This slice preserves the published `rustok-product::product@1` and
`rustok-product::product_variant@1` contracts while adding versioned graph-ready
schemas:

- `rustok-product::product@2`
- `rustok-product::product_variant@2`

The v1 schema fingerprints, source names, cursor shapes, and replay event domains remain
unchanged. Each schema identity continues to use exactly one stable replay source across all
versions:

- `product-postgres-primary` serves Product v1 and v2;
- `product-variant-postgres-primary` serves ProductVariant v1 and v2.

This is required by `IndexSourceCatalog`, which intentionally rejects a replay-source change
within one schema identity.

## Product v2

Product v2 remains locale-required and carries every v1 scalar plus:

- `id: uuid`;
- `channel_restricted: boolean`;
- `allowed_channel_slugs: many<string>`;
- `variant_ids: many<uuid>`;
- a many-cardinality `variants` link to `product_variant@2.id`.

`allowed_channel_slugs` follows the existing Storefront metadata contract at
`metadata.channel_visibility.allowed_channel_slugs`: values are trimmed, lowercased,
deduplicated, and sorted. An absent or empty allowlist means unrestricted visibility, represented
as `channel_restricted = false` and an empty list. A restricted Product is represented as
`channel_restricted = true` with the canonical explicit allowlist.

A Storefront channel predicate can therefore preserve the current behavior without runtime
source-table fan-out:

```text
channel_restricted == false
OR allowed_channel_slugs CONTAINS canonical_request_channel_slug
```

Product v2 scans in stable `(product_id, locale)` order with one-row lookahead. Targeted loads
require exact locale-bearing Product keys. `index_revision` remains only the mutation
`source_version`; it is never part of enumeration cursor ordering.

## ProductVariant v2

ProductVariant v2 remains non-localized and adds only the stable `id: uuid` identity field to
the v1 scalar set. It keeps the stable `variant_id` scan cursor and exact non-localized targeted
loads.

There is no reverse Variant-to-Product link. A locale-free Variant cannot target one exact
locale-required Product record without inventing locale semantics, and Product translation
changes do not advance Variant revision. Queries traverse Product v2 to its Variants instead.

## Membership revision

Product v2 records contain the complete ordered set of current Variant IDs. A Product-owned
PostgreSQL trigger therefore advances the parent Product `index_revision` when a Variant is
inserted, deleted, or moved between Product/tenant identities. Ordinary Variant field updates do
not advance Product revision because they do not change Product link membership.

The existing Product row trigger still enforces exactly `OLD.index_revision + 1` and fails on
revision exhaustion.

## Durable hard-delete continuation

The follow-up [Product tombstone replay contract](m7-product-tombstone-source.md) retains exact
Product translation and ProductVariant identities after physical deletion. The same two stable
sources now emit versioned `IndexMutation::Delete` values without changing any v1/v2 schema
fingerprint, source name, cursor shape, or event domain.

## Why there is no Product-to-SalesChannel link yet

The current owner-authoritative visibility model stores Channel slugs in Product metadata, not
durable Channel UUID relations. Resolving those slugs to `channels.id` inside the Product source
would make Product link targets change when a Channel is created, deleted, or renamed without
advancing Product `index_revision`. That would violate monotonic mutation ordering and could
leave stale links.

This slice therefore publishes exact filterable channel-visibility scalars but no
Product/ProductVariant-to-SalesChannel `IndexLink`. A future link requires a durable owner
relation or another explicit cross-owner revision/tombstone contract. The source does not query
the `channels` table and does not depend on `rustok-channel`.

## Ownership

`rustok-product` owns Product/Variant storage, normalized metadata, monotonic revisions,
Variant-membership revision, and retained hard-delete identities. It still has no dependency on
`rustok-index`.

`rustok-distribution` owns the selected generic conversion adapter because it composes Product
and Index contracts. Index core and server remain Product-agnostic.

## Explicitly open

- incremental event ingestion and broker acknowledgement;
- tombstone retention/purge admission after consumer checkpoints are proven newer;
- a durable Product/ProductVariant-to-SalesChannel relation and revision contract;
- persisted per-tenant v2 schema application;
- repeatable-read replay snapshot and concurrent membership reconciliation semantics;
- authoritative Storefront query cutover;
- retained PostgreSQL replay, hard-delete/recreate, freshness, restart, drift, and equivalence
  evidence;
- retry/backoff/dead-letter scheduling and graceful host task ownership.

Runtime schema/source presence does not establish persisted schema readiness. Consumers must not
query Product v2 authoritatively until exact tenant schemas are applied and replay/evidence
admission is complete.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript guards, PostgreSQL execution, and CI are
maintainer-run. The implementation agent did not execute them.
