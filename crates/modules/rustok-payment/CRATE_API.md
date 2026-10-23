# rustok-payment / CRATE_API

## Public Modules
- `checkout_compensation`: Payment compensation ports and requests for checkout rollbacks.
- `checkout_execution`: Checkout payment execution port, admission checks, and request types.
- `dto`: Input and response transfer objects for payment collections, payments, and refunds.
- `entities`: SeaORM entity models for payment storage, collections, and provider events.
- `error`: Payment domain errors and result aliases.
- `migrations`: SeaORM migration definitions for payment schemas.
- `module`: Core `PaymentModule` definition and platform registration.
- `ports`: Payment collection ports, status requests, and snapshots.
- `providers`: Payment provider SPI registry, descriptors, capabilities, and manual provider.
- `services`: Domain services for payments, refunds, provider event journals, and admission.

## Platform Capability Bindings
- `PaymentModule` implements `rustok_core::module::RusToKModule` and `MigrationSource`.
- Declared as `ModuleKind::Optional`.
- Self-check returns `HealthStatus::Healthy`.
- Slug: `payment`.
- Permissions: `PAYMENTS_CREATE`, `PAYMENTS_READ`, `PAYMENTS_UPDATE`, `PAYMENTS_DELETE`, `PAYMENTS_LIST`, `PAYMENTS_MANAGE`.

## Domain Collaborations
- Default payment submodule of the ecommerce family.
- Owns payment collection and payment-attempt storage.
- Owns refund record storage and refund lifecycle state machines.
- Exposes `PaymentProviderRegistry` and `PaymentProvider` SPI for pluggable gateways.
- Provides `CheckoutPaymentExecutionPort` for checkout pipeline collection preparation, authorization, and capture.
- Publishes transactional payment lifecycle domain events through `rustok-outbox`.
- Storefront Leptos package in `storefront/` (`rustok-payment-storefront`).

## Minimum Contract Set

### Input DTOs/Commands
- `CreatePaymentCollectionInput`: cart/order ID, currency, amount, payment method, metadata.
- `AuthorizePaymentInput`: collection ID, payment ID, amount, provider details.
- `CapturePaymentInput`: collection ID, payment ID, amount, provider transaction ID.
- `CancelPaymentInput`: collection ID, cancellation reason.
- `CreateRefundInput`: payment ID, amount, reason, metadata.
- `CompleteRefundInput`: refund ID, provider refund reference.
- `CancelRefundInput`: refund ID, cancellation reason.
- `PrepareCheckoutPaymentCollectionRequest`: checkout identity and metadata.
- `AuthorizeCheckoutPaymentCollectionRequest`: checkout identity, collection ID, provider details.
- `CaptureCheckoutPaymentCollectionRequest`: checkout identity, collection ID, metadata.

### Domain Invariants
- Denies float arithmetic (`clippy::float_arithmetic`) in favor of exact `Decimal` calculations.
- Strict collection state transitions: `Pending -> Authorized -> Captured` or `Cancelled`.
- Provider operation journals enforce tenant isolation and idempotency before external execution.
- Webhook delivery ID and replay/idempotency keys are strictly validated and tracked to prevent double-crediting.
- Amounts must be strictly positive; currencies must be 3-letter ISO codes.

### Events / Outbox Side Effects
- Emits `PaymentCollectionCreated`, `PaymentAuthorized`, `PaymentCaptured`, `PaymentFailed` domain events.
- Emits `RefundCreated`, `RefundCompleted`, `RefundFailed` domain events.
- Staged in outbox within the database transaction.

### Errors / Failure Codes
- `PaymentError::Validation`: input validation failures (negative amounts, invalid currency).
- `PaymentError::NotFound`: target collection, payment, or refund not found in tenant.
- `PaymentError::InvalidStatusTransition`: requested state transition not allowed.
- `PaymentError::Provider`: payment gateway execution error.
- `PaymentError::DuplicateOperation`: idempotent operation conflict detected.
- `PaymentError::Database`: underlying storage failures.
