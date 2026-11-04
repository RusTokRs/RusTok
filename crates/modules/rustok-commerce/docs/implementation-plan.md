# RusToK ecommerce implementation plan

## Product downstream continuation (2026-10-30)

Ordinary Product step-8 work on `arena/71cd6d1f-rustok`, based on
`16cb22206ca19de8218f47a1056d946105c7b09f`; no audit cursor is activated.
This bounded source slice is written, not compiled or runtime-certified. The maintainer
runs tests; step 8 as a whole is still in progress. Existing readiness labels below are not
promoted or evidence for this continuation.

### Targeted Pricing review (2026-10-30)

The maintainer asked to inspect the selected Product -> Pricing handoff before further feature
work. This is a targeted source review on the same branch/base, not a full audit or a readiness
promotion. The previous formatter/arity/marker results did **not** establish compilation:

| Finding in the live path                                                                                                                                         | Source correction                                                                                                                                                                                                                    |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Native decrement passed `Option<String>` to the string locale constructor                                                                                        | Admit the actual caller locale, then use saved Cart locale or that caller through the shared adapter; malformed saved locale refuses                                                                                                 |
| Mounted GraphQL repricer called the aliased Pricing factory with 2 arguments after the signature became 3                                                        | Supply the resolver-scoped full selected Product runtime; include aliases in source call-shape checks                                                                                                                                |
| REST/GraphQL/runtime repricers mixed saved Cart selectors with page channel, and several did not correlate the returned quote                                    | All 8 migrated existing-line quote sites (repricing/quantity edits) use `read_storefront_cart_line_quote`; saved ID-only/global scope clears page slug, and UUID/parent/quantity/currency/region/channel correlation precedes writes |
| Staged Pricing reconstructed a synthetic service/correlation and `en` locale instead of retaining the caller                                                     | Capture the admitted Cart `PortContext`, retain real request correlation/claims and its key/deadline; saved-first locale/region still follow Cart owner policy; reject tenant/Cart mismatches before price IO                        |
| Owner `retryable: false` became `true` in Checkout/GraphQL/mounted adapters; a transient Cart-pricing boundary also became non-retryable at final staged mapping | Retain the exact owner bit and static messages; keep the final typed staged Cart-pricing retry branch                                                                                                                                |

The shared adapter delegates to the supplied Pricing port; it neither selects another provider
nor performs price selection/math, authorizes Cart access, reserves money/stock, changes quantity
tiers, or supplies a write/CAS receipt. Only two now-unused duplicated request builders were
removed after tracing their Rust consumers. No historical migration or persisted state changed.

The source also corrects imprecise lock documentation: Pricing observations happen **before**
the Cart write lock; Cart validates/applies the plan under that lock. Recovery uses existing
locked snapshots rather than fresh quotes. This is not a cross-owner transaction guarantee.

Added regression **sources, unrun**: 7 shared quote/error-admission cases, 4 staged context/error
cases, and 3 GraphQL/mounted retry-envelope cases (14 Rust cases); the guard companion adds
4 cases (13 total). Prior Product/Pricing fixtures remain unrun. Final scoped static results
are recorded here; Cargo/rustc are absent.

Executed source checks for this review (not tests or Rust compilation):

- Scalar rust-fmt 0.3.0: syntax/format/idempotence for 12 review-touched Rust files;
  this cannot check Rust types, imports, feature combinations or lifetimes.
- Node syntax and Prettier 3.7.4: 8 touched MJS files, including the **unexecuted** regression
  companion. No `node --test` or Rust test command was run.
- Tree-sitter targeted dependency/call-shape scan: 120 calls, including the previously missed
  `contextual_pricing_read_port` alias and the 4-argument staged resolver constructor;
  0 arity mismatches. This is not name/type/feature resolution or whole-crate correctness proof.
