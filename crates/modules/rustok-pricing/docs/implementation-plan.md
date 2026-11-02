# Implementation Plan for `rustok-pricing`

## Product downstream owner continuation (2026-10-30; source only)

Selected Channel pair correlation now delegates to
`ChannelReadPort::read_selected_channel`; Pricing still owns trusted-context admission and
all price math. Global fallback source rows do not erase the exact request context.
Commerce adds quote/parent/quantity/context checks without a second resolver. New Channel
and Commerce regression sources are unrun.

Selected Product identity composition is implemented in source. The existing
`ProductCatalogReadRuntime` carries an optional `ProductVariantIdentityReadPort`; embedded
construction registers the same CatalogService and external construction requires explicit
capability supply. Pricing captures the full selected runtime. No capability, failed owner
read, wrong tenant/parent/SKU, omission, duplicate or reordering activates a local Product
fallback or converts an identity refusal into an unpriced item. Exact proof precedes Channel,
Region and tenant-less price-row IO. Inspection pages use that same selected catalog and
bounded identity capability; metadata projections validate tenant/root/SKU shape before rows.

Both canonical and module-path read/write factories require `(db, event_bus, product_runtime)`
and enter the admitted wrappers. Commerce REST/GraphQL, Cart/native buyer, native Pricing and
Order/Payment/Fulfillment preparation preserve that selection. Durable staged completion takes
`*_with_product_runtime`, not a bare catalog Arc that loses capabilities; its Pricing resolver
captures the same runtime as its plan builder. All eight canonical reads retain the caller's
positive, at-most-30-second wait budget. Standalone metadata SDK projections also retain that
budget and full caller context. Timeout does not prove SQL/network cancellation.

The remaining direct native Cart decrement constructor is also migrated: canonical Pricing
factory, selected Product runtime, actual actor/claims/correlation, saved Cart locale and saved
channel pair only. Missing/mismatched request context and malformed saved quantity/channel
refuse before repricing; a page/default selector never fills a missing Cart selector. This is
selection/context correction, not proof of quantity CAS, aggregate tier policy or convergence.
Public inspection reuses the same stored-money validator as quotes and rejects malformed
currency/precision rather than advertise an invalid sale; existing row bounds stay enforced.
Stable-link correlation preserves Product's requested/fallback/any-locale lookup behavior.

Native admin preview now uses the context-aware port with the actual authenticated actor,
claims, tenant, locale, correlation and request channel. Exact tenant/SKU proof precedes price
preview IO; embedded apply helpers check tenant before preview too. A missing Price List is not
reported as a missing Product. Owner retryability is retained, including permanent unsupported
capabilities; public messages remain static. Root embedded SDK construction is an explicit
baseline, not request-time fallback after selected-capability failure.

SKU price authoring still locks local Product Variant state. External Product profiles therefore
refuse SKU upsert/discount/legacy preview with
`pricing.product_write_capability_unavailable` (Unavailable, non-retryable) rather than pretend
that read identity is a distributed write protocol. Missing selected read capability reports
`pricing.product_identity_capability_unavailable`. Remote writes, receipts/CAS, list invalidation
and remote Inventory are not implemented or certified by this slice. No second money engine,
provider-selection registry, persisted identity or historical rewrite is introduced.

