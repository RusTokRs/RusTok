# M7 SalesChannel schema and bounded replay source

Status: `source_complete_owner_execution_pending`

This slice publishes the first Channel-owned current-state replay contract for
`rustok-channel::sales_channel@1` without adding Channel semantics to Index core or the
server.

## Ownership

`rustok-channel` owns the `channels` table, the `ChannelRuntimeSelected` composition marker,
and the positive monotonic `channels.index_revision` storage contract. The owner crate does
not depend on `rustok-index` and does not construct generic Index mutations.

`rustok-distribution` owns the selected cross-module adapter because it already composes both
Channel and Index. The adapter registers one schema and one host-database-aware source factory
only when `ChannelModule` participated in runtime extension registration.

`rustok-index` owns generic schema/source registration, deterministic event identity, immutable
source registry materialization, replay jobs/checkpoints, mutation persistence, and query
execution. The server remains Channel-agnostic.

## Schema

`rustok-channel::sales_channel@1` is `LocaleMode::None` and declares six required scalar
fields:

- `id: uuid`
- `slug: string`
- `name: string`
- `is_active: boolean`
- `is_default: boolean`
- `status: string`

The required `id` field duplicates `EntityKey.entity_id` intentionally so future link schemas
can target stable Channel UUID identity without changing the already published v1 fingerprint.
The schema itself is link-free. `settings` is intentionally omitted because the generic Index
value contract does not currently expose a JSON field type. Channel targets, module bindings,
OAuth applications, and resolution policies remain owner-domain data and are not flattened into
this schema.

## Replay source

The selected `sales-channel-postgres-primary` source:

- reads only the Channel-owned `channels` table;
- requires exact tenant and exact schema scope;
- scans in stable `channel_id` UUID order with one-row lookahead;
- persists an opaque cursor containing only `channel_id`;
- never orders or paginates by mutable `index_revision`;
- supports bounded targeted loads over exact non-localized entity keys;
- rejects locale-bearing targeted keys;
- rejects nil tenant/channel identities, non-positive revisions, and empty required strings;
- emits generic `IndexMutation::Upsert` records with no links;
- derives stable event UUIDs from tenant, channel, and source revision;
- maps storage failures to one bounded retryable code and contract/backend failures to bounded
  permanent codes.

`index_revision` is the mutation `source_version`; it is not the scan cursor. A PostgreSQL
trigger advances it exactly once for every Channel update and rejects revision exhaustion.
The SeaORM owner model and public Channel DTOs do not expose the storage-internal column.

## Composition

The core distribution registers the Channel bridge before immutable schema registry
materialization. Product bridges remain separately feature-gated. A partial/test registry that
omits `ChannelModule` receives neither a false SalesChannel schema nor a false source factory.

The host later constructs all selected PostgreSQL source factories atomically before freezing
`SharedIndexSourceRegistry` and publishing the replay runtime. No scheduler or background task is
started by this slice.

## Explicitly open

- Channel hard-delete tombstones;
- incremental Channel event ingestion and broker acknowledgement;
- persisted per-tenant schema application;
- repeatable-read full-scan snapshot and concurrent-insert reconciliation semantics;
- versioned Product/ProductVariant to SalesChannel links;
- Channel target, binding, and resolution-policy schemas;
- authoritative Storefront/Admin/Search consumer cutover;
- retry/backoff/dead-letter scheduling and graceful host task ownership;
- retained PostgreSQL replay, restart, drift, freshness, and equivalence evidence.

Runtime capability presence does not establish persisted schema readiness. Consumers must not
query this schema authoritatively until the exact tenant schema is applied and the relevant
replay/evidence admission is complete.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript guards, PostgreSQL execution, and CI are
maintainer-run. The implementation agent did not execute them.
