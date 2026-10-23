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
