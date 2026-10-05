# Admin shipping-option command owner-port cutover

Status: `source_complete_unvalidated`

## Scope

Mounted Commerce admin REST shipping-option writes now execute through the Fulfillment-owned
`ShippingOptionAdminCommandPort` published in the preceding owner-capability slice.

The cutover covers the four mounted handlers in `controllers/admin/shipping.rs`:

- `create_shipping_option`;
- `update_shipping_option`;
- `deactivate_shipping_option`;
- `reactivate_shipping_option`.

Those handlers no longer construct `rustok_fulfillment::FulfillmentService`.

## Runtime composition

`CommerceHttpRuntime` now carries a `ShippingOptionAdminCommandRuntime` and exposes only its typed
command port to the mounted handlers. A host-supplied runtime from `HostRuntimeContext` is preferred;
when none is supplied, Commerce selects the Fulfillment-owned in-process runtime, matching the
existing Fulfillment admin-command composition pattern.

Concrete `FulfillmentService` construction therefore remains inside `rustok-fulfillment` for this
command path.

## Preserved transport admission

The existing permissions and response envelopes remain unchanged:

- create requires `FULFILLMENTS_CREATE` and still returns `201` plus `ShippingOptionResponse`;
- update/deactivate/reactivate require `FULFILLMENTS_UPDATE` and still return
  `ShippingOptionResponse`;
- create/update still validate Commerce-owned shipping-profile slugs before crossing the owner
  boundary.

Each owner call now receives a bounded `PortContext` carrying the admitted tenant, authenticated
user actor, effective request locale, optional request channel, a stable correlation identity, a
two-second deadline, and the caller-owned `Idempotency-Key` unchanged through the owner write policy.

Shipping-option create now has durable owner receipt/replay semantics. The owner admits the caller key through
the shared receipt ledger and atomically commits the new Shipping Option, its translations, the translation
change-journal evidence, and the completed receipt. Reusing the same key with a different request is rejected
by the durable request hash. Update/deactivate/reactivate retain caller-owned idempotency admission and
state-setting semantics without create-style resource-allocation replay.

## Error boundary

The existing stable Commerce HTTP error families are preserved through the shared `PortError`
mapper. Raw owner/database details are not returned to clients. The mapper is shared by the existing
shipping-option reads and the new command calls and logs only bounded owner/context facts.

## Still open

The canonical ecommerce topology item
`Move remaining mounted Commerce REST/GraphQL construction of Product, Order, Payment, and
Fulfillment concrete services behind host-composed owner ports` remains open because other mounted
concrete owner construction still requires separate source slices.

Shipping-profile CRUD remains Commerce-owned and is intentionally outside this Fulfillment owner
cutover.

## Validation status

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, mounted REST
scenarios, workflows, CI reruns, database scenarios, restart scenarios, or remote-adapter scenarios
were executed for this slice. The accompanying verifier is source-only evidence for maintainer
execution.
