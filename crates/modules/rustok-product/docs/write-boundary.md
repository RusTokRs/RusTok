# Product write boundary

Status: `decided`.

## Decision

Product is the single writer of its own aggregate. Every write that another module, another
deployment or the admin surface performs must go through one of the Product owner ports below or
through the Product transport contract (Commerce GraphQL/REST, or the FFA `#[server]` boundary of
the admin packages). Calling `CatalogService`, `ProductCatalogSchemaService`, `ProductRelationService`
or `BundleService` directly from a foreign crate is not an allowed integration path: an owner port
carries the call policy (deadline, idempotency key, tenant/actor scope) with it, a direct service
call does not.

## Ports

| Write family | Owner port | Operations | Call policy |
|---|---|---|---|
| Product lifecycle, variants, variant axes, images | `ProductCatalogCommandPort` | 14 operations: create/update/delete product, `publish_product`, `unpublish_product`, `archive_product`, create/update/delete variant, `set_variant_axes`, image add/update/delete/reorder | `PortCallPolicy::write()`: deadline + caller idempotency key; owner maps failures onto a bounded `PortError` vocabulary |
| Attribute definitions, options, schemas, category schemas, category bindings, attribute values (product and variant) | `ProductCatalogSchemaWritePort` | 13 operations | Durable receipts: `rustok_outbox::idempotency::admit` → owner write transaction → `complete`/`fail` inside that transaction, replay decoded from the stored receipt |
| Product relations | `ProductRelationsPort` + `ProductRelationService::create_relation_idempotent` | list/reverse list/get/create/update/delete/reorder + receipt-bound create | `ProductRelationCommandContext` is the write-policy carrier (tenant + actor + durable caller key); reads stay plain |
| Product bundles | `BundlePort` + `BundleService::{create_bundle_idempotent, add_bundle_item_idempotent}` | list/get/get-by-slug/create/update/delete/add item/remove item | `BundleCommandContext` is the write-policy carrier (tenant + actor + durable caller key); reads stay plain |

## Why attribute values are not on `ProductCatalogCommandPort`

`ProductCatalogCommandPort` stays the lifecycle/variant/image boundary. EAV authoring and value
writes are a different ownership surface: they are already exposed through the receipt-bound
`ProductCatalogSchemaWritePort`, which every host composes (`rustok_product` FBA registry) and which
composes the same `owner_operation_receipts` ledger as the lifecycle commands. Moving them into the
command port would duplicate the receipt protocol and re-open the boundary that `PROD-VALID-001`
just closed, so the minimal extension was exactly one operation: `archive_product`.

## Receipt identities

- Lifecycle commands: caller key scoped by `commerce-admin-product:{operation}:{digest}` (REST) or
  `commerce-graphql-product:{operation}:{digest}` (GraphQL).
- Relations: `product_relations` owner, `create_relation`, tenant-scoped receipt.
- Bundles: `product_bundles` owner, `create_bundle` / `add_bundle_item`, tenant-scoped receipts.
- Reusing one key with a different payload is rejected as `outbox.operation_receipt_conflict`
  instead of aliasing two logical writes.

## Consequences for new integrations

1. Importers, assistants and marketplace bridges write through the owner transport contract or the
   owner port; a new port operation is added to `ProductCatalogCommandPort` only when the write is
   part of the product lifecycle.
2. Every new externally reachable write must bring a durable idempotency key; a mutation without a
   key argument fails the wave verifiers of this module.
3. A write family that is not in the table above is not externally writable, and shipping one
   requires extending this document together with the port.

## Evidence

- `crates/modules/rustok-product/src/catalog_command_port.rs`, `catalog_schema_write_port.rs`.
- `crates/modules/rustok-product-relations/src/services/receipts.rs`.
- `crates/modules/rustok-product-bundles/src/services/receipts.rs`.
- `scripts/verify/verify-product-relations-bundles-idempotency.mjs` (owner receipts on both modules).
- `scripts/verify/verify-product-lifecycle-events.mjs` (published lifecycle facts on the command surface).
