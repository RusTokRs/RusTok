# rustok-fulfillment / CRATE_API

## Public Modules
- `checkout_execution`: Checkout fulfillment execution port, command inputs, and requests.
- `dto`: Input and response transfer objects for fulfillment operations and shipping options.
- `entities`: SeaORM entity models for fulfillments, items, and shipping options.
- `error`: Fulfillment domain errors and result aliases.
- `migrations`: SeaORM migration definitions for fulfillment schemas.
- `module`: Core `FulfillmentModule` definition and platform registration.
- `ports`: Shipping selection port, snapshots, and projection requests.
- `providers`: Fulfillment provider SPI registry, descriptors, capabilities, and manual carrier.
- `services`: Domain business logic, fulfillment operations, shipping options, and translation services.
- `status`: Typed fulfillment lifecycle state machines and status transitions.

## Platform Capability Bindings
- `FulfillmentModule` implements `rustok_core::module::RusToKModule` and `MigrationSource`.
- Declared as `ModuleKind::Optional`.
- Self-check returns `HealthStatus::Healthy`.
- Slug: `fulfillment`.
- Permissions: `FULFILLMENTS_CREATE`, `FULFILLMENTS_READ`, `FULFILLMENTS_UPDATE`, `FULFILLMENTS_DELETE`, `FULFILLMENTS_LIST`, `FULFILLMENTS_MANAGE`.

## Domain Collaborations
- Default fulfillment submodule of the ecommerce family.
- Owns shipping-option and fulfillment storage, including item-level delivery progress tracking.
- Provides `ShippingSelectionPort` for storefront and checkout carrier option selection.
- Provides `CheckoutFulfillmentExecutionPort` for checkout pipeline fulfillment initialization.
- Exposes `FulfillmentProviderRegistry` and `FulfillmentProvider` SPI for pluggable carrier integrations.
- Integrates with `rustok-translation-targets` for localized shipping option copy.
- Publishes transactional fulfillment lifecycle events through `rustok-outbox`.
- Admin Leptos package in `admin/` (`rustok-fulfillment-admin`) and storefront package in `storefront/` (`rustok-fulfillment-storefront`).

## Minimum Contract Set

### Input DTOs/Commands
- `CreateFulfillmentInput`: order ID, tracking number, carrier ID, fulfillment items list, metadata.
- `CreateFulfillmentItemInput`: order line item ID, quantity.
- `ShipFulfillmentInput`: fulfillment ID, tracking number, carrier reference.
- `DeliverFulfillmentInput`: fulfillment ID, delivered note, delivery confirmation.
- `ReopenFulfillmentInput`: fulfillment ID, reopen reason.
- `ReshipFulfillmentInput`: fulfillment ID, new tracking number, carrier reference.
- `CancelFulfillmentInput`: fulfillment ID, cancellation reason.
- `CreateShippingOptionInput`: name, price, currency, provider ID, allowed shipping profile slugs.
- `UpdateShippingOptionInput`: name, price, active flag, allowed profile slugs.

### Domain Invariants
- Denies float arithmetic (`clippy::float_arithmetic`) in favor of exact `Decimal` calculations.
- Strict fulfillment status transitions: `Pending -> Shipped -> Delivered` (terminal) or `Cancelled` (terminal).
- Reopen/reship transitions are explicit and audit-logged into metadata.
- Per-item quantity tracking: shipped and delivered quantities cannot exceed total item quantity.
- Tenant isolation enforced across all database queries and provider operation journals.

### Events / Outbox Side Effects
- Emits `FulfillmentCreated`, `FulfillmentShipped`, `FulfillmentDelivered`, `FulfillmentCancelled` domain events.
- Emits `ShippingOptionCreated`, `ShippingOptionUpdated`, `ShippingOptionDeactivated` events.
- Staged in transactional outbox.

### Errors / Failure Codes
- `FulfillmentError::Validation`: input validation errors (negative quantities, invalid currency).
- `FulfillmentError::FulfillmentNotFound`: target fulfillment ID not found in tenant.
- `FulfillmentError::ShippingOptionNotFound`: target shipping option not found.
- `FulfillmentError::InvalidStatusTransition`: requested lifecycle transition violates state machine.
- `FulfillmentError::Provider`: carrier provider operation failure.
- `FulfillmentError::Database`: underlying storage failures.

## Prepared shipment stock and recovery (2026-11-01; source only)

F owns `ShipmentStockPort`, `PreparedShipmentStock`, `PreparedFulfillmentStock`,
`ShipmentStockScope`, `ShipmentStockItemScope`, `ShipmentStockRequest`, `ShipmentStockAction`,
`ShipmentStockError`, `NoopShipmentStockPort`, `UnconfiguredShipmentStockPort` and
`MAX_SHIPMENT_STOCK_ITEMS = 1024`. The protocol is re-exported at the crate root; its module
is private. F has no Order/Inventory peer dependency. C selects the saved Order reader and
I-backed implementation, without peer I/O inside F's write transaction.