- All 20 original guard entrypoints in [Selected-identity source checks](#selected-identity-source-checks-2026-10-30)
  plus the following 4 read-only mounted/HTTP-path guards exited 0 (24/24):

  ```bash
  node scripts/verify/verify-commerce-graphql-reprice-typed-error.mjs
  node scripts/verify/verify-commerce-storefront-checkout-runtime-error-safety.mjs
  node scripts/verify/verify-commerce-storefront-staged-checkout-cutover.mjs
  node scripts/verify/verify-commerce-storefront-cart-http-error-safety.mjs
  ```

- 47/47 local file-link targets in 8 review-touched documents resolve; general anchors were
  not checked. The new review heading was checked explicitly. `git diff --check` passes;
  tracked historical migrations remain unchanged. No schema/data operation was performed.

Initial guard failures were stale checks endorsing the two-argument alias, the optional-locale
constructor, a formatting-dependent missing-variant call and no-execution documentation.
The HTTP guard also still described the replaced pre-workflow Cart-add path; it now checks
actual guarded owner composition and shared quote admission. Guards were corrected against
actual call/admission/refusal paths and truthful static-only results, not by executing regressions or treating token presence as compilation.

Remaining limits include unexecuted Rust/SQL/concurrency/mounted/remote behavior, Cart quantity/
aggregate-tier/CAS and Inventory effects, and remote SKU writes. At this review's handoff the
shared HTTP body still contained only `code`/`message`; that specific gap is addressed by the
separate canonical HTTP continuation below. Native server-function adapters retain their existing
static envelopes. Neither source slice certifies end-to-end mounted transport behavior.

### Canonical HTTP retry advice (2026-10-30)

Ordinary bounded continuation after the targeted review, on the same branch/base; no audit or
readiness state is advanced. The owner remains `rustok-web`, not a Commerce/Pricing-local DTO.

- `HttpError` / `ErrorBody` now carry optional exact owner advice. `Some(false)` is serialized,
  unknown is omitted, and a present null or non-boolean fails canonical DTO deserialization.
  Neither 503/504/409 nor existing public message wording determines that bit.
- 32 forwarding points in 31 function scopes retain existing typed-owner or final runtime policy:
  the shared port mapper, direct Commerce/Media port mappers, nested post-order OwnerPort,
  staged storefront checkout, and the already-classified paid/reversal recovery branches.
  Helpers without an explicit owner decision stay unknown; no new domain retry policy is invented.
- Existing status/code/static-message/redaction mappings are unchanged. The host delegates the
  same `HttpError` response. OpenAPI registers the DTO at module/host composition and 45 existing
  storefront Product/Cart/checkout error declarations reference its optional non-nullable member.
- The existing Next Admin registry REST helper (one status read plus eight commands) consumes
  `shared/api/http.ts`. It keeps status/code/public copy and exact advice, distinguishes absent
  from invalid evidence, admits 16 KiB within a two-second metadata budget, and does not await
  failed/stalled cancellation. Raw HTML/nested diagnostics never become fallback public text.
  It does not issue another request, create a new key, infer outcome, or authorize replay.
- Current Next buyer Product/Pricing/Cart calls use the host GraphQL executor; Leptos uses
  existing native/GraphQL adapters. This HTTP slice does not invent buyer REST clients or change
  those wire contracts. GraphQL error extensions and native envelopes remain separate transport
  projections, not evidence that a flat REST response carried metadata before this change.

Added regression **sources, unrun**: 5 canonical HTTP unit functions (including all seven kinds
with both owner bits), 2 host-delegation functions, 1 host-composed OpenAPI function and
11 Next HTTP reader functions. Coverage
includes false on 503, true on 409, unknown, malformed/null wire values, schema optionality,
body size/UTF-8/deadline and cancellation, unchanged sanitization and ignored outcome/key claims.
The existing `verify-api-surface-contract.mjs` source gate pins the completed forwarding/client
slice. No Rust compilation, OpenAPI generation, test suite, mounted/native/browser/remote IO,
SQL or concurrency proof is implied. Product step 8, quantity/tier/CAS/Inventory and downstream
Order/Fulfillment/outbox/index gates remain open; no historical migration or persisted data changed.

Executed scoped static evidence for this HTTP continuation (not tests):

- All 24 read-only source entrypoints from the targeted Pricing review were rerun, plus the
  extended API-surface gate: **25/25 exited 0**. No regression companion/test runner was invoked.
- Scalar rust-fmt 0.3.0 and Tree-sitter parse the **25 changed Rust files**; formatting is
  idempotent. This is not Rust compilation, feature/import/lifetime resolution or Cargo fmt.
- A repository source inventory finds **33 typed PortError -> HttpError functions**: each either
  forwards the bit directly or delegates to the shared mapper. All **31 changed mapper scopes**
  retain their pre-change policy ASTs after excluding the exact metadata decorators and
  formatting-only parentheses/commas/redundant expression blocks; **32 forwarding points** are
  pinned. Broad remaining retry mentions were inspected as success-report fields, delegation,
  GraphQL projections or tests, not silently assumed to be additional HTTP retry decisions.
- TypeScript 6.0.3 strictly type-checks the isolated new HTTP reader; the other two changed TS
  entrypoints parse, and their **9 existing REST calls** remain. This is not an application build.
  Node syntax checks cover the two MJS files, not execution of the eleven regression functions.
- Prettier 3.7.4 checks all **5 touched TS/MJS files**. Admin formatting uses the existing app
  options without its unavailable Tailwind plugin; these files contain no class-sorting surface.
- **94/94 local file-link targets in 12 changed documents** resolve; general anchors were not
  checked, while the new HTTP contract and handoff headings were checked explicitly. Three
  TOML files parse. Lock comparison only adds approved, already-resolved `serde_json`/`utoipa`
  dependencies to `rustok-web`; no package version changes or Cargo resolution is claimed.
- `git diff --check` passes, the branch/base remain unchanged, and no tracked historical
  migration changed. All **8 new Rust / 11 MJS regression functions remain unrun**. Cargo/rustc
  are unavailable; generated OpenAPI, mounted/browser/native/remote IO and concurrency remain
  unverified. No runtime/readiness gate or Product step-8 completion is asserted.

### Inventory-owned reservation adoption (2026-10-31)

Ordinary bounded step-8 continuation on `arena/71cd6d1f-rustok`, base
`16cb22206ca19de8218f47a1056d946105c7b09f` (observed local main
`920c0e3f5c4bfb6698caedfbc683684f4c51fa0c`). No audit ledger or integration is advanced.
The [Inventory owner contract](../../rustok-inventory/CRATE_API.md#reservation-adoption-into-order-2026-10-31-source-continuation)
and [ADR 10h](../../../../DECISIONS/2026-09-18-unified-variant-axis-architecture.md#10h-inventory-owned-reservation-adoption-into-order-2026-10-31)
define the new boundary. This is source implementation, not runtime certification.

**Concrete cutover.** The previous Commerce adoption service read and mutated Inventory
`inventory_items`/`reservation_items` while updating its own mapping in one native transaction.
No accepted native transaction-sharing exception covered this adoption. It now calls the existing
identity port's bounded batch command; only Inventory validates/locks/mutates its reservations.
The actual order-stage constructor passes the **same supplied Inventory capability** to reserve
and order creation/adoption; order creation and adoption constructors require it explicitly.
There is no silently constructed embedded Inventory provider or single-row fallback.

**Preserved lineage and allocation.** Frozen Order inputs/responses already carry
`checkout.cart_line_item_id`; Order and fulfillment continue using these UUIDs, not array positions
or fresh Product/Pricing reads. Saved reservation/external/Variant/quantity/location and source/target
line IDs must match the complete owner response. Current Cart source, known previously adopted
source and explicitly marked lossy compaction are distinguished without rewriting historical state.
Commerce's immutable journal remains authoritative for Cart/operation identity. Custom orders with
no variant-backed reservations make no Inventory call; every actual batch is nonempty and bounded.

**Distinct transactions and recovery.** Inventory validates the whole batch and reserved-level
coverage under deterministic tenant-proved item/reservation/level locks before binding writes.
Exact already-adopted rows do not rewrite metadata/timestamps. Original exact release identity
survives; quantities, levels, external IDs and allocation location do not change. Commerce checks
all response identities before its own full-source compare-and-set journal update, then checkpoints
`order_created` separately. Lost replies, local journal/checkpoint failures and deadline expiry are
not a distributed rollback or proof of a no-effect failure.
The owner checks the full prepared metadata against the physical 32 KiB limit for the whole batch
before any write, preserving payloads and returning a nonretryable capacity refusal instead of a
deterministic generic-DB retry loop. PostgreSQL JSONB/SQLite byte representations are measured
natively in one bounded projection; this SQL remains unexecuted here. Missing/zero deadlines are
rejected before provider polling (zero `timeout` alone is not sufficient).

**Advice is not outcome.** Sanitized owner code (bounded to the journal's 100-character envelope)
and exact retry advice survive creation, pipeline, staged/journal/recovery wrappers and final staged
storefront projection. Owner/transport retryable failures retain the original operation/Order/
reservation IDs. A false hint on an unverified handoff uses the journal's existing
`reconciliation_required` state, not automatic compensation, cancellation, stock release or key
rotation. The new journal entry point reuses the canonical lease fence, settling admission, and
transactional existing parked/admission events. Private owner/journal causes are retained internally
but are not copied into public messages or the new staged boundary log.
Local pre-call journal/provenance failures can follow an earlier committed attempt, so they retain
the same bounded recovery classification; they do not authorize compensation. Recovering checkout
preserves the original typed adoption failure before the compensation lookup can erase it after a
successful park. Owner codes remain journal/bounded diagnostic facts; buyer projections retain the
existing public unavailable/reconciliation codes and static messages, not an owner receipt.

**Contract closure only.** Inventory/Commerce FBA registries now declare the existing live identity
capability and all three commands; six Inventory cases remain runtime-pending. An existing Cart
registry/static-matrix drift from the prior quantity/context slices is synchronized in both
directions: no locked-observation/context assertions are dropped and no execution status is raised.
No buyer DTO, API route, new lifecycle status, schema, applied migration, receipt, provider,
reservation outbox publisher or stock hold is introduced. Existing REST/GraphQL/native callers that
select the staged runtime keep their outer error/command contracts; no broader transport cutover
or mounted parity is implied.

#### Inventory adoption source evidence

Read-only source command:
`node scripts/verify/verify-commerce-checkout-inventory-adoption.mjs`.
Executed source-only evidence (2026-10-31; **not tests or Rust/SQL/runtime certification**):

- **11/11 read-only contract guards pass:** this adoption guard, checkout completion cutover,
  execution admission, operation events, Inventory owner/local/port diagnostic guards, both
  ecommerce FBA registry/evidence guards, and the preceding Cart priced-write/context atomicity
  guards. The new guard follows 17 sources and the actual wrapper/selected-provider/error path.
- **16/16 Rust files** pass Scalar 0.3.0 syntax/format and idempotence with edition/style edition
  2024, plus independent Tree-sitter 0.25.2 / Rust grammar 0.24.0 parsing. No types, macro expansion,
  lifetimes or feature combinations are proven. Rust constructors' separate source guard passes
  across **6150 files**.
- A source inventory over **6223 Rust files**, narrowed to 12 matching files, checks **26 targeted
  calls / 11 DTO constructors / three canonical field declarations**, with no call-arity, missing,
  extra or duplicate DTO-field findings. This is source shape, not compiler evidence.
- Both MJS files pass Node syntax-only and Prettier 3.7.4 checks (explicit formatter defaults;
  no unavailable repository Tailwind plugin). Companion imports/exports and location were reviewed
  without importing/executing its tests. Eight touched registry/evidence JSON files parse.
- **346/346 local file-link targets across 15 documents** resolve; **17 new heading links**
  resolve. Older general anchors were not exhaustively checked. `git diff --check` passes.
  The historical Inventory migration files remain unchanged; no manifest/dependency is added.
- **34 new Rust / 16 new MJS regression definitions remain unrun**, with no removed Rust test
  functions relative to the slice's before-images. The new persistence child is registered under
  `inventory_service_test`; the recovery child remains in registered `checkout_service_test`.
  Sources cover admission (all four public paths), ledger/provenance/replay/terminal markers, exact metadata byte
  boundaries/UTF-8/no truncation, selected provider/context/deadline, full reply correlation,
  bounded wrapper advice, journal fencing/events and conservative compensation disposition.

Canonical broader gates were attempted, not bypassed or called passed: architecture dependency
verification cannot find Cargo; remediation rejects quoted source-marker prose in the preceding
Cart plan; canonical docs still reports **44 errors** (ADR registry/topology/older missing targets).
These do not become green by the scoped checks, and no audit ledger/readiness status is advanced.

Maintainer-run targets: Inventory library, Commerce library, registered
`inventory_service_test` and `checkout_service_test`, plus
`scripts/verify/verify-commerce-checkout-inventory-adoption.test.mjs`. No regression test is executed
by this continuation. Cargo/rustc are absent; real rollback/contention/lost-response recovery,
installed-state migration histories, remote/mounted execution, auth/revocation fences and complete
shipment/cancellation/return/event/outbox/Index convergence remain explicit gates. Step 8 stays open.

### Atomic Cart context adoption (2026-10-31)

Continuation of the same bounded Product step-8 work; no audit cursor, readiness label, branch
or historical migration is changed. The [Cart owner contract](../../rustok-cart/CRATE_API.md#atomic-context-and-pricing-adoption-2026-10-31-source-continuation)
and [owner record](../../rustok-cart/docs/implementation-plan.md#atomic-context-pricing-adoption-2026-10-31)
own transaction/admission semantics.

Actual context writers were traced before replacement: REST patch, GraphQL context mutation and
native shipping each committed context before a separate repricer. They now build the normalized
hypothetical target, validate shipping in saved Cart Channel scope, prepare selected owner prices
and issue **one neutral Cart command** carrying the original pre-projection observation. Native
payment preparation also reuses that command builder; its formerly shared native repricer's last
live caller was migrated before removing the duplicate snapshot/adjustment builder.

The bounded preparer groups by expected Product parent and per-line quantity, deduplicates SKU
UUIDs within each group, chunks at 256 and correlates ordered response coverage/parent/SKU/context
and explicit price presence. One inherited wait budget covers all price groups; missing/unsupported
batch functionality never falls back to single reads, stored prices or another provider. Original
caller principal/claims/correlation/deadline and original Cart guard survive target projection.

Country selection/null is no longer replaced by the region's first country; explicit region/country
membership is checked. Missing shipping group patch remains missing instead of overriding scalar
null with old values. One Cart interpreter handles projection and persistence, including canonical
resolved-group overlap refusal. Native shipping requires admitted request/principal agreement and
saved Channel resolution; it does not invent a request or borrow a page slug.

Direct availability-only checkout recovers completed/checking-out carts before context handling,
skips already-current materialized context and uses explicit Preserve only without unit-price-basis
change. Pure context validation refusal stays checkout Validation, not an opaque stage failure. Snapshot preparation delegates atomic owner prepare; frozen adapter writes refuse. Existing
Some-price-plan checkout remains available. This is Cart-local atomicity, not a price/stock hold,
distributed transaction, monotonic revision, replay receipt or new automatic mutation identity.

GraphQL price preparation keeps the established `CART_REPRICE_FAILED` envelope and exact owner retry
bit. Source guards follow the new call boundary/shared adopter rather than requiring two writes.
An adjacent real compatibility diagnostic Debug leaked typed cause text; its consuming diagnostic
projection is now redacted. Retired unquoted quantity assertions are replaced with live shared
workflow checks, not restored compatibility code.

**20 new Rust / 8 new MJS regression functions remain unrun.** Source guard:
`node scripts/verify/verify-cart-context-pricing-atomicity.mjs`. Cargo/rustc are absent; tests,
compilation/types/lifetimes/features, real DB locking/rollback/contention, generated transports,
native/browser/auth/cookie transitions and remote/provider execution remain maintainer gates.
The canonical remediation gate was attempted and still rejects quoted inclusion markers in
pre-existing source checks/documentation; the architecture guard separately cannot find Cargo.
Neither is bypassed or called passed. Step 8, durable Cart mutation/outbox effects and downstream
Order/Fulfillment/Inventory/Index convergence remain open. The [final source-only checks](../../rustok-cart/docs/implementation-plan.md#executed-scoped-context-source-evidence)
record 44 scoped guards, syntax/format and call/constructor inventory, not runtime proof.

### Cart quantity adoption (2026-10-30)

The [Cart owner contract](../../rustok-cart/CRATE_API.md#priced-mutation-adoption-2026-10-30-source-continuation)
and [owner plan](../../rustok-cart/docs/implementation-plan.md#priced-quantity-adoption-2026-10-30)
own this ordinary source continuation. No audit/readiness state is advanced.

- Public Cart add previously lost the pricing guard at `OwnerCartPort`; that actual provider now
  forwards it. Root/module factories share one admitted storefront implementation.
- Priced add, quantity change and batch adoption compare original tenant/Cart/saved scope and
  line identity/quantity/money/fulfillment inputs under the existing Cart lock. New/prior line
  amounts use canonical fixed-point bounds. Pricing/Inventory IO remains before the lock.
- Bulk updates carry quoted quantity and reject duplicate/unknown/stale entries before any write.
  Unquoted storefront quantity writes and their old validator callers are retired, not retained
  as a fallback. Missing SKU identity refuses rather than scaling saved money.
- One shared workflow now backs REST/GraphQL/native quantity changes. It validates the same selected
  Product/publication/fulfillment/parent and Pricing quote, then calls Inventory for checked target
  replacement + same-SKU sibling demand. It never uses aggregate stock quantity as a price tier.
- Full priced observations admit at most 1024 lines; new priced insertion at capacity refuses.
  Explicit target-only removal can reduce oversized/corrupt-price historical carts and cannot
  authorize price adoption. Native decrement-to-remove retains its original quantity observation.
- Native reads/writes use guarded ports, shared request context/claims and one public-safe owner
  error mapper; obsolete native money/error helpers are removed after tracing their callers.
  Public endpoint/input/cart DTO shapes are unchanged; native technical error text now uses the
  same static inspect-before-retry envelope as the existing buyer adapter.
- Both Marketplace Cart snapshot mutations now acquire the same Cart row lock before
  reading/inserting/rebinding snapshots; a parallel writer cannot split Cart line money from its
  marketplace projection while a priced adoption is in flight. Read-only snapshot reads stay reads.
- Cart FBA registry/static matrices/invocation descriptors reflect seven storefront operations
  and the actual owner provider source. No artifact is upgraded to runtime evidence.

Added **14 Rust / 12 MJS regression functions**, all **unrun**. Tests are maintainer-run.
Compilation, lifetime/name/feature resolution, SQL/locking/concurrency, mounted/browser/native/
remote execution remain unverified. This is not a monotonic whole-Cart revision, price/stock
hold, authorization token or durable command receipt. Context update then repricing is still a
separate-operation boundary; unknown-outcome replay, Cart outbox/receipt publication and the
remaining downstream Order/Fulfillment/stock/Index effects remain open. No stored SKU/line/
price/stock/history or historical migration is transformed or deleted.

Executed source-only verification for this continuation:

- 29 changed Rust files parse/format/idempotently round-trip through Scalar 0.3.0 and Tree-sitter;
  14 MJS files pass Node syntax, 15 MJS/JSON files pass Prettier 3.7.4. Original style is retained
  in four other large guards to avoid cosmetic churn.
- 29 scoped read-only source entrypoints and 9 additional shared public-surface consumer gates
  pass (38 unique entrypoints). The broader FBA metadata gate now uses the existing canonical
  root-export checker: valid inline public facades are accepted without reopening Inventory's
  implementation. All ten direct checker consumers pass; three new helper regressions are unrun.
  The FBA policy-simulation runner is source-updated/syntax-checked but deliberately not executed.
- 6219 Rust sources / 27 targeted calls / 26 request/update constructors
  (including `vec!` bodies) have zero scoped arity/required-field errors; retired executable
  unquoted quantity symbols are absent. This is not type/name/feature resolution.
- 102/102 local file-link targets in 14 changed docs resolve; general anchors are unchecked and
  new dated headings are checked separately. Diff whitespace, branch/base and historical tracked
  migrations remain unchanged. All 14 Rust / 12 MJS regression functions remain unrun.

The [Cart owner evidence](../../rustok-cart/docs/implementation-plan.md#formal-blockers-reported-not-bypassed)
records Cargo/architecture unavailability and the attempted remediation gate's rejection of
pre-existing JS `include!` marker strings in the cumulative diff. Neither blocker is suppressed
or mislabeled as passed. No generated schema, mounted/native/browser/remote/SQL/concurrency,
receipt or downstream event/outbox completion is claimed; Product step 8 remains open.

### Source changes

- `services/checkout_catalog.rs` admits at most 1024 lines for **new checkout processing**,
  validates Cart/line identity and currency, and totals positive quantities by stable Variant
  UUID with checked i32 addition. A SKU with contradictory known parents refuses. This is
  not a historical Cart storage limit: no persisted line, snapshot or journal is rewritten.
- Both direct and staged preflight use that demand and exact selected Product/tenant/SKU,
  publication, channel, physical/digital and effective shipping-profile checks. Inventory is
  queried once per distinct SKU for its combined demand, and the response UUID/quantity is
  checked. Original line IDs, quantity-tier prices, labels, SKU text and metadata stay separate.
- Cart/checkout quote admission correlates SKU/parent, quantity, currency, region, saved channel
  and no explicit Price List handoff. Legitimate automatic-list sources and global fallback
  price rows remain valid; their source scope is not the request context. Cart add now reads
  Inventory for the combined existing + new demand of the exact SKU in the canonical quote
  channel, correlates the response and caps waiting at two seconds. Product's descriptive global
  quantity/`in_stock` is not admission authority; backorders stay entirely with Inventory. Price
  tiers still apply to each new line, not an invented aggregate-money quantity. REST, GraphQL
  and native composition provide the mandatory Inventory port explicitly (currently in-process,
  as staged checkout); a selected remote Inventory runtime is not certified by this wiring.
- `ChannelReadPort::read_selected_channel` completes explicit saved ID/slug selectors through
  the Channel owner. No selectors means global, never page/default context; malformed, missing,
  inactive, foreign, contradictory or unavailable selectors refuse. Commerce waits at most two
  seconds for that read; this is not proof of database-query cancellation. The resulting slug
  controls preflight/shipping visibility and each request's new reservation allocations.
- Inventory new identity allocation probes at most 4096+1 candidate SKU levels and refuses
  overflow; only active tenant-owned channel-visible locations remain candidates, including
  for backorders. Exact existing receipts precede current eligibility; release uses the original
  location. Only new reservations receive owner-controlled `allocated_channel_slug` metadata.
  Journaled Reserved rows are not reselected or repriced; a nil new location reply refuses.
- Nil guest actor IDs delegate as the Commerce service, never as fabricated User identities.
  This supplies call identity, not customer/guest authorization. Inventory's public root and
  `ports` paths now use the same admitted wrapper; the raw persistence implementation is private.
  Touched Inventory entry logs retain bounded request facts, not caller payloads.

### Selected Product identity -> Pricing continuation (source only)

The same full `ProductCatalogReadRuntime` now reaches canonical Pricing read/write factories,
REST and resolver-scoped GraphQL Cart operations, shipping/context repricing, CheckoutRuntime,
and the staged Pricing resolver. The richer completion APIs are
`complete_storefront_checkout_with_product_runtime` and
`complete_storefront_checkout_input_with_product_runtime`; a bare catalog Arc is insufficient
because it loses lean identity selection. Native Product/Cart/Pricing and Order/Payment/
Fulfillment preparation retain the host selection, with embedded baseline only when no host
runtime was selected. Native admin metadata and quotes capture one runtime per request.

Pricing validates exact tenant/SKU/optional parent and response order before money IO; bounded
inspection pages use the same selected catalog/identity owner. Missing or failed capability is
an explicit refusal, never local substitution or an unpriced SKU. Native admin preview uses
actual authenticated request context and canonical tenant-aware admission. Price List refusal
no longer becomes missing Product; public retryability does not promote unsupported capabilities
to retry loops. External SKU authoring/legacy preview refuses until a real write protocol replaces
local Variant locking. One Pricing engine, stable UUIDs and persisted journals remain unchanged.

The old direct native Cart decrement PricingService construction is migrated too. It now uses
the selected Product runtime/canonical read factory, actual caller metadata and saved Cart
locale/channel pair without page fallback. Missing/mismatched request context and malformed
saved quantity/channel refuse. Broader quantity CAS, aggregate tier semantics and stock/effect
convergence remain future verification/implementation, not a claim from this routing correction.
Public inspection validates stored money with Pricing's existing quote row validator; link
correlation retains Product's any-locale lookup rather than inventing a stricter locale resolver.

Focused Rust regressions and the new source-guard companion are **not executed**. The positive
external-identity fixture intentionally creates Pricing tables but no Product tables; this is
regression source, not evidence that a real remote adapter/database works. All real remote,
compilation, persistence, cancellation and mounted gates remain open.

### Selected-identity source checks (2026-10-30)

- Scalar `@scalar/rust-fmt` 0.3.0 WASM parsed/formatted 35 selected Rust sources with
  edition/styleEdition 2024; a second format pass was identical. This is not compilation.
- Prettier 3.7.4 and `node --check` cover eight touched MJS files, including the new companion
  **without importing/executing its tests**. Rust and Node regression suites are unrun.
- A tree-sitter source inventory found no arity mismatches in 75 relevant calls to the changed
  factories/SDK/completion APIs. This is not Rust name, lifetime, trait or type resolution.
- Four SSR-only Product dependency/feature declarations and their existing Cargo.lock entries
  are structurally consistent. Cargo is unavailable; dependency resolution, compilation and
  the Cargo-metadata architecture guard remain unproven/blocked, not passing.
- The following **20 read-only source guard entrypoints exited 0** after constructor/operation
  markers were updated to the actual selected-runtime, shared batch and eight-read contract:

```bash
node scripts/verify/verify-commerce-checkout-plan-product-projection-context.mjs
node scripts/verify/verify-commerce-checkout-plan-inventory-context.mjs
node scripts/verify/verify-commerce-checkout-plan-marketplace-snapshot-context.mjs
node scripts/verify/verify-commerce-fulfillment-requirement-boundary.mjs
node scripts/verify/verify-inventory-reservation-owner-context.mjs
node scripts/verify/verify-inventory-reservation-local-context.mjs
node scripts/verify/verify-inventory-port-diagnostic-safety.mjs
node scripts/verify/verify-pricing-storefront-boundary.mjs
node scripts/verify/verify-product-storefront-boundary.mjs
node scripts/verify/verify-cart-storefront-boundary.mjs
node scripts/verify/verify-cart-storefront-native-client-error-safety.mjs
node scripts/verify/verify-commerce-graphql-cart-pricing-context.mjs
node scripts/verify/verify-pricing-read-local-context.mjs
node scripts/verify/verify-pricing-write-local-context.mjs
node scripts/verify/verify-pricing-product-identity-context.mjs
node scripts/verify/verify-pricing-owner-port-error-safety.mjs
node scripts/verify/verify-pricing-admin-boundary.mjs
node scripts/verify/verify-commerce-graphql-query-pricing-error-safety.mjs
node scripts/verify/verify-commerce-graphql-pricing-owner-diagnostic-safety.mjs
node scripts/verify/verify-commerce-graphql-pricing-error-safety.mjs
```

`git diff --check` passes; tracked historical migration modifications remain empty. File targets
of 290 relative links in 11 touched owner/verification/ADR documents resolved at this handoff;
anchors were not checked. These observations do not certify SQL, remote adapters, cancellation,
concurrency, auth policy, mounted requests, browser rendering, deployment or FFA/FBA readiness.
No test, Rust compile, database or mounted execution is claimed, and step 8 stays in progress.

### Live identity and persisted-state inventory

| Owner/path   | Persisted identity or observation                                                | Preservation / limit                                                                           |
| ------------ | -------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Product      | Product UUID, stable Variant UUID, sparse axis assignments and revision/receipts | No tuple/text/position fallback; no SKU deletion in this slice                                 |
| Pricing      | `prices.variant_id`, context/tier provenance; quote observations                 | One Pricing engine; displayed/catalog money is not authority                                   |
| Cart         | `cart_line_items.id`, `product_id`, `variant_id`, money/fulfillment snapshots    | Demand aggregation does not merge or recatalog stored lines                                    |
| Checkout     | prepared Cart hash, immutable order plan, reservation journal IDs/external keys  | Existing checkpoints preserve original request, locale, lines and recovery identities          |
| Inventory    | inventory-item Variant UUID, level/location UUID, reservation identity           | New-only scoped selection; exact replay/release never relocate an old receipt                  |
| Order        | order line UUID/Variant UUID and frozen money/labels                             | Order plan copies separate lines; no current-catalog rewrite of history                        |
| Fulfillment  | Order-line/reservation linkage and allocated stock location                      | No text-derived SKU mapping; shipment/cancellation/return effects still need full verification |
| Index/events | Product revision/refresh and inventory/pricing invalidation consumers            | No retirement justified by names; event/outbox and mounted convergence evidence remains open   |

### Regression source and verification boundary

Unexecuted Rust regression sources cover checked SKU grouping/admission, exact sparse SKU
membership and fulfillment, aggregate preflight against stock 4/5, incorrect owner replies,
quote context/provenance, scoped Cart admission/overflow/owner failure/timeout, canonical
Inventory backorders, saved Channel pairs, visible/hidden stock,
archived-location receipt replay and exact repeated release, and public Inventory entry wiring.
The persistence cases are included by the explicitly registered `inventory_service_test` target
(`autotests = false`); the new file alone is not an executable target.

The projection-context source guard reads the real shared helper and both call paths rather
than obsolete inline markers. Its fixture regressions are added but not executed. Current
commands and the source/runtime distinction are listed in
[verification](../../../../docs/verification/README.md#product-downstream-sku-contracts).
The Cart storefront SSR Inventory dependency and local lock entry are synchronized manually,
not resolved by Cargo. Completed scoped checks on 2026-10-30:

- Scalar parses and idempotently formats 26 selected Rust sources (not native rustfmt,
  macro expansion, type checking or compilation).
- Eight edited MJS files pass syntax checks; test modules are not imported/executed.
- Twelve selected read-only source-guard entrypoints pass, including shared checkout/Cart
  SKU demand, fulfillment propagation, Inventory entry diagnostics, Product/Cart/Pricing
  storefront boundaries and all eight contextual GraphQL Pricing read operations.
- The Cart SSR Inventory dependency/local lock entry passes structural TOML checks only;
  `git diff --check` passes and tracked historical migrations have no diff.
- The architecture dependency guard is **blocked**: its `cargo metadata` subprocess cannot
  start because Cargo is absent. This is not a passing architecture result.

No tests, Rust compilation, database/concurrency, mounted/native/GraphQL/browser, deadline
cancellation, deployment or CI evidence is claimed. Selected documentation file links are
checked separately from their anchors; this is not a full documentation/readiness audit.

### Remaining step-8 work (not closed by this slice)

- Selected Product identity is composed into Pricing in source, but real remote adapters,
  selected-provider parity and mounted tenant/auth/channel/locale behavior are unexecuted.
  Missing capability refuses. SKU price authoring remains embedded-only until remote write
  coordination/CAS/receipts replace local Product Variant locking; do not retire identity state
  based only on source wiring or introduce a second resolver.
- Trace Cart quantity mutations/tier repricing, Order lineage and the complete reservation-to-
  fulfillment/shipment/cancellation/return chain under the selected runtimes and auth/tenant/
  channel/locale context. Current source correlation is not end-to-end execution evidence.
- Verify inventory/pricing events, transactional outbox, Index invalidation and cancellation/
  recovery concurrency. Identity allocation updates stock and its receipt transactionally but
  this slice adds no reservation outbox publication; downstream convergence is not certified.
- Preserve historical migrations and installed data. Step 9 may retire only consumer-traced,
  proven-unused code/state; no Product/SKU/price/stock/Cart/Order history is removed here.

Last reviewed: 2026-09-25

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- Consumer registry: `crates/modules/rustok-commerce/contracts/commerce-fba-registry.json`
- Provider registries:
  - `crates/modules/rustok-payment/contracts/payment-fba-registry.json`
  - `crates/modules/rustok-fulfillment/contracts/fulfillment-fba-registry.json`
  - `crates/modules/rustok-order/contracts/order-fba-registry.json`
  - `crates/modules/rustok-pricing/contracts/pricing-fba-registry.json`
  - `crates/modules/rustok-inventory/contracts/inventory-fba-registry.json`
  - `crates/modules/rustok-product/contracts/product-fba-registry.json`
  - `crates/modules/rustok-customer/contracts/customer-fba-registry.json`
  - `crates/modules/rustok-cart/contracts/cart-fba-registry.json`

## Audit 2026-09-25: mounted owner-runtime composition

- [x] Require host-composed `CommercePaymentReadRuntime` and `CommercePaymentCommandRuntime` for mounted Commerce GraphQL schema composition. The mandatory Payment capability now fails closed instead of silently synthesizing an in-process wrapper when host composition is incomplete.
- [x] Compose the Commerce Payment GraphQL wrapper runtimes in `apps/server::attach_commerce_provider_registries`, preserving host/server-selected owner runtimes and deterministic in-process construction only at the explicit host-composition boundary.
- [x] Remove obsolete duplicate admin REST implementations for order changes and returns. Each route now has one canonical transport adapter: owner-port reads/commands or the dedicated Commerce orchestration boundary.
- [x] Extend `verify-commerce-fulfillment-requirement-boundary.mjs` with the Payment runtime-composition guard.
- [ ] Execute the Commerce/server Rust test suites and the static verifier against a repository checkout; this environment cannot run the repository build because the GitHub source is not mounted locally.

## Audit 2026-09-25: Tax port error-envelope hardening## Audit 2026-09-25: Tax validation detail hardening## Audit 2026-09-25: Admin shared HTTP error-envelope hardening## Audit 2026-09-25: Order owner port diagnostic hardening## Audit 2026-09-25: Mounted Admin Order Detail owner-port cutover## Audit 2026-09-25: Mounted Store Product runtime capability integrity## Audit 2026-09-25: Store shared HTTP error-envelope hardening

- [x] Replace StoreContext validation/currency public messages with stable envelopes; internal
      `StoreContextError` details are no longer exposed by the mounted shared HTTP mapper.
- [x] Remove full StoreContextError, Customer PortError, tenant/user identities, and channel
      identifiers/slugs from shared storefront diagnostics; retain only bounded shape/kind facts.
- [x] Update `verify-commerce-storefront-shared-http-error-safety.mjs` to require the bounded
      diagnostic contract and explicitly forbid the removed raw payload patterns.
- [ ] Continue the mounted Store/Admin non-PortError envelope audit across remaining controllers.

- [x] Restore `crates/modules/rustok-commerce/src/controllers/store/products.rs` to its
      explicit legacy compatibility source; the mounted route remains `products_owner_list.rs`.
- [x] Add the missing `CommerceHttpRuntime::product_storefront_http_read_port()` delegation
      required by the already-mounted Product owner-list handler.
- [x] Extend the Product REST owner-read verifier to require the Commerce runtime accessor
      so this source-integrity blocker cannot recur while legacy compatibility source remains intact.
- [ ] Continue the mounted Commerce route audit for any remaining foreign owner construction.

- [x] Replace the mounted `show_order` PaymentService/FulfillmentService direct constructions
      with the existing `PaymentOrderReadPort` and `FulfillmentReadPort` capabilities from
      `CommerceHttpRuntime`, preserving the existing optional payment/fulfillment read semantics.
- [x] Remove the now-orphaned domain-level order-detail PaymentError/FulfillmentError
      mappers and their diagnostic facts rather than retaining a divergent second mapping path.
- [x] Extend `verify-commerce-admin-order-detail-fulfillment-error-safety.mjs` to require
      owner read-port handoffs and forbid direct foreign owner construction and obsolete mappers.
- [ ] Audit the remaining mounted Commerce controllers for direct foreign owner service
      construction and migrate each live path to its canonical owner capability.

- [x] Remove complete `DbErr`/`OrderError` debug payloads and raw tenant/transition/validation
      values from checkout-identity and checkout-completion owner-port diagnostics.
- [x] Centralize Order owner diagnostic facts into bounded context/error-shape helpers covering
      validation text length, resource UUID shape, transition field lengths, and opaque causes.
- [x] Extend the broad ecommerce public-port verifier and canonical fixture regression coverage
      to require the bounded Order diagnostics and reject the former raw patterns.
- [ ] Continue the same audit for remaining non-PortError envelopes and compatibility adapters.

- [x] Remove generic Debug-based raw error logging from the shared Commerce admin HTTP
      envelope helper; it now records only stable owner/error-kind/public-code/status facts.
- [x] Remove the remaining full PortError debug payload from post-order owner-port error
      handling in the same shared admin module.
- [x] Update the existing admin order/fulfillment HTTP safety verifier so this shared
      helper is required to remain bounded and raw Debug logging is explicitly forbidden.
- [ ] Continue the broader non-PortError public-envelope audit across mounted REST/GraphQL
      and client transport adapters.

- [x] Make the Tax request-validation helper accept only static public detail, eliminating
      accidental future `Display`/formatted-input leakage through `PortError::Validation`.
- [x] Replace the duplicate-country-rule message that previously embedded the submitted
      country code with a stable public message and add a verifier regression guard.
- [ ] Continue owner-level mapper review for remaining Tax/Promotion and non-`PortError`
      public envelopes, then collect the maintainer-owned execution evidence.

- [x] Stop the legacy TaxCalculationPort implementation from copying arbitrary
      TaxError::Validation text into public PortError messages; validation now uses a
      stable tax calculation envelope.
- [x] Remove full PortError debug payloads from the canonical in-process tax adapter
      diagnostics while retaining bounded error-kind/code/message-length facts.
- [x] Extend the ecommerce public-port safety verifier and fixture regression coverage
      to protect the TaxCalculationPort and canonical tax diagnostic boundary.
- [ ] Continue the broader mapper cleanup for promotion and remaining non-PortError
      public envelopes, plus compile/runtime evidence for all new source guards.

## Audit 2026-09-25: Fulfillment lifecycle status typing

- [x] Replace raw string comparisons in the Fulfillment admin command owner adapter with
      FulfillmentStatusKind accessors, including ship admission, reship replay/transition,
      and cancel replay/transition.
- [x] Replace the remaining Commerce fulfillment orchestration/facade lifecycle filters
      with the Fulfillment-owned typed status view.
- [x] Extend verify-ecommerce-typed-lifecycle-statuses.mjs to assert these Fulfillment
      surfaces and reject their former raw lifecycle comparisons.
- [ ] Continue the broader typed-status cutover for remaining critical checkout,
      compensation, order, payment, fulfillment, and durable journal state-machine paths.

## Audit 2026-09-25: correlation-safe Cart Checkout diagnostics

- [x] Replace raw Cart Checkout owner-boundary diagnostics with bounded correlation/context
      facts and typed error-shape metadata; validation, database, tenant-context, local-owner,
      and service-owner logs no longer emit tenant IDs, actor values, transport context,
      PortError debug payloads, or arbitrary validation text.
- [x] Extend the broad ecommerce public-port verifier with isolated Cart Checkout mapper
      sections and source guards so the bounded diagnostic contract cannot regress.
- [ ] Keep the broader correlation-safe mapper cleanup open for payment execution/
      compensation, fulfillment, inventory, customer, tax, promotion, remaining ecommerce
      adapters, and non-PortError public envelopes.

## Audit 2026-09-25: source-integrity blockers

- [x] Repair the malformed `CommerceGraphqlRuntimeData` initializer so every composed owner runtime is separated correctly and the GraphQL runtime source remains compilable.
- [x] Repair the malformed marker array in `verify-order-post-order-command-port.mjs` so the post-order owner-boundary guard is valid JavaScript.
- [x] Bound the PostgreSQL legacy fulfillment-index backfill and validation casts so malformed or oversized legacy metadata fails closed instead of being evaluated as an unsafe integer cast during migration.
- [x] Restore checkout identity metadata on `fulfillment_items` during the typed-identity migration rollback for PostgreSQL, SQLite, and MySQL, preserving clean/upgraded/down/reapply symmetry.
- [x] Sanitize Commerce GraphQL checkout validation envelopes so `CommerceError` and owner `PortError` diagnostics cannot leak raw validation text to clients; add a static guard for both paths.
- [x] Sanitize the legacy GraphQL cart shipping-option helper so owner `PortError` messages cannot bypass the checkout public error envelope.
- [x] Make the shared `legacy_graphql_error` mapper always use its caller-owned public message instead of copying an arbitrary `async_graphql::Error` message into the client envelope; existing cart helper verification now passes its source contract again.
- [ ] Run the Rust compiler and static verification suite after this repair; runtime, migration, and test evidence remains maintainer-owned for this continuation.

## Payment workstream

- Payment storefront and webhook boundary is guarded by `scripts/verify/verify-payment-storefront-boundary.mjs`.
- Durable webhook event ingress relies on signature-verified provider result identities instead of untrusted transport headers.
- Refund creation identity and refund `creation_key` are guarded by `scripts/verify/verify-payment-refund-identity.mjs`.
- Provider outcome lifecycle and error taxonomy are guarded by `scripts/verify/verify-payment-provider-outcome-contract.mjs`.
- Stripe payment provider runtime attachment is guarded by `scripts/verify/verify-payment-stripe-runtime.mjs`.
- Commerce storefront transport handoff and native adapter `storefront/src/transport/native_server_adapter.rs` are guarded by `scripts/verify/verify-commerce-storefront-transport-handoff.mjs`.
- [x] Update the legacy GraphQL runtime parity refund mutation helper to pass explicit idempotency keys.

## Source of truth

This file is the only human-maintained source of truth for ecommerce execution
order, completion marks, verification state, and promotion gates.

Rules:

- `[x]` means source or retained execution evidence exists in `main` or in the
  implementation branch that updates this plan.
- `[ ]` means implementation or required evidence is still missing.
- Source implementation and runtime verification are separate tasks.
- Local owner plans and FBA registries may contain owner detail but must not
  contradict this plan.
- No FBA or FFA status is promoted from source inspection alone.
- Newly discovered work is recorded here before or with implementation.
- Legacy migrations must not invent actor, locale, provider, or financial facts.
- A broad invariant must be reopened when one production path still violates it.

`rustok-commerce` owns cross-domain ecommerce orchestration. Product, cart,
customer, region, pricing, inventory, order, payment, fulfillment, tax, promotion,
and market/store remain owner bounded contexts. Marketplace persistence belongs to
the explicit `rustok-marketplace-*` family and must never be folded into
`rustok-commerce`.

## Scope

`rustok-commerce` owns cross-domain ecommerce orchestration.

## Current state

- Ecommerce audit gate: `reopened_p0`.
- Ecommerce FFA: `in_progress`.
- Ecommerce FBA: `boundary_ready`.
- Payment FFA: `in_progress`.
- Payment FBA: `boundary_ready`.
- Marketplace family source gate: `open`.
- Marketplace family FFA: `in_progress`.
- Marketplace root FFA: `not_started`.
- Marketplace root FBA: `in_progress`.
- Marketplace seller FFA: `in_progress`.
- Marketplace seller FBA: `in_progress`.
- Marketplace listing FFA: `in_progress`.
- Marketplace listing FBA: `in_progress`.
- Marketplace financial source: `reversal_recovery_and_seller_balance_transfer_v3_source_ready_unvalidated`.
- Marketplace production gate: `closed` until compiled contracts, clean/upgraded
  migrations, tenant isolation, contention, restart, mounted transports, remote
  profiles, and financial reconciliation evidence are retained.

### Accepted fulfillment capability cutover

The static `commerce -> fulfillment` module edge and unconditional startup requirement
are removed. Commerce now supports a digital-only operating mode without shipping or
Fulfillment. Product publishes the canonical typed requirement; Cart and Order persist
the immutable line snapshot; checkout validates that snapshot against current Product
state; digital lines receive no shipping profile, delivery group, shipping charge,
fulfillment item, or fulfillment row; mixed checkout invokes Fulfillment only for the
physical subset. Existing in-flight physical work may block provider disable or require
explicit reconciliation, while historical completed state remains readable under
Fulfillment ownership.

The remaining Commerce gap is owner-port parity on non-checkout mounted/admin paths;
that is separate from the completed typed fulfillment requirement cutover.

The canonical state matrix and disable rules are in
[`docs/architecture/settings.md`](../../../../docs/architecture/settings.md). No
current FFA/FBA status implies that this cutover is already implemented.

## Milestones

### Audit 2026-07-22: reopened P0 work

The following items were rechecked against current `main` after the staged checkout,
payment webhook, marketplace allocation, commission, and ledger source waves.

- [x] Reinspect the current checkout, compensation, order port, payment boundary,
      marketplace economics checkpoint, and master implementation plan.
- [x] Confirm that `CheckoutCompletionPort` ignored `cart_id` and
      `shipping_option_id`, created and confirmed an order through separate calls, and
      returned `order.checkout_result_projection_unavailable` for recovery reads.
- [x] Confirm that staged checkout and compensation constructed foreign
      `OrderService`, `PaymentService`, and `FulfillmentService` instances directly.
- [x] Confirm that checkout order creation and compensation queried the owner-owned
      `orders` table directly through JSON metadata instead of a typed order owner port.
- [x] Confirm that multiple ecommerce port mappers still place raw database/internal
      error text into public `PortError.message`.
- [x] Confirm that checkout/order/payment/fulfillment orchestration still relies on
      string lifecycle matching in critical paths.
- [x] Confirm that the marketplace economics checkpoint migration omitted MySQL
      integrity/immutability guards and left the PostgreSQL trigger function behind on
      rollback.
- [x] Harden the checkpoint migration source with PostgreSQL/SQLite/MySQL guard
      parity, fully immutable update behavior, and explicit backend cleanup before down.
- [x] Make economics checkpoint source admission concurrency-safe: after a failed
      insert, roll back the possibly aborted transaction, adopt the committed row when
      evidence is identical, and classify different evidence as a typed conflict rather
      than exposing a backend unique-violation error.
- [x] Add focused SQLite source coverage for concurrent identical adoption and
      concurrent conflicting evidence classification.
- [ ] Execute the checkpoint concurrency tests and retain PostgreSQL/MySQL contention
      evidence in addition to the SQLite source coverage.
- [ ] Run clean/upgraded/down/reapply checkpoint migration tests on SQLite,
      PostgreSQL, and MySQL; retain evidence.
- [x] Add owner-owned `order_checkout_identities` storage and an immutable order-local
      journal with typed reads by checkout operation, order, and known source cart.
- [x] Backfill only valid checkout operation and hash facts from legacy order metadata;
      keep an unknown historical cart or missing hashes as `NULL`.
- [x] Publish `CheckoutOrderIdentityPort` with typed read, bind, and explicit
      owner-local legacy adoption operations.
- [x] Cut staged checkout order identity and compensation identity recovery over to
      the order-owner port.
- [x] Remove direct `orders` SQL, local order-identity lookup helpers, and
      `order.metadata` identity validation from commerce creation and compensation.
- [x] Implement idempotent `CheckoutCompletionPort` create/place/replay and result
      reads by checkout operation and cart.
- [x] Cut staged checkout order creation/confirmation over to the owner completion
      command and explicit owner-provided recovery projection.
- [x] Remove `OrderService` construction from the staged order stage and pipeline.
- [x] Publish `CheckoutOrderCompensationPort` and
      `CheckoutPaymentCompensationPort` as owner-local compensation boundaries.
- [x] Cut the mounted compensation service over to typed order/payment owner ports;
      the old compensation source remains unmounted compatibility source.
- [x] Keep order identity/adoption, order cancellation, payment collection mutation,
      provider journal inspection, and provider cancel execution inside owner modules.
- [x] Preserve the pre-cutover `payment_collection:{collection_id}:cancel` provider
      key and immutable request payload so upgraded retry adopts one journal row instead
      of issuing a second provider cancellation.
- [x] Publish `CheckoutPaymentExecutionPort` for collection prepare, authorize,
      capture, and recovery reads with payment-owned provider journal and lifecycle policy.
- [x] Publish `CheckoutFulfillmentExecutionPort` for typed fulfillment set
      create/adopt/read without commerce SQL over fulfillment tables.
- [x] Publish `CheckoutOrderPaymentSettlementPort` for owner-local paid transition and
      payment-reference replay validation.
- [x] Cut mounted payment and fulfillment stages plus pipeline recovery over to the
      payment, fulfillment, and order owner ports.
- [x] Preserve canonical authorize/capture provider keys and legacy immutable request
      payload values so upgraded retries adopt existing provider-operation journal rows.
- [x] Keep payment provider result checkpointing, local collection mutation,
      fulfillment persistence/recovery, and order paid transition inside owner modules.
- [x] Add focused owner-port source tests and static commerce/order identity and
      completion-cutover boundary verifiers.

- [x] Add static compensation and owner-stage guards plus versioned order/payment/
      fulfillment workflow contracts without promoting FBA/FFA status.
- [x] Execute isolated unit fixtures for the completion-cutover verifier: 3/3 pass.
- [x] Add fail-closed `PortError` sanitization for unavailable/invariant failures at
      construction and serde transport boundaries, plus static source guards.
- [x] Harden pricing read/write port mapping: technical causes are internal-only and
      logged with correlation id, tenant, owner operation, and stable error code.
- [x] Harden `PaymentCollectionPort` create/reuse/status mapping with correlation-aware
      validation, lifecycle, provider, and database logging plus stable public messages.
- [ ] Finish correlation-safe mapper cleanup for order, payment execution/compensation,
      fulfillment, inventory, customer, tax, promotion, remaining ecommerce adapters, and
      non-`PortError` public envelopes.
- [x] Publish canonical typed lifecycle views for cart, order, order change, order
      return, payment collection, payment, refund, and fulfillment while preserving
      backward-compatible string transport fields and fail-closing unknown values.
- [x] Cut checkout order payment settlement and order compensation over to
      `OrderStatusKind` with manual reconciliation for unknown or effectful states.
- [x] Cut payment checkout compensation over to `PaymentCollectionStatusKind`, including
      cancel-race adoption, captured/manual-reconciliation routing, and provider-cancel
      admission.
- [x] Cut payment owner checkout execution authorize/capture admission and replay over
      to `PaymentCollectionStatusKind`; unknown states require manual reconciliation.
- [x] Cut the mounted commerce payment stage over to typed payment collection and order
      lifecycle views for authorization, capture, replay, and order admission.
- [x] Mount fulfillment ensure/read through the root typed lifecycle factory; pending,
      shipped, and delivered states are replay-safe while cancelled and unknown states
      require manual reconciliation.
- [x] Cut order checkout recovery and mounted order projection validation over to
      `OrderStatusKind`; pending resumes through owner confirmation and unknown states fail
      closed without raw string policy matching.
- [x] Cut atomic cart admission, mounted checkout finalization, and mounted compensation
      release/result validation over to canonical cart/order/payment lifecycle views;
      unknown cart states require manual reconciliation or fail closed.
- [x] Cut payment owner core transitions, refund creation, public provider
      orchestration, normalized webhook application, and mounted fulfillment captured-state
      recovery over to canonical typed lifecycle views.
- [x] Cut mounted REST, GraphQL, native, and root storefront checkout completion over to
      one staged recovery entrypoint with explicit idempotency and host payment-provider
      registry composition.
- [ ] Remove private legacy `CheckoutService` / `JournaledCheckoutService`
      compatibility source and public re-exports after compile, replay, and upgraded-path
      evidence; compatibility tests still construct those facades.
- [ ] Execute order identity/completion/compensation/payment/fulfillment Rust tests and
      the full static verifier set against a repository checkout.
- [ ] Execute order identity clean/upgraded/down/reapply, tenant mismatch, contention,
      legacy adoption, completion/compensation/payment/fulfillment kill-point, restart,
      and remote-profile evidence on SQLite, PostgreSQL, and MySQL.
- [ ] Remove the temporary owner metadata adoption bridges and superseded
      creation/confirmation/compensation/pipeline source after all recovery consumers use
      typed owner identity.
- [ ] Replace remaining direct foreign owner service construction outside the mounted
      staged checkout path with typed owner ports or explicit owner-provided adapters.
- [ ] Propagate typed lifecycle statuses through owner ports and remove string status
      matching from critical checkout, compensation, order, payment, and fulfillment paths.
      The 2026-09-25 slice now covers FulfillmentAdminCommandPort plus Commerce fulfillment
      orchestration/facade lifecycle checks; remaining critical paths still require cutover.

## Audit 2026-07-27: standalone dependency and topology P0

The marketplace/ecommerce isolation audit reopened the following topology work.
These are source-contract defects, not verification-only tasks.

- [x] Confirm that ecommerce and marketplace owner crates do not depend on AI, MCP,
      Alloy, or Media orchestration.
- [x] Confirm that `rustok-commerce` currently has undeclared hard dependencies on
      marketplace allocation, commission, ledger, and root orchestration.
- [x] Confirm that Commerce migrations and `MarketplaceFinancialRuntime` require
      marketplace owners even though `CommerceModule`, `rustok-module.toml`, and the
      `mod-commerce` feature do not declare or enable them.
- [x] Make marketplace financial integration an explicit optional Commerce capability;
      base Commerce composes and migrates without any marketplace owner in source.
- [x] Align the base `rustok-module.toml` / `CommerceModule::dependencies` contract with
      marketplace-free `mod-commerce`, and align Cargo capability features,
      distribution/server composition, migration sources, listeners, GraphQL/REST/OpenAPI
      surfaces, runtime injection, and worker startup with explicit marketplace financial
      selection.
- [x] Add `scripts/verify/verify-commerce-marketplace-financial-capability.mjs` as the
      lightweight source guard for the minimal Commerce topology; it is source-reviewed
      but not executed in this slice.
- [ ] Execute the minimal/base and marketplace-financial compile/migration profiles and
      the new topology guard, then retain evidence when validation policy allows it.
- [ ] Move remaining mounted Commerce REST/GraphQL construction of Product, Order,
      Payment, and Fulfillment concrete services behind host-composed owner ports.
- [x] Route checkout-plan shipping-option reads through the typed
      `ShippingOptionReadPort`; the builder no longer constructs `FulfillmentService`
      and accepts a host-selected shipping-option read port.
- [x] Make Fulfillment optional at the Commerce module-composition boundary: root/package
      dependency declarations no longer require it, and paid-order label listeners register only
      when the Fulfillment provider capability is actually composed.
- [x] Remove the remaining direct GraphQL cart shipping validator path in favor of the
      typed Fulfillment shipping-option read port, and reject selections without a matching
      physical delivery group.- [x] Publish Product-owned `ProductCatalogCommandPort` / `ProductCatalogCommandRuntime`,
      host-compose embedded or external command providers, and cut mounted admin REST
      product create/update over to the owner port with deadline, payload-bound deterministic
      write identity, channel, actor, locale, and stable public errors.
- [x] Cut mounted admin REST Product detail over to the host-selected
      `ProductCatalogReadPort`, preserving `PRODUCTS_READ`, tenant/actor/channel context,
      requested locale plus tenant fallback locale, a bounded deadline, and stable public errors.
- [x] Publish an optional fail-closed admin-list capability on `ProductCatalogReadPort`
      without changing the existing `AdminProductListQuery` public shape, and cut mounted
      admin REST Product list to the host-selected owner port while preserving exact legacy
      status/vendor/product-type/search filtering, created-at-desc ordering, pagination,
      locale fallback, empty missing-title projection, normalized/default shipping-profile
      slug, tags, request context, deadline, and public error envelope.
- [x] Cut mounted admin GraphQL Product catalog reads to the host-selected
      `ProductCatalogReadRuntime` / `ProductCatalogReadPort`, preserving tenant/permission
      admission, typed catalog filters, pagination validation, requested/default locale,
      request channel, authenticated actor, bounded deadline, response projection, and
      correlation-aware stable public owner errors.
- [x] Publish an optional filtered storefront-list capability without changing
      `PublishedProductsRequest`, and cut mounted storefront GraphQL Product catalog reads
      to the host-selected `ProductCatalogReadRuntime` / `ProductCatalogReadPort` while
      preserving search/category/sort/attribute filters, pagination validation, channel
      visibility, requested/default locale, service actor/deadline context, response
      projection, and correlation-aware stable public owner errors.
- [x] Publish Product-owned `ProductCatalogSchemaReadPort` as an optional secondary
      capability on `ProductCatalogReadRuntime`, preserving existing catalog-read runtime
      construction, and cut mounted GraphQL `productAttributes`, `catalogCategories`, and
      `productAttributeSchemas` to it with current-tenant/permission admission,
      authenticated actor, trimmed locale, request channel, bounded deadline, and the
      shared stable Product GraphQL public error mapper.
- [x] Cut mounted GraphQL `productEffectiveForm` to the host-selected
      `ProductCatalogSchemaReadPort::read_effective_form`, preserving current-tenant and
      `PRODUCTS_READ` admission, authenticated actor, trimmed locale, request channel,
      bounded deadline, existing product/category input precedence, and the shared stable
      Product GraphQL public error mapper.
- [x] Cut mounted GraphQL `productAttributeValues` to the host-selected
      `ProductCatalogSchemaReadPort::read_product_attribute_values`, preserving current-tenant
      and `PRODUCTS_READ` admission, authenticated actor, trimmed locale, request channel,
      bounded deadline, the existing GraphQL projection, and the shared stable Product
      GraphQL public error mapper.
- [x] Cut mounted GraphQL `storefrontCatalogSearchOptions` to the host-selected
      `ProductCatalogSchemaReadPort` using its existing `list_categories` and
      `list_attributes` capabilities, preserving storefront-channel admission, required
      trimmed locale, current-tenant scope, service actor, request channel, bounded deadline,
      exact category/attribute option projection semantics, and the shared stable Product
      GraphQL public error mapper.
- [x] Publish optional fail-closed `ProductCatalogReadPort::list_legacy_admin_products`
      and cut mounted legacy admin GraphQL `product` / `products` to the host-selected
      Product read runtime, preserving current-tenant and Product permission admission,
      the detail resolver's existing storefront-channel gate, authenticated actor, request
      channel, requested/default locale fallback, bounded deadline, `NotFound -> null`,
      status/vendor/search filters, created-at-desc ordering, clamped pagination,
      `Untitled product`, normalized/default shipping-profile fallback, tags, telemetry,
      response projection, and shared stable Product GraphQL public errors.
- [x] Cut mounted legacy storefront GraphQL `storefrontProduct` / `storefrontProducts`
      to host-composed Product read capabilities while preserving storefront admission,
      current tenant, id-over-handle detail semantics, locale/channel visibility, public
      inventory, legacy filters/order/pagination, title/shipping fallback, tags, telemetry,
      and stable public errors.
- [x] Cut mounted admin REST Product delete/publish/unpublish lifecycle commands to the
      host-composed Product command runtime with required caller `Idempotency-Key`, scoped
      command identity, bounded context, unchanged permissions/responses, and stable owner
      errors.
- [x] Cut mounted GraphQL Product create/update/publish/delete lifecycle execution to
      the host-selected `ProductCatalogCommandRuntime` / `ProductCatalogCommandPort`, keep
      create/update compatibility validation and public error identities, and require an
      explicit caller `idempotencyKey` for every lifecycle execution with scoped command
      identity and no compatibility-generated caller identity.
- [x] Teach Product Admin FFA lifecycle commands to retain one GraphQL caller
      idempotency key across explicit retries, then make mounted GraphQL `idempotencyKey`
      non-null/mandatory after updating current regression callers to supply explicit keys.
- [ ] Complete Product schema-write durable idempotency: mounted consumers already use
      typed Product owner write capabilities with mandatory caller identity. Source-complete
      durable owner receipts cover all six schema create operations; update-style schema writes still need explicit owner replay semantics.
- [x] Publish the order-owned `OrderReadPort` for complete order, return, and
      order-change detail/list projections with canonical read context/deadline policy,
      stable typed errors, filters, ordering, totals, and explicit unvalidated evidence.
- [x] Host-compose `CommerceOrderReadRuntime`, require it in Commerce HTTP and GraphQL
      schema data, and cut admin REST order list/detail to the owner port while preserving
      public envelopes and payment/fulfillment detail aggregation.
- [x] Cut mounted GraphQL order list/detail to the host-selected runtime with validated
      actor, resolved channel, locale fallback, deadline, and embedded-schema fallback.
- [x] Cut storefront HTTP order detail and shared ownership reads to the host-selected
      runtime while preserving customer resolution, locale fallback, public envelopes,
      and the concrete operations after ownership validation.
- [x] Cut storefront return and order-change list reads to the same host-selected
      runtime while preserving filters, ordering, complete DTOs, totals, and envelopes.
- [x] Cut GraphQL return/order-change detail and list reads to the scoped host-selected
      runtime with validated actor, resolved channel/effective-locale context, typed
      not-found compatibility errors, and no concrete owner service storage.
- [x] Cut mounted admin return/order-change detail and list reads to the host-selected
      runtime while preserving `ORDERS_READ`, filters, clamped pagination, totals, public
      envelopes, actor/channel/effective-locale/deadline context, and unchanged mutation,
      payment, and fulfillment ownership.
- [ ] Execute compile and mounted parity for the four admin post-order GET routes.
      The superseded compatibility GET handlers are already absent from the source;
      only validation evidence remains.
- [ ] Retain compile, mounted parity, deadline/failure, restart, and remote-adapter
      evidence for order and post-order projections before status promotion.
- [ ] Correct Product's declared dependency contract or extract its direct
      inventory/pricing persistence operations behind owner ports; the current manifest
      declares only Taxonomy while production code uses Inventory and Pricing persistence.
- [ ] Make the server's AI dependency truly optional so a non-AI ecommerce deployment
      can be composed without compiling the AI host.

## FBA/FFA architecture invariants

- [x] Keep owner persistence, lifecycle policy, receipts, events, and provider policy
      inside owner modules.
- [ ] Use typed FBA ports rather than foreign entities, direct foreign service
      construction, or cross-module DB access on every production orchestration path.
- [x] Carry tenant, actor, effective locale, channel, correlation, deadline, and
      idempotency context across published owner calls.
- [x] Keep in-process providers behind the same contracts expected by remote adapters.
- [x] Build FFA as module-owned core/model/transport/i18n/thin-UI packages; hosts only
      compose them.
- [x] Require explicit native/GraphQL transport selection; silent fallback is
      forbidden unless explicitly contracted and verified.
- [x] Use `core_only -> core_transport -> core_transport_ui`.
- [x] Keep the commerce admin FFA boundary locked by
      `scripts/verify/verify-commerce-admin-boundary.mjs`; the native adapter lives at
      `admin/src/transport/native_server_adapter.rs`.
- [x] Keep root GraphQL and state-machine aliases removed so callers use explicit
      module paths rather than umbrella re-exports.
- [ ] Keep provider raw payloads, signatures, SQL errors, SDK errors, and internal
      invariant details out of owner persistence and public errors on every ecommerce port.
- [x] Preserve unknown historical attribution as typed unknown provenance rather than
      sentinel UUIDs or guessed locales.
- [x] Allow request-scoped hosts to inject typed owner ports, authorization, and
      canonical `PortContext`; FFA packages must not construct foreign providers.
- [ ] Retain compiled remote-profile evidence before FBA `transport_verified`.
- [ ] Retain mounted native/GraphQL parity before FFA `phase_b_ready`.

## Commerce orchestration

### Checkout

- [x] Require stable checkout idempotency across REST, GraphQL, native, and UI.
- [x] Route production checkout through staged recovery orchestration.
- [ ] Resolve cart, product, pricing, inventory, order, payment, and fulfillment only
      through typed owner boundaries on every production path; mounted storefront checkout
      completion is cut over, while other ecommerce orchestration paths still require
      cutover/audit.
- [x] Persist immutable plans, operation identity, hashes, lease, stages, errors, and
      owner ids.
- [x] Keep the checkout inventory reservation entity aligned with the adopted order-line
      column introduced by the checkout lifecycle migration.
- [x] Resume persisted stages and adopt already committed owner outcomes where owner
      identity is available.
- [x] Prevent a second active checkout for the same cart.
- [x] Provide durable compensation state and block provider execution during open
      reconciliation.
- [x] Persist typed marketplace cart/checkout snapshots and fail closed when marketplace
      identity or economics are missing.
- [x] Run marketplace allocation and commission assessment before payment capture.
- [x] Persist a lease-bound allocation/commission economics checkpoint and adopt it on replay.
- [x] Post marketplace ledger after capture through a durable financial operation and gate
      fulfillment on saved ledger evidence.
- [x] Add backend-parity source guards and reversible cleanup for the marketplace
      economics checkpoint migration.
- [x] Add concurrent economics-checkpoint admission/adoption and typed conflict source
      coverage without promoting runtime evidence.
- [x] Add order-owned typed checkout operation identity storage, truthful legacy
      backfill, immutable guards, and owner-local reads/journal source.
- [x] Populate, read, bind, and adopt order identity through
      `CheckoutOrderIdentityPort` from staged creation and owner-local compatibility paths.
- [x] Remove direct `orders` table reads and order metadata identity validation from
      checkout creation and compensation.
- [x] Implement one idempotent owner command for order create/place/replay and durable
      checkout-result reads by operation/cart.
- [x] Cut staged checkout creation/confirmation and payment-ready recovery over to
      `CheckoutCompletionPort` plus an explicit owner-provided full-order projection.
- [x] Preserve crash compatibility with the previous staged request hashes only inside
      the owner recovery adapter; new creation always uses the owner completion command.
- [x] Allow reservation adoption after the owner order is confirmed and retain durable
      `inventory_reserved -> order_created -> payment_ready` commerce checkpoints.
- [x] Cut checkout compensation over to `CheckoutOrderCompensationPort` and
      `CheckoutPaymentCompensationPort`.
- [x] Keep provider journal inspection and provider cancellation inside payment owner;
      preserve the canonical pre-cutover cancel identity for upgraded replay.
- [x] Keep order identity resolution, cancellation, and lifecycle replay inside order
      owner; paid/shipped/delivered states fail closed into manual reconciliation.
- [x] Cut payment prepare/authorize/capture/read over to
      `CheckoutPaymentExecutionPort`; commerce owns only stage checkpoints.
- [x] Cut fulfillment create/adopt/read over to
      `CheckoutFulfillmentExecutionPort`; commerce no longer queries fulfillment storage.
- [x] Cut order paid transition/replay over to
      `CheckoutOrderPaymentSettlementPort`.
- [x] Mount owner-port pipeline recovery so payment and fulfillment state reloads use
      the same owner stages as live execution.
- [x] Preserve canonical provider authorize/capture keys and pre-cutover request payload
      values for upgraded journal replay.
- [x] Add static guards for order identity, completion, compensation, and mounted
      payment/fulfillment/pipeline owner boundaries.
- [x] Use canonical typed order/payment collection lifecycle views in payment owner
      execution/compensation and mounted payment execution; unknown values fail closed.
- [x] Route mounted fulfillment ensure/read through
      `TypedCheckoutFulfillmentExecutionPort`; cancelled and unknown owner states require
      manual reconciliation before checkout can continue.
- [x] Use `OrderStatusKind` in the owner recovery adapter and mounted order projection
      policy; legacy pending resumes through the owner confirm command.
- [x] Use canonical `CartStatus` in atomic checkout admission, mounted finalization, and
      mounted compensation release; compensation snapshots expose typed order/payment
      status views without changing wire fields.
- [x] Use canonical payment collection/payment/refund statuses in owner core lifecycle,
      refund admission/replay, public provider orchestration, webhook application, and
      mounted fulfillment captured-state validation.
- [x] Route REST, GraphQL, native, and root storefront completion through
      `RecoveringStagedCheckoutService`; all mounted transports preserve the caller
      idempotency key and host payment-provider registry.
- [ ] Remove private legacy checkout completion and journal facade source after
      compile/replay/upgraded evidence, then migrate compatibility tests to the staged
      harness and remove public re-exports.
- [x] Replace fulfillment metadata identity with owner-owned typed persistence and a
      concurrency-safe uniqueness constraint.
- [ ] Remove temporary metadata write/adoption bridges and old executor/compensation/
      pipeline source after upgraded/restart evidence proves every recovery path uses typed
      identity.
- [ ] Retain REST/GraphQL/native staged parity plus order completion/adoption, payment
      execution, fulfillment execution, settlement, cart finalization/release, compensation,
      kill-point, restart, and PostgreSQL/MySQL contention evidence.
- [ ] Execute complete mounted operator compensation/reconciliation workflows.

### Return completion

- [x] Keep refund/exchange/claim completion orchestration in commerce.
- [x] Persist durable return-completion operation and immutable command snapshots.
- [x] Atomically admit command and pending operation before provider/owner effects.
- [x] Adopt existing refunds/order changes/completed returns and reject conflicting
      replay payloads.
- [x] Classify uncertain external outcomes as `reconciliation_required`.
- [x] Publish tenant-scoped operator list/show/retry without exposing command payloads.
- [x] Remove superseded REST/GraphQL return-completion helper paths after both
      transports moved to `ReturnCompletionOrchestrationService`.
- [x] Enforce return-completion operation SHA-256 request-hash and terminal
      stage/status invariants at the persistence layer across supported database backends.
- [ ] Apply return-completion migrations on clean/upgraded SQLite/PostgreSQL/MySQL.
- [ ] Execute replay, conflict, admission/claim contention, lease expiry, process-exit,
      restart, and reconciliation-resolution evidence.

## Marketplace Family

### Naming and composition

- [x] Use mandatory `rustok-marketplace-*` crate names and `marketplace_*` slugs.
- [x] Publish `rustok-marketplace` as a composition/orchestration root with no owner
      tables.
- [x] Publish seller, listing, allocation, commission, ledger, and payout as separate
      owner modules.
- [x] Keep Marketplace modules opt-in and outside default module/server sets.
- [x] Keep catalog, prices, stock, orders, payments, and generic orchestration in their
      existing owner modules.
- [x] Register owner migrations through the composed `MigrationSource` graph without
      cross-owner foreign keys.

### Marketplace root

- [x] Compose seller directory over `MarketplaceSellerReadPort`.
- [x] Compose listing directory and eligibility over `MarketplaceListingReadPort`.
- [x] Keep root consumers free of SeaORM, owner entities, and owner DB access.
- [x] Compose order commission posting and financial reversals over typed commission and
      ledger ports with deterministic child idempotency keys.
- [x] Advertise ledger v3 seller balance transfer capability without adding root persistence.
- [ ] Add deterministic multi-order payout orchestration over per-order ledger transfers.
- [ ] Compile/execute root consumers and retain remote timeout/degraded/fallback
      evidence.
- [ ] Keep root FFA absent until an aggregate control room can compose owner view
      models without owning policy.

### Seller FBA completed

- [x] Own seller identity, legal profile, onboarding/lifecycle, memberships, roles,
      and seller policy.
- [x] Keep platform RBAC separate from seller membership policy.
- [x] Keep base seller rows language-agnostic and localized `display_name` in
      normalized translation rows.
- [x] Enforce tenant/seller/locale identity, normalized locale tags, and
      `VARCHAR(32)` storage.
- [x] Use exact effective locale from `PortContext`; owner-side fallback is forbidden.
- [x] Return `resolved_locale` through FBA and FFA projections.
- [x] Create seller, translation, and initial owner membership atomically.
- [x] Persist durable actor-bound command receipts and reject idempotency conflicts.
- [x] Use SQL `ON CONFLICT` for concurrent translation upsert.
- [x] Route all seller FBA writes through the receipt executor.
- [x] Persist append-only seller lifecycle/moderation events with truthful command or
      legacy-snapshot provenance and bounded newest-first timeline reads.
- [x] Commit onboarding review, suspension, and reactivation state, immutable event,
      completed receipt, and normalized response snapshot in one owner transaction.
- [x] Commit seller creation, profile update, onboarding submit, member add, and member
      update state, locale-bound immutable event, completed receipt, and normalized response
      snapshot in one owner transaction.
- [x] Publish every completed live seller/member contract through the transactional
      outbox before receipt completion and transaction commit.
- [x] Prove completed-receipt replay does not append another lifecycle/member event and
      event persistence failure rolls back state plus the pending receipt.
- [x] Backfill legacy seller prose as truthful snapshot events, project current prose
      from the immutable timeline, and drop mutable compatibility columns in source.

### Seller FFA completed

- [x] Publish module-owned seller admin core/model/transport/i18n/Leptos package.
- [x] Implement native and GraphQL source workflows over the same ports/envelope.
- [x] Use canonical request effective locale and return `resolved_locale`.
- [x] Preserve idempotency key for retries and forbid implicit transport fallback.
- [x] Add bounded lifecycle/member event history models, native server transport,
      GraphQL adapter, and timeline UI over the owner read port.

### Seller remaining

- [ ] Add normalized verification facts and KYC provider SPI without raw payloads.
- [ ] Compile seller/core/GraphQL/admin packages, apply clean/upgraded PostgreSQL
      migrations, and execute locale, replay, tenant, contention, rollback, outbox,
      mounted FFA, and remote-profile evidence.

### Listing FBA completed

- [x] Own seller/master-variant/market/channel identity, seller SKU, versioned terms,
      lifecycle, approval, immutable events, and deterministic eligibility.
- [x] Keep canonical localized product copy, prices, stock, and fulfillment state in
      their owner modules.
- [x] Store no localized product copy or locale-keyed JSON maps in listing tables.
- [x] Use no cross-module DB foreign keys.
- [x] Publish typed listing read/command ports with deadline, error, and idempotency
      rules.
- [x] Resolve seller/product facts through seller and product FBA ports.
- [x] Persist durable listing command receipts and normalized result snapshots.
- [x] Persist append-only listing events with typed provenance and bounded newest-first
      timeline reads.
- [x] Route all eight listing commands through atomic owner state/terms + event +
      receipt executors.
- [x] Check completed receipt replay before provider reads for create, publish, and
      reactivate, then re-check admission after provider preflight.
- [x] Require actor/effective locale for command events.
- [x] Keep `MarketplaceListingService` read-only; direct write bypasses are removed.
- [x] Import legacy approval/suspension notes as `legacy_snapshot` events with null
      actor/locale and source-column metadata.
- [x] Remove mutable `approval_note` and `suspension_reason` from final entity, DTO,
      write paths, and post-cutover schema.
- [x] Register listing in modules, distribution, and server as opt-in backend owner.
- [x] Restore evented module registration and FBA command routing after parallel source
      drift.
- [x] Define the sealed, typed nine-event `MarketplaceListingEvent` family and preserve
      it through transactional outbox relay and Iggy transport.
- [x] Publish one external contract event from receipt completion in the same owner
      transaction as state/terms, internal event, and completed receipt.
- [x] Keep moderation notes, reasons, arbitrary metadata, and imported legacy snapshots
      out of the external contract payload and live relay path.

### Listing FFA source completed

- [x] Publish `rustok-marketplace-listing-admin` with framework-neutral models/core,
      explicit transport facade, English/Russian catalogs, and a thin Leptos adapter.
- [x] Model listing directory/detail, current terms, immutable history, and all eight
      listing commands.
- [x] Preserve the same command/idempotency key for explicit retry.
- [x] Render `legacy_snapshot` events as unknown attribution rather than command facts.
- [x] Require a request-scoped host runtime with typed listing ports, authorization,
      and canonical `PortContext` construction for native execution.
- [x] Publish listing GraphQL roots and a real GraphQL admin adapter over the same
      host-composed typed ports as authenticated native execution.
- [x] Register the module-owned admin package and locale path in the listing manifest.
- [x] Register the nested admin crate in workspace/admin hydrate and SSR feature graphs.
- [x] Add platform `marketplace_listings` permissions and module-owned workflow mapping.

### Listing remaining

- [ ] Add product matching/approval before automated EAN/GTIN matching,
      deduplication, or buy-box ranking.
- [ ] Compile listing/root/provider/admin contracts and execute clean/upgraded
      migrations, replay, provider-preflight races, locale/provenance constraints, tenant
      isolation, PostgreSQL event/outbox atomicity, contention, rollback, relay, restart,
      and mounted transport evidence.

### Marketplace order allocation and finance

- [x] Own immutable order-line allocations without duplicating customer order aggregates.
- [x] Snapshot seller, listing, commission result, fulfillment ownership, and monetary
      allocation at checkout.
- [x] Prevent one seller lifecycle operation from mutating another seller allocation.
- [x] Create versioned deterministic commission policy owner.
- [x] Create immutable double-entry ledger before balances or payouts.
- [x] Derive pending, available, reserved, paid, and negative seller balances from immutable
      seller-payable entries.
- [x] Add append-only refund/chargeback reversals with exact original-entry links and
      cumulative capacity.
- [x] Add payment-owner processed-event observers, durable reversal inbox/recovery, safe
      operator transports, and a durable historical adaptation-failure journal.
- [x] Add append-only pending release, reserve hold/release, payout settlement/reversal
      ledger transfers with exact reference-entry lineage and cumulative capacity.
- [x] Keep PSP split-payment optional; internal allocation/ledger correctness does not
      depend on a PSP.
- [x] Add payout scheduling owner and exclusive ledger-entry assignment.
- [ ] Add payout provider accounts, operation journal, verified webhook inbox, transfer
      execution, lookup recovery, and deterministic multi-order settlement orchestration.
- [ ] Add accounting/vendor surfaces and retained contention/reconciliation evidence.

## Payment workstream

- [x] Keep collections, payments, refunds, provider-operation journals, and webhook
      inbox state in payment.
- [x] Publish typed payment collection ports and idempotent refund identity.
- [x] Publish `CheckoutPaymentCompensationPort` for owner-local collection read,
      provider-journal replay, external cancel, local cancel, and safe outcome projection.
- [x] Preserve the canonical pre-port provider cancel key and immutable request payload
      so upgraded compensation retries do not duplicate the PSP call.
- [x] Publish `CheckoutPaymentExecutionPort` for owner-local collection prepare,
      authorize, capture, and recovery reads.
- [x] Preserve canonical authorize/capture keys and immutable legacy payload values so
      upgraded staged checkout retries adopt existing provider journal rows.
- [x] Checkpoint normalized provider success before local collection mutation and route
      local persistence failure after provider success to reconciliation.
- [x] Route Fulfillment local provider-result reconciliation through the owner receipt-aware lifecycle boundary so `provider_operation` metadata and journal commit remain transactionally coupled; validate replay request identity before local mutation.
- [x] Guard provider operations through the provider registry with CAS journals and
      explicit reconciliation outcomes.
- [x] Route uncertain external outcomes to reconciliation and forbid auto-reclaim.
- [x] Keep refund reservation validation in the idempotent
      `PaymentRefundCreationService` and remove the superseded `PaymentService` duplicate.
- [x] Add tenant-scoped Stripe source and deployment-owned secret resolution.
- [x] Mount verified webhook ingress and persist only normalized immutable facts.
- [x] Recover received/failed/expired events; isolate dead letters and require
      operator-only replay.
- [x] Normalize `refund.completed` and `chargeback.completed`, run host-composed observers
      only after payment owner application, and mark provider events processed only after observers
      succeed.
- [x] Keep marketplace reversal consumers free of raw provider payloads and signatures.
- [x] Use `PaymentCollectionStatusKind` in checkout execution, checkout compensation,
      and the mounted payment stage; unknown collection states require manual reconciliation
      or fail closed.
- [x] Use canonical payment/refund lifecycle policy in `PaymentService`, idempotent
      refund creation, public provider orchestration, normalized webhook application, and
      mounted fulfillment captured-state recovery.
- [x] Remove production storefront completion use of the legacy `CheckoutService`;
      mounted REST, GraphQL, native, and root completion use staged payment-owner execution.
- [x] Make `PaymentCollectionPort` error mapping correlation-aware and keep validation,
      lifecycle, provider, configuration, outcome, and storage causes out of public messages.
- [ ] Audit payment execution/compensation and remaining admin/provider adapters, then
      remove the compatibility checkout service/journal source after retained compile and
      replay evidence.
- [ ] Detect marketplace-associated reversal events that omit required typed marketplace facts
      and route them to durable operator review.
- [ ] Execute checkout compensation and payment execution provider replay, crash,
      concurrent claim, unknown-outcome, and reconciliation-required evidence.
- [ ] Execute production-like Stripe, real signature, redelivery, restart, replica,
      degraded, reconciliation, observer replay, and operator evidence.
- [ ] Prove adapters never own payment/refund lifecycle state.
- [x] Harden Marketplace Payout ledger-boundary error mapping so internal owner messages
      cannot cross the payout port boundary; preserve stable codes and classify forbidden,
      unavailable/timeout, and invariant failures as safe public categories.
- [x] Extend the ecommerce public-port static verifier to cover Marketplace Payout
      ledger-boundary redaction and exact typed error-kind preservation.
- [x] Extend the same fail-closed error-boundary treatment through Marketplace
      Commission and Marketplace Ledger: foreign owner messages are redacted at each hop and
      typed PortError kinds are preserved instead of being collapsed by retryability.
- [x] Remove direct RegionService construction from mounted staged checkout; the checkout
      plan builder now receives the RegionReadPort through the owner factory.
- [x] Move Marketplace Allocation, Commission, and Ledger checkout composition behind
      owner-provided in-process command-port factories; staged Commerce no longer constructs
      foreign marketplace services directly.

## Verification

### Verification and promotion checklist

Source inspection is not execution evidence.

### Static

- [ ] `cargo fmt --all -- --check`
- [ ] `npm run verify:ecommerce:fba`
- [ ] `npm run verify:marketplace`
- [x] `node scripts/verify/verify-marketplace-seller-events.mjs`
- [ ] `node scripts/verify/verify-marketplace-listing-event-contract.mjs`
- [ ] `node scripts/verify/verify-marketplace-listing-provenance-cutover.mjs`
- [ ] `node scripts/verify/verify-order-read-port.mjs`
- [ ] `node scripts/verify/verify-commerce-graphql-order-read-shim.mjs`
- [ ] `node scripts/verify/verify-commerce-storefront-order-read-cutover.mjs`
- [ ] `node scripts/verify/verify-commerce-storefront-post-order-read-cutover.mjs`
- [ ] `node scripts/verify/verify-commerce-admin-post-order-read-cutover.mjs`
- [ ] `node scripts/verify/verify-commerce-admin-order-route-error-context.mjs`
- [ ] `node scripts/verify/verify-commerce-order-identity-boundary.mjs`
- [ ] `node --test scripts/verify/verify-commerce-order-identity-boundary.test.mjs`
- [ ] `node scripts/verify/verify-commerce-checkout-completion-cutover.mjs`
- [x] `node --test scripts/verify/verify-commerce-checkout-completion-cutover.test.mjs`
      (isolated fixture execution: 3/3 pass)
- [ ] `node scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs`
- [ ] `node scripts/verify/verify-commerce-checkout-owner-stage-boundary.mjs`
- [ ] `node scripts/verify/verify-commerce-storefront-staged-checkout-cutover.mjs`
- [ ] `node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs`
- [ ] `node scripts/verify/verify-commerce-marketplace-financial-capability.mjs`
- [ ] `node scripts/verify/verify-commerce-product-command-port.mjs`
- [ ] `node scripts/verify/verify-commerce-product-rest-lifecycle-command-cutover.mjs`
- [ ] `node scripts/verify/verify-commerce-product-graphql-lifecycle-command-cutover.mjs`
- [ ] `node scripts/verify/verify-product-admin-lifecycle-retry-consumer.mjs`
- [ ] `node scripts/verify/verify-commerce-product-admin-detail-read.mjs`
- [ ] `node scripts/verify/verify-commerce-product-admin-list-read.mjs`
- [ ] `node scripts/verify/verify-commerce-product-admin-graphql-read.mjs`
- [ ] `node scripts/verify/verify-commerce-product-storefront-graphql-read.mjs`
- [ ] `node scripts/verify/verify-product-catalog-schema-read-port.mjs`
- [ ] `cargo xtask module validate commerce`
- [ ] `cargo xtask module validate order`
- [ ] `cargo xtask module validate payment`
- [ ] `cargo xtask module validate fulfillment`
- [ ] `cargo xtask module validate marketplace`
- [ ] `cargo xtask module validate marketplace_seller`
- [ ] `cargo xtask module validate marketplace_listing`
- [ ] Inspect marketplace ledger v3 and seller-balance-transfer v1 source guards.
- [x] Add a static guard that forbids direct `orders` SQL, local identity lookup helpers,
      and `order.metadata` identity reads in commerce creation/compensation source.
- [x] Add a static guard that forbids old order create/confirm executors and
      `OrderService` in the staged order stage/pipeline.
- [x] Add a static guard that forbids direct order/payment services and payment provider
      journal access in the mounted compensation source.
- [x] Add a static guard that forbids `PaymentService`, `FulfillmentService`,
      `OrderService`, provider journal access, and fulfillment SQL in mounted payment,
      fulfillment, and pipeline source.
- [x] Add static guards for fail-closed public `PortError` transport sanitization and
- [x] Add a static fulfillment-requirement boundary guard spanning Product, Cart, Order, and Checkout.
      typed order/payment/fulfillment/cart lifecycle use in checkout execution, recovery,
      finalization, settlement, compensation, provider orchestration, webhook application,
      and mounted owner stages.
- [x] Add a storefront staged-cutover guard that forbids `CheckoutService` and
      `JournaledCheckoutService` construction in mounted REST, GraphQL, native, root, and
      staged completion paths.
- [x] Extend the public-error guard to pricing and `PaymentCollectionPort`, including
      correlation, tenant, operation, stable code, and raw-cause bans.
- [x] Add static source guards for host-selected complete order and post-order read
      runtime composition plus admin REST complete/post-order, mounted GraphQL
      complete/post-order, and storefront order/return/change cutovers without changing
      mutation, payment, or fulfillment ownership; unmounted admin compatibility GET
      handlers remain explicit source debt.
- [ ] Execute the new public-error, typed-lifecycle, storefront-cutover, order-read,
      marketplace-financial topology, Product command-port, Product REST lifecycle,
      Product GraphQL lifecycle, Product admin-detail read, Product admin-list read,
      Product admin-GraphQL read, Product storefront-GraphQL read, and Product
      schema-directory read static guards against a repository checkout and retain their
      output.

### Compile/tests

- [ ] `cargo check -p rustok-commerce --lib --no-default-features`
- [ ] `cargo check -p rustok-commerce --lib --features marketplace-financial`
- [ ] `cargo test -p rustok-commerce --test checkout_marketplace_economics_checkpoint`
- [ ] `cargo check -p rustok-order --all-features`
- [ ] `cargo check -p rustok-server --no-default-features --features mod-commerce`
- [ ] `cargo check -p rustok-server --no-default-features --features commerce-marketplace-financial`
- [ ] Targeted `OrderReadPort` seven-operation, locale fallback, context, typed-error,
      host-composition, admin REST, mounted GraphQL, and storefront HTTP transport tests.
- [ ] `cargo test -p rustok-order --test order_checkout_identity`
- [ ] `cargo test -p rustok-order --test checkout_order_identity_port`
- [ ] `cargo test -p rustok-order --test checkout_completion_port`
- [ ] Targeted staged checkout completion/adoption/replay, compensation, and order
      payment settlement tests.
- [ ] Targeted REST/GraphQL/native staged checkout parity, idempotency conflict, provider
      registry composition, and reconciliation mapping tests.
- [ ] `cargo check -p rustok-pricing --all-features`
- [ ] Targeted pricing database, validation, rich/core invariant, correlation, and
      transport round-trip tests.
- [ ] `cargo check -p rustok-payment --all-features`
- [ ] Targeted payment collection error mapping plus owner lifecycle,
      compensation/execution canonical-key, legacy-payload replay, refund admission, and
      webhook adaptation tests.
- [ ] `cargo check -p rustok-fulfillment --all-features`
- [ ] Targeted fulfillment create/adopt/read, cancelled/unknown lifecycle, duplicate
      identity, partial set, and concurrent create tests.
- [ ] `cargo check -p rustok-cart --all-features`
- [ ] Targeted atomic cart admission, finalization, release, unknown lifecycle, and
      compensation snapshot tests.
- [ ] `cargo check -p rustok-marketplace --lib`
- [ ] `cargo check -p rustok-marketplace-ledger --all-targets`
- [ ] `cargo test -p rustok-marketplace-ledger`
- [x] `cargo check -p rustok-marketplace-seller --all-targets`
- [x] `cargo test -p rustok-marketplace-seller`
- [ ] `cargo check -p rustok-marketplace-seller-admin --all-features`
- [ ] `cargo check -p rustok-marketplace-listing --lib`
- [ ] `cargo test -p rustok-marketplace-listing`
- [ ] `cargo check -p rustok-marketplace-listing-admin --all-features`
- [ ] `cargo check -p rustok-server --features mod-marketplace`
- [ ] Targeted checkout, order identity/completion/compensation/settlement,
      payment/fulfillment execution, return-completion, marketplace financial recovery,
      seller balance transfer, remaining seller/listing lifecycle, localization, outbox
      replay/rollback, and tenant-isolation tests.

### Database/runtime

- [ ] Apply clean/upgraded SQLite/PostgreSQL/MySQL and rollback/reapply paths, respecting the
      intentionally irreversible listing provenance cutover.
- [ ] Specifically prove marketplace economics checkpoint identity, amount,
      immutability, tenant/order linkage, and cleanup on all supported backends.
- [ ] Prove order checkout identity backfill truthfulness, tenant/order integrity,
      operation/order/cart uniqueness, monotonic enrichment, rollback, legacy adoption,
      completion replay, compensation identity, payment settlement, and owner-port recovery
      on all supported backends.
- [ ] Prove payment compensation/execution adopts pre-cutover provider journal rows and
      never repeats an executing, succeeded, or reconciliation-required external effect.
- [ ] Prove fulfillment create/adopt/read returns one exact immutable set and routes
      cancelled/unknown lifecycle states to reconciliation under duplicate, concurrent,
      process-exit, restart, and upgraded metadata scenarios.
- [ ] Prove atomic cart admission, complete/release replay, unknown lifecycle,
      process-exit, and restart behavior through the mounted finalization/compensation paths.
- [ ] Execute receipt/event/outbox/provider-operation/checkpoint contention and restart scenarios.
- [ ] Execute seller/listing tenant isolation and cross-locale/provenance scenarios.
- [ ] Execute reversal observer/inbox/adaptation recovery and safe operator scenarios.
- [ ] Execute seller balance transfer replay, duplicate source, cumulative reference capacity,
      concurrent admission, projection rebuild, and append-only trigger scenarios.
- [ ] Prove declared routers and module-owned UI packages are mounted.
- [ ] Exercise authenticated REST/GraphQL/native checkout, recovery, compensation, seller
      admin, listing admin, reconciliation, and replay.
- [ ] Retain remote-profile and real payment/provider evidence.

## Immediate execution order

1. [x] Reaudit current checkout/order/payment/marketplace source and reopen false-complete P0s.
2. [x] Harden marketplace economics checkpoint migration source for backend parity and rollback.
3. [x] Add economics checkpoint concurrent insert adoption and exact conflict classification source.
4. [x] Add order-owned checkout operation identity storage, migration, journal, and typed reads.
5. [x] Publish the typed order identity port, populate/adopt it from staged checkout,
       and remove direct commerce JSON metadata/SQL identity reads.
6. [x] Complete idempotent `CheckoutCompletionPort` create/place/read source semantics.
7. [x] Cut staged checkout order creation/confirmation over to the completed owner port.
8. [x] Cut compensation over to typed order/payment owner ports and remove foreign
       services/journal access from the mounted compensation path.
9. [x] Cut payment and fulfillment stages plus pipeline recovery over to explicit
       payment, fulfillment, and order owner ports.
10. [ ] Finish raw public ecommerce port error removal and correlation-safe owner logging;
        central fail-closed transport sanitization plus channel, region, cart checkout,
        pricing, and payment collection mapper hardening are complete. Remaining owners and
        non-`PortError` envelopes stay open.
11. [x] Finish typed lifecycle and mounted storefront checkout source cutover: REST,
        GraphQL, native, root completion, payment owner/provider/webhook policy, order
        recovery/settlement/compensation, typed fulfillment recovery, and cart
        atomic/finalization/compensation all use staged or canonical typed paths.
        Compatibility source removal and execution evidence remain separate open tasks.
12. [x] Cut complete order projection consumers over to `CommerceOrderReadRuntime`:
        admin REST list/detail, mounted GraphQL list/detail with request context, and
        storefront HTTP detail/shared ownership use the typed owner port.
13. [x] Publish return/order-change detail/list operations on `OrderReadPort` and cut
        storefront return/order-change lists to the host-selected runtime.
14. [x] Cut GraphQL return/order-change detail/list reads to the scoped runtime with
        validated actor/channel/effective-locale context and compatible error shapes.
15. [x] Cut mounted admin return/order-change detail/list reads to the host-selected
        runtime while preserving `ORDERS_READ`, filters, clamped pagination, totals,
        public envelopes, request context, and existing mutation/payment/fulfillment ownership.
16. [ ] Run compile and mounted parity for the four admin post-order GET routes.
        The superseded GET handlers in `admin/returns.rs` and `admin/changes.rs`
        were already removed from the source; only compile/parity evidence remains.
17. [ ] Run checkout admission, duplicate request, kill-point, restart, and contention evidence.
18. [ ] Run checkpoint and order identity clean/upgraded/down/reapply and contention evidence on all supported databases.
19. [x] Mount authenticated request-scoped listing native composition.
20. [x] Publish listing GraphQL roots and replace the declared-unmounted adapter.
21. [ ] Add payout provider journal, webhook inbox, multi-order settlement orchestration, and
        reconciliation surfaces.
22. [ ] Run static verifiers and fix remaining source drift.
23. [ ] Compile remaining commerce/order/payment/Marketplace packages and server features.
24. [ ] Apply clean/upgraded migrations and targeted regression tests.
25. [ ] Run contention, restart, kill-point, tenant, locale, provenance, outbox, ledger
        transfer, and mounted transport scenarios.
26. [ ] Execute production-like payment and payout provider evidence.
27. [ ] Reassess FBA/FFA promotion strictly from retained evidence.

## Completed source waves retained for history

- [x] Complete durable return-completion admission and operator recovery source.
- [x] Create Marketplace root, seller owner, and seller FFA source.
- [x] Add seller receipts and exact-locale multilingual storage.
- [x] Create listing owner with terms, receipts, eligibility, and opt-in composition.
- [x] Add complete listing lifecycle events and remove direct write bypasses.
- [x] Backfill truthful legacy listing snapshots and remove mutable note columns.
- [x] Publish the initial module-owned listing FFA source package.
- [x] Define and atomically publish the sealed listing transactional outbox events.
- [x] Add listing permissions and workspace/admin feature registration.
- [x] Add immutable seller lifecycle/moderation event storage and bounded timeline reads.
- [x] Route onboarding review, suspension, and reactivation through atomic seller
      state + event + receipt completion.
- [x] Add typed marketplace allocation, commission, post-capture ledger, reversal recovery,
      adaptation-failure recovery, seller balance projections, and bucket-transfer primitives.
- [x] Extend seller event production to create/profile/onboarding-submit/member commands.
- [x] Add seller event history to native and GraphQL FFA transports.
- [x] Cut mounted checkout compensation over to typed order/payment owner ports while
      retaining fail-closed manual reconciliation for uncertain external outcomes.
- [x] Cut mounted payment, fulfillment, order settlement, and pipeline recovery over to
      owner ports while preserving canonical provider replay identities.
- [x] Add fail-closed public `PortError` transport sanitization and canonical typed
      lifecycle owner views without changing persisted or transport status strings.
- [x] Cut payment owner execution, order settlement, order/payment compensation, and
      mounted payment execution admission/replay over to typed owner lifecycle status views.
- [x] Mount fulfillment ensure/read recovery through the typed root factory and fail
      closed cancelled or unknown owner lifecycle states.
- [x] Cut order checkout recovery and mounted order projection validation over to
      `OrderStatusKind`.
- [x] Cut atomic cart admission, mounted finalization, and mounted compensation
      release/result validation over to canonical typed lifecycle views.
- [x] Cut payment core/refund/provider/webhook and mounted captured-state recovery over
      to canonical typed lifecycle views without changing persisted status strings.
- [x] Cut mounted REST, GraphQL, native, and root storefront checkout completion over to
      one staged recovery runtime with explicit idempotency and host provider composition.
- [x] Harden pricing and payment collection owner-port errors with stable public
      messages plus correlation-aware internal logging.
- [x] Publish and host-compose `OrderReadPort`, cut complete order projections across
      admin REST, mounted GraphQL, and storefront HTTP, extend the boundary to
      return/order-change projections, and cut storefront, GraphQL, and mounted admin
      post-order reads while preserving mutations, refunds, unmounted admin compatibility
      GET source, and unvalidated status.
- [x] Make marketplace financial integration an explicit optional Commerce capability:
      keep base Commerce marketplace-free, gate financial migrations/transports/listeners/
      workers, compose owner modules only through explicit capability features, and fail
      closed before capture when marketplace lines reach a base-only checkout.
- [x] Publish and host-compose Product catalog command ports, then cut mounted admin
      REST product create/update away from `CatalogService` with payload-bound write
      identity and stable public error mapping.
- [x] Cut mounted admin REST Product detail over to the host-selected Product read port
      while preserving locale fallback, request context, permissions, and public envelope.
- [x] Cut mounted admin REST Product list over to the optional host-selected Product
      read-port capability while preserving legacy filters, ordering, pagination, locale
      fallback, empty-title/default-shipping envelope, and source compatibility for existing
      Product read-port adapters.
- [x] Cut mounted admin GraphQL Product catalog reads to the host-selected Product read
      runtime/port with authenticated actor, request channel/locale context, bounded deadline,
      existing filter/pagination semantics, unchanged response projection, and safe stable
      public owner errors.
- [x] Publish the optional filtered storefront Product read capability and cut mounted
      storefront GraphQL catalog reads to the host-selected Product runtime/port without
      changing the existing published-list request, filter/pagination/channel semantics, or
      GraphQL projection.
- [x] Publish the optional Product schema-directory read capability on the host-selected
      Product read runtime and cut `productAttributes`, `catalogCategories`, and
      `productAttributeSchemas` away from direct `ProductCatalogSchemaService` construction.
- [x] Cut mounted `productEffectiveForm` to the host-selected Product schema read port,
      preserving tenant/permission/actor/channel/locale/deadline context and moving aggregate
      reconstruction plus invariant handling back behind the Product owner boundary.
- [x] Cut mounted `productAttributeValues` to the host-selected Product schema read port,
      preserving tenant/permission/actor/channel/locale/deadline context, the existing GraphQL
      projection, and shared stable Product public error mapping.
- [x] Cut mounted `storefrontCatalogSearchOptions` to existing Product schema-directory
      owner capabilities while preserving storefront admission, current tenant, required
      locale, request channel/deadline context, exact option projection, and stable public
      owner errors without adding a redundant Product port method.
- [x] Publish the optional legacy admin Product list compatibility capability and cut
      mounted legacy admin GraphQL `product` / `products` to the host-selected Product read
      runtime while preserving legacy detail/list admission, locale, filtering, ordering,
      pagination, title/shipping fallback, tags, telemetry, projection, and stable errors.
- [x] Cut mounted legacy storefront Product detail/list reads to host-selected Product
      capabilities with lifecycle/channel/inventory/localization and legacy list parity.
- [x] Make the Order post-order command boundary durable and replay-safe for change/return mutations, with caller-owned REST `Idempotency-Key` admission and typed conflict/corrupt-receipt failures; runtime and database evidence remains open.
- [x] Cut mounted REST Product delete/publish/unpublish lifecycle commands to the
      host-composed Product command runtime with required caller idempotency identity.
- [x] Cut mounted GraphQL Product create/update/publish/delete execution to the
      host-selected Product command runtime, preserve lifecycle public-error identities,
      complete Product Admin retry identity retention, and require non-null explicit caller
      idempotency with no compatibility-generated lifecycle identity.

## Audit 2026-09-25: Product owner-port diagnostic hardening

- [x] Remove raw Product owner-port error, tenant, variant, and context values from
      `rustok-product/src/ports/`; owner failures now retain bounded error-shape facts and
      correlation-safe context facts while exposing stable public `PortError` messages.
- [x] Account for every current `CommerceError` variant in the Product owner-port mapper,
      including an explicit `DuplicateSku` conflict envelope instead of an accidental fallback.
- [x] Extend `verify-ecommerce-public-port-error-safety-v2.mjs` with Product owner-port
      diagnostic guards and require the bounded context/error fact contract.
- [ ] Continue the broader correlation-safe mapper cleanup for remaining payment execution/
      compensation, fulfillment, inventory, customer, tax, promotion, remaining ecommerce
      adapters, and non-`PortError` public envelopes; compiler/runtime/test evidence remains
      maintainer-owned.

## Audit 2026-09-25: storefront auxiliary HTTP error safety

- [x] Remove raw `PortError`, tenant/cart identity, actor, channel, locale, deadline, and
      internal-code logging from the mounted storefront auxiliary controller path used by
      `/store/regions` and `/store/shipping-options`.
- [x] Remove the generic `Debug`-bounded storefront auxiliary public-error helper; its
      callers now emit stable status/error-kind facts without serializing owner error values.
- [x] Harden the legacy Product HTTP mapper in the same controller file so no unmounted
      compatibility branch retains a raw owner error diagnostic that could regress into a
      mounted path.
- [x] Extend `verify-ecommerce-public-port-error-safety-v2.mjs` to guard the storefront
      auxiliary controller and require its bounded diagnostic helpers.
- [ ] Continue the broader non-`PortError` public-envelope and mounted owner-boundary audit
      across remaining ecommerce controllers and transports.

## Audit 2026-09-25: mounted storefront Cart and Order error safety

- [x] Harden the mounted storefront Cart HTTP port mapper so raw `PortError`, tenant/cart
      identifiers, and internal public-code values are not serialized into diagnostics.
- [x] Harden the mounted storefront Order/Customer/Payment HTTP mappers so raw owner errors,
      request context, customer/order identifiers, and internal error messages are not logged.
- [x] Keep transport behavior stable while retaining bounded owner kind/code-length, identity
      presence, retryability, correlation, and HTTP status facts needed for diagnosis.
- [x] Extend `verify-ecommerce-public-port-error-safety-v2.mjs` to guard the mounted
      storefront Cart and Order controllers against regression.
- [ ] Continue the mounted non-`PortError` public-envelope audit and owner-port cleanup across
      remaining ecommerce controllers/transports.

## Audit correction 2026-09-25: active Admin Order command path

- [x] Confirm the mounted Admin Order controller is `controllers/admin/orders_owner_ports.rs`;
      the legacy `controllers/admin/orders.rs` had never been mounted.
- [x] Delete the unmounted legacy controller instead of keeping a second editable copy of the same
      routes; the read, mutation, and route-error-context verifiers now pin
      `controllers/admin/orders_owner_ports.rs` and fail if the duplicate is reintroduced.
- [x] Revert the accidental edits previously made to that unmounted legacy controller; no
      legacy command migration is claimed as completed.
- [x] Confirm the active controller already routes mark-paid, ship, deliver, and cancel through
      `OrderAdminCommandPort` with typed owner requests.
- [x] Fix the active command context so reads do not invent idempotency keys and writes require
      the caller-owned `Idempotency-Key` header and propagate it unchanged to the owner port.
- [x] Align the Admin Order/fulfillment and order-detail verifiers with the mounted
      `orders_owner_ports.rs` source.
- [ ] Continue the same active-path audit for remaining mounted Commerce controllers.

## Audit 2026-09-25: storefront line-item resolution error safety

- [x] Remove raw Product and Pricing `PortError` diagnostics from the mounted line-item
      resolution path; keep bounded owner kind/code-length, retryability, tenant/channel,
      correlation, and resource-shape facts.
- [x] Remove raw Inventory `CommerceError`, variant/product identifiers, channel values,
      and locale values from the line-item HTTP error boundary.
- [x] Extend `verify-ecommerce-public-port-error-safety-v2.mjs` with line-item resolution
      guards for the formerly raw Product/Pricing/Inventory diagnostics.
- [ ] Continue auditing mounted Commerce helper boundaries and provider adapters for raw
      owner error/context serialization.

## Audit 2026-09-25: Admin checkout-operation diagnostic boundary

- [x] Remove the redacted-`Debug` error wrapper from the mounted Admin checkout-operation
      HTTP mapper; the helper now accepts only the already-classified policy and emits bounded
      identity-state facts.
- [x] Rename diagnostic fields to explicit `*_state`/`*_length` semantics so logs cannot
      be mistaken for serialized tenant, actor, order, payment, or reservation identifiers.
- [x] Extend the ecommerce public-port safety verifier to forbid the previous raw/redacted
      diagnostic patterns and require the bounded Admin checkout-operation contract.
- [ ] Continue the same no-serialization rule across remaining mounted Commerce transport
      helpers and provider-adapter boundaries.

## Audit 2026-09-25: active Admin Order idempotency and cart lifecycle typing

- [x] Make mounted Admin Order read contexts free of synthetic idempotency keys.
- [x] Require a caller-owned `Idempotency-Key` header for mark-paid, ship, deliver, and
      cancel, validate its size, and propagate the exact key through `OrderAdminCommandPort`.
- [x] Remove raw owner error-code values from active Admin Order HTTP diagnostics; retain
      only bounded code-length, error-kind, retryability, identity shape, and status facts.
- [x] Replace the active storefront payment-collection guard's raw `cart.status == "completed"`
      comparison with `CartResponse::lifecycle_status()` and `CartStatus::Completed`.
- [x] Fail closed on unknown cart lifecycle values instead of treating them as a valid
      non-completed state.
- [x] Align Admin Order and order-detail verification scripts with the actually mounted
      `orders_owner_ports.rs` controller.
- [ ] Continue the same caller-owned idempotency and typed-lifecycle audit in remaining
      mounted Commerce write boundaries.

## Audit 2026-09-25: caller-owned Payment and Fulfillment idempotency

- [x] Require and propagate caller-owned `Idempotency-Key` headers for all mounted Admin
      Fulfillment writes: create, ship, deliver, reopen, reship, and cancel.
- [x] Remove payload-hash and resource/operation-derived synthetic Fulfillment idempotency
      keys; retry identity is now supplied explicitly by the caller.
- [x] Require and propagate caller-owned `Idempotency-Key` headers for mounted Admin Payment
      collection transitions (authorize/capture/cancel) and refund transitions (complete/cancel).
- [x] Keep existing caller-owned refund-creation idempotency and normalize Payment HTTP
      diagnostics to bounded owner-code length rather than internal code serialization.
- [x] Extend the ecommerce public-port verifier to reject generated Payment/Fulfillment keys
      and require caller-owned replay propagation.
- [ ] Continue the same idempotency review for any remaining mounted Commerce write boundary
      that crosses an owner port or provider.

## Audit 2026-09-25: storefront return idempotency contract

- [x] Require a caller-owned `Idempotency-Key` for storefront order-return creation and
      propagate the exact key to `OrderPostOrderCommandPort`.
- [x] Expose the required write-operation header in the affected Admin Order, Admin
      Fulfillment, Admin Payment, and storefront return OpenAPI contracts.
- [x] Keep read contexts free of invented idempotency keys so reads remain observational
      and cannot accidentally claim a write replay identity.
- [ ] Continue auditing remaining mounted write endpoints for explicit caller-owned replay
      identity and consistent OpenAPI contracts.

## Change rules

1. Update this file with every completed or newly discovered ecommerce task.
2. Keep owner plans, registries, manifests, guards, and this plan aligned.
3. Owner modules retain policy, persistence, receipts/events, and commands.
4. Family roots and hosts only compose typed ports and FFA packages.
5. Do not invent legacy actor, locale, provider, or financial facts during migration.
6. Update `docs/modules/registry.md` only when an FFA/FBA status changes.
7. Marketplace names must preserve `rustok-marketplace-*` / `marketplace_*` identity.
8. Do not restore a `[x]` for an audited invariant until every production path and the
   required retained evidence satisfy it.

## References

- [Commerce documentation](./README.md)
- [Commerce FBA registry](../contracts/commerce-fba-registry.json)
- [Public ecommerce port error safety](./public-port-error-safety.md)
- [Order read port](../../rustok-order/docs/order-read-port.md)
- [Order checkout compensation contract](../../rustok-order/contracts/order-checkout-compensation-v1.json)
- [Order checkout payment settlement contract](../../rustok-order/contracts/order-checkout-payment-settlement-v1.json)
- [Payment FBA registry](../../rustok-payment/contracts/payment-fba-registry.json)
- [Payment checkout compensation contract](../../rustok-payment/contracts/payment-checkout-compensation-v1.json)
- [Payment checkout execution contract](../../rustok-payment/contracts/payment-checkout-execution-v1.json)
- [Fulfillment checkout execution contract](../../rustok-fulfillment/contracts/fulfillment-checkout-execution-v1.json)
- [Marketplace root plan](../../rustok-marketplace/docs/implementation-plan.md)
- [Marketplace root FBA registry](../../rustok-marketplace/contracts/marketplace-fba-registry.json)
- [Marketplace ledger FBA registry](../../rustok-marketplace/contracts/marketplace-fba-registry.json)
- [Seller balance transfer contract](../../rustok-marketplace-balance/contracts/seller-balance-transfer-v1.json)
- [Marketplace seller plan](../../rustok-marketplace-seller/docs/implementation-plan.md)
- [Marketplace listing plan](../../rustok-marketplace-listing/docs/implementation-plan.md)

## 2026-10-18 diagnostic guard and authenticated-scope continuation

The three outstanding Product HTTP / GraphQL helper source guards now follow current
owner boundaries instead of removed Commerce umbrella variants/direct command calls.
Typed shared Product reads and mounted Product read/command ports have separate bounded
mapper assertions. Mapper-body extraction no longer accidentally includes idempotency
hashing and treats its formatting as an error-payload leak. Archive and current request/
header delegation are included. Product error-policy checks target rustok-product errors;
Order helper checks target PortError rather than the owner's internal error enum.

Source review also fixed actual production defects rather than only changing guards:

- Product shared/mounted handlers check tenant/auth/request principal agreement before IO
  and receipt admission; command/delegated routes retain permissions and retry identity.
  Media maintenance delegates to that same scope rule, preserving its existing public code.
- Mounted Product read contexts preserve upstream correlation IDs. Storefront Order
  context carries request correlation/locale/channel and permission claims, with a fresh
  opaque correlation only when no RequestContext exists, not an order-UUID-derived value.
- Storefront Order helper rejects mismatched authenticated dimensions before customer and
  owner reads; Forbidden is distinct from validation at the GraphQL public boundary.
  Missing trusted tenant context returns a static unavailable envelope, not framework detail.
- Shipping-option profile validation returns static public policy text. Owner validation
  detail, raw error Debug/UUID/slug fields were removed from that helper's mapper log.
  This does not affect Product's receipt-aware Shipping Profile admission.

Executed: all three corrected standalone source guards, Product shipping admission,
catalog schema/receipts and Media retention guards pass. Four changed Rust sources
pass WASM formatting/idempotence checks; this is not a Rust build or cargo fmt check.
Four added Rust unit scenario sources cover mixed authenticated scope, sanitized HTTP
owner error envelopes, static Shipping validation messages and distinct Order Forbidden.
They are unrun. No test suite was executed per maintainer instruction; cargo/rustc,
mounted extractors, real owner/database/transport execution and CI remain unverified.
Diagnostic evidence metadata records source execution separately from runtime proof.
Broader ecommerce mapper cleanup, authorization revocation fencing and pending-request
recovery are not claimed complete. Finish correlation-safe mapper cleanup remains a
separate tracked gate.

## Return-restock identity source evidence

Date: 2026-10-31. Ordinary bounded Product step-8 continuation on
`arena/71cd6d1f-rustok`, base `16cb22206ca19de8218f47a1056d946105c7b09f`;
observed main `920c0e3f5c4bfb6698caedfbc683684f4c51fa0c`. This is not an audit
cursor, commit/integration, module-readiness promotion or full step-8 handoff.

### Implemented source contract

- [x] Add a mandatory bounded saved-SKU/quantity batch on the existing selected
      `OrderReadPort`, backed by one tenant/Order-joined, price-free owner query. No
      whole priced Order, Catalog/Pricing refresh, provider switch or embedded fallback.
- [x] Validate operator and complete owner-response scope/sets/quantity before
      resolution effects; freeze parent/item facts through owner completion. Only explicit
      SKU-less Order bindings plan no stock. Native/REST/GraphQL share the existing service.
- [x] Inventory validates canonical context and 1..=1024 distinct items of one return;
      proves all Variants before tenant-less item/reservation reads, takes sorted item/level
      locks, refuses missing/ambiguous allocation/levels and terminal history, and proves
      saved locations' tenants without fresh publication/channel visibility. Fresh writes lock
      unarchived receiving locations in sorted UUID order before levels/receipts; archived receipt
      replay is read-only and does not require an active location.
- [x] Preserve the existing `inventory_return_restocks` receipt and applied schema.
      Exact replay correlates tenant/return/item/Order-line/item/quantity and saved location,
      without current reservation selection or timestamp writes. Global PK conflicts cannot
      adopt another tenant's receipt. All new receipts and checked stock additions share
      one Inventory transaction; no reservation/counter/metadata repair or relocation occurs.
- [x] Enforce positive bounded budgets; fully correlate Inventory outcome before the
      separate Commerce checkpoint. Recover only with original operator intent/return-item
      identities. Nonretryable owner advice parks reconciliation, not no-effect inference,
      automatic compensation, key rotation or success. Journal notes are bounded projections;
      the caller retains the original typed cause/advice.

The [accepted amendment](../../../../DECISIONS/2026-10-21-shipment-stock-port-and-return-restock.md#return-restock-identity-amendment-2026-10-31)
and [source evidence](../contracts/evidence/return-restock-identity-source.json) own the detailed
limits. Exact historical replay still requires retained tenant/Variant item and saved-location
proof; missing identity is not backfilled. Operator physical inspection authorizes restock;
this slice does not certify historical shipped/restored unit arithmetic or lifecycle overlap.
No new receipt, migration, ledger, stock event publisher or public client DTO exists.

### Archived receiving-location correction (2026-10-31)

The initial source handoff declared 32 Rust/19 MJS definitions, all unrun. Targeted review
found a real source/storage mismatch: fresh stock was allowed at an archived location, while
`m20260713_000021_bound_reservation_metadata_and_inactive_locations` rejects nonzero quantities
there on both PostgreSQL and SQLite. Such a write would be rolled back and mapped to generic
retryable storage unavailability, not an actionable history refusal.

The correction keeps that potentially applied migration unchanged. Native Inventory now locks
all distinct fresh receiving locations in UUID order after tenant/Variant item proof and before
levels/receipts. It uses the canonical location `is_active()` rule and refuses archived writes
with nonretryable `inventory.return_restock_location_archived`; Commerce retains that bounded
code and parks the original operation for reconciliation. A replay location cache is not fresh
write proof. Hidden but unarchived saved locations still need no fresh channel/publication
eligibility. Archived exact replay does not activate a location or modify stock/receipts.

One additional Rust regression source installs the real storage guard and covers hidden active
movement, archived exact replay, fresh/mixed-batch refusal and unchanged stock/receipt/location
metadata. Four additional MJS mutation definitions retain receiving preflight, active-state
refusal, exclusive lock and deterministic distinct lock order. They are all unrun. There is no
new warehouse, migration, receipt or operator reactivation workflow; explicit owner reconciliation
of a closed saved location remains required, never relocation, key rotation or compensation.

SeaORM 2.0.2 executor source was also inspected: `exec_without_returning` returns the driver's
`rows_affected()` directly. This supports the source-level 0/1 fence branch; PostgreSQL/SQLite
outcomes, lock/archival races, fixture execution and all tests remain unverified. Shipment/cancel/
reopen remains the next separate tail rather than being advanced over this correction.

### Source checks and maintainer evidence

Added **33 Rust and 23 MJS regression definitions**, all unrun. Independent source inventory
confirms the counts and no removed Rust regression names. The 23 MJS definitions contain
one positive control, 21 source-mutation cases and one evidence-mutation case. Static anchor
inspection found all 21 source anchors; it did not apply mutations or execute those tests.

Executed scoped checks (not tests, Rust compilation or provider/SQL execution):

- All 15 focused read-only guards pass, including return identity, Order read/diagnostics,
  selected storefront/operator composition, return-decision/error/refund binding, checkout
  adoption, Inventory context, FBA registry/evidence and constructor-arity guards. The initial
  three guard failures were reviewed against actual selected-owner composition and refreshed
  current source/evidence projections, without changing runtime consumers to satisfy guards.
- Nine Rust files pass Tree-sitter syntax and Scalar rust-fmt 0.3.0 edition-2024 formatting/
  second-pass idempotence. Six MJS files pass Node syntax and Prettier 3.7.4; four JSON files
  parse. None of these tools resolves Rust types, imports, traits, lifetimes or features.
- Targeted source shape checks cover 39 DTO constructions and all 22 native restock calls,
  including calls inside macro token trees; the native command receives explicit context.
  Repository caller/adapter searches found only the existing native adapter and test double,
  both declaring the mandatory seventh method. This is not macro expansion or type proof.
- All 666 tracked migration files compare byte-for-byte with HEAD, with no new migration.
  The existing return ledger and previous adoption migrations are unchanged. No schema or
  data operation ran. Local links/new anchors and `git diff --check` are checked separately
  and recorded in the source evidence.

Maintainer execution must verify SeaORM affected-row/no-insert behavior, PostgreSQL/SQLite
locks and rollback, fixture/FK/archive assumptions, contention and crash/recovery behavior;
source checks do not establish these properties.

Maintainer commands (not executed here):

```bash
cargo test -p rustok-order --lib inventory_binding_tests
cargo test -p rustok-inventory --lib return_restock::tests
cargo test -p rustok-commerce --lib return_restock::tests
cargo test -p rustok-commerce --lib return_completion_orchestration::tests
cargo test -p rustok-commerce --test order_inventory_reservation_test
npm run test:verify:commerce:return-restock-identity
```

- [ ] Compile and execute targeted source regressions; run PostgreSQL/SQLite native
      tenant/collision/checked-overflow/rollback/contention and replay-after-interruption evidence.
- [ ] Verify mounted REST/GraphQL/native behavior and selected/remote capability failures.
- [x] Implement the bounded shipment/cancel/reopen source tail through selected saved Order
      bindings and tenant/Variant/original-allocation proof; see the separate source handoff below.
      Historical cancelled reopening still requires retained native proof, never guessed history.
- [ ] Compile and execute the separate shipment/cancel/reopen regressions and runtime gates.
- [ ] Complete stock event/outbox/Index convergence and historical movement/lifecycle overlap
      policy. No automatic historical backfill or movement ledger is introduced by this slice.

Broader gates were also attempted without bypass flags: the scoped remediation diff invariants
pass, but its architecture phase and the direct architecture guard fail because `cargo` is
absent. The canonical documentation gate exits 1 with 44 errors (ADR registry/metadata,
topology and missing links outside this return slice). No broader gate, package aggregate,
workflow or CI success is claimed. Tests remain with the maintainer.
Step 8 remains in progress; unrelated Pricing/Cart/Inventory-adoption handoffs stay intact.

No-goods decisions do not acquire an Inventory/binding-read dependency or impose stock-batch
quotas on financial-only returns. Parent tenant/Order/return/completed-status scope remains
mandatory; a nonempty stock decision must still freeze and correlate all required item facts.

## Shipment-stock provenance source evidence

Date: 2026-11-01. Status: `source_implemented_runtime_unvalidated`. Ordinary Product step-8
continuation on `arena/71cd6d1f-rustok`, base `16cb22206ca19de8218f47a1056d946105c7b09f`;
observed main `920c0e3f5c4bfb6698caedfbc683684f4c51fa0c` is not refreshed/integrated evidence.
No audit cursor, commit, readiness promotion or full step-8 completion is claimed.

### Implemented source boundary

- [x] F owns lifecycle/counters and the `ShipmentStockPort::prepare` / `PreparedShipmentStock::apply`
      consumer protocol. C composes the selected price-free saved Order binding read before carrier
      or F write transaction. The opaque plan is checked against complete locked F facts; peer I/O
      cannot run inside F's transaction. F remains a leaf without Order/Inventory dependencies.
- [x] I owns native checked physical arithmetic and original allocation/location provenance.
      Prove all Variants' tenants before tenant-less items; lock lines and items deterministically,
      reject missing/ambiguous/terminal allocation, retain consumed-zero audit rows, and never pick
      an oldest global row. Positive budgets cap at 30 seconds and decrease before carrier/apply.
      These phase checks are not database statement timeouts.
- [x] Add Inventory's `m20261101_000025_create_shipment_stock_proofs`. Its private table binds the global
      F-item key to tenant/Order/F/line, saved nullable Variant, original item/reservation/location,
      planned and cumulative shipped/outstanding restored units. Initial key probes roll back with
      no stock/reservation writes; native movement/proof/F progress/provider checkpoint commit on
      F's one transaction. Nonempty down refuses. Migration execution remains unverified.
- [x] Refuse unknown historical movement, zero/deleted/cancelled-state inference, changed saved
      identity, overflow, archived original location or insufficient unreserved restored units.
      No historical backfill, allocation reselection, warehouse relocation, Product/Pricing refresh,
      existing return-item PK change or applied migration payload rewrite is introduced.
- [x] Serialize shipment/return overlap on I's item locks. Retained return-restock facts block
      restoration/reconsumption of that line; outstanding native restored units block fresh return
      restock. Exact return receipt replay stays read-only and precedes the new table dependency.
      This conservative refusal is not unpaid-return policy or partial-inspection top-up support.
- [x] Keep failing Unconfigured and explicit Noop opt-out. Fresh empty typed scope is unknown,
      not SKU-less: native composition refuses; only explicit Noop may admit the empty batch.
      Pending cancel, delivery and undo-delivery do not acquire stock dependencies. Creation and
      complete native projections are bounded to 1024 items; oversized retained work refuses
      without truncation/deletion. Paginated child reads have a checked aggregate SQL probe bound.
- [x] F claims/releases only Pending/uninvoked Executing rows with null result/reference/completion/
      commit markers. One unresolved predicate fences siblings, legacy `provider_error`, malformed
      Pending and incomplete Committed. SQLite acquires a writer fence before claim-local reads.
      Explicit operator no-effect resolution requeues the same Recon/ERROR identity Pending only
      without retained reference/result/commit evidence; success resolution preserves and CASes
      the observed reference. ERROR never automatically authorizes Executing. The worklist excludes
      Executing until explicit stale quarantine and orders by UpdatedAt/Id.
- [x] Register forward-only F `m20261101_000121_enforce_provider_effect_admission`, after
      `m20261003_000120_create_checkout_identity_anchor`, without historical row/payload rewrite.
      Its PostgreSQL/SQLite guards match runtime admission/recovery; raw fixtures install it last.
      Both new I/F migrations remain unexecuted and require a drained, coordinated writer rollout.
      Local recovery validates current read policy/budget before journal access, retains the literal
      saved key before write admission, strips private request descriptors and never calls a carrier.
      Recovery/native wrappers deduct journal/receipt-read time before preparation rather than
      refreshing the current relative deadline.
      Committed recovery needs no O/I; kernel deliver/reopen receipts bind the request and exact
      response on their local transaction. Plain domain methods do not provide those kernel receipts.
- [x] C preserves selected F precedence, refuses one-sided cancellation capabilities, freezes
      bounded page totals/full child sets, and asks the owner for pending-only conditional cancel.
      HTTP/GraphQL preserve canonical principal, permission/scope claims and request correlation;
      GraphQL lifecycle/create helpers verify Auth/Tenant/target scope. Five lifecycle fields now
      require caller-owned keys of 1..=191 bytes. Distinct later intents need distinct keys; the
      separate GraphQL create identity is unchanged. Recovery never regenerates historical hashes.

The [owning ADR amendment](../../../../DECISIONS/2026-10-21-shipment-stock-port-and-return-restock.md#shipment-identity-and-native-movement-provenance-amendment-2026-11-01)
and [source record](../contracts/evidence/shipment-stock-provenance-source.json) define this
bounded implementation. Plans are per-call in-memory projections, not durable receipts,
authorization, carrier-outcome evidence or cross-owner distributed atomicity.

### Targeted shipment correction (2026-11-01; not an audit)

The requested source recheck corrected terminal Cancelled/Shipped shortcuts that bypassed the
original key/payload, and moved pending-only fresh preconditions behind committed replay.
Retained tenant/Fulfillment/literal-key lookup pins the original provider before mutable shipping
option selection; admission/claim is parent-serialized and refuses bounded cross-provider
ambiguity including Pending/Committed rows. Committed replay rereads the native projection.
Delivered reship freezes/rechecks its movement-free native scope and original budget without O/I.

Late validated provider success is retained in quarantine, never discarded, automatically
promoted or overwritten by contradictory callbacks. The unapplied F121 guard preserves every
nonnull result/reference, and permits explicit identical success resolution without replacing
stored JSON/reference/completion time. Stale-execution quarantine also preserves an already
saved completion timestamp. Lifecycle and create-label callers honor quarantine;
known unavailable registry preflight leaves the original uninvoked intent Pending. Native
create-label `ProviderSucceeded` resumes label persistence/checkpoint with its saved result and
without another carrier call; only Committed is an immediate local-completion replay.

Pending cancellation reopens Pending when actual `shipped_at` and item shipped progress are
absent (the outer ActiveValue wrapper is not evidence). Owner recovery compares literal saved
keys without trimming; the legacy-key regression now includes surrounding whitespace. Native
Inventory checks elapsed budget after awaited locks/reads, before/between writes and on return;
preparation still rolls back on refusal. F carries that budget through root reads, progress
receipt admission, local reship and final stock/receipt commit gates. Lifecycle item reads use
the complete 1024-item bound, including movement-free progress and local recovery.

HTTP helper scope agreement is defensive: the canonical RequestContext extractor already
checks Auth/Tenant/channel consistency; this is not evidence of an upstream HTTP vulnerability.
GraphQL also rejects a contradictory supplied RequestContext, preserving its existing absent-
context embedded fallback. No new actor/role fallback or unrelated audit work is introduced.

Thirteen Rust regression definitions and ten Node mutation definitions were added in this review;
all remain uncompiled/unrun. Source/parse/format checks do not certify database or carrier races.

### Focused regression sources (unrun)

58 focused Rust definitions are declared, uncompiled and unexecuted: 16 physical Commerce cases,
22 leaf F owner cases, 2 selected-recovery cases, 5 selected-pagination cases, 7 I scope/budget
cases, 4 internal F current-schema/outcome cases and 2 HTTP context cases. The evidence selects
these four internal cases explicitly; unrelated pre-existing tests are not counted.
They cover partial ship/deliver/cancel/reopen accounting, protected reservations, missing/ambiguous/
terminal/history/archive/overflow refusals, return overlap/exact replay, global proof collision,
preparation rollback, late local failure and real Manual result recovery without a registry,
header-only/Noop, immutable keys/context, bounded projections and pending-only cancellation.
The companion Node module declares 30 source-mutation cases, all unrun and maintainer-only.
These include current-schema registration, original budgets, explicit no-effect resolution and
retained-reference admission. Marker cases are not database/provider behavior tests.

Maintainer commands, not executed by the agent:

```bash
cargo check -p rustok-inventory
cargo check -p rustok-fulfillment
cargo check -p rustok-commerce
cargo test -p rustok-commerce --test order_inventory_reservation_test shipment_stock_provenance
cargo test -p rustok-fulfillment --test shipment_stock_owner_test
cargo test -p rustok-fulfillment --test provider_operation_journal_test
cargo test -p rustok-fulfillment --lib services::provider_operation_recovery::tests
cargo test -p rustok-commerce --lib services::order_cancellation::selected_fulfillment_tests
cargo test -p rustok-commerce --lib services::fulfillment_reconciliation::tests
cargo test -p rustok-inventory --lib shipment_stock::scope::tests
cargo test -p rustok-commerce --lib controllers::admin::fulfillments::shipment_context_tests
cargo test -p rustok-commerce --test graphql_runtime_parity_test
npm run test:verify:commerce:shipment-stock-provenance
```

Read-only source guard: `npm run verify:commerce:shipment-stock-provenance`.
Post-review checks passed: six pure source guards, syntax/unchanged final formatting for 54
selected Rust files, seven Node syntax checks, focused 58/30 definition/JSON integrity, and
211 relative documentation targets across 16 owner/handoff documents. The three historical
F111/F113/I024 migration payloads remain byte-identical to the recorded source base.
Source guards, Tree-sitter parsing, JSON/Node syntax checks and WASM formatting do not establish
Rust types, SQL semantics or runtime behavior. Cargo/rustc are unavailable in this environment;
tests remain with the maintainer. Do not rerun previous adoption one-shot edit scripts.

### Remaining gates

- [ ] Rust compilation and all targeted regressions; actual migration installation/rollback.
- [ ] PostgreSQL/SQLite locks, collisions, simultaneous callers, aggregate arithmetic, transaction
      upgrade/rollback and restart/lost-response evidence. Preparation does not reserve a future
      carrier window; any later failure must preserve the known/unknown provider outcome honestly.
- [ ] Mounted HTTP/GraphQL/native parity, human/OAuth principal scope, selected/unavailable/remote
      owner adapters, current-context recovery and legacy retained-state reconciliation.
- [ ] Unpaid-return eligibility and incremental partial-inspection top-ups; no new receipt identity
      may be invented from a changed restock amount.
- [ ] Full stock event/outbox/Index invalidation, Order projection and deployment convergence.

Product step 8 remains open. Historical proof is never manufactured to make a gate green.

## Native stock outbox continuation source evidence

Native I now owns canonical `InventoryUpdated` construction/publication alongside its physical
stock/proof/return-receipt effects. Shipment uses the supplied F transaction; new returned
stock uses I's existing transaction. C composition and the frozen return plan are unchanged;
no substitute C/host stock publisher or new SDK capability/schema/migration exists. Publication
failure cannot acknowledge event-less local stock. It does not compensate external carrier,
Order/financial effects or a separate C checkpoint.

The [Inventory API](../../rustok-inventory/CRATE_API.md#native-stock-outbox-2026-11-01-source-only)
and [evidence](../../rustok-inventory/contracts/evidence/native-stock-outbox-source.json)
preserve descriptive global availability, per-physically-changed-level invalidation even
when available counters match, preparation/replay/SKU-less exclusions, tenant ownership,
representability and current-human actor semantics. Canonical unconditional Outbox writes
now reject zero-row insertion; the exact write-once replay contract is not changed.

Nine additional physical SQLite regression definitions are a child of the registered
`order_inventory_reservation_test` target; five pure I and two Outbox definitions complete
the 16 added Rust cases. Ten source-guard mutation definitions are also supplied. **None were
compiled or run.** The earlier 58 Rust/30 Node shipment/recovery packet remains unrun.
Source-only checks do not establish SQL, concurrency, runtime delivery, mounted or remote
certification. Existing Search consumes invalidation and refetches live state. Reservation facts
are now source-wired in the later continuation below; real delivery, generic Index and consumer
durability/restart remain open. Unpaid-return
policy and inspection top-ups remain separate. Product step 8 is not complete.

## Anonymous Inventory quantity prerequisite (2026-11-01)

The next reservation-notification trace found an owner allocation defect: quantity release
selected arbitrary active rows and drained level counters independently. The Inventory owner
now plans only proved anonymous rows, binds reductions to exact original tenant locations,
aggregates all consumers' protected claims and requires exact-one transactional effects. Explicit
Cart/Order/line selectors refuse; checkout continues through the selected identity capability.

See the [owner plan](../../rustok-inventory/docs/implementation-plan.md#anonymous-quantity-reservation-release-2026-11-01-source-only)
and [source evidence](../../rustok-inventory/contracts/evidence/quantity-release-source.json).
There are **27 additional Rust/10 Node definitions, all uncompiled/unrun**; prior adoption,
shipment and physical-stock counts/statuses are unchanged. This is a prerequisite fix, not
reservation notification implementation or Product step-8 completion by itself. The later
reservation-fact source continuation below wires Inventory facts/current context/typed consumers;
canonical Rust digests and actual delivery/consumer
wiring, Search refetch and generic Index/restart/lag proof remain open. Existing v1 snapshots
must not clamp or prohibit legitimate `continue` backorders, and Product-owned revision/ledger
refresh events must not be reused as Inventory facts.

## Inventory reservation fact/delivery continuation (2026-11-01; source only)

After the anonymous prerequisite, Inventory now publishes quantity-free sealed
`inventory.reservation.changed` v1 on fresh positive identity/quantity reserve/release, in the
same counter/receipt transaction. Current `PortContext` reaches owner services from ports and
authenticated native calls. Exact replay/released replay/zero/adoption publish nothing; quantity
commands gain no durable receipt. Signed responses share one bounded checked evaluator, not
per-level saturation. C/host never publishes on Inventory's behalf or invents Product revisions.

Existing Core/Memory module listeners now offer bounded lifecycle-owned awaited sealed delivery.
Both configured profiles reach explicit Search refetch and durable artifact subscription intake;
Iggy accepts the original frame before required local acknowledgement. Local failure retains
that identity for Outbox retry, potentially resending remotely: at-least-once, not distributed
atomicity or persistent remote receive/ACK. Search serializes PostgreSQL Product writes/rebuilds/
deletes before fresh reads; its direct-table projection is not generic Index or sellability.

See the [Inventory owner plan](../../rustok-inventory/docs/implementation-plan.md#reservation-fact-delivery-continuation-2026-11-01-source-only)
and [source packet](../../rustok-inventory/contracts/evidence/reservation-events-source.json).
**34 additional Rust/10 Node definitions are uncompiled/unrun.** Canonical Rust digests,
PostgreSQL/MySQL locks/rollback, real relay/Iggy/cursor/restart/DLQ/Search ordering and mounted/
remote/deployment evidence remain open. Inventory-owned generic Index source/revision/consumer
is still missing; Product refresh is not an alternative. **Product step 8 remains incomplete.**

## Inventory catalog deletion source evidence

The Inventory → generic Index continuation first hardens the real native Product deletion path.
[Inventory owns](../../rustok-inventory/CRATE_API.md#catalog-deletion-safety-2026-11-01-source-only)
positive tenant/item proof, any-reference reservation/shipment/return retention, complete bounded
zero-level planning, original-location/current locks and exact effects. Product owns the full
child graph, publication/default-SKU policy, enclosing transaction and rollback; Commerce only
maps the canonical finite public policies for HTTP/GraphQL. No host SQL, fake absence/tombstone,
stock disposal, history purge or new Inventory event/revision is introduced.

The [source packet](../../rustok-inventory/contracts/evidence/catalog-deletion-source.json)
records 31 new Rust/10 Node definitions, all uncompiled/unrun. Fifteen Commerce persistence
sources are registered under `catalog_service_test`, using full local owner/price/event snapshots,
separate nullable shipment-proof snapshots, four ignored-delete rollback fixtures and immutable
refusal-receipt replay. Two HTTP and two GraphQL sources cover bounded copy and current
correlation; provider retry advice cannot stand in for actual finalized receipt classes.
Only the existing deletion-success event fixture changes initial stock from 10 to 0.

I026 adds three lookup indexes after real Inventory history migrations; it has not run and
requires controlled backend rollout/query-plan evidence. Applied migration bytes and receipt
keys remain unchanged. PostgreSQL READ COMMITTED contention, MySQL current locking reads,
SQLite conflicts/trigger rollback, actual receipt finalization/replay and mounted/native/remote
transports require maintainer execution. Leptos remaining CRUD uses generic GraphQL gateway/
redaction and non-durable retained keys; full actionable recovery parity and a general
retirement workflow are incomplete. No new destructive SKU UI is enabled. Generic Index and
Product step 8 remain open; no runtime/readiness promotion or distributed atomicity is claimed.

## Selected Inventory Index consumer (2026-11-02; source only)

Distribution now authors the actual native Inventory schema/source/sealed handler bridge and
Index owns durable min-version/stale-fenced persistence, original-ID initial enqueue and bounded
ModuleWork replay. Current authenticated service authority is independent of Product/event/
baseline actors; no Product revision or refresh proxy is introduced. Existing Product and remote
consumers remain selected separately. [Actual contract](../../rustok-distribution/docs/inventory-index.md)
and [source packet](../../rustok-inventory/contracts/evidence/source-index-consumer-source.json).

- [ ] Integrate failed-job retry and original-job operator recovery, then cross-client recovery.
- [ ] Compile/run focused definitions; validate current credentials/policy, SQL/concurrency/broker/
      restart/deployment and older-baseline cutover. No historical facts or checkpoint reset inferred.
- [ ] Admit full downstream Product step-8 evidence. Source/worker registration and initial pending
      enqueue are authored only; no test/build/runtime/readiness evidence is claimed here.