Focused regression sources cover an external selected owner with **no Product tables**, valid
unpriced items, exact metadata, malformed/foreign/reordered replies, deadlines, pages, stable
links, Price List refusal and explicit unsupported writes. They are **unrun**. Static check
commands/results are recorded in the [Commerce plan](../../rustok-commerce/docs/implementation-plan.md)
and [verification guide](../../../../docs/verification/README.md#product-downstream-sku-contracts).
No Rust compilation, tests, database, mounted or real remote-provider execution is claimed.

### Targeted consumer review (2026-10-30; source fixes, not runtime proof)

Source inspection found two compilation defects missed by the earlier formatter/arity scan:
Cart's optional locale reached a string constructor, and the actually mounted GraphQL repricer
called an aliased Pricing factory without its selected Product runtime argument. Both are
corrected. All eight migrated existing-line repricing/quantity-edit sites now share Commerce's
`read_storefront_cart_line_quote`: saved Cart channel replaces page context, locale inherits the
admitted caller only when absent from Cart, and the owner response is correlated before writes.
The staged resolver retains the real request context instead of constructing a synthetic service
caller and `en` locale. Permanent owner refusal metadata stays non-retryable in Checkout,
GraphQL and the mounted runtime SDK; a typed transient Cart-pricing boundary also remains
retryable at the final staged mapper. The subsequent [canonical HTTP continuation](../../rustok-commerce/docs/implementation-plan.md#canonical-http-retry-advice-2026-10-30)
adds optional exact advice to the shared REST envelope and known owner mappers; it does not
alter native/GraphQL wire contracts or certify quantity/tier/CAS/Inventory release gates.

Focused regression sources are unrun. The [Commerce review record](../../rustok-commerce/docs/implementation-plan.md#targeted-pricing-review-2026-10-30)
distinguishes current source checks from the original handoff and records remaining gates.

## Current state

`rustok-pricing` owns price resolution, price lists, pricing rules and scopes,
and its admin/storefront UI packages. The resolver already covers the active
price-list overlay, channel-aware context, typed percentage adjustments, and
the deterministic base-row precedence rules. Admin and storefront use
module-owned core/transport/Leptos layers; native server functions are host-neutral
and retain GraphQL as the parallel selected path.

`PricingReadPort` / `pricing.read_projection.v1` is implemented by
`PricingService`. The contract registry and no-compile smoke lock deadline
policy, typed error mapping, and declared fallback profiles, but the FBA
provider has not yet been executed against live persistence or a remote
consumer path.

Canonical root read construction uses `InProcessPricingReadPort`. The wrapper
retains the delegated `PortContext`, typed identity and bounded request-shape facts,
and exact local outcome context around the `PricingService` port implementation.
The wrapper does not implement price selection; the quote continuation below evolves
that owner execution and its DTOs. Known identifying validation, not-found, and conflict envelopes are
returned with stable non-identifying messages while preserving kind, code, and
retryability. The module-path read factory forwards the same admitted root wrapper;
it is not a raw-service bypass.

Canonical root write construction now uses `InProcessPricingWritePort`. The wrapper
retains the delegated write/idempotency context and safe typed identity or bounded
shape facts for all four `PricingWritePort` operations. Known identifier-, handle-,
SKU-, quantity-, and owner-value-bearing messages are normalized while preserving
kind, code, and retryability. `PricingService`, the write trait and DTOs, GraphQL
mutations and saved projections retain one owner implementation; module-path construction
now forwards the canonical wrapper with the selected Product runtime. Existing event and
receipt limitations remain separate release work.

The port accepts variant-first resolution when a cart snapshot has no product
id and returns the full resolved-price projection required to persist pricing
adjustments; cart storefront repricing therefore no longer calls
`PricingService::resolve_variant_price` directly.
The durable checkout pricing resolver also calls `PricingReadPort` with typed
tenant, admitted caller (or explicit service delegation), locale, channel, correlation, and deadline context.
REST, GraphQL, and native storefront cart repricing, plus REST and GraphQL
add-to-cart line-item resolution, use the same projection port instead of
resolving variants through `PricingService` directly. The commerce GraphQL
admin/storefront pricing roots now resolve the complete bounded SKU set through
the batch projection port while preserving an absent-price result as `null`.
The storefront active-price-list GraphQL root now uses a typed list projection
operation with context-derived locale and channel scope.
The admin product-pricing GraphQL root also uses a typed owner projection with
an authenticated actor, locale, channel, correlation, and deadline context.
The storefront product-pricing-by-handle GraphQL root uses the matching public
projection operation and retains its channel-visibility input.
The GraphQL discount preview uses the typed read port with an authenticated actor.
`PricingWritePort` owns the GraphQL admin mutations for variant-price upsert,
percentage-discount application, active price-list percentage rules, and price-list
channel scope. The write provider checks deadline/idempotency-key admission before it
invokes the pricing owner service (it does not implement durable receipts/CAS), returns the saved owner projection, and preserves
the effective locale plus fallback locale for rule updates. This is static boundary
evidence only; no live provider-consumer transport execution has been recorded.

Pricing storefront GraphQL public errors are source-hardened at the selected transport
facade: captured network, HTTP, unauthorized, GraphQL-rejection, and unknown displays
are mapped to static public messages with a per-call correlation id and bounded request
shape diagnostics. Pricing admin GraphQL public errors now use the same owner policy for
bootstrap, active-price-list, product-list, and product-detail reads. Request validation,
The original diagnostic slice did not change GraphQL/native execution. The quote
continuation below deliberately evolves read DTOs, documents and owner calls; native
mutation durability remains open. Source/type checks do not promote runtime or mounted status.

## Authoritative quote continuation (2026-10-29)

The current work first corrects the Pricing owner contract before Product's
axis-aware storefront consumes it. See the
[split ADR continuation](../../../../DECISIONS/2026-03-25-commerce-module-split-product-pricing-inventory.md#pricing-resolution-continuation-2026-10-29).

Implementation boundary:

- Product-owned bounded SKU identity read; no full-product-per-variant validation;
- shared single/batch resolver with normalized context, one Pricing snapshot and
  timestamp, explicit unpriced SKUs and source provenance;
- Channel-owned identity completion and contradictory/foreign context refusals;
- strict stored-rule/price validation, equal-priority conflict refusal, currency
  precision and non-stacking sale semantics that cannot raise a current sale;
- canonical native/GraphQL effective-price consumers use the same batch port;
- public atlas counts/currency coverage come from `storefrontPricingCatalog`, not
  fabricated values or eight product-detail fetches; canonical URL encoding keeps
  selector values from changing query boundaries;
- no contextual-price fallback to raw catalog rows in Product/Pricing clients.

This block is source-only until the maintainer runs the scoped owner, persistence,
transport and UI checks. Tests are not executed by the agent at the user's request.
The subsequent Product block still owns real sparse-tuple storefront selection,
URL/accessibility/locale parity and downstream identity verification.

Outstanding Pricing extensions are deliberately not disguised as complete:
`PricingWritePort` currently validates idempotency-key presence but has no durable
command receipt or expected-price state/CAS protocol; list rule/scope changes need
complete typed invalidation/outbox semantics before price caching is introduced;
there is no installed-state ambiguity repair inventory, tax-inclusive quote policy,
customer-group eligibility or general promotion stacking engine. These require
owner commands/ADRs and concrete consumers, not UI-local arithmetic or fallback.

## FFA/FBA status

- FFA status: `in_progress` — the owner UI surfaces exist and must retain
  native/GraphQL parity and the core/transport/UI boundary.
- FBA status: `boundary_ready` — provider metadata and static contract evidence
  are ready, while runtime contract and fallback execution remain pending.
- Structural shape: `core_transport_ui`
- Canonical root providers: `InProcessPricingReadPort` and
  `InProcessPricingWritePort`; owner execution remains `PricingService` in
  `ports.rs`, and `rustok_pricing::ports` is a compatibility path.
- Evidence: `crates/modules/rustok-pricing/contracts/pricing-fba-registry.json`,
  `crates/modules/rustok-pricing/contracts/evidence/pricing-contract-test-static-matrix.json`,
  `crates/modules/rustok-pricing/contracts/evidence/pricing-runtime-contract-smoke.json`,
  `crates/modules/rustok-pricing/contracts/evidence/storefront-graphql-error-safety-source.json`,
  `crates/modules/rustok-pricing/contracts/evidence/admin-graphql-error-safety-source.json`,
  `crates/modules/rustok-pricing/docs/read-local-context.md`,
  `crates/modules/rustok-pricing/docs/write-local-context.md`,
  `crates/modules/rustok-pricing/docs/storefront-graphql-error-safety.md`,
  `crates/modules/rustok-pricing/docs/admin-graphql-error-safety.md`,
  `scripts/verify/verify-pricing-read-local-context.mjs`,
  `scripts/verify/verify-pricing-write-local-context.mjs`,
  `scripts/verify/verify-commerce-domain-fba-runtime-smoke.mjs`,
  `scripts/verify/verify-pricing-admin-graphql-error-safety.mjs`,
  `scripts/verify/verify-pricing-storefront-graphql-error-safety.mjs`,
  `scripts/verify/verify-pricing-admin-boundary.mjs`, and
  `scripts/verify/verify-pricing-storefront-boundary.mjs`.

## Open results

1. Execute `PricingReadPort` against live persistence for
   `resolve_product_price` and `read_price_list_projection`, including the
   declared embedded and GraphQL fallback profiles. The owner has a targeted
   SQLite integration test for the two successful read operations and
   missing-deadline rejection; the in-process provider test passed. Done when
   the observed calls also prove the consumer degraded modes rather than only
   static markers.
   Dependency: runnable commerce consumer composition for the declared
   fallback profiles. Verification: `npm run verify:ecommerce:fba` plus
   `cargo test -p rustok-pricing --test pricing_read_port_runtime`.
2. Complete the dedicated pricing transport handoff from the umbrella
   `rustok-commerce` facade. Done when the owner exposes its selected public
   transport contract directly and commerce composes it without re-exporting
   pricing services, DTOs, or entity aliases.
   Dependency: an approved atomic public-contract migration. Verification:
   `npm run verify:pricing:admin-boundary` and
   `npm run verify:pricing:storefront-boundary`.
3. Finish the remaining Pricing 2.0 rule semantics: tiers, adjustments, and
   deterministic rounding across active price-list rules. Done when resolution
   tests cover precedence and rounding for every supported context; multi-layer
   promotions orchestration remains owned by `rustok-commerce`.
   Dependency: the stable owner transport and product variant data. Verification:
   targeted pricing resolution and money-semantics tests.
4. Prove canonical pricing diagnostics and retire compatibility bypasses.
   **Status:** source-complete / unvalidated for root read/write construction and
   storefront/admin GraphQL public error envelopes. Execute invalid-context,
   invalid-actor, product/variant mismatch, missing price/list, duplicate identity,
   inventory conflict, storage, invariant, network, HTTP, unauthorized, and GraphQL
   rejection scenarios. Retain traces proving that raw handles, SKUs, UUID-bearing
   messages, tenant identifiers, locale/search/status values, quantities, currencies,
   prices, percentages, and GraphQL/HTTP cause text do not cross canonical public
   boundaries. Audit direct `rustok_pricing::ports` callers and either migrate or
   explicitly accept them. Shared `port.*` admission diagnostics, runtime evidence,
   browser evidence, and remote-profile parity remain open.

## Verification

- `npm run verify:pricing:admin-boundary`
- `npm run verify:pricing:storefront-boundary`
- `node scripts/verify/verify-pricing-admin-graphql-error-safety.mjs`
- `node scripts/verify/verify-pricing-storefront-graphql-error-safety.mjs`
- `node scripts/verify/verify-pricing-read-local-context.mjs`
- `node scripts/verify/verify-pricing-write-local-context.mjs`
- `npm run verify:ecommerce:fba`
- `cargo test -p rustok-pricing --test pricing_read_port_runtime`

## Boundaries

- Pricing owns resolution, price-list/rule lifecycle, and pricing UI policy.
- Product owns catalog and variant data; commerce owns orchestration and any
  multi-layer promotions workflow.
- Hosts only compose owner UI packages and pass effective locale, channel, and
  runtime context without creating package-local fallback chains.
- Keep raw handles, SKUs, tenant or resource identifiers, locale/search/status values,
  currency values, prices, percentages, GraphQL/HTTP cause text, and returned pricing
  rows out of owner diagnostics; retain only typed identity and bounded shape facts.


## Quote continuation verification and next boundary

Implemented source: Product identity batch/page port; Pricing single/batch/catalog
reads; typed resolution refusals; bounded deadline and row probes; source/context/time
DTOs; Commerce GraphQL and checkout metadata; native Pricing/Product consumers;
Next/Leptos Pricing read projections and selector links; checkout identity/quantity guards
and preserved definitive/transient error kinds; Product contextual-unpriced
refusal and explicit unavailable presentation. Focused Rust and JS regression sources
are added, not executed.

Scoped Next frontend API/core and expanded changed UI/host strict TypeScript checks
passed in the implementation workspace. The expanded check does not certify a full
application build or the earlier Next admin block. Rust source is parsed/formatted
with Scalar rustfmt WASM, not compiled. Final non-test source/format/document checks
are recorded with the handoff; no tests or runtime smoke suites are executed.

Next: finish Product's real sparse SKU axis selector in both storefronts, preserve
explicit SKU/quantity/context in URLs, distinguish unavailable/unpriced/not-requested
states throughout the buyer UI, refresh quotes after quantity/context changes, and
wire the cart handoff through the actual cart/context owner. The existing Product
storefront context sanitizers/default derivation and quantity controls are not a
certified implementation of that next boundary. A price-list preview must not be
presented as customer eligibility or a locked checkout price.

Maintainer checks (unrun by the agent):

```sh
cargo check -p rustok-pricing
cargo check -p rustok-product
cargo check -p rustok-commerce
cargo check -p rustok-pricing-admin --features ssr
cargo check -p rustok-pricing-storefront --features ssr
cargo check -p rustok-product-storefront --features ssr
cargo test -p rustok-pricing --test pricing_read_port_runtime
cargo test -p rustok-commerce --test pricing_service_test
cargo test -p rustok-commerce --test graphql_runtime_parity_test pricing
node --experimental-strip-types --test apps/next-frontend/packages/rustok-pricing/src/core.test.mjs
node --experimental-strip-types --test apps/next-frontend/packages/rustok-product/src/catalog/pricing.test.mjs
```

PostgreSQL/MySQL repeatable-read, concurrent list/rule/row changes, SQLite snapshot
behavior, read-timeout cancellation, mounted native/GraphQL and browser evidence are
still required. Installed-state remediation and historical migration safety were not
inferred from fixtures.


### Recorded non-test checks for this continuation

- Strict Next frontend API/core TypeScript check: passed, including unused-local/
  parameter checks. Expanded changed Pricing/Product UI + Product host page check:
  passed, zero TypeScript errors (not a full application build).
- Scalar `@scalar/rust-fmt` 0.3.0 WASM: 39 scoped Rust sources parsed/formatted with
  edition/style edition 2024. This is not `rustc`, Cargo, macro or SQL verification.
- Prettier: 15 scoped Next TS/TSX/MJS sources formatted. Fluent: four catalogs parsed,
  no duplicate keys, two EN/RU key-set pairs equal; pinned next-fluent type generation
  ran. GraphQL: 28 selected documents syntax-parsed, not executed/schema-certified.
- Source-only guards passed: Pricing read local context, native/GraphQL storefront
  error safety and aggregate storefront boundary, Commerce query pricing safety,
  Pricing public error policy, Product lifecycle collaboration, ecommerce FBA
  registries. The lifecycle guard was aligned with the existing claimed persisted-
  override reread; no recovery implementation was changed.
- Tests, Cargo/Rust compilation, Clippy, architecture checks requiring Cargo metadata,
  database/concurrency, runtime smoke suites and mounted/browser checks remain unrun.
  The earlier remediation gate is blocked by missing Cargo; it is not reported green.
- Historical migrations and persisted data are unchanged. No installed-state repair
  inventory or destructive retirement was attempted.


## Targeted recheck (2026-10-30)

This is a bounded review of the quote continuation and its actual consumers, not an
activated audit or completion of Product's remaining buyer flow. The Pricing math,
priority, snapshot and error paths were reread; this recheck does not certify them
without the maintainer's Rust/database execution.

Corrections made in source:

- Next selections, results, discovery and failures are scoped to tenant, auth, locale
  and executor identity. Scope changes derive a clean selection before the first IO,
  rather than fetching once with another scope's handle/channel/list/region.
- Failed re-quotes clear the quote, but retain only same-scope non-quote discovery
  controls. Lists/currency coverage also match the channel intent. Changing currency
  clears the old region, and coverage includes the explicitly selected Product even
  outside the first eight catalog items. Reset is an explicit clean-route action.
- Pricing/Product consumers reject mismatched quote/inspection mode, currency,
  quantity, missing quote context/time and effective-price currencies. These are
  transport-projection integrity checks, not another pricing or eligibility engine.
  Next uses the Pricing SDK guard; Rust storefront cores guard their own projections
  on both selected paths. A missing/foreign SKU is still not an unpriced quote.
- Next Pricing reads reject GraphQL errors even alongside partial data. An errored
  nullable product field cannot be interpreted as a successful not-found result.
- The shared Next Apollo client disables in-flight query deduplication: Apollo's
  query/variable key excludes tenant/auth headers. Keeping the endpoint-shared client
  must not combine different viewers' concurrent reads. This deliberately trades
  redundant same-viewer requests for scope isolation; no per-token client/cache registry
  or alternate transport was introduced.
- Native Pricing admin bootstrap/active-list reads use the canonical list projection
  port and trusted request context. Explicit ID-only/slug-only selectors do not borrow
  the host channel's other half; contradictory selectors and inconsistent tenants refuse.
- Product's identity-page row probe uses the actual requested Product count × 256 + 1,
  rather than reserving a full 48-Product probe for every smaller page.
- The actual Next `/pricing` RSC route no longer passes an ordinary executor function
  to a client component. A host-owned client wiring bridge imports the shared executor.
  Its server props are serializable and carry the original query even after an SSR read
  failure. Region context is retained, omitted quantity remains omitted for inspection,
  malformed quantities refuse instead of `parseInt`/default-one coercion, and invalid
  requests have an explicit localized reset view. The existing `currency` route key is
  retained to match Rust; no alternate query vocabulary was introduced.
- The single Next URL quantity parser now lives in the Pricing SDK; Product's host
  imports it there without a compatibility alias. The Rust Pricing route validates raw
  quantity in core/transport before either provider starts IO. Product's separate
  unfinished Rust buyer-route hardening is not claimed complete.

Regression sources added/extended (not executed): Next Pricing scope/discovery,
context, wire-response, partial-error, URL and RSC wiring cases; shared GraphQL
concurrent tenant/auth isolation and partial-error cases; Rust quote-projection
integrity, pre-provider URL quantity rejection and native admin channel-selector cases.

Recorded non-test verification for the final review source:

- Scoped strict Next API/core typecheck and expanded affected UI/host typecheck:
  zero errors, including the actual Pricing RSC route/client bridge. These use cached
  tooling with one canonical React declaration path, not a full application build.
- Scalar rustfmt WASM: 40 scoped Rust files parsed/formatted idempotently, not compiled.
  Prettier: 20 scoped TS/TSX/MJS files. Three MJS files syntax-checked, never executed.
- Ten source-only guards pass: the previous eight plus Pricing admin boundary and admin
  GraphQL error safety. Pinned Fluent type generation includes the new EN/RU messages.
- No tests, Cargo/Rust compilation, Clippy, database/concurrency, mounted/RSC runtime,
  hydration or browser checks were run. Cargo/rustc/psql are absent. Existing static
  evidence does not turn any runtime gate green. Historical migrations remain unchanged.

Additional maintainer commands (unrun):

```sh
cargo test -p rustok-pricing-admin --features ssr --lib
cargo test -p rustok-pricing-storefront --lib
cargo test -p rustok-product-storefront --lib
node --experimental-strip-types --test apps/next-frontend/src/shared/lib/graphql.test.mjs
```

Mounted Next `/pricing` should be checked for RSC serialization/hydration, malformed
and valid URLs, missing/failed initial data, tenant/auth/locale changes and failed
re-quote recovery. Native/GraphQL channel-selector parity and concurrent multi-tenant
Apollo execution still require the maintainer's runtime evidence. Product's real
axis selector, quantity refresh, URL/cart context handoff, Pricing write receipts/CAS
and list invalidation/outbox completeness remain pending as recorded above.

## Product buyer quote admission continuation (2026-10-30)

The previously pending sparse buyer selection/URL/quantity/context handoff now has source
implementations in both clients; see the Product plan. Shared quote admission requires explicit
nullable effective money, complete canonical context and valid RFC3339 observation time. Next
rejects prose/impossible calendar dates rather than trusting Date.parse normalization; Rust uses
the existing workspace Chrono parser. Public quote helpers are exported rather than duplicated in
Product or Cart. Quotes remain observations, not reservations, tax or list/customer eligibility.

This is not Pricing write CAS/receipts, list invalidation, downstream certification or mounted
buyer proof. Tests are intentionally unexecuted; Rust/toolchain and backend verification remain
maintainer gates.

## Cart quantity adoption continuation (2026-10-30; source only)

The [Cart-owned fence](../../rustok-cart/CRATE_API.md#priced-mutation-adoption-2026-10-30-source-continuation)
now binds quoted quantity/context/material inputs before adopting line money under the Cart lock.
Commerce shares one REST/GraphQL/native quantity workflow and checks aggregate **replacement**
stock demand separately from target-line tiers. Batch updates reject stale/duplicate/unknown
lines; no stored-price fallback remains for missing buyer SKU identity. This does not add a
Pricing write receipt, monotonic Cart revision or price hold. The [Commerce handoff](../../rustok-commerce/docs/implementation-plan.md#cart-quantity-adoption-2026-10-30)
retains unrun regression, downstream/outbox and runtime gates. The 2026-10-31 continuation below
supplies Cart-local context/pricing atomicity, without claiming remote or runtime execution.


## Cart target-context batch continuation (2026-10-31; source only)

Commerce now prepares complete target-context Cart prices with the existing selected Pricing batch
port, not a second engine. Groups retain expected Product parent plus per-line quantity before
Product identity proof and tenant-less money reads; variant deduplication/chunking stays within the
256 bound. Ordered response/parent/SKU/context/explicit-price correlation and one inherited operation
deadline precede Cart's single locked context/price adoption. Unsupported batch capability is not
recovered through single reads, local providers or old prices. No Pricing API/schema, price/stock
hold, write receipt or historical data rewrite is introduced. See the [Commerce handoff](../../rustok-commerce/docs/implementation-plan.md#atomic-cart-context-adoption-2026-10-31)
and [Cart contract](../../rustok-cart/CRATE_API.md#atomic-context-and-pricing-adoption-2026-10-31-source-continuation).
Regression sources remain unrun, compilation and remote/mounted/database execution unproven.