`prepare(&PortContext, &ShipmentStockScope)` returns a per-call `Arc<dyn PreparedShipmentStock>`;
`apply(&DatabaseTransaction, &[ShipmentStockRequest])` runs one complete batch and must not
commit. The opaque F token captures complete immutable owner facts/context and the original
budget. Locked snapshot mismatch refuses; remaining positive budget is checked before carrier
and apply, capped at 30 seconds. The token is not authorization, a receipt, persisted history
or a distributed transaction. Native creation, single and paginated projections refuse more
than 1024 child items without truncating retained history; batch SQL reads have a checked
aggregate probe bound. Typed children are complete, with unique per-parent Order lines.

Missing configuration returns nonretryable `fulfillment.shipment_stock_not_configured` for
fresh movement. Empty-header Ship still needs preparation/application, so only explicit Noop
may opt out; native I-backed scope refuses unknown typed facts. Pending cancel, delivery and
undo-delivery are movement-free. Cancelled reopen reconsumes only exact native restored units;
delivered reopen merely undoes delivery. Old cancelled status is not restoration proof.

`FulfillmentAdminCommandPort::cancel_pending_fulfillment` has a failing default and a native
locked Pending-only implementation; unsupported selected capabilities never emulate full
cancel. `retry_local_persistence(PortContext, RetryLocalFulfillmentPersistenceRequest)` also
fails by default. Native recovery checks current read policy/deadline before journal access,
loads tenant-scoped retained identity, keeps its literal key and current actor/claims/roles/
locale/channel/tracing/causation, then validates write admission. It never invokes a carrier.
Committed recovery bypasses O/I. Unknown saved request/stock identity refuses reconciliation;
no historical key reconstruction or metadata-derived stock history is allowed.

New provider execution claims only Pending rows with null result/reference/completion/commit
markers; private release is limited to known-uninvoked Executing with the same absent evidence.
One owner predicate fences all unresolved states, malformed Pending and incomplete Committed
for sibling claims and local progress. ERROR never automatically authorizes Executing.

`FulfillmentProviderOperationRecovery::resolve_unknown_as_failed` is explicit operator no-effect
confirmation, not inference from failure metadata: Recon/legacy ERROR requeues Pending with the
same immutable key/request only without reference/result/commit evidence. Success resolution
preserves the retained reference and CASes observed reference/result/no commit evidence. An
identical already-retained late result can be explicitly promoted from quarantine without
replacing its JSON/reference or completion timestamp; contradictory results refuse.
The worklist uses the shared predicate and UpdatedAt/Id order; Executing needs explicit stale
quarantine. Recovery/native wrappers deduct journal/receipt reads from the original budget.

The forward-only `m20261101_000121_enforce_provider_effect_admission`, after
`m20261003_000120_create_checkout_identity_anchor`, corrects PostgreSQL/SQLite admission guards
without rewriting rows or applied migration payloads. It is registered, unexecuted and refuses
rollback to ambiguous ERROR execution. SQLite claim writer fencing precedes local reads;
PostgreSQL retains parent locking. Raw fixtures install the current guard last.
Tenant/Fulfillment/literal-key lookup precedes mutable shipping-option provider selection;
parent-serialized admission and claim refuse bounded cross-provider ambiguity in any state.
Terminal status/provider metadata never substitutes an exact key/payload receipt. Committed
replay rereads the native projection. Owner journal/local recovery never trims a retained key.
A quarantined late success stays retained and blocks automatic local completion, including
create-label callers. Fresh registry preflight leaves known-uninvoked work Pending.
Delivered reship carries a movement-free native scope/context to claim and local revalidation;
it requires no stock peer. Pending cancellation without shipped progress/actual shipped_at
reopens Pending, not Shipped merely because an ActiveValue wraps a null timestamp.
Owner root/read/receipt time and final stock/progress commits share the decreasing budget.
Deliver/reopen kernel commands bind key/payload/exact response in their local receipt transaction.
The plain domain methods are not equivalent kernel receipts. Native movement/proof/F progress
and provider checkpoint share one transaction, but carrier, saved O reads and Order projection do
not. The I-owned additive proof migration is unexecuted; all Rust/SQL/provider/remote validation
and complete stock event/outbox/Index convergence remain open. See the
[handoff](../rustok-commerce/docs/implementation-plan.md#shipment-stock-provenance-source-evidence).

## Native stock outbox continuation (2026-11-01; source only)

The native I implementation now appends canonical `InventoryUpdated` through the same supplied
F transaction after stock/proof/reservation writes and before the final elapsed-budget gate.
F owns lifecycle/progress/consumer protocol and does not import Inventory/Order or publish substitute
Inventory events. Outbox failure requires rollback with F counters/checkpoint; carrier and
other-owner effects are not rolled back. Preparation and movement-free/replay paths publish
none; equal available counters after a physical shipment still invalidate. Existing global
quantity semantics, bounds/ownership, actor and rollback errors are defined by the
[Inventory API](../rustok-inventory/CRATE_API.md#native-stock-outbox-2026-11-01-source-only).
No F public contract or event schema changes. The additional 16 Rust/10 Node definitions are
unrun; source checks do not certify runtime delivery, remote owners, Index or full step 8.
