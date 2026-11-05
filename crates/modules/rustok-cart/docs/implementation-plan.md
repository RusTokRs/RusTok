# Implementation plan for `rustok-cart`

Last reviewed: 2026-08-01

## Atomic context Pricing adoption (2026-10-31)

Ordinary bounded continuation on the same assigned branch/base, not audit execution or a
readiness promotion. The previous three buyer context paths committed context before separate
Pricing IO/adoption; checkout also considered new context before recovery, and the snapshot
adapter split lifecycle/context writes.

The downstream Inventory adoption continuation also synchronized the existing Cart FBA declarations
and static matrix: complete context assertions are retained on both context commands, and locked
pricing-observation assertions on all four priced line operations. All fourteen Cart cases remain
`static_locked_runtime_pending`; this is declaration consistency, not new execution evidence.

### Current owner implementation

- One `CartContextUpdateRequest` replaces both former context DTO names without compatibility
  aliases. Actual providers, guarded adapters, checkout and repository-owned fixtures consume it.
- Owner-normalized hypothetical target and original pre-projection observation remain separate.
  `Reprice` requires exact target scope and complete original line-ID/quantity coverage; the
  shared batch adopter validates all coverage/duplicates/unknown rows/quantities/money before
  line writes. Context, pricing adjustments, marketplace projection and existing shipping/tax/
  totals commit in the same Cart transaction. Post-commit read failure is still not a receipt.
- `Preserve` is explicit checkout/materialization semantics, not an unavailable-price fallback.
  Nonempty Cart currency/region/channel pair/locale may not change without a complete plan.
  Country is correlated but feeds current tax/shipping: Pricing has no country selector. Empty
  carts have no unit-price basis. Buyer storefront providers require `Reprice` even when empty.
- Both persisted/hypothetical shipping patches use one owner interpreter. Null scalar clears a
  single group, `Some([])` clears all groups, missing vector preserves group state and overlapping
  resolved groups refuse. REST/GQL no longer synthesize an old vector over explicit scalar null.
  Country/null is retained; an explicit region cannot silently accept another region/country.
- Context is active-only. Direct CheckoutService recovers completed/checking-out first and skips
  already-current context after atomic preparation. A frozen atomic adapter refuses context writes;
  snapshot preparation delegates to existing atomic owner prepare. Planless preparation verifies
  its effective preserved unit-price basis and is bounded to 1024 observed lines.
- Commerce batches by expected parent plus per-line quantity, deduplicates variants per group,
  chunks at Pricing's 256 bound and correlates count/order/UUID/parent/context/price presence under
  one inherited price-operation deadline. There is no stored-price, single-read or local-provider
  fallback. All three buyer paths and native payment preparation use this shared command builder.
- Native shipping requires the admitted request/principal/claims and resolves saved Cart Channel
  through its owner, never a page slug or fabricated request. Outer buyer/cart DTOs are unchanged.
- Source guards were migrated to the real shared adopter/atomic prepare; old compatibility quantity
  assertions now fence the live shared workflow. A real compatibility diagnostic Debug that retained
  private typed error text was corrected to a consuming redacted projection.

**20 Rust and 8 MJS new regression functions are source only and unrun.** They cover complete
adoption/refusal/rollback, stale/purpose/lifecycle fencing, frozen recovery, null/group overlap,
selected batch capability and correlation, per-line tiers/dedup/chunking, inherited timeout,
country membership, checked money and diagnostic redaction. Cargo/rustc are unavailable; the
maintainer runs tests. Existing FBA/FFA/runtime evidence statuses are not advanced.

