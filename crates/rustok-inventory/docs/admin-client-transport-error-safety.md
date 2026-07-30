# Inventory Admin client transport error safety

Status: **source-ready / unvalidated**

## Scope

This source slice covers the eight public Inventory Admin transport operations in
`crates/rustok-inventory/admin/src/transport/mod.rs`.

Covered operations:

- `fetch_bootstrap`
- `fetch_products`
- `fetch_product`
- `set_variant_quantity`
- `adjust_variant_quantity`
- `reserve_variant_quantity`
- `check_variant_availability`
- `release_reservation_quantity`

## Confirmed gap

The facade previously implemented `From<ServerFnError>` by storing
`value.to_string()` in `InventoryTransportError::ServerFn`. Every operation then
used `.map_err(Into::into)`. A framework, transport, serialization, or unexpected
server-function message could therefore become the public UI error text even when
the mounted server function itself attempted to use a static owner envelope.

## Source policy

Each facade operation now creates an `InventoryTransportErrorContext` after the
existing request normalization and before the unchanged native adapter call.
Only a final returned `ServerFnError` is mapped.

The original typed framework error is retained only in structured diagnostics
with:

- owner and exact operation;
- a per-call correlation id;
- the client transport boundary and a stable error code;
- request-field presence and character lengths;
- numeric-input presence without the quantity, adjustment, or requested quantity.

Tenant, product, variant, locale, search, status, and numeric values are not
written to the client transport diagnostic event.

`InventoryTransportError::ServerFn` is now a unit variant. No caller can attach a
raw string payload to the public error. Its displayed message is always:

`Inventory admin request could not be completed`

## Preserved behavior

This slice does not change:

- mounted server-function endpoints;
- authentication, tenant, permission, and owner policy;
- `HostRuntimeContext` or event-bus composition;
- request normalization;
- read or write invocation order;
- request and response DTOs;
- Inventory Admin native-only transport selection.

The server-side native adapter keeps its existing operation-specific static
messages and owner diagnostics. The new facade policy is an independent final
client boundary in case a framework or unexpected server-function string reaches
the caller.

## Evidence boundary

The retained JSON evidence is source-only. Tests, focused verifiers, Cargo,
formatting, workflows, CI, hydrate compilation, SSR compilation, browser behavior,
and mounted runtime failure behavior were not executed by this implementation
agent.

The broad ecommerce mapper-cleanup item remains open for other owners and other
non-`PortError` public envelopes.
