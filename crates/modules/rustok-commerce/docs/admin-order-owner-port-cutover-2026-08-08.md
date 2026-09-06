# Commerce admin Order owner-port cutover — 2026-08-08

Status: source-complete for the mounted admin Order route, execution evidence pending and unvalidated.

## Scope

This slice moves the mounted `/admin/orders` route family away from direct construction of Order, Payment, and Fulfillment owner services. The HTTP surface remains a Commerce transport adapter: it owns permission checks, trusted request-context translation, public HTTP error envelopes, and response assembly, while business persistence and lifecycle semantics stay behind owner ports.

The canonical broad ecommerce topology task remains open because other mounted Commerce REST/GraphQL surfaces still construct owner services directly.

## Mounted route

`crates/rustok-commerce/src/controllers/admin/mod.rs` mounts `orders_owner_ports.rs` as the `orders` module. The previous `orders.rs` source remains in the tree for compatibility/history but is not the mounted admin Order implementation.

The mounted adapter uses:

- `CommerceOrderReadRuntime` / `OrderReadPort` for order list/detail reads;
- `OrderAdminCommandRuntime` / `OrderAdminCommandPort` for mark-paid, ship, deliver, and cancel lifecycle commands;
- `PaymentOrderReadRuntime` / `PaymentOrderReadPort` for latest payment collection by Order;
- the existing `CommerceFulfillmentLifecycleReadRuntime` / `FulfillmentReadPort` for latest fulfillment by Order.

The mounted adapter does not import or construct `OrderService`, `PaymentService`, or `FulfillmentService`.

## Owner boundaries

Order now exposes a transport-neutral admin lifecycle command port. Its in-process implementation delegates to the existing `OrderService` inside the Order owner crate, so existing serialized lifecycle transitions, outbox publication, validation, and persistence remain owner-controlled.

Payment now exposes an Order-scoped read capability for latest payment collection lookup. Its in-process implementation delegates to the existing `PaymentService` inside the Payment owner crate.

Fulfillment already exposed `find_latest_fulfillment_by_order_projection`; Commerce reuses that existing owner read capability instead of creating another adapter.

## Host composition

`CommerceHttpRuntime` requires the new Order command and Payment Order-read runtimes from `HostRuntimeContext`. The server composition prefers a runtime already selected by the host, then an existing server-shared runtime, and only then composes the built-in in-process owner adapter. This prevents the mounted Commerce route from silently replacing a host-selected external provider with local persistence.

## Context and write admission

The HTTP adapter carries trusted tenant/user identity, locale, optional channel, correlation identity, deadline, and a transport-owned non-empty attempt id into `PortContext`. The attempt id satisfies the generic write-port admission contract without changing the public legacy HTTP body/permission surface.

This slice does **not** claim durable Order command replay or lost-response recovery. A generated transport attempt id is not a durable owner receipt. Durable command idempotency, if required for these lifecycle transitions, remains a separate owner concern.

## Public surface preservation

Permissions remain:

- list: `orders:list`;
- detail: `orders:read`;
- mark-paid / ship / deliver / cancel: `orders:update`.

The mounted route retains the existing public status/code/message families for validation, not-found, state conflict, unavailable storage, and safe internal failure. Owner technical details stay behind bounded `PortError` diagnostics.

## Source guard

`scripts/verify/verify-commerce-admin-order-owner-port-cutover.mjs` source-locks the mounted module path, owner runtime requirements, owner port calls, absence of concrete service construction in the mounted route, and host-composition preference for externally selected runtimes.

The verifier was added in this source slice but was not executed.

## Remaining ecommerce topology work

The canonical topology item remains unchecked until all mounted Commerce REST/GraphQL consumers are behind host-composed owner capabilities. Follow-up source slices should inspect the remaining Payment, Fulfillment, Order post-order, checkout/orchestration, and GraphQL construction sites individually rather than treating this admin Order cutover as global completion.

No tests, Cargo commands, formatting, verifier execution, workflows, CI, runtime HTTP calls, or external-provider execution evidence are claimed by this slice.
