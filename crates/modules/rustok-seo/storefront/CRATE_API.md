# Minimum Contract Set

- **Crate**: `rustok-seo-storefront`
- **Category**: Storefront UI / Module Adapter
- **Primary Owner**: SEO Module Domain Team
- **Purpose**: Provides storefront rendering adapters, framework-neutral SEO data structures, dual-path GraphQL transport queries, and HTML `<head>` meta-tag generation for storefront hosts.

## Platform Capability Bindings

- **Runtime**: Leptos 0.8 / SSR & Hydrate
- **Transport**: GraphQL (`SEO_PAGE_CONTEXT_QUERY`)
- **Sanitization**: `rustok_core::html_escape` for attribute and tag escaping
- **Content Security Policy**: Nonce-aware `<script type="application/ld+json">` rendering via `render_head_html_with_nonce`

## Domain Collaborations

- **Upstream Module**: `rustok-seo` domain module (provides `SeoPageContext`, `SeoDocument`, `SeoRouteContext`, and structured data definitions)
- **UI Hosts**: `apps/storefront` and headless storefront clients consuming page SEO metadata

## Public surface

### Types & Models
- `SeoPageContext`
- `SeoRouteContext`
- `SeoDocument`
- `SeoRobots`
- `SeoOpenGraph`
- `SeoTwitterCard`
- `SeoVerification`
- `SeoVerificationTag`
- `SeoPagination`
- `SeoStructuredDataBlock`
- `SeoSchemaBlockKind`
- `SeoFieldSource`
- `SeoFieldState`
- `SeoAlternateLink`
- `SeoLinkTag`
- `SeoMetaTag`
- `SeoImageAsset`
- `SeoRedirectDecision`

### Functions & Rendering
- `render_head_html(context: &SeoPageContext) -> String`
- `render_head_html_with_nonce(context: &SeoPageContext, csp_nonce: Option<&str>) -> String`

### Transport & Queries
- `SEO_PAGE_CONTEXT_QUERY`: GraphQL query document for fetching page SEO context
- `SeoPageContextVariables`: variables struct for page context query
- `SeoPageContextResponse`: deserialization DTO for page context response

### Components
- `SeoView`: hidden SEO view component for Leptos storefront templates

## Domain Invariants

1. **Deterministic Ordering**: Rendered tags in HTML `<head>` follow strict canonical ordering: description meta, canonical link, robots meta, alternate links, OpenGraph tags, Twitter card tags, search engine verification tags, pagination links, structured data (JSON-LD), custom link tags, and custom meta tags.
2. **HTML & Breakout Escaping**: All dynamic content is passed through `rustok_core::html_escape`. Structured data blocks escape script breakout tags (`</script>` -> `<\/script>`).
3. **CSP Nonce Support**: When a nonce is provided, all JSON-LD `<script>` elements carry the `nonce="..."` attribute.
4. **Framework Isolation**: Pure rendering logic resides in `core.rs` / `render.rs` with zero UI framework dependencies.

## Errors/Failure Codes

- Storefront rendering does not panic on missing or empty fields; it safely omits non-present or empty tags.
- GraphQL query transport failures are modeled at the network/client transport layer via standard Result types.

## Observability/Trace Context

- Storefront head rendering is stateless and side-effect free.
- Query execution logs and metrics are emitted by the consuming host application or transport adapter.

## Maintenance & Lifecycle

- **MSRV**: 1.85.0
- **Documentation**: Generated via `cargo doc -p rustok-seo-storefront --no-deps`
- **Tests**: Locked via unit tests and `contract_tests.rs`.
