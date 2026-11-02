# rustok-pricing / CRATE_API

## Public Modules

- `entities`: Pricing-owned persistence projections (`price`, `price_list`, translations).
- `migrations`: immutable ordered Pricing migrations and dependency descriptors.
- `module`: `PricingModule` registration.
- `ports`: typed Pricing read/write requests and snapshots.
- `services`: Pricing, Price List lifecycle and translation services.

## Platform Capability Bindings

- `PricingModule` implements `RusToKModule` and `MigrationSource`; slug `pricing`, kind `Optional`.
- Runtime dependencies: `product`, `region`. Channel identity uses the core Channel owner.
- Canonical factories: `in_process_pricing_read_port(db, event_bus, product_runtime)` /
  `in_process_pricing_write_port(db, event_bus, product_runtime)`, retaining the full selected
  `ProductCatalogReadRuntime` and delegated `PortContext`. The `ports` module-path factories
  forward the same admitted wrappers, not raw services or a second resolver. Embedded SDK
  `PricingService::new(db, event_bus)` is an explicit baseline; host composition must bind its
  selected runtime with `with_product_catalog_read_runtime`.
- All eight admitted reads require a positive deadline of at most 30 seconds and bound waiting;
  this is not proof that dropping the future cancels SQL or network work. Metadata SDK
  list/detail/handle projections now take a full `PortContext`, not a fabricated actor/locale.
- `PricingReadPort` operations: `resolve_product_price`, `resolve_product_prices`,
  `list_storefront_product_pricing`, `read_price_list_projection`,
  `list_active_price_list_projections`, `read_admin_product_pricing_projection`,
  `read_storefront_product_pricing_projection`, `preview_variant_discount`.
- `PricingWritePort` operations: `upsert_variant_price`, `set_price_list_scope`,
  `apply_variant_discount`, `set_price_list_percentage_rule`.

## Domain Collaborations

- Product owns stable SKU UUIDs and Product membership. Pricing obtains the small,
  tenant-scoped `ProductVariantIdentityReadPort` projection; quote validation does not load
  a complete merchandising document per SKU or read Product's private tables. The capability
  comes from the selected Product runtime. Responses must include exact tenant, ordered SKU
  set and optional expected parent before Pricing can read tenant-less price rows. External
  capability absence/failure never selects local Product. Inspection pages share that selected
  owner (at most 48 Products / 256 SKUs each); admin catalog metadata is not quote authority.
- SKU price writes/legacy discount preview still depend on local Product Variant locks. External
  Product profiles receive an explicit non-retryable capability refusal; lean read identity is
  not remote write CAS, an eligibility grant or a reservation.
- `ChannelReadPort` completes ID-only/slug-only selectors and rejects contradictory,
  foreign, missing or inactive channels. No explicit selector means the supplied context's
  channel, not a UI-invented default or an implicit global override.
- `RegionReadPort` validates region ownership and currency. Pricing does not own region
  lifecycle or compute taxes.
- `rustok-core::money` owns currency normalization, exponent, exact minor-unit
  representability and rounding. Legacy two-decimal mirrors are never quote inputs.
- Commerce publishes the GraphQL adapters (`storefrontPricingCatalog`,
  `storefrontPricingProduct`, `adminPricingProduct`) and consumes the owner port for
  cart/checkout pricing. Next and Leptos use the same owner catalog/quote projections.

## Minimum Contract Set

### Input DTOs/Commands

- `ResolveProductPriceRequest`: Variant UUID, optional expected Product UUID, currency,
  optional region/list/channel ID/channel slug/quantity. An absent price is
  `NotFound` with code `pricing.price_not_found`; missing identities are different errors.
- `ResolveProductPricesRequest`: optional expected Product UUID, **1–256 unique non-nil
  Variant UUIDs**, shared `PriceResolutionContext` in `pricing`.
- `ResolvedProductPricesSnapshot`: normalized context, one UTC `resolved_at`, ordered
  `items` containing canonical Product/Variant UUIDs and an optional `ResolvedPrice`.
  `price: null` is an authoritative unpriced SKU, never an identity/transport refusal.
- `ResolvedPrice`: exact decimal amount/compare-at, actual rounded discount percentage,
  sale flag, selected row scope/tier/list and `PriceResolutionSource` (`base`,
  `price_list_override`, `price_list_rule`) with the real price-row UUID and applied rule.
- `ResolvedProductPriceSnapshot`: the same price/source plus mandatory canonical Product/Variant UUIDs,
  normalized request context and resolution time, including for variant-first input.
- `StorefrontProductPricingListRequest`: locale fallback, channel selector, page/per-page;
  **1–48 Products per page**. Actual SKU counts and unscoped base-price currency/sale
  coverage are inspection data, not context-specific selling quotes.
- `StorefrontProductPricingProjectionRequest`: handle, locale fallback, optional public
  channel ID/slug. Public Product visibility remains Product-owned.
- `UpsertVariantPriceRequest`: Variant UUID, currency, amount/compare-at, optional list,
  channel ID/slug and minimum/maximum quantity.
