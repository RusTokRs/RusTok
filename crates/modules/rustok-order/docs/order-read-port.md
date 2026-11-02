# Order read port

Status: owner port and host runtime published; all mounted complete/post-order reads cut over, unvalidated.

## Scope

`OrderReadPort` is the order-owned boundary for complete order, return, and
order-change projections needed by mounted Commerce REST and GraphQL queries, plus
the bounded saved Order-line inventory bindings used by return-restock orchestration.
It is separate from:

- `CheckoutCompletionPort`, which owns checkout create/place/replay;
- `CheckoutOrderIdentityPort`, which owns checkout/order identity;
- `CheckoutOrderCompensationPort`, which owns checkout cancellation recovery;
- `CheckoutOrderPaymentSettlementPort`, which owns captured-payment settlement;
- order lifecycle, return, and order-change mutations, which remain on existing
  order-owner commands and services.

The owner boundary and host-selected runtime are published. Mounted admin REST,
mounted GraphQL, and storefront HTTP complete-order and post-order query consumers
use the port. Runtime evidence remains unvalidated. The superseded admin GET
functions have already been removed from the admin return/change source; compilation
and mounted-parity verification remain open. The original six-projection publication
is retained as historical evidence, not the current operation inventory.

## Operations

The owner publishes seven read-only operations:

- `read_order_projection` returns one complete `OrderResponse` by order id;
- `list_order_projections` preserves page, per-page, status, customer filtering,
  descending creation ordering, and owner pagination total;
- `read_order_return_projection` returns one `OrderReturnResponse` by return id;
- `list_order_return_projections` preserves page, per-page, order-id/status filters,
  descending creation ordering, return items, and owner pagination total;
- `read_order_change_projection` returns one `OrderChangeResponse` by change id;
- `list_order_change_projections` preserves page, per-page, order-id/status/type
  filters, descending creation ordering, and owner pagination total;
- `read_order_line_inventory_bindings` returns a nonempty, distinct batch of at most
  1024 saved Order-line Variant/quantity bindings for one tenant-owned Order. Missing,
  foreign or partial batches refuse; explicit `variant_id: None` alone means SKU-less.
  The nullable field is required on deserialization: omission is not an implicit null.

Complete order operations use `PortContext.locale` as requested locale and accept
an optional tenant-default fallback locale. Return and order-change DTOs are not
localized in the current owner model, so their typed requests do not invent locale
fields. The original six operations preserve their complete owner DTOs. The new saved-binding
batch has an Order-owned price-free DTO, not a partial Commerce-owned Order model. It
never reads current Catalog/Pricing, translates labels, or refreshes eligibility.

## In-process adapter

`InProcessOrderReadPort` owns concrete `OrderService` construction. The root
factory is:

```rust
in_process_order_read_port(db, event_bus)
```

Every operation:

1. requires `PortCallPolicy::read()`;
2. parses tenant identity from `PortContext`;
3. delegates to the existing owner service operation;
4. maps every current `OrderError` variant to stable `PortError` policy;
5. preserves the operation's owner projection: the original complete DTOs/filters/totals
   or the bounded saved SKU/quantity batch.

The adapter does not inspect owner error messages for control flow. Validation
maps to `Validation`; missing order/return/change maps to `NotFound`; invalid
transitions map to `Conflict`; database failures map to retryable `Unavailable`;
and owner-core failures fail closed as non-retryable `InvariantViolation`.

## Commerce runtime composition

`CommerceOrderReadRuntime` contains one host-selected `Arc<dyn OrderReadPort>` and
exposes:

- `new` for an externally selected adapter;
- `in_process` for the deterministic local adapter;
- `order_read_port` for consumer injection.

The default application host:

1. reuses an existing `CommerceOrderReadRuntime` from `ServerRuntimeContext`;
2. otherwise obtains the shared `TransactionalEventBus` and builds the in-process
   runtime once;
3. caches the runtime in `ServerRuntimeContext`;
4. attaches the same value to `HostRuntimeContext`.

`CommerceHttpRuntime` requires this value. Commerce GraphQL schema-data composition
also requires it. The mounted resolver extension scopes the same runtime into the
safe-query compatibility facade.

The resolver extension separately scopes request-owned order context:

- authenticated actors come only from validated `AuthContext` data;
- unauthenticated reads use the stable `rustok-commerce.graphql-order-query`
  service actor;
- channel and effective locale come from the host-resolved `RequestContext`;
- complete-order methods preserve their explicit requested/fallback locale;
- directly embedded schemas without the mounted extension use the service actor,
  no channel, and the truthful `und` locale marker for non-localized post-order
  calls rather than inventing attribution.

## Complete order consumer cutover

Admin REST order list/detail, mounted GraphQL order list/detail, storefront order
detail, and the shared storefront customer-ownership check use the host-selected
port. Existing authorization, locale fallback, filters, pagination, detail
aggregation, public envelopes, authenticated actor, resolved channel, correlation,
and two-second deadline behavior are preserved.

## Storefront post-order read cutover

`GET /store/orders/{id}/returns` calls `list_order_return_projections` after the
shared typed ownership check. It preserves customer resolution, exact order
ownership, filters, descending ordering, complete items, owner total, and the
existing `PaginatedResponse<OrderReturnResponse>` envelope.

`GET /store/orders/{id}/changes` calls `list_order_change_projections` after the
same ownership check. It preserves page/per-page, order/status/type filters,
descending ordering, owner total, and the existing
`PaginatedResponse<OrderChangeResponse>` envelope.

