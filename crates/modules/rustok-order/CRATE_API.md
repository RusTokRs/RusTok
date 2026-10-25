# rustok-order / CRATE_API

## Public Modules
- `analytics`: Dashboard order statistics snapshot helpers.
- `checkout_order_recovery`: Recovery adapter for staged checkout order adoption.
- `checkout_owner_context`: In-process context ports for checkout order compensation and settlement.
- `dto`: Input and response transfer objects for order operations.
- `entities`: SeaORM entity models for order database storage.
- `error`: Order domain errors and result aliases.
- `migrations`: SeaORM migration definitions for order schemas.
- `module`: Core `OrderModule` definition and platform registration.
- `ports`: Order identity, completion, and status ports.
- `services`: Order domain services and checkout identity recording.
- `status`: Typed order, change, and return status state machines.

## Platform Capability Bindings
- `OrderModule` implements `rustok_core::module::RusToKModule` and `MigrationSource`.
- Declared as `ModuleKind::Optional`.
- Self-check returns `HealthStatus::Healthy`.
- Slug: `order`.
- Permissions: `ORDERS_CREATE`, `ORDERS_READ`, `ORDERS_UPDATE`, `ORDERS_DELETE`, `ORDERS_LIST`, `ORDERS_MANAGE`.

## Domain Collaborations
- Default order submodule of the ecommerce family.
- Owns order write-side schema, persistence, and lifecycle transitions.
- Persists order snapshots and line items independently from catalog ownership.
- Persists item-level return lines in `order_return_items` with quantity and line-item validation.
- Persists typed adjustments (`order_adjustments`) as language-neutral promotion/discount snapshots.
- Exposes `CheckoutCompletionPort` and `CheckoutOrderIdentityPort` for durable transactional checkout completion.
- Publishes transactional order lifecycle domain events through `rustok-outbox`.
- Leptos admin package in `admin/` (`rustok-order-admin`).

## Minimum Contract Set

### Input DTOs/Commands
- `CreateOrderInput`: customer and currency context, line items, adjustments, tax lines, shipping total, metadata.
- `CreateOrderLineItemInput`: product/variant references, unit price, quantity, fulfillment requirement, metadata.
- `CreateOrderAdjustmentInput`: adjustment amount, source type, source ID, description, metadata.
- `CreateOrderTaxLineInput`: tax amount, rate, title, provider ID, metadata.
- `MarkPaidOrderInput`: order ID, payment transaction reference, payment method.
- `ShipOrderInput`: order ID, tracking number, carrier reference.
- `DeliverOrderInput`: order ID, delivery confirmation details.
- `CancelOrderInput`: order ID, cancellation reason code.
- `CreateOrderChangeInput`: order ID, change type, item diffs, adjustment diffs.
- `ApplyOrderChangeInput`: order change ID.
- `CancelOrderChangeInput`: order change ID, cancellation reason.
- `CreateOrderReturnInput`: order ID, return reason, item lines.
- `CompleteOrderReturnInput`: return ID, resolution type, refund/exchange references.
- `CancelOrderReturnInput`: return ID, cancellation reason.

### Domain Invariants
- Denies float arithmetic (`clippy::float_arithmetic`) in favor of exact `Decimal` calculations.
- Strict lifecycle state transitions: `Pending -> Confirmed -> Paid -> Shipped -> Delivered` (terminal), or `Cancelled` (terminal).
- In-process command receipts prevent duplicate replay and detect payload conflicts.
- Order lines maintain immutable catalog snapshots (`product_id`, `variant_id`, title, SKU, price) so catalog changes do not corrupt historic orders.
- Tenant isolation enforced across all database queries, migrations, and event payloads.

### Events / Outbox Side Effects
- Emits `OrderCreated`, `OrderPaid`, `OrderShipped`, `OrderDelivered`, `OrderCancelled` domain events.
- Emits `OrderChangeCreated`, `OrderChangeApplied`, `OrderReturnCreated`, `OrderReturnCompleted` events.
- All domain events are atomically staged in `rustok-outbox` within the database transaction.

### Errors / Failure Codes
- `OrderError::Validation`: input validation failures (negative quantities, invalid decimal amounts).
- `OrderError::NotFound`: target order, line item, change, or return record not found in tenant.
- `OrderError::InvalidStatusTransition`: attempted transition violates lifecycle state machine.
- `OrderError::DuplicateCommand`: duplicate command receipt with mismatched payload.
- `OrderError::Database`: underlying storage failures.
- `OrderError::Outbox`: transactional event bus failure.
