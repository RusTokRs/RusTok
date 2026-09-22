# Commerce REST storefront cart shipping owner-read cutover

Status: `source_complete_unvalidated`

## Scope

Mounted Commerce REST cart handlers now route Fulfillment shipping-option reads through the
host-selected `ShippingOptionReadPort` already carried by `CommerceHttpRuntime`.

The cutover covers two mounted cart behaviors:

- cart shipping enrichment uses `ShippingOptionReadPort::list_shipping_option_projections`;
- selected shipping-option validation during `POST /store/carts/{id}` uses
  `ShippingOptionReadPort::read_shipping_option_projection`.

No new Fulfillment owner contract is introduced. `CommerceShippingOptionReadRuntime` remains the
existing host-composed capability and the server composition remains unchanged in this slice.

## Mounted REST parity

The following mounted handlers now obtain `runtime.shipping_option_read_port()` before shipping
projection work:

- `POST /store/carts`;
- `GET /store/carts/{id}`;
- `POST /store/carts/{id}`;
- `POST /store/carts/{id}/line-items`;
- `POST /store/carts/{id}/line-items/{line_id}`;
- `DELETE /store/carts/{id}/line-items/{line_id}`.

Cart context update preserves the previous operation order: resolve requested StoreContext, validate
selected shipping options, update Cart owner context, reprice line items, then enrich shipping
projections.

The selected-option compatibility policy is unchanged:

- multi-delivery-group carts still reject the legacy single selected-option shortcut;
- option currency must match cart currency case-insensitively;
- option metadata must be visible for the current public channel;
- option shipping-profile compatibility still uses normalized profile slugs with `default` fallback.

Shipping enrichment preserves currency filtering, public-channel filtering, shipping-profile
compatibility, delivery-group projection, and single-group selected-option reconciliation.

## Owner call context

Fulfillment owner reads reuse the existing storefront cart `PortContext` builder. Calls carry:

- admitted tenant id;
- authenticated user actor when present, otherwise the existing stable storefront service actor;
- request locale;
- request channel slug when present;
- cart/operation-bound correlation identity;
- two-second deadline.

The calls are reads and do not add an idempotency key.

## Error compatibility

The REST shipping boundary maps bounded `PortError` kinds to the established public families:

- validation -> `commerce_store_shipping_invalid` / 400;
- not found -> `commerce_store_not_found` / 404;
- conflict -> `commerce_store_shipping_state_conflict` / 409;
- unavailable or timeout -> `commerce_store_shipping_unavailable` / 503.

Unexpected forbidden or invariant owner failures fail closed as
`commerce_store_shipping_failed` / 500.

Diagnostics retain correlation identity, non-sensitive tenant/cart shape, owner error kind, owner
code length, retryability, selected public code, and status. Raw owner/backend messages are not
logged or returned by the new mounted helper.

## Deliberately still open

This is a bounded mounted REST cart slice. Existing compatibility/shared helpers outside the new
mounted cart adapter are not removed in this change, and GraphQL/shared shipping compatibility
source is not promoted or rewritten here.

The canonical ecommerce topology item remains open:

`Move remaining mounted Commerce REST/GraphQL construction of Product, Order, Payment, and Fulfillment concrete services behind host-composed owner ports.`

A fresh mounted-path audit is still required before closing that broad P0.

## Validation status

No tests, Cargo commands, Node verifiers, formatter, REST scenarios, workflows, CI reruns, database
scenarios, provider calls, restart scenarios, or remote-adapter scenarios were executed for this
slice, per maintainer instruction. The source verifiers are retained for maintainer execution.
