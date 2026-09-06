# Payment storefront native error safety

Status: **source-complete / unvalidated**

## Scope

This slice hardens the payment-owned native server-function adapter in:

- `crates/rustok-payment/storefront/src/transport/native_server_adapter/server_functions.rs`.

It covers refund-summary read, payment-collection read, and payment-collection create/reuse. The GraphQL adapter, public DTOs, transport selection, commerce runtime operations, and payment facade variants are unchanged.

## Public boundary

Public `ServerFnError` messages remain static for:

- request-context extraction;
- tenant-context extraction;
- authentication-context extraction;
- missing host-composed payment runtime dependencies;
- refund-summary owner failure;
- payment-collection read failure;
- payment-collection create/reuse failure.

UUID validation messages and the three outer `PaymentTransportError::ServerFn` wrappers are unchanged. No internal framework or owner error is serialized into a public envelope.

## Bounded framework and owner diagnostics

The four generic error mappers no longer require `E: Debug`. They retain only the Rust error type through `std::any::type_name::<E>()`; complete framework and owner errors are not logged through either debug or display formatting.

Where `RequestContext` is available, diagnostics retain:

- payment owner and exact owner operation;
- correlation ID, stable code, and native boundary;
- tenant UUID non-nil state when tenant context exists;
- channel UUID presence and non-nil state;
- channel-slug presence and length;
- locale presence and length.

The missing-runtime diagnostic uses the same bounded context shape. In all native diagnostic paths, tenant, channel, slug, and locale values are not logged. Request-context extraction failures cannot carry a correlation ID because extraction itself failed, so that mapper retains only owner operation, stable code, boundary, and Rust error type.

## Preserved behavior

This slice preserves:

- all three `#[server]` endpoint paths;
- `read_storefront_order_refunds`;
- `read_storefront_payment_collection`;
- `create_storefront_payment_collection` and its metadata payload;
- request, tenant, optional-auth, and runtime-composition order;
- UUID validation messages;
- static public failure messages;
- the three outer `PaymentTransportError::ServerFn` wrappers;
- the payment GraphQL adapter and native/GraphQL selection policy;
- payment FBA/FFA status.

The payment-collection read endpoint continues to extract `RequestContext` only for attribution and diagnostics. Its owner request and response remain unchanged.

## Static evidence

Focused guard:

```text
scripts/verify/verify-payment-storefront-native-error-safety.mjs
```

Retained evidence:

```text
crates/rustok-payment/contracts/evidence/payment-storefront-native-error-safety-source.json
```

The guard requires four type-only mapper sites, correlation-aware bounded context fields, all three endpoint and owner-call contracts, static public envelopes, and unchanged outer wrapper counts. It rejects complete error payloads, obsolete `Debug` bounds, full tenant/channel/slug/locale fields, and raw public conversions.

No test, verifier, Cargo command, formatting command, workflow, CI job, mounted request, or runtime failure-injection trace was executed for this source slice.

## Remaining gaps

The master ecommerce mapper-cleanup task remains open for payment compensation/execution consumers, other ecommerce transports, remaining owner adapters, and non-`PortError` public envelopes. Compile, mounted parity, remote transport, and runtime evidence are also still open.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-payment-storefront-native-error-safety.mjs
node scripts/verify/verify-payment-storefront-boundary.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-payment-storefront --all-features
```
