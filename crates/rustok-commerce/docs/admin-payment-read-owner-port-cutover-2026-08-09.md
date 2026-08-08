# Commerce admin Payment read owner-port cutover — 2026-08-09

Status: source-complete for mounted admin Payment/refund GET routes; execution evidence pending and unvalidated.

## Scope

This slice advances the canonical ecommerce topology P0 without closing it. The four mounted admin read routes now cross a Payment-owned read capability instead of constructing `PaymentService` in Commerce:

- `GET /admin/payment-collections`;
- `GET /admin/payment-collections/{id}`;
- `GET /admin/refunds`;
- `GET /admin/refunds/{id}`.

Payment lifecycle/provider mutations are intentionally out of scope. The existing Commerce payment orchestration source remains mounted for authorize, capture, cancel, refund creation, refund completion, and refund cancellation and requires a separate owner-boundary slice.

## Owner contract

`rustok-payment` publishes `PaymentAdminReadPort` and `PaymentAdminReadRuntime` with complete projection operations for:

- filtered/paginated payment collection list plus total;
- payment collection detail;
- filtered/paginated refund list plus total;
- refund detail.

The in-process adapter is the only new source in this slice that constructs `PaymentService`; persistence, validation, filter normalization, ordering, response construction, and tenant-scoped owner reads stay inside `rustok-payment`.

## Mounted adapter

`crates/rustok-commerce/src/controllers/admin/mod.rs` now mounts `payments_owner_reads.rs` as the public `payments` module and loads the prior `payments.rs` as private `payments_legacy` compatibility source.

The mounted module owns the four read handlers and wildcard-reexports the legacy public compatibility surface so existing mutation handlers and generated OpenAPI path symbols remain available. Its local GET handlers shadow the legacy GET names, so mounted payment/refund reads execute only through the new owner port while mutation execution remains unchanged.

The mounted read adapter preserves:

- `PAYMENTS_READ` admission;
- the existing list query shapes and pagination behavior;
- payment collection `status`, `order_id`, `cart_id`, and `customer_id` filters;
- refund `payment_collection_id`, `order_id`, and `status` filters;
- existing payment/refund DTOs and totals;
- stable public validation/not-found/conflict/unavailable/internal error families.

## Context

Commerce translates trusted `TenantContext`, `AuthContext`, and `RequestContext` into `PortContext` with:

- tenant id;
- authenticated user actor;
- effective locale;
- optional resolved channel;
- deterministic correlation scope;
- bounded two-second read deadline.

The Payment owner enforces `PortCallPolicy::read()` and tenant UUID admission before storage access.

## Runtime selection

`CommerceHttpRuntime` first accepts a host-injected `PaymentAdminReadRuntime`. When the host has not selected an external adapter, it creates the built-in in-process Payment owner runtime from the host database connection. Commerce never constructs `PaymentService` for the mounted read routes.

This keeps an explicit external-adapter injection point without coupling this focused read cutover to the larger payment mutation/provider-runtime composition work.

## Source guard

`scripts/verify/verify-commerce-admin-payment-read-owner-port-cutover.mjs` source-locks:

- the mounted/legacy module split;
- Payment owner read port/runtime exports;
- all four mounted owner read calls;
- trusted context and deadline propagation;
- absence of `PaymentService` construction in the mounted adapter;
- host-injected runtime preference with in-process owner fallback;
- legacy mutation/OpenAPI compatibility re-exports.

The verifier is added but intentionally not executed in this slice.

## Remaining topology work

The canonical broad item “Move remaining mounted Commerce REST/GraphQL construction of Product, Order, Payment, and Fulfillment concrete services behind host-composed owner ports” remains open. The next Payment source slice should move the mounted authorize/capture/cancel/refund lifecycle orchestration behind Payment-owned execution capabilities while retaining current provider journal identities, reconciliation behavior, caller idempotency, permissions, and public envelopes.

No tests, Cargo commands, formatting, verifier execution, workflows, CI, runtime HTTP calls, or external-provider execution evidence are claimed here.
