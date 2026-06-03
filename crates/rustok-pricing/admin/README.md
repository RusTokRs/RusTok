# rustok-pricing-admin

Leptos admin UI package for the `rustok-pricing` module.

## Responsibilities

- Exposes the pricing operations admin root view used by `apps/admin`.
- Keeps pricing visibility, price-health UX, and base-price operator actions inside the pricing-owned package.
- Participates in manifest-driven admin composition through `rustok-module.toml`.
- Uses native `#[server]` functions as the default admin transport and keeps the existing `rustok-commerce` GraphQL facade in parallel behind the module-owned `admin/src/transport.rs` facade.
- Exposes operator-side effective price inspection for `currency + optional region_id + optional price_list_id + optional quantity` without moving pricing ownership back into the host app.
- Resolves active tenant-scoped price lists through `rustok-pricing::PricingService`
  so operators can select overlays from pricing-owned data instead of typing raw UUIDs.
- Exposes normalized `discount_percent` on effective prices and variant price rows so sale math is available as typed read-side metadata instead of being inferred ad hoc in the host app.
- Exposes a module-owned variant-price write path for base rows and active `price_list_id` overrides, including quantity-tier authoring via `min_quantity` / `max_quantity`.
- Rejects invalid write-side scope identifiers such as malformed `price_list_id` instead of silently falling back to the base row.
- Extends that variant-price write path with optional `channel_id` / `channel_slug` so pricing operators can author channel-scoped base rows and channel-scoped active `price_list` overrides inside the pricing boundary instead of inferring scope from display data. The UI now resolves those scopes from the `rustok-channel` read model through a selector instead of asking operators to type raw IDs or slugs.
- Exposes a typed percentage-discount preview/apply flow for the canonical row in the current scope, including channel-scoped base rows and selected active `price_list` overrides, so operators can author simple sale adjustments without mutating quantity tiers.
- Exposes a selected-price-list rule editor so active `price_list` records can carry a typed percentage fallback rule when no explicit override row exists.
- Exposes active `price_list` channel-scope authoring and keeps existing override rows aligned with the updated list scope, using the same channel selector contract.
- Re-resolves the active `price_list` selector from the currently selected effective `channel` context, so operator-side overlay selection and rule editing do not drift from the explicit channel override shown in the toolbar.
- Keeps selector/bootstrap parity even in degraded transport mode by reusing commerce GraphQL pricing helper fields for `available_channels` and context-aware active `price_list` options when native `#[server]` functions are unavailable.
- Keeps pricing-detail parity in that same degraded mode by reading `adminPricingProduct` from the commerce GraphQL facade, including raw scoped price rows and `effective_price` for the explicit `currency/price_list/channel/quantity` context.
- Validates pricing resolution context consistently before GraphQL fallback and inside native `#[server]` handlers with the same contract as `PricingService`: `currency_code` must be a three-letter ASCII code, `quantity` must be at least `1`, `region_id`, `price_list_id`, or `quantity` require explicit currency, and malformed explicit `channel_id` is rejected instead of falling back to host context.
- Accepts deep links with query `id=` and prefilled resolution context so
  neighboring module-owned UIs can land directly on a specific pricing detail view.
- Links back to the product admin module with the same stable product `id` so
  pricing operators can return to catalog ownership without deriving identity
  from localized titles, handles, vendors, or other display fields.
- Ships package-owned `admin/locales/en.json` and `admin/locales/ru.json` bundles declared through `[provides.admin_ui.i18n]`.
- Keeps Leptos render/bind code in `admin/src/ui/leptos.rs`; `admin/src/lib.rs` only wires modules and re-exports `PricingAdmin`.

## Entry Points

- `PricingAdmin` - root admin view rendered from the host admin registry.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Reads tenant-scoped pricing detail and writes base rows, percentage discount adjustments, active price-list overlays, selected active `price_list` rules, and channel scope for both rows and active lists through `rustok-pricing` runtime services over native `#[server]` functions; the parallel `rustok-commerce` GraphQL facade now also exposes admin pricing write mutations for variant-price updates, typed discount preview/apply, and selected active `price_list` rule/scope updates. The current effective `price_list_id` and operator-supplied `channel_id` / `channel_slug` now scope discount preview/apply to the canonical row for that exact override boundary.
- Reads the effective UI locale from `UiRouteContext.locale`; pricing detail and editor context resolve localized product copy against that host-owned locale and only fall back when that locale is missing.

## Documentation

- See [platform docs](../../../docs/index.md).