Both handlers use the validated user actor, resolved channel, request locale,
resource-scoped correlation id, two-second deadline, and stable storefront public
error policy.

## GraphQL post-order read cutover

The Commerce safe-query compatibility facade calls the original six complete/post-order
projection operations. The saved-binding batch is consumed by return-restock orchestration,
not added as a public query. Return/order-change detail and list methods use the scoped host-selected runtime
instead of constructing `OrderService`. Existing DTOs, filters, totals, and query
method signatures are unchanged.

The facade stores only `Arc<dyn OrderReadPort>`. It retains the resolved actor,
channel, effective locale, correlation id, and two-second deadline. Typed
`NotFound` results map back to `OrderReturnNotFound` or
`OrderChangeNotFound`, preserving the existing GraphQL error boundary.

## Admin post-order read cutover

The mounted admin router now directs these four GET endpoints to the dedicated
`post_order_reads` module:

- `GET /admin/returns` -> `list_order_return_projections`;
- `GET /admin/returns/{id}` -> `read_order_return_projection`;
- `GET /admin/order-changes` -> `list_order_change_projections`;
- `GET /admin/order-changes/{id}` -> `read_order_change_projection`.

The handlers preserve the existing `ORDERS_READ` authorization contract, request
and response DTOs, optional filters, descending owner ordering, clamped pagination,
owner totals, public HTTP codes/messages, validated user actor, resolved channel,
effective request locale, resource-scoped correlation id, and two-second deadline.
All POST routes remain mounted to their existing mutation/orchestration services.

The superseded GET functions in `admin/returns.rs` and `admin/changes.rs` have
already been removed from the source. The mounted router now has only the dedicated
`post_order_reads` handlers for these four endpoints. Compile, mounted-parity,
deadline/failure, restart, and remote-adapter evidence remain open.

## Unchanged behavior

This source wave deliberately leaves these paths unchanged:

- storefront return creation uses the existing selected `OrderPostOrderCommandPort`
  after typed ownership validation;
- storefront refund listing uses the existing selected `PaymentOrderReadPort` after
  typed ownership validation;
- admin order, return, and order-change mutations remain on their current owner or
  orchestration services;
- admin order detail payment and fulfillment aggregation remain unchanged.

No mutation, payment, or fulfillment policy moved into the read port.

## Context and diagnostics

The owner boundary retains operation, correlation id, tenant, actor, channel
length, requested/fallback locale lengths, causation/trace presence, deadline,
order/return/change/customer/filter facts, stable owner code, error kind, and
retryability. Only database and core failures retain the typed internal cause in
technical logs. Public `PortError.message` values are stable and do not contain raw
storage or owner-invariant details.

## Remaining source work

1. compile all seven owner read operations, verify the original six mounted query
   projections, and exercise the saved-binding consumer through return completion;
2. retain deadline/failure, restart, and remote-adapter evidence before status
   promotion.

## Evidence

Source evidence is retained at:

`crates/modules/rustok-order/contracts/evidence/order-read-port-source.json`

Its status is `mounted_consumer_cutover_unvalidated`. The historical 2026-07-30
publication recorded six owner projection reads. The current source inventory records
those six plus the mandatory saved-binding batch, host composition, and the mounted
admin REST, GraphQL, and storefront query cutovers. Compatibility-handler source removal
is recorded; compilation, mounted parity, deadline/failure execution, restart and
remote-adapter evidence remain false or open. Executed source-verifier/format flags
are separate from runtime and regression evidence.

## Intended checks

```bash
node scripts/verify/verify-order-read-port.mjs
node scripts/verify/verify-commerce-graphql-order-read-shim.mjs
node scripts/verify/verify-commerce-storefront-order-read-cutover.mjs
node scripts/verify/verify-commerce-storefront-post-order-read-cutover.mjs
node scripts/verify/verify-commerce-admin-post-order-read-cutover.mjs
node scripts/verify/verify-commerce-admin-order-route-error-context.mjs
cargo check -p rustok-order --lib
cargo check -p rustok-commerce --lib
cargo check -p rustok-server --features mod-commerce
```

The original publication did not execute tests, Cargo, formatting, verifiers, workflow
checks or CI. The 2026-10-31 continuation executed only the scoped source guards and
syntax/format checks recorded in the [return source handoff](../../rustok-commerce/docs/implementation-plan.md#return-restock-identity-source-evidence).
No tests, Cargo commands, SQL, mounted runtime, workflows or CI were executed.

## Saved-binding return-restock continuation (2026-10-31)

The existing `ReturnCompletionOrchestrationService` receives the host-selected read port;
`prepare_restock_plan` invokes that same capability, once for a nonempty goods selection.
The in-process implementation delegates through `OrderService` to a single bounded query
joining Order lines to their tenant-owned Order. It selects only IDs, saved Variant and
quantity and returns all requested lines or an error. It does not add a read provider,
public HTTP/GraphQL operation, price lookup or embedded fallback.

Commerce validates response tenant/Order/set/uniqueness/quantity before resolution effects,
freezes the plan, and requires matching completed-return item facts before Inventory writes.
See the [source handoff](../../rustok-commerce/docs/implementation-plan.md#return-restock-identity-source-evidence).
The added operation and regressions remain uncompiled/unexecuted; source checks do not
certify SQL, failure timing, mounted clients, remote parity or readiness.
