# rustok-cart-storefront

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../../docs/UI/module-package-verification.md)

Leptos storefront UI package for the `rustok-cart` module.

## Responsibilities

- Exposes the module-owned storefront cart route used by `apps/storefront`.
- Shows cart read-side state, line items, and delivery-group snapshots from the cart boundary.
- Shows typed adjustment totals, language-neutral source identity, adjustment `scope`, and sanitized
  metadata from the cart boundary instead of collapsing everything into summary-only counters.
- Supports safe cart-owned line-item decrement and remove actions without taking over checkout orchestration,
  while repricing line items through the pricing resolver on quantity change.
- Uses native Leptos `#[server]` calls for monolith/hydrate builds through `HostRuntimeContext` and keeps GraphQL as the headless/CSR selected path.
- Keeps cart UI policy and display/view-model mapping in a framework-agnostic `core/` layer and keeps Leptos rendering in `ui/leptos.rs`, routing actions through a thin `transport` facade.
- Leaves checkout completion and broader cross-domain orchestration inside `rustok-commerce`.

## Entry Points

- `CartView` - root storefront view re-exported from `ui::leptos` and rendered from the host storefront slot registry.
- `core::view_model` helpers re-exported from `core` - framework-agnostic display mapping for cart summary, adjustments, delivery groups, and line items.
- `transport::fetch_cart` - thin UI-facing facade for build-profile-selected cart reads, while preserving core validation errors without retry.
- `transport::decrement_line_item` - thin UI-facing facade for safe line-item decrement with the same validation-vs-selected-path policy.
- `transport::remove_line_item` - thin UI-facing facade for safe line-item removal with the same validation-vs-selected-path policy.
- `transport/{native_server_adapter,graphql_adapter}.rs` internals - native `#[server]` and GraphQL adapter calls behind the selected-path facade.

## Interactions

- Consumed by `apps/storefront` via manifest-driven `build.rs` code generation.
- Reads guarded Cart owner ports through server functions, resolves DB/event bus handles from the host runtime context, and enforces customer-owned cart access with the host auth context.
- Stays compatible with locale-prefixed module routes via `UiRouteContext::module_route_base()`.
- Coexists with the `rustok-commerce` storefront/transport layer while checkout and shipping orchestration remain aggregate concerns.

## Documentation

- See [CRATE_API.md](CRATE_API.md) for full contract specification.
- See [platform docs](../../../../docs/index.md).

## Product buyer lifecycle

`use_buyer_cart` exposes read-only, actor-bound session state to Product and keeps checkpoint,
authorization fences and command execution in Cart. Native/GraphQL create/add use the Commerce
workflow and guarded Cart ports; restore is a read-only owner GET, not repricing. Known original
scope references and pending warning hints are preserved conservatively. See the [Cart plan](../docs/implementation-plan.md#product-buyer-continuation-2026-10-30).
The source is uncompiled/unrun; hints are not server receipts or a multi-tab/exactly-once protocol.


## Known Limitations / Pending Implementation

- Product step-8 Cart add now passes the Inventory owner explicitly into the shared Commerce
  workflow. Stock is probed for combined same-SKU demand in the canonical quote channel, not
  inferred from Product's global stock numbers; UUID/quantity mismatches and owner failure/timeout
  refuse before line insertion. Native, REST and GraphQL currently compose the in-process owner;
  this does not certify selected remote Inventory or reserve stock. SSR Cargo/lock dependency
  updates are manual, not resolver/compiler evidence. The
  [Commerce plan](../../rustok-commerce/docs/implementation-plan.md#product-downstream-continuation-2026-10-30)
  retains remaining owner/mounted/concurrency gates; regression sources are unrun.

- Quantity/decrement/remove now share the guarded Cart observation contract. Native decrement
  uses Commerce's selected-owner workflow, not raw CartService writes or native price math.
  Target-only removal preserves the original quantity and can reduce larger historical carts;
  it cannot authorize repricing. The shared native owner-error envelope is static and asks the
  user to inspect before retrying. These source changes do not provide durable replay/receipts,
  atomic context patch + repricing, remote execution or live authorization-revocation proof.
