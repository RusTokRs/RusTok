# @rustok/pages-frontend

> Storefront package for the `rustok-pages` module in the Next.js storefront (`apps/next-frontend`).

## Responsibilities

- Render published Page Builder landing pages directly via SSR without an `<iframe>`.
- Provide an Islands Architecture bridge (`FlyActionBridge`) that confines `fly:action` events to their landing, requests Cart controls, and hands purchases to Product for SKU validation, pricing and confirmation.
- Read a published page together with its published landing in one GraphQL request (`fetchPublishedPage`: id, status, locale, SEO fields and `landing { locale html css artifactHash }`).

## Architecture & Rendering Model

Published Page Builder landing artifacts in RusToK are pre-compiled and sanitized by the Page Builder compiler into static landing artifacts stored in PostgreSQL (`page_static_landing_artifacts`).

The published landing is the only source of markup the storefront renders. It is read as the typed GraphQL field `pageBySlug.landing`: body markup and stylesheet as separate fields, taken from the artifact that the reviewed publish bound to the page. The page body (`pageBySlug.body`) is the authoring document (`format: "grapesjs"`): it is not sanitized for rendering, the server returns it only to callers that may save or publish it, the storefront query does not request it, and no code path turns it into markup. A page without a published landing for the requested locale and channel, and a failed read, result in `notFound()`.

Rather than isolating the public storefront within a sandboxed `<iframe>`, `@rustok/pages-frontend` performs direct SSR HTML and CSS injection:

1. **Server Component (`PageBuilderContent`):**
   - Automatically scopes the stylesheet via `scopeLandingCss` using modern CSS `@scope (.fly-page-container)` and remaps root selectors (`body`, `html`) to `:scope`.
   - Applies `style={{ isolation: "isolate", contain: "style layout" }}` to the container: its own stacking context, and `position: fixed`/`absolute` content of the landing is laid out against the container instead of the viewport. Keeping landing *styles* off the host shell, navbar and footer is the job of the `@scope` wrapper above, not of `contain`.
   - `@scope` requires Chrome/Edge 118+, Safari 17.4+ or Firefox 128+; older browsers ignore the block and render the landing unstyled (the host is unaffected).
   - Injects the scoped stylesheet `<style>` into the document render stream.
   - Injects the sanitized HTML into `<div className="fly-page-body" dangerouslySetInnerHTML={{ __html: html }} />`.
   - Mounts the client island `<FlyActionBridge />`.

2. **Client Island (`FlyActionBridge` + canonical `fly-actions.js`):**
   - Side-effect imports `components/fly-actions.js` — a byte-exact copy of the canonical runtime asset the Rust storefront inlines (`crates/ui/fly/browser/assets/fly-actions.js`); it self-starts idempotently (SSR-safe) and exposes `globalThis.FlyActions`.
   - Natively handles DOM actions: `toggle_visibility`, `switch_tab`, `toggle_accordion`, `switch_slide` (incl. touch/swipe), `scroll_to`, `animate`. Targets and group traversal stay within the nearest landing, including when the host or another landing nests a group.
   - Handles host `emit_event` actions only; provider actions stay with their provider. Cart open/close/toggle use the actual Cart capability; a disabled Cart reports localized feedback and its count is unknown, not zero. Handler and counter updates occur in a layout effect so they reflect a committed capability change before subsequent host layout effects.
   - `add_to_cart`, `cart:add` and `buy_now` navigate to the Product buyer rather than mutating Cart from unpriced landing data. Hash targets stay inside their landing, and safe root-relative links use Next client routing.

- `PageBuilderContent` — React Component rendering scoped HTML + CSS + client action bridge.
- `FlyActionBridge` — Client Component handling interactive actions, cart integration, and dynamic island sync.
- `scopeLandingCss` — Utility wrapping styles with `@scope (.fly-page-container)` and remapping root selectors.
- `fly-actions.js` — Vendored byte-exact copy of the canonical action runtime asset (`crates/ui/fly/browser/assets/fly-actions.js`); imported for side effects only, typed by the sibling `fly-actions.d.ts` (`tsc` resolves `.js` → `.d.ts`; `allowJs` stays `false`). Never edit the copy by hand — re-copy it from the crate and keep `npm run verify:fly-actions-parity` green.
- `fetchPublishedPage` — One GraphQL read of the published page and its landing; returns `null` when no published page matches the slug and throws on any GraphQL error. `PageDetail`, `PageTranslation` and `PublishedLanding` are its types.

### Shared purchase handoff

A landing `emit_event` for `add_to_cart`, `cart:add` or `buy_now` supplies a non-nil UUID
`variantId`, optional positive i32 `quantity` (default 1 only if omitted), and a same-origin,
root-relative `buyerHref` in the canonical shape `/[locale]/products/[handle]` or
`/products/[handle]`. Example:

