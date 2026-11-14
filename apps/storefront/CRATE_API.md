# apps/storefront / CRATE_API

## Purpose

`rustok-storefront` (`apps/storefront`) is the canonical Leptos 0.8 e-commerce storefront application host for RusToK. It serves as the primary customer-facing SSR application host, providing catalog browsing, product detail rendering, cart drawer integration, checkout flow handoff, account management, and manifest-driven storefront module mounting.

## Architecture & Responsibilities

- **Host & Composition Root:** Hosts and mounts module-owned storefront Leptos packages (`crates/modules/*/storefront`) via dynamic manifest registration and slot positioning (`StorefrontSlot`).
- **SSR & Document Delivery:** Executes SSR via Axum integration. Home, generic module and canonical Forum HTML with a response nonce is `private, no-store`. Canonical Pages compositions without a response nonce may use private ETag revalidation when a complete composition identity is available; a nonce-bearing Pages composition is `private, no-store` and never returns a 304.
- **Canonical Route & SEO Resolution:** Resolves canonical URLs, legacy path aliases, and localized route prefixes (`/ru/...`) via `canonical_route` and `rustok-seo-storefront`, rendering meta tags and JSON-LD structured data.
- **Storefront Module Registry:** Manages slot components (HeaderNavigation, HeaderActions, HomeAfterHero, HomeAfterCatalog, HomeBeforeFooter, FooterNavigation, CheckoutHandoffs) and route segments (`/modules/:route_segment`), fail-closed filtering against active tenant modules.
- **Dual Transport Integration:** Interacts with `apps/server` through native `#[server]` functions for monolith SSR and GraphQL queries for decoupled/client interactions. Contract alignment with `apps/next-frontend` does not by itself establish mounted runtime parity.

## Public Surface

- `render_shell`: Asynchronously renders the full SSR HTML document shell with locale and query params.
- `storefront_routes`: Constructs Axum router configuration binding storefront endpoints, static asset fallbacks, and request context layers.
- `modules::registry`: Storefront registration types:
  - `StorefrontSlot`: Layout slots enum (`HeaderNavigation`, `HeaderActions`, `HomeAfterHero`, `HomeAfterCatalog`, `HomeBeforeFooter`, `FooterNavigation`, `CheckoutPaymentHandoff`, `CheckoutResultHandoff`, `CheckoutShippingHandoff`).
  - `StorefrontComponentRegistration`: Slot component registration descriptor.
  - `StorefrontPageRegistration`: Custom storefront page descriptor.
  - `StorefrontPageLookup`: Lookup result enum (`Found`, `Disabled`, `Missing`).
  - `register_component()`, `register_page()`: Dynamic manifest registration functions.
  - `components_for_slot()`, `page_for_route_segment()`: Query methods filtering by enabled tenant modules.
- `shared::context::canonical_route`: Canonical route resolution and redirect helpers (`ResolvedCanonicalRoute`, `build_module_route`, `build_redirect_location`).
- `shared::local`: Framework-neutral storefront host catalogs (`locale_strings`, `featured_products`).

## Invariants & Guarantees

1. **Fail-Closed Module Boundaries:** Pages or slot components from disabled modules are never served or rendered to storefront visitors.
2. **Nonce isolation:** A request's `CspNonce` is passed as `UiCspNonce` to module-owned trusted inline Page Builder styles/scripts in home slots as well as module routes; trusted SEO and enabled bootstrap scripts also use it. The host does not add a nonce to tenant-authored markup. The public `render_shell`/`render_module_page` helpers do not accept a request nonce; nonce-aware HTTP routes use the internal renderers.
3. **Locale Negotiation Priority:** Storefront routes resolve locale from explicit URL path prefix first, falling back to query parameter or the default locale (`en`).
4. **ETag revalidation:** Eligible non-nonce Pages compositions use private revalidation ETags bound to the complete composition. Nonce-bearing responses do not cache or revalidate; the real SSR/host tests and enforced-CSP deployment remain to be verified.
5. **Landing Cart boundary (source, unverified):** With built buyer assets and a nonce, one host-mounted Cart controller supplies the drawer, the `rustok:cart:*` bridge and the Product buyer. The host passes a tenant/locale marker, a fail-closed enabled-module projection, and guarded URL callbacks; Cart admission is still checked server-side. Counts are owner observations, not event acknowledgements. This integration requires locked builds and mounted/real-service verification; see the [storefront bridge](../../crates/modules/rustok-page-builder/storefront/README.md#purchase-and-cart-bridge).

## Dependencies & Boundaries

- **Upstream UI & Framework Crates:** Depends on `leptos`, `leptos_router`, `leptos_axum`, `rustok-auth-ui`, `rustok-ui-leptos`, `leptos-ui-routing`, `rustok-ui-core`, `rustok-ui-i18n`, `rustok-ui-transport`, `rustok-graphql`, `rustok-web`.
- **Module Storefront Packages:** Mounts module storefront crates (`rustok-*-storefront`) via public component and page registrations.
- **Zero Cross-Module Direct Database Queries:** Interacts with modules exclusively through public APIs, ports, or GraphQL surfaces.
