# RusToK Page Builder Storefront

Leptos storefront renderer for Page Builder documents with an optional authenticated inline editor.

## Responsibilities

- Render canonical Page Builder documents for storefront routes via `PageBuilderStorefront`.
- Render pre-compiled immutable static landing artifacts directly via `StaticLandingStorefront` (direct SSR HTML and scoped CSS without `<iframe>`).
- CSS scoping: every surface that emits project CSS (`PageBuilderStorefront`, `StaticLandingStorefront`, `LocalizedPageBuilderStorefront`, and the inline editor) wraps it in a native CSS `@scope` rule and remaps the document-level selectors (`body`, `html`, `:root`) to `:scope`, so landing styles do not reach the host shell, navbar, or footer. The scope root is the landing body element the surface renders itself (`LANDING_SCOPE_SELECTOR`, `[data-fly-page-body]`), not the `class` a host may replace. The container's `isolation: isolate; contain: style layout;` gives it its own stacking context and containing block; it does not confine CSS rules.
- Event and Navigation Bridge: inlines `FLY_STOREFRONT_BRIDGE_JS` for validated Product handoffs, Cart open/close/toggle requests and landing-local hash links. It never submits a purchase or assigns a price itself.
- Inline lightweight `fly-actions.js` runtime when interactive actions (`data-fly-action`) are present. Trusted script/style elements receive the host's `UiCspNonce`, when supplied; author markup never receives it.
- Resolve locale-aware routes.
- Mount Fly inline editing only when the feature and host authorization allow it.

## Interactions

The crate consumes `rustok-page-builder`, `fly`, `fly-browser`, and optionally `fly-leptos`. Storefront hosts provide route, locale, and authorization context.
Published pages use `StaticLandingStorefront` to avoid an `<iframe>`. CSS rule containment has the limitations below; the same published action payload must work on both storefronts.

### Purchase and Cart bridge

For an `emit_event` named `add_to_cart`, `cart:add` or `buy_now`, the payload must supply a
non-nil UUID `variantId` and a root-relative `buyerHref` shaped as
`/[locale]/products/[handle]` (or `/products/[handle]`); optional `quantity` is a positive i32,
defaulting to 1 only when omitted. For example:

```json
{"buyerHref":"/ru/products/cup?currency=AED","variantId":"11111111-1111-4111-8111-111111111111","quantity":2}
```

The Next host routes that URL to its Product page. Leptos maps the same payload to
`/ru/modules/products?handle=cup&currency=AED&variant_id=…&quantity=2`. Duplicate query keys,
`handle`, `variant_id`, `quantity`, `cart_id` and `axis.*` in the supplied URL are rejected rather
than overwritten. The host resolves locale support; Product validates the actual SKU, obtains a
Pricing observation and asks the user to confirm. The bridge does not guess a SKU, price, locale or
currency and does not call Cart's add mutation. Invalid payloads show localized feedback.
Provider actions are left to their provider rather than interpreted as host events.

Cart open/close/toggle requests are cancelable `rustok:cart:{open,close,toggle}` events; an actual
host consumer must acknowledge an accepted request with `preventDefault()`. Without acknowledgement
the landing reports `cart_unavailable`. A Cart owner can broadcast `rustok:cart:updated` with a
non-negative safe-integer `count` (or `itemCount`), or `{status: "unknown"}` to invalidate a previous
observation on a scope/restore change. Counts start unknown (`—`); on install the bridge asks the
owner for its current observation with `rustok:cart:request-count`, and newly inserted landing
bodies are synchronized. Neither an event nor an acknowledgement proves a cart mutation.

## Entry points

- `PageBuilderStorefront` — dynamic/preview renderer from GrapesJS project data with scoped CSS.
- `StaticLandingStorefront` — direct SSR renderer for pre-compiled landing artifact HTML and scoped CSS.
- `scope_landing_css` / `LANDING_SCOPE_SELECTOR` (`src/css_scope.rs`) — pure CSS scoping and root-selector remapping. It is the Rust twin of `scopeLandingCss` in `apps/next-frontend/packages/rustok-pages`; both are asserted against the shared vectors in `../contracts/landing-css-scope-parity.json` and must change together.
- `FLY_STOREFRONT_BRIDGE_JS` — browser bridge for `fly:action` cart and navigation events.
- `fly_browser` — re-export of Fly browser and action assets (`FLY_ACTIONS_JS`).
- `src/inline_edit.rs` — feature-gated editor integration.

## Known Limitations / Pending Implementation

- **No fallback for browsers without `@scope`.** It needs Chrome/Edge 118+, Safari 17.4+ or Firefox 128+. An older browser drops the whole block: the landing renders unstyled there (the host is unaffected). There is no selector-prefixing fallback.
- **At-rules that define document-wide names are not confined.** `@keyframes`, `@font-face`, `@property` and similar keep their names global where a browser accepts them inside `@scope`; a landing can still collide with a host animation or font name.
- **Landing markup is not scoped, only its stylesheet.** The body HTML is emitted as produced by the Fly sanitizer (`inner_html`); `scope_landing_css` adds no second sanitization pass.
- **Leptos Cart integration is not release-verified.** Source now mounts a shared Cart controller, event consumer and count publisher, and injects the host's Cart capability into Product. When assets or a valid owner are absent, controls remain unavailable and counts unknown. Synthetic bridge tests (including late installation) do not prove mounted WASM, tenant/auth transitions or owner-side admission.
- **Real host execution is not yet verified.** The added SSR/nonce and response-cache tests still need to compile and run in the real workspace, followed by mounted hydration, client navigation, CSP and accessibility checks. The isolated JavaScript browser tests do not prove those paths.

See the [Page Builder documentation](../docs/README.md) and the [module UI guide](../../../../docs/UI/module-package-implementation.md).

For the formal Rust API contract and guarantees, see [`CRATE_API.md`](CRATE_API.md).