- `ApplyVariantDiscountRequest`: Variant UUID, currency, percentage and optional list/channel.
- `SetPriceListPercentageRuleRequest`: Price List UUID, optional adjustment percentage,
  locale fallback. `None` clears the rule.
- `SetPriceListScopeRequest`: Price List UUID and optional channel ID/slug. There is no
  customer-group field in the current model.

### Domain Invariants

- No floating-point money arithmetic. Decimal boundary values must satisfy the canonical
  money owner's currency and exact minor-unit contract; zero is a real price.
- Quantity is a positive 32-bit integer, defaulting to one only when omitted. Nil UUIDs,
  duplicate/oversized SKU sets and malformed contexts are rejected before price lookup.
- Quote reads are deadline-bounded, at most 30 seconds. Pricing loads at most **4096 + 1**
  candidate price rows; overflow refuses the read rather than truncating a quote. Public
  catalog inspection has its own **16,384 + 1** base-price row probe.
- Price List lifecycle/rule metadata and prices share one Pricing transaction/snapshot:
  repeatable-read/read-only on PostgreSQL/MySQL, a read transaction on SQLite. A single
  captured UTC time uses `starts_at <= time < ends_at` for list eligibility.
- Priority: explicit selected-list override; otherwise matching base row with an eligible
  list rule, otherwise matching base row. Row specificity is channel, region, higher
  minimum quantity, lower maximum quantity. An authored minimum-one tier still outranks
  an untiered base row. Equal winning priority is a conflict, not a UUID tie-break.
- Percentage list sales do not stack: offer = rounded percentage of regular reference
  (`compare_at`, otherwise current amount); the lower of current amount and offer wins.
  A weak/rounded-away discount cannot raise or relabel an existing sale. Sale flags and
  discount percentages describe final rounded money. Overrides are not discounted again.
- Unsupported/partial rules and invalid applicable stored prices fail closed. A missing
  quote cannot select another currency, SKU or raw catalog row.
- Quotes observe Pricing state; they do not reserve prices, stock, Product identity,
  eligibility or a cross-owner transaction. Checkout must revalidate/reprice.
- Client projection guards check quote-versus-inspection mode, currency/quantity,
  quote context/time presence and effective-price currency agreement. They neither
  recalculate money nor replace Channel/Region/Product owner validation. GraphQL
  errors accompanying partial Next data are failures, not successful not-found reads.
- Native Pricing admin active-list/bootstrap selectors delegate to the same canonical
  list port with trusted tenant/actor/locale/correlation/deadline context. Explicit
  selector halves are completed by Channel, never by the host's other selector half.

### Events / Outbox Side Effects

- Price writes publish existing `PriceUpdated` events transactionally with price rows.
- Reads publish no mutation events and introduce no quote cache.
- **No durable Pricing mutation receipt/CAS guarantee is implemented by the four write-port
  methods.** Required idempotency-key presence is admission metadata, not durable replay
  proof. List rule/scope event and invalidation completeness remains pending.

### Errors / Failure Codes

- Canonical `PortErrorKind` classification and safe `PricingPublicError` presentation remain
  shared by native and GraphQL boundaries; payloads/SQL are not public messages.
- `pricing.product_identity_capability_unavailable`: selected Product lacks lean SKU identity;
  `pricing.product_write_capability_unavailable`: no supported external SKU write/preview
  protocol. Both are Unavailable/non-retryable, publicly `PRICING_CAPABILITY_UNAVAILABLE`.
- `pricing.variant_identity_invalid`, `pricing.catalog_identity_invalid`: malformed selected
  owner response; no price-row IO or local substitution.
- `pricing.price_list_not_found` is a selector refusal, not an absent Product success response.
- `pricing.price_not_found`: single-SKU price absent after valid ownership/context checks.
- `pricing.variant_not_found`, `pricing.variant_product_mismatch`: identity refusal, never unpriced.
- `pricing.price_ambiguous`: equal-priority price rows; operator correction required.
- `pricing.stored_data_invalid`: unsupported/inconsistent persisted pricing data.
- `pricing.price_row_limit`, `pricing.catalog_price_row_limit`: bounded read refused, not truncated.
- `pricing.channel_invalid`, `pricing.channel_mismatch`, `pricing.region_invalid`,
  `pricing.region_currency_mismatch`: rejected owner scope, not base-price fallback.
- `pricing.resolution_timeout`, `pricing.deadline_invalid`: enforced quote/read budget.
- Generic storage/domain failures retain owner kind, code and retryability through the
  canonical policy. GraphQL public presentation remains static and correlation-safe.

## Known Limitations / Pending Implementation

Source-only continuation; Rust compilation, database isolation/concurrency, mounted native /
GraphQL execution and browser verification are not certified. Durable write receipts/CAS,
list-change event completeness, installed-state diagnosis/repair, customer-group eligibility,
tax-inclusive quote policy and general promotions/stacking are not implemented by this slice.
Product's sparse-tuple storefront selection, route/context hardening, live quantity refresh
and cart-context handoff remain in the Product implementation plan.
