# rustok-pricing

## Purpose

`rustok-pricing` is the default pricing submodule of the `Ecommerce` family.

## Responsibilities

- Own the pricing service, price-related migrations, and pricing runtime metadata.
- Keep the decimal money contract working on the current runtime while preserving
  compatibility with legacy `prices.amount` and `compare_at_amount` storage.
- Maintain one canonical typed price-resolution contract over
  `currency_code + optional region_id + optional quantity`, and now allow
  explicit `price_list_id` overlays when the requested list is active for the
  current tenant/time window, plus a normalized `discount_percent` on resolved
  sale prices; the resolver now also consumes host-provided `channel_id` /
  `channel_slug` and supports channel-scoped base rows and price lists without
  making `rustok-pricing` the owner of channel identity. Pricing resolution
  contracts validate `currency_code` as a three-letter ASCII ISO-style code,
  reject non-positive quantities, and require `currency_code` whenever a caller
  supplies resolution modifiers such as region, price list, or quantity.
  Malformed explicit `channel_id` is also rejected, and pricing UI wrappers now
  pre-validate that contract before falling back from native `#[server]`
  transport to GraphQL.
- Expose a typed percentage-adjustment preview/apply contract inside
  `PricingService`, while keeping legacy `apply_discount` as a compatibility
  wrapper over the new adjustment path; the typed path now supports both the
  canonical base row and active `price_list` override rows.
- Expose active tenant-scoped price lists as a pricing-owned read contract so
  admin and storefront selectors do not depend on raw UUID-only entry, and now
  carry typed rule metadata plus optional channel scope for fallback sale
  semantics.
- Expose first-class `price_list` percentage rules so an active list can derive
  effective sale prices from base rows even without explicit override rows.
- Keep native admin `price_list` rule/scope mutation paths aligned with runtime
  lifecycle validation, so inactive/draft, future, and expired lists plus
  channel mismatches are
  rejected without hidden fallback or unintended override-row writes.
- Provide a module-owned Leptos admin UI package in `admin/` for pricing visibility,
  sale markers, currency-coverage inspection, effective price inspection, and
  operator updates for base rows and active price-list override rows on variant prices,
  including quantity tiers and typed percentage-discount adjustments on the
  canonical base row or the selected active price-list override row.
- Provide a module-owned Leptos storefront UI package in `storefront/` for
  public pricing discovery, currency coverage, sale-marker visibility, and
  effective price previews.

## Interactions

- Depends on `rustok-commerce-foundation` for shared commerce DTOs, entities, and errors.
- Depends on `rustok-product` data model through variant references.
- Used by `rustok-commerce` as the umbrella/root module of the ecommerce family.
- Provides the owner-managed `PricingReadPort` factory consumed by durable
  commerce checkout pricing, REST/GraphQL storefront add-to-cart/repricing,
  GraphQL effective-price projection, and storefront active-price-list
  projection plus admin product-pricing projection, without exposing
  `PricingService` across those boundaries; GraphQL storefront product-pricing
  detail uses the same owner port.
- `apps/admin` consumes `rustok-pricing-admin` through manifest-driven composition,
  and now gets native module-owned base-price and active price-list override write
  actions there, plus base-row percentage-discount preview/apply and selected
  price-list rule/scope editing; the remaining backlog is the broader promotions
  engine rather than the core pricing transport path.
- The parallel `rustok-commerce` GraphQL facade now also exposes admin pricing
  write mutations for variant-price updates, percentage-discount
  preview/apply, and selected active `price_list` rule/scope updates, so
  GraphQL remains a supported transport alongside native `#[server]`
  functions instead of falling back to read-only pricing behavior.
- `apps/storefront` consumes `rustok-pricing-storefront` through manifest-driven
  composition for a public pricing atlas route.
- Current verification baseline for this slice is a wide pricing sweep across
  `pricing_service_test`, the full `graphql_runtime_parity_test`, and SSR suites
  for `rustok-pricing-admin` / `rustok-pricing-storefront`.
- `PricingWritePort` is the typed owner boundary for commerce GraphQL admin
  variant-price upserts, discount application, price-list rules, and scope writes;
  its no-compile evidence remains boundary-level rather than transport execution.

## Entry points

- `PricingModule`
- `PricingService`
- `PriceResolutionContext`
- `ResolvedPrice`
- `rustok-pricing-admin`
- `PricingView`

See also `docs/README.md` and [CRATE_API.md](CRATE_API.md).

## Pricing public error continuation (2026-10-18)

