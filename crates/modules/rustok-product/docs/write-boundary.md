# Product write boundary

Status: `decided` for the owner split; receipt and Media-retention status is recorded explicitly
below.

## Decision

Product is the single writer of its own aggregate. Every write that another module, deployment, or
admin surface performs must use the relevant Product-owned transport or command boundary. Calling
a plain `CatalogService`, `ProductCatalogSchemaService`, `ProductRelationService`, or
`BundleService` mutation directly from a foreign crate is not an allowed integration path: that
bypasses the policy-carrying command context and can bypass durable receipt behavior.

The currently mounted Commerce and FFA adapters invoke the explicit receipt-bound Relation and
Bundle commands while a dedicated cross-module write-port replacement is not yet defined. Those
adapters must not call the corresponding plain `ProductRelationsPort`/`BundlePort` mutators.

## Ports and command surfaces

| Write family | Owner boundary | Operations | Current call/receipt policy |
|---|---|---|---|
| Product lifecycle, variants, variant axes, images | `ProductCatalogCommandPort` | create/update/delete product; publish/unpublish/archive; create/update/delete variant; set axes; add/update/delete/reorder image | `PortCallPolicy::write()` requires deadline and caller key. **The key is validated but lifecycle commands do not currently admit, complete, fail, or replay a durable owner receipt.** Do not represent this as durable idempotency. |
| Attribute definitions, options, schemas, category schemas/bindings, and attribute values | `ProductCatalogSchemaWritePort` | mounted schema/EAV operations | Durable receipts: `rustok_outbox::idempotency::admit` → Product write transaction → `complete`/terminal `fail`; receipt-result encoding, a missing result, or completion failure awaits transaction rollback before the error leaves the owner, as does any early owner error; only deterministic failures become terminal, while retryable failures retain a reclaimable lease; replay decodes the stored response. |
| Product relations | `ProductRelationService::create_relation_idempotent`, `update_relation_idempotent`, `delete_relation_idempotent`, and `reorder_relations_idempotent` with `ProductRelationCommandContext` | create, update, delete, reorder | Tenant-scoped durable receipt and same-transaction response completion for every command. Plain relation-port mutations are not external ingress. |
| Product bundles | `BundleService::create_bundle_idempotent`, `update_bundle_idempotent`, `delete_bundle_idempotent`, `add_bundle_item_idempotent`, and `remove_bundle_item_idempotent` with `BundleCommandContext` | create, update, delete, add item, remove item | Tenant-scoped durable receipt and same-transaction response completion for every command. Plain Bundle-port mutations are not external ingress. |

## Lifecycle receipt gap

`ProductWriteTransaction` can complete a bound receipt inside its transaction, but the catalog
lifecycle methods currently build or read their public `ProductResponse`, `VariantResponse`, or
image response after their transaction commits. Binding receipt admission around those methods
would therefore either fail closed at commit or store a response that was not obtained in the owner
transaction.

A lifecycle receipt cutover must first refactor every covered concrete command so it serializes its
actual public response before `ProductWriteTransaction::commit`. Only then may
`ProductCatalogCommandPort` use the shared owner-operation receipt ledger. Until that complete
cutover, caller keys at this boundary are policy/trace identities, not replay guarantees.

## Product image Media retention gap

Product currently performs only read-side Media admission before adding an image when a host
composes `ProductMediaValidatedCommandPort`. It does **not** acquire or release a durable Media
reference hold. A validated asset may therefore still be deleted after the pre-write check.

The proposed architecture is recorded in
[Product image relations use Product-owned IDs and Media durable reference retention](../../../../DECISIONS/2026-10-17-product-image-media-reference-retention.md).
No Product-to-Media hold workflow may be improvised until that ADR is accepted: it requires a
Product-owned preallocated image ID, Media write-port composition, conservative post-commit
release, and owner reconciliation.

## Why attribute values are not on `ProductCatalogCommandPort`

`ProductCatalogCommandPort` remains the lifecycle/variant/image boundary. EAV authoring and value
writes are a separate owner surface exposed through the receipt-bound
`ProductCatalogSchemaWritePort`. Moving them into the lifecycle command port would duplicate the
receipt protocol and broaden an already incomplete lifecycle-receipt surface.

## Receipt identities

- Lifecycle: caller keys are scoped by `commerce-admin-product:{operation}:{digest}` (REST) or
  `commerce-graphql-product:{operation}:{digest}` (GraphQL), but are not yet persisted as Product
  lifecycle receipts.
- Schema/EAV: Product owner receipt scope and operation are bound by
  `ProductCatalogSchemaWritePort`.
- Relations: `product_relations` owner; `create_relation`, `update_relation`, `delete_relation`,
  and `reorder_relations`; tenant-scoped receipts. Native FFA and Commerce GraphQL derive the
  same owner key with `rustok_product_relations::scoped_caller_idempotency_key`.
- Bundles: `product_bundles` owner; `create_bundle`, `update_bundle`, `delete_bundle`,
  `add_bundle_item`, and `remove_bundle_item`; tenant-scoped receipts. Native FFA and Commerce
  GraphQL derive the same owner key with `rustok_product_bundles::scoped_caller_idempotency_key`.
- Reusing a receipt key with a different payload is rejected as
  `outbox.operation_receipt_conflict` instead of aliasing two logical writes.

## Receipt failure finalization

Every Product receipt ingress, including the Product, Variant, Image, and schema/EAV Translation
adapters, waits for a receipt-bound owner transaction that returned early to roll back before it
attempts an independent failure finalization. It records a terminal receipt only for a deterministic
`PortError`; an unavailable or timeout error retains the processing lease so admission can reclaim
an unknown infrastructure outcome instead of replaying a terminal transient failure. If persisting a
deterministic failure itself fails or discovers a reclaimed lease, the ingress returns that retryable
receipt error—not the original terminal domain error—so the caller retries with the same key and
observes the durable replay or reclaim outcome.

## Consequences for new integrations

1. Importers, assistants, and marketplace bridges write through the owner transport or an explicit
   receipt-bound owner command. A new lifecycle operation requires extending
   `ProductCatalogCommandPort` and this document.
2. Every externally reachable Relation or Bundle write carries a durable caller-owned idempotency
   key. A mutation without the key is rejected by the owner transport contract.
3. A new Product lifecycle write must carry a caller key but cannot claim durable replay until the
   lifecycle receipt gap is closed for the entire command surface.
4. A write family that is not in the table above is not externally writable; shipping one requires
   extending this document together with the owner contract.

## Evidence

- `crates/modules/rustok-product/src/catalog_command_port.rs` and
  `src/services/catalog/commands.rs` (current lifecycle receipt gap).
- `crates/modules/rustok-product/src/catalog_schema_write_port.rs` and
  `src/services/write_transaction.rs` (receipt-bound schema/EAV writes).
- `crates/modules/rustok-product-relations/src/services/receipts.rs`.
- `crates/modules/rustok-product-bundles/src/services/receipts.rs`.
- `scripts/verify/verify-product-relations-bundles-idempotency.mjs`.
