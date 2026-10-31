# apps/storefront / CRATE_API

## Purpose

`rustok-storefront` (`apps/storefront`) is the canonical Leptos 0.8 e-commerce storefront application host for RusToK. It serves as the primary customer-facing SSR application host, providing catalog browsing, product detail rendering, cart drawer integration, checkout flow handoff, account management, and manifest-driven storefront module mounting.

## Architecture & Responsibilities

- **Host & Composition Root:** Hosts and mounts module-owned storefront Leptos packages (`crates/modules/*/storefront`) via dynamic manifest registration and slot positioning (`StorefrontSlot`).
- **SSR & Anonymous Document Delivery:** Executes Server-Side Rendering (SSR) via Axum integration, serving cached, revalidated pages with ETags and secure script nonces. Ensures anonymous page deliveries omit unnecessary client-side bootstrap overhead.
- **Canonical Route & SEO Resolution:** Resolves canonical URLs, legacy path aliases, and localized route prefixes (`/ru/...`) via `canonical_route` and `rustok-seo-storefront`, rendering meta tags and JSON-LD structured data.
- **Storefront Module Registry:** Manages slot components (HeaderNavigation, HeaderActions, HomeAfterHero, HomeAfterCatalog, HomeBeforeFooter, FooterNavigation, CheckoutHandoffs) and route segments (`/modules/:route_segment`), fail-closed filtering against active tenant modules.
- **Dual Transport Integration:** Interacts with `apps/server` through native `#[server]` functions for monolith SSR and GraphQL queries for decoupled/client interactions, maintaining parity with `apps/next-frontend`.

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
2. **SEO & Nonce Isolation:** Only trusted structured data and required hydration scripts receive cryptographic `CspNonce` attributes; anonymous public documents do not leak script nonces.
3. **Locale Negotiation Priority:** Storefront routes resolve locale from explicit URL path prefix first, falling back to query parameter or the default locale (`en`).
4. **ETag Revalidation:** Storefront composite page responses emit private revalidation ETags bound to all underlying section hashes, allowing immediate 304 Not Modified cache hits.

## Dependencies & Boundaries

- **Upstream UI & Framework Crates:** Depends on `leptos`, `leptos_router`, `leptos_axum`, `leptos-auth`, `leptos-ui`, `leptos-ui-routing`, `rustok-ui-core`, `rustok-ui-i18n`, `rustok-ui-transport`, `rustok-graphql`, `rustok-web`.
- **Module Storefront Packages:** Mounts module storefront crates (`rustok-*-storefront`) via public component and page registrations.
- **Zero Cross-Module Direct Database Queries:** Interacts with modules exclusively through public APIs, ports, or GraphQL surfaces.