Canonical ports and direct service consumers now share the Pricing-owned Foundation/Core/Rich
classifier. Product native pricing and Commerce mutation public errors use static owner
presentation with fresh references; query failures retain owner retryability. Ten owner entry
logs no longer emit request payload. See the [current contract](docs/pricing-owner-port-error-safety.md#nested-error-and-public-consumer-continuation-2026-10-18).
This is source-only, unintegrated work; five new Rust scenarios are unrun, and compilation,
mounted execution, and broader Pricing/Translation/price-list auditing remain open.


## Authoritative quote continuation (2026-10-29)

The owner contract is extended, not replaced by a second pricing engine. Single and
bounded batch quotes use Product-owned SKU identity projections, Channel/Region owner
reads, canonical money validation and one Pricing snapshot/time. The batch preserves
request order, actual Product membership, explicit unpriced SKUs and price-row/rule
provenance. Equal-priority rows and unknown/partial rules refuse a quote instead of
choosing a UUID or silently returning a base price. Percentage list sales are
non-stacking offers and cannot increase an existing sale; sale percentages describe
final rounded money.

The Pricing atlas now exposes `list_storefront_product_pricing` /
`storefrontPricingCatalog`: real SKU counts and base-price currency/sale coverage, not
Next-local `1 / 0 / USD` placeholders or per-product GraphQL waterfalls. These are
inspection rows, distinct from selling quotes. Both selected transports carry source,
normalized context and resolution time; the Leptos GraphQL adapter no longer discards
effective prices. Contextual unpriced results do not substitute raw catalog prices.
Checkout adjustment metadata retains price-row/source/time/rule provenance.

Native Pricing requires trusted request context; an extraction failure is not an
absent channel. ID-only/slug-only channel requests are completed by Channel without
mixing half of an explicit selector with half of a host default. Admin GraphQL reads
also verify that the requested tenant is the authenticated current tenant.

See [CRATE_API.md](CRATE_API.md), [the owner plan](docs/implementation-plan.md) and the
[ADR continuation](../../../DECISIONS/2026-03-25-commerce-module-split-product-pricing-inventory.md#pricing-resolution-continuation-2026-10-29).

### Quote-consumer recheck (2026-10-30)

Next read state and selectors are scoped before IO to tenant/auth/locale/executor.
Only non-quote discovery survives a failed re-quote; a currency change clears the
previous region and selected Products outside the first catalog page contribute their
actual currency coverage. Pricing/Product SDKs reject inconsistent quote observations,
and partial GraphQL failures are not successful not-found/unpriced reads. The shared
Next Apollo client does not deduplicate header-distinct viewers' in-flight queries.

The Next Pricing server route passes serializable data/query/reset-href props through
a client wiring bridge, not an executor function through RSC. The route preserves
region/quantity intent, rejects malformed quantity without defaults and keeps the
canonical Rust `currency` URL key. Native admin active-list selectors now use the
same canonical owner projection and channel completion as GraphQL. Rust storefront
quote observation checks remain projection-only; Pricing still owns all price math.

See the [recheck record](docs/implementation-plan.md#targeted-recheck-2026-10-30).
Regression sources are added but unrun; the limitations below still apply.

## Known Limitations / Pending Implementation

- Product step-8 now delegates selected Channel ID/slug correlation to the Channel owner while
  retaining Pricing's trusted-context rules. However, `PricingService::variant_identity_read_port`
  still constructs local `CatalogService`: selecting a remote Product catalog reader does not
  select a remote identity owner for Pricing. This capability must be traced/composed before
  claiming selected-provider parity or retiring local Product identity state; see the
  [downstream plan](../rustok-commerce/docs/implementation-plan.md#remaining-step-8-work-not-closed-by-this-slice).

- This block is source implementation, not runtime certification. Tests are reserved
  for the maintainer at the user's request; Rust/Cargo and PostgreSQL are unavailable
  in the implementation environment. Static formatting/type checks do not prove SQL,
  async traits/macros, isolation, deadline cancellation or mounted parity.
- The four `PricingWritePort` operations check an idempotency key but do not persist
  command receipts or expose expected-price state/CAS. Native mutation flows still
  include direct Pricing owner-service calls. Do not apply Product's receipt/journal
  guarantees to Pricing writes or advertise durable retries for them.
- Price List rule/scope event/invalidation completeness must be implemented before
  price caching. This quote block adds no cache or promotion provider fallback.
- There is no installed-state price ambiguity inventory or safe repair command. No
  historical migration was edited and no existing price/SKU identity was deleted.
- Quotes are unreserved observations, not tax-inclusive/customer-eligible final
  checkout commitments. Tax basis, customer groups, general stacking and explicit
  global preview from a channel-bound host require owner contracts and real consumers.
- Product storefront sparse-tuple/quantity selection and quote-to-cart handoff now have source
  implementations in both clients, but mounted buyer execution and complete downstream owner
  verification remain open. Existing no-context base rows are inspection data, not a checkout guarantee.