```json
{"buyerHref":"/ru/products/cup?currency=AED","variantId":"11111111-1111-4111-8111-111111111111","quantity":2}
```

The Next host passes this to its Product page with `variant_id` and `quantity`; Leptos maps the
same payload to its `/[locale]/modules/products?handle=…` route. Duplicate query keys or supplied
`handle`, `variant_id`, `quantity`, `cart_id` or `axis.*` are refused. Product validates the SKU
and price context and asks for confirmation. A landing is not an authoritative price quote, and
the bridge does not call `addItem` or infer a locale, currency or product variant. See the
[Leptos bridge contract](../../../../crates/modules/rustok-page-builder/storefront/README.md#purchase-and-cart-bridge).

## Caching

The page route is rendered per request and keeps nothing in the Next.js data cache: `storefrontGraphql` sends every request with `cache: "no-store"`. `generateMetadata` and the page component share one read per request through React `cache`.

The cache of the published landing lives in one place, on the server: `pageBySlug.landing` is read through the generation-aware Pages cache (60 s TTL, key rotated by every page event), the same cache the Rust storefront and the REST artifact endpoint use. A publish, an unpublish or a channel change is therefore visible on the next request, and there is no second cache to invalidate: the application has no revalidation endpoint and no revalidation secret.

## Preview

The storefront renders published content only and holds no author credential, so it has no Draft Mode. The working copy of a page is previewed in the Page Builder; the admin links to the storefront only for published pages.

## Storefront Parity

Both storefronts render the published landing and nothing else, and both receive it in the same shape (`locale`, `html`, `css`, `artifactHash`): this package and the headless Leptos build from `pageBySlug.landing`, the Leptos monolith build from its server adapter, which calls the same artifact owner. The canonical Leptos host route decides `not_found` for a published page without a landing, as Next does; the separately embedded Leptos page card can still render an empty-state notice.

`scopeLandingCss` has a Rust twin, `scope_landing_css` in `crates/modules/rustok-page-builder/storefront/src/css_scope.rs`, used by the Leptos storefront. Both must return the same text for the same stylesheet and selector. The shared vectors live in `crates/modules/rustok-page-builder/contracts/landing-css-scope-parity.json` and are asserted by `src/pages-frontend.test.mjs` and by the Rust unit tests; change the two implementations together and add a vector for every behaviour change. `apps/next-frontend/tests/e2e/landing-css-scope.spec.ts` gives the same vectors and a list of unbalanced and hostile stylesheets to a browser: for each one the CSS object model must hold exactly one `@scope` rule, the `<style>` element must hold all of the text, and a host element outside the landing must keep its style.

## Known Limitations / Pending Implementation

- **No fallback for browsers without `@scope`.** They drop the block and render the landing unstyled; there is no selector-prefixing fallback.
- **At-rules that define document-wide names are not confined.** `@keyframes`, `@font-face`, `@property` and similar keep their names global where a browser accepts them inside `@scope`.
- **No channel scoping.** The page request carries the tenant and the locale; the Next storefront has no channel selection, so the server applies its default channel resolution to both the page and its landing.
- **Landing markup is injected as received.** The package trusts the sanitized landing markup and adds no second sanitization pass before `dangerouslySetInnerHTML`.
- **Strict Next CSP host wiring is implemented, but acceptance is incomplete.** The Next storefront proxy issues a request-local nonce and strict policy in production (or with `RUSTOK_STRICT_CSP=1`); the published-page route passes the validated nonce to `PageBuilderContent`, whose single scoped landing `<style>` carries it and has no style attribute. The production HTTP test (`npm run test:csp-runtime` after `npm run build`) verifies nonce/policy agreement and rotation on the home response, rejection of a client-supplied nonce, no-store headers and nonce-compliant custom 404. It does not exercise a published landing or blog JSON-LD against live services. Fatal `global-error` rendering, Chromium behavior, real services and deployed CSP remain unverified.
- **Every page view costs one GraphQL request.** The Next storefront keeps no cache of its own; the server answers the landing part of that request from the Pages cache. The page record (status, translation, channel list) is read from the database on every request.
- **Mounted acceptance remains open.** `npm --prefix apps/next-frontend run test:e2e:fly` exercises the shipped browser assets and real React bridge, Product buyer and Cart components with fake router/translation/GraphQL boundaries, not a deployed Next host, live Product/Pricing services, or full accessibility/CSP/navigation behavior. It requires Playwright's Chromium (`npm --prefix apps/next-frontend run test:e2e:install`) or `PLAYWRIGHT_CHROMIUM_EXECUTABLE`. The Leptos Cart event consumer and the Leptos Product buyer's host-capability guard remain unwired.