Read-only source gate: `node scripts/verify/verify-cart-context-pricing-atomicity.mjs`.
See [Commerce details](../../rustok-commerce/docs/implementation-plan.md#atomic-cart-context-adoption-2026-10-31)
and the [owner API](../CRATE_API.md#atomic-context-and-pricing-adoption-2026-10-31-source-continuation).
Source formatting/guard/link/call-shape results are recorded separately from compilation and runtime.

No schema/historical migration, storage deletion, new revision/receipt/outbox protocol, price/stock
hold, distributed transaction, automatic retry or mutation-key rotation is added. SQL contention,
remote/unsupported profiles, native/browser/mounted authorization/cookie transitions and downstream
Order/Fulfillment/Inventory/Index convergence remain maintainer/step-8 gates. The prior formal
Cargo/remediation blockers below are not waived by this source continuation.

### Executed scoped context source evidence

- **36 Rust files** pass Tree-sitter syntax parsing and idempotent Scalar rust-fmt 0.3.0 formatting
  (edition/styleEdition 2024). This does not establish compilation/types/lifetimes/features.
- **44/44 scoped read-only source entrypoints** pass after final formatting. The separate canonical
  `verify-rust-constructor-arity.mjs` also passes across **6148 sources**. No test or policy-model
  runner was invoked.
- **6221 Rust sources** were inventoried; **42 matching sources** were parsed for **34 targeted calls
  and 62 constructors**, including five macro-contained constructors, with zero scoped arity/
  required-field errors. The new selected-Pricing probe's eight method signatures match the owner
  trait in source. Both former context DTO names have no executable Rust references. None of this
  resolves Rust names, generics or types.
- Node syntax checks pass for **11 affected MJS files**, including the unexecuted regression
  companion; Prettier 3.7.4 format checks pass for those and two JSON source-evidence artifacts.
  Cart matrix cases retain `execution_status: static_locked_runtime_pending`; the smoke descriptor
  remains `executable_no_compile`, not runtime certification.
- **336/336 local file-link targets in 16 synchronized documents** resolve, as do all 24 new dated
  owner/API/Commerce source-heading references. General historical anchors are not certified.
- `git diff --check` passes. The assigned branch/HEAD/local-main refs remain unchanged; no branch
  switch, commit, integration, schema/history rewrite or stored-data deletion occurred here.

The canonical scoped remediation gate was attempted without bypass flags. It **rejects** quoted
Rust-file inclusion markers in pre-existing JS source checks and an explanatory plan line in the
cumulative diff, rather than new Rust inclusion code. These before-images predate this context
slice; the gate was not edited/disabled to claim a pass. The architecture dependency guard separately
fails because Cargo is absent. Clippy/Cargo checks, SQL/locking/concurrency and mounted/native/remote
runtime behavior are **not verified**. All **20 Rust / 8 MJS** new regression functions remain unrun.

The canonical `verify-docs.mjs` was also run. It fails on broader ADR registry/required-section drift,
stale module topology and missing links outside these synchronized documents (44 top-level reported
errors). No Cart-context link/anchor failure is reported. That repository-wide documentation gate
is not called passed or repaired by editing unrelated owners/audit ledgers in this bounded task.

## Priced quantity adoption (2026-10-30)

Ordinary Product step-8 continuation on `arena/71cd6d1f-rustok`, working base
`16cb22206ca19de8218f47a1056d946105c7b09f`. The observed local main ref is
`920c0e3f5c4bfb6698caedfbc683684f4c51fa0c`; this shallow checkout cannot resolve a merge
base. No branch change/rebase/integration or audit cursor is performed. The working source is
authoritative; existing FFA/FBA/runtime labels are not promoted by this uncompiled slice.

### Root causes and source corrections

- The public `OwnerCartPort` dropped the already-supplied add pricing guard. It now forwards
  it to the locked owner adoption path. The module import path re-exports the same admitted
  provider as the root; the duplicate raw `CartService` storefront implementation is removed.
- `CartPricingContextGuard` binds tenant/Cart, saved currency/region/channel/locale/country,
  customer/guest admission evidence, update witnesses and exact line identity/quantity/money/
  fulfillment inputs. Canonical money validation and checked totals are reused, not a second
  price engine. Priced observation queries admit 1024 lines plus a refusal sentinel; insertion
  at capacity refuses. A timestamp is an extra witness, not the sole material-input check.
- Explicit removal has a target-only purpose/observation and remains available to reduce larger
  or invalid-price historical carts. It cannot authorize priced writes. Decrement-to-remove
  binds the original target quantity rather than silently deleting a raced, larger quantity.
- Bulk updates explicitly carry quoted quantity. Duplicate, unknown/deleted and changed-quantity
  inputs refuse before any line/adjustment/marketplace mutation; no HashMap overwrite, ignored
  UUID or scaling of an old tier quote by a new quantity remains in this adoption path.
- The unquoted storefront quantity operation/DTO and their actual callers/old stock helpers are
  retired. Buyer quantity changes always use selected Pricing/Product plus explicit Inventory
  owner reads. Stock uses checked same-SKU replacement demand; money uses target-line quantity.
  Current Product/fulfillment identity must agree with the retained snapshot; no stored-price,
  Product-global-stock, first-SKU or unavailable-capability fallback is introduced.
- REST, GraphQL and native use one Commerce workflow. Native uses guarded reads/writes and actual
  caller principal/claims/correlation/locale; it does not authorize sanitized guest metadata.
  Native owner errors use one static inspect-before-retry message with bounded typed diagnostics;
  endpoint/input/cart DTO shapes are unchanged. The obsolete native price/error mappers are gone.
- Both Marketplace Cart snapshot mutations now acquire the same Cart row lock before
  reading/inserting/rebinding snapshots; a parallel writer cannot split Cart line money from its
  marketplace projection while a priced adoption is in flight. Read-only snapshot reads stay reads.
- FBA operation sets/source descriptors and guards follow the seven surviving storefront
  operations. Existing statuses/evidence are not upgraded to runtime proof.

New regression sources: **14 Rust functions** and **12 MJS functions** (9 Cart guard and 3 shared public-surface cases), all **unrun**. They cover
state/quantity/money/fulfillment drift, bad/duplicate/foreign/batch inputs, both factory paths,
quoted tier discount adoption with manual adjustments preserved, removal purpose isolation and
oversized-cart cleanup, aggregate replacement demand/overflow and source guard mutations.
The maintainer runs tests; Cargo/rustc, SQL/locking/concurrency, mounted/native/browser and
remote/provider execution remain unverified. No historical migration or stored state is rewritten.

### Executed scoped source evidence

- Scalar rust-fmt 0.3.0 and Tree-sitter parse/format the **29 changed Rust files** with idempotent
  formatting. This cannot establish type/name/lifetime/feature resolution or compilation.
- **29 scoped read-only source entrypoints exit 0** (the earlier Product/Pricing/HTTP guards plus the new
  priced-write/native/Cart-owner checks). Nine additional shared port-surface consumer gates also
  pass: **38 unique entrypoints overall**. No test runner or FBA policy-model simulation is run.
- Source inventory: **6219 Rust files, 27 targeted method/helper calls and
  26 request/update constructors**, including constructors hidden inside `vec!` token trees;
  zero targeted arity/required-field errors. All retired executable unquoted quantity references
  are absent. Same-named UI free functions are explicitly not treated as CartService calls.
- Node syntax checks cover **14 changed MJS files**, including unexecuted regression companions.
  Prettier 3.7.4 checks **15 MJS/JSON files**; four large existing guards retain their original
  layout to avoid unrelated style churn. No test companion is imported/executed by source guards.
- **102/102 local file-link targets in 14 changed documents** resolve. General anchors are not
  validated; the new dated Cart/Commerce/API headings are checked explicitly. `git diff --check`
  passes; branch/base and tracked historical migrations remain unchanged.
- All **14 new Rust / 12 new MJS regression functions remain unrun**. Rust/Cargo, generated schema,
  native/browser/remote providers, SQL/locking/contention/restart and runtime behavior are not
  certified. Existing tax/shipping recalculation remains in Cart's transaction; only the new
  Product/Pricing/Inventory observation path is asserted to precede locking.

### Formal blockers (reported, not bypassed)

- `python scripts/architecture_dependency_guard.py` cannot obtain metadata: Cargo is absent.
  No Rust build/clippy/feature check is presented as passed.
- The family FBA gate initially rejected Inventory's valid inline public facade because it
  required the exact `pub mod ports;` spelling. The existing canonical `requirePortSurface`
  now accepts inline or referenced public modules while still requiring every declared root
  port export, and the family gate reuses it. The family gate and all ten direct helper consumers
  pass in source-only mode; no Inventory implementation was reopened and no runtime status is
  promoted. Three added helper-regression sources remain unrun.
- The scoped remediation gate was attempted with `--crate rustok-cart --files ...` and rejected
  `include!("cart.rs")` / `include!("helpers.rs")` **source-marker strings in an existing MJS guard**
  in the cumulative branch diff. Both markers are present in the pre-task backup; this slice
  introduced no Rust `include!`. The policy was not disabled/changed to claim a pass. The gate
  remains blocked; architecture/Cargo checks would still require a Rust-capable environment.

### Remaining owner gates

- No monotonic whole-Cart revision, durable Cart command receipt, exact replay or automatic
  retry/key rotation is supplied by this fence. Existing per-dispatch native keys do not certify
  replay. A failed response after commit remains an unknown-outcome/recovery concern.
- The 2026-10-31 continuation above closes the context/repricing split at the Cart transaction.
  Pricing/stock are still observations, not reservations or a distributed transaction; selected
  remote execution and live authorization revocation fences still need retained evidence.
- CartService does not publish Cart-mutation outbox/receipt effects here. Downstream Order,
  Fulfillment, stock release/return and event/Index convergence remain Product step-8 work.


## Current state

Tax recalculation consumes `RegionTaxPolicy` through
`RegionService::read_tax_policy_in_tx` in the active Cart transaction. Region
owns the mappings and channel-provider selection. The owner query reads base
policy and country rules in one statement. Cart and Commerce fixtures use
Region migrations and commands. The
[Region storage decision](../../../../DECISIONS/2026-11-04-region-storage-and-cart-tax-policy.md)
records this native boundary and its verification requirements.

`rustok-cart` owns cart state, line items, pricing and tax snapshots, cart
lifecycle, and the persisted storefront context. Its storefront package owns
cart inspection and safe decrement/remove mutations. `rustok-commerce` keeps
cross-domain checkout, channel, and deliverability orchestration; it must not
recover cart presentation or a duplicate cart service.

Native storefront server functions use `HostRuntimeContext` and the shared
transactional event bus. The owner package keeps GraphQL as the selected
fallback path and returns full shipping-option summaries. Inspection is a guarded read, not a
repricing write; decrement uses the canonical quoted quantity workflow and explicit removal uses
its target-only owner fence. Delivery grouping uses canonical
`shipping_profile_slug + seller_id`; legacy `seller_scope` is not a fallback
identity.

The Axum guest-cart capability adapter is owner-owned in
`rustok_cart::guest_access_http`; hosts compose it around REST/GraphQL and native
server-function requests without reimplementing token parsing, cookie emission,
or task-local scope. Native storefront server functions must validate the same
request-scoped guest capability before operating on a guest cart.

The canonical `CartStatus` remains the only cart lifecycle type. Atomic checkout
admission, existing-lock adoption, pricing resolution, and transition-race
recovery use `CartResponse::lifecycle_status()` or `CartStatus::parse`; unknown
persisted values fail closed. Mounted commerce finalization and compensation use
the same owner type for complete/release/replay decisions. Persisted and
transport `status` fields remain strings for backward compatibility.

## Native Region policy verification (2026-11-05)

This scoped continuation started from local `main` at
`f465936127921b0f57b2165afead3456c409a7ff`. It does not certify the full Cart
runtime, remote providers, or client parity.

- `cargo test -p rustok-cart --locked --offline --lib --test cart_service_test`
  passed 38 crate tests and 56 integration tests. These cover country-specific
  rates and inclusion, channel-provider precedence/normalization, discounted
  taxable amounts, foreign/missing Region refusal, and storage-failure rollback
  preserving the previous Cart line and tax snapshot.
- The added `tax::cart_without_region_recalculates_without_region_schema` test
  passed with the same integration target and `-- --exact`. It reverses the
  shipped Region migration chain in its isolated SQLite fixture and verifies
  committed Cart totals and line persistence without that schema.
- The Cart storefront source-boundary verifier and architecture dependency guard
  passed. The full Region reference lint remains open in the
  [Region owner plan](../../rustok-region/docs/implementation-plan.md#remaining-reference-gates).
- The scoped remediation gate passed with `--crate rustok-cart --files <changed paths>`,
  including `cargo clippy --package rustok-cart --all-targets -- -D warnings`.
- Commerce's focused `context_service_test` target passed all five tests after
  correcting its obsolete missing-Region assertion to the owner port's
  `region.not_found` code and adding the same refusal for a foreign Region.
- `cargo test -p rustok-commerce --locked --offline --test pricing_service_test --no-run`
  could not certify the older Pricing fixture consumers. That target imports
  private Pricing entities and has three Product constructors missing `brands`.
  It must migrate to the current owner contracts before its Region fixtures can
  be executed. Region and Cart tests do not establish that target's validity.

Workspace suites, long E2E, mounted client parity, and remote providers were not
run. The Commerce context target compiled with existing unrelated unused and
deprecated Brand API warnings; it does not establish a clean Commerce lint run.

## Accepted fulfillment-requirement cutover

**Implemented on 2026-09-25:** Cart persists the Product-owned digital/physical requirement in its immutable line
snapshot.
Cart shipping economics now read ShippingOption owner projections through an injected
typed port; Cart no longer queries Fulfillment entities directly, and a digital-only
recalculation short-circuits before any shipping-owner call. Delivery groups, shipping selections, and shipping totals cover physical
lines only. Digital-only carts have none of those records; mixed carts keep digital
lines outside every delivery group. Provider absence may reject a physical checkout
operation but must not make digital cart reads or checkout unavailable. The canonical
matrix is in
[`docs/architecture/settings.md`](../../../../docs/architecture/settings.md).

## FFA/FBA boundary

- FFA status: `phase_b_ready`
- FBA status: `boundary_ready` — the in-process cart checkout provider is
  exercised by compiled commerce checkout consumer tests for both successful
  completion and inventory-preflight release paths. Remote/fallback,
  context-update, and recovery execution remain open.
- Structural shape: `core_transport_ui`
- FBA provider contract: `CartCheckoutPort` / `cart.checkout.v2`, covering the
  checkout snapshot plus context update and checkout lifecycle writes.
  in `crates/modules/rustok-cart/contracts/cart-fba-registry.json`.
- Static and no-compile runtime evidence:
  `crates/modules/rustok-cart/contracts/evidence/cart-contract-test-static-matrix.json`
  and `crates/modules/rustok-cart/contracts/evidence/cart-runtime-contract-smoke.json`.
- `scripts/verify/verify-cart-storefront-boundary.mjs` locks the storefront
  core/transport/UI split, native host runtime, GraphQL fallback, and removal
  of the legacy API layer.
- `scripts/verify/verify-ecommerce-typed-lifecycle-statuses.mjs` locks the
  canonical cart type plus atomic checkout, mounted finalization, and mounted
  compensation source cutover.
- Storefront repricing calls the pricing-owned `PricingReadPort` with a
  variant-first request and full resolved-price projection; it no longer calls
  `PricingService::resolve_variant_price` directly.
- Storefront native client error safety: `source_ready_unvalidated` — fetch,
  decrement, and remove preserve validation messages while technical native
  failures are remapped before `UiTransportError` aggregation to one static
  public envelope with correlation-aware, shape-only private diagnostics.
- The compiled commerce checkout channel-inventory regression executes the
  in-process cart checkout provider before product and inventory preflight.
  It is bounded provider-consumer evidence; lifecycle recovery and fallback
  execution remain open.
- Compiled provider-consumer evidence:
  `basic::complete_checkout_builds_order_payment_and_fulfillment_flow` proves
  snapshot/begin/complete lifecycle execution, while
  `validation::complete_checkout_rejects_line_item_without_channel_visible_inventory`
  proves snapshot/begin/release execution. Both run from
  `checkout_service_test` against the in-process provider.
- Storefront payment-collection reads use the owner-managed
  `CartCheckoutPort` factory rather than constructing `CartService` directly.
- Storefront REST and GraphQL cart reads and mutations consume
  `CartStorefrontPort`; the port preserves tenant, actor, channel, locale,
  deadline, and write-idempotency context at the owner boundary.
- Admin cart-promotion preview and application consume `CartPromotionPort`.
  Promotion target validation, tenant parsing, policy rejection, and owner
  mapping retain correlation plus context/request/error shape only. Tenant UUID
  rejection records a boolean parse-failure fact rather than the parser cause,
  while policy and owner events use a closed seven-value error-kind label rather
  than debug formatting. Raw tenant, actor, channel, cart/line-item identities,
  source, amount, metadata, lifecycle, tax, validation, database, parser-cause,
  and complete error values are not written by the guarded boundary. Status:
  `source_ready_unvalidated`.
- No new compile, lifecycle, restart, contention, or remote evidence is claimed
  by the typed source cutover or promotion diagnostic hardening.

## Checkout lifecycle source checklist

- [x] Keep one canonical `CartStatus` in the cart owner.
- [x] Use typed status admission in atomic checkout prepare and lock adoption.
- [x] Fail closed when a cart projection contains an unknown lifecycle value.
- [x] Use typed cart/payment/order lifecycle views in mounted checkout
  finalization.
- [x] Use typed cart and owner compensation snapshot views in mounted checkout
  compensation.
- [x] Guard processed paths against raw cart lifecycle matching.
- [ ] Execute atomic checkout, finalization, release, duplicate request,
  process-exit, restart, and unknown-status evidence.
- [ ] Prove remote/fallback lifecycle parity before any FBA promotion.

## Open results

1. **Keep cart-owned UI within the cart boundary.** Do not add quantity
   increase, add-to-cart, or checkout controls to the package until their
   cross-domain validation and orchestration contract is explicitly owned and
   composed.
   **Depends on:** commerce-owned checkout and validation contracts.
   **Done when:** any new surface consumes an owner-owned public contract with
   no cart business logic or presentation duplicated in the umbrella.

2. **Prove checkout provider fallback and recovery behavior against a live adapter.**
   Turn the locked in-process/remote case matrix into provider execution and
   fallback evidence before considering FBA promotion.
   **Depends on:** a commerce consumer and remote-adapter test environment.
   **Done when:** deadline, typed-error, degraded-mode, and snapshot parity are
   proven for the published `cart.checkout.v2` contract, including write
   idempotency, context update, lifecycle recovery, and degraded behavior.

3. **Prove cart ports through live provider-consumer execution.**
   Commerce production adapters use `CartCheckoutPort`, `CartStorefrontPort`,
   and `CartPromotionPort`; static evidence confirms no direct
   `CartService` construction outside owner-side composition and locks safe
   promotion diagnostics without claiming execution.
   **Depends on:** compiled or live provider-consumer execution.
   **Done when:** transport execution covers checkout, storefront writes,
   promotion preview/application and failure mapping, fallback, recovery, and
   unknown lifecycle behavior.

4. **Document operational changes with checkout changes.** Add diagnostics only
   where runtime pressure identifies a concrete cart or snapshot failure mode,
   and update module and commerce documentation in the same change.
   **Depends on:** observed runtime signals or a changed checkout contract.
   **Done when:** the runbook, metrics, and recovery guidance match the actual
   owner and orchestration boundaries.

## Verification

- `npm run verify:cart:storefront-boundary`
- `node scripts/verify/verify-cart-promotion-port-error-safety.mjs`
- `node scripts/verify/verify-ecommerce-typed-lifecycle-statuses.mjs`
- `npm run verify:ecommerce:fba`
- `cargo xtask module validate cart`
- `cargo xtask module test cart`
- Targeted cart lifecycle, atomic checkout, finalization, compensation,
  repricing, shipping-selection, promotion, and checkout-preflight tests.

No verification command was executed in this source wave.

## Change rules

1. Keep cart persistence and storefront presentation in this module.
2. Update local docs, `rustok-module.toml`, and the umbrella commerce plan with
   a cross-module checkout contract change.
3. Update this status block and `docs/modules/registry.md` only with proven
   FFA/FBA boundary changes.

## Product buyer continuation (2026-10-30)

Buyer continuation: source-only checks; no tests or compilation.

Commerce now owns the Product-to-Cart workflow through selected Product/Pricing reads and the
guarded Cart port. Buyer command DTOs carry stable SKU UUID, positive-i32 quantity, ordinary price
context and exact expected locale, never displayed money or arbitrary price-list authority.
Authorization and immutable context are rechecked under the Cart write lock before distinct-line
insertion. This fulfills the source ownership prerequisite for the new buyer control, not the
remaining checkout/remote-provider execution gates above.

`use_buyer_cart` projects actor-bound visible state and retains known create IDs under their
original opaque tenant/token scope. JSON checkpoints are bounded and reject unknown fields.
Pending warnings precede IO and survive reload; failed persistence refuses writes. Initial URL
adoption is explicit; later actor changes restore their own hints. Next also marks generic Cart
mutations before IO and clears only its own matching marker. Neither implementation infers
principal continuity from token rotation. These are local recovery hints, not durable receipts,
multi-tab locks, exactly-once execution, rollback or proof that an unknown operation was cancelled.

Native Cart GET is now a guarded owner read, matching GraphQL: it does not reprice or mutate Cart
on restore/inspection. Only a successful missing result may clear a valid reference; inactive or
failed references remain refused. Sanitized guest metadata is not re-authorized as a guest secret.
Decrement uses the canonical quoted quantity workflow; explicit removal does not fetch replacement prices.

A common native contract declares endpoints once; SSR-only handlers and mapping are separate.
All five native and GraphQL operations (read, decrement, remove, create, add) use final public-safe
error mapping and bounded diagnostic facts. No raw technical string becomes a public error.

Rust, database/concurrency, guest capability/cookie continuity, mounted auth/tenant transitions,
WASM and remote-port verification remain unproven. New source regressions were added, not run.
