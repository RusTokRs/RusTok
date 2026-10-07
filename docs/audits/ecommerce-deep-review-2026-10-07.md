# E-commerce money path — deep engineering audit

**Date:** 2026-10-07
**Base commit:** `a53a4dd3a3beeb78604321f8c362441fd5d3adac` (branch `arena/407ef6de-rustok`)
**Scope:** `rustok-cart`, `rustok-order`, `rustok-payment`, `rustok-pricing`, `rustok-inventory`,
`rustok-fulfillment`, and the `rustok-commerce` orchestration/transport surface they are composed
into (checkout, payment, refunds, inventory reservation, fulfillment, money events, analytics).
**Method:** source-level tracing of the storefront/admin money paths (HTTP controllers, GraphQL,
service layer, owner ports, provider adapters, migrations), invariant mapping for tenancy,
authorization, transaction/idempotency, currency and event correctness, plus static pattern sweeps
over the seven money crates.

**Environment limitation (explicit):** this checkout has **no Rust toolchain**
(`cargo`/`rustc` are not installed), so no `cargo check`, `cargo clippy`, gatekeeper run, or test
suite was executed by the author of this audit. Everything below is static source evidence.
Findings marked as *verified* were confirmed by reading the complete code path; findings marked as
*needs runtime confirmation* require a compiled run or a database fixture to prove reachability.

---

## Executive conclusion

The **staged checkout pipeline** (`rustok-commerce::services::staged_checkout` +
`checkout_operation` journal + `checkout_stage_pipeline` + `checkout_payment_stages` +
`checkout_inventory_reservation_executor`) is genuinely well engineered for the hard part of this
domain: durable per-operation journal with CAS checkpoints and lease ownership, idempotency keys
derived from stable identities, replay-safe provider calls, payment identity validation against the
order (`collection.amount == order.total_amount`), marketplace economics checkpoints before capture,
and inventory reservation **before** order creation and capture (so no oversell window).

The money **correctness** problems are concentrated elsewhere:

1. **Currency/minor-unit semantics have no canonical owner.** Four different conversion sites
   (`rustok-cart`, `rustok-payment::stripe_provider`, `rustok-order`, `rustok-pricing`) implement
   different tables; the order and pricing event conversions used to degrade silently to `0` and are
   fixed, while the cart marketplace-snapshot conversion still falls back silently
   (`services/cart.rs:233`, `services/cart/helpers.rs:1394` use `unwrap_or_default`/previous value).
   Two of the event values are consumed by the admin revenue dashboard.
2. **The payment module cannot publish domain events at all** (no `rustok-outbox`/`rustok-events`
   dependency, zero `publish_in_tx` calls, no `payment.*`/`refund.*` event variants). Payment and
   refund lifecycle changes are therefore invisible to the rest of the platform except by polling
   the payment tables.
3. **A second, production-dead checkout implementation ships**: legacy `CheckoutService` (1 320
   lines) plus `*_legacy` stage modules wired through `include!()`/`#[path]`. Its compensation
   helpers used to swallow every error (`let _ =`); they now log every failure, but the duplicate
   path itself is still composed and still has weaker invariants than the staged path.
4. **Round-trip money math was hard-coded to 2 decimals** in promotions/pricing, which is wrong for
   3-decimal currencies (BHD/JOD/KWD/OMR/TND) and interacts badly with the zero-total rule. The cart
   promotion sites are fixed; the pricing price-list rule sites remain.
5. **Analytics is duplicated and semantically wrong**: the same "revenue" SQL exists twice
   (module + admin app), counts *placed* (not paid) orders, and sums different currencies into one
   integer without a currency dimension.

The parked-reconciliation state was also made operable and moved out of the database: a checkout that
failed after capture used to block its cart permanently, because the database guards admitted no
transition out of `reconciliation_required` and no admin surface could close the row (ECOM-RECON-01).
The follow-up change set replaced the guard-based state machine with a typed Rust one and put an
action registry in front of the parked state (ECOM-RECON-01, ECOM-DB-01).

Fixes applied in this change set are listed in §"Remediation log"; everything not fixed is recorded
with its precise location, impact, and required owner decision.

---

## Severity summary

| ID | Severity | Area | Finding | Status |
|---|---|---|---|---|
| ECOM-MONEY-01 | **P1 / money** | Order events | `OrderPlaced.total` uses an unconditional ×100 conversion and `unwrap_or(0)`; wrong scale for 0/3-decimal currencies, silent 0 on overflow. | fixed |
| ECOM-MONEY-02 | **P1 / money** | Currency policy | Four divergent currency-exponent implementations with no canonical owner; provider table disagrees with platform table for ISK/VUV/MGA/IQD/LYD (possible 100× amount at the PSP). | partially fixed, ADR required |
| ECOM-MONEY-03 | **P1 / money** | Pricing events | `PriceUpdated.new_amount`/`old_amount` use `decimal_to_cents(...).unwrap_or(0)`; silent 0, wrong scale. | fixed |
| ECOM-MONEY-04 | P2 / money | Promotions, pricing | Money rounding hard-coded to `round_dp(2)` regardless of currency (precision loss for 3-decimal currencies). | fixed (cart), reported (pricing rules) |
| ECOM-EVT-01 | **P1 / architecture** | Payment events | `rustok-payment` publishes no outbox events and has no payment/refund event contract; lifecycle is not observable transactively. | needs owner decision |
| ECOM-EVT-02 | P2 / observability | Checkout events | The checkout journal changed `checkout_operations.status` without publishing any event, so a parked or closed operation was invisible to consumers; the park metric label (`manual`) had drifted from the documented contract label (`manual_reconciliation`). | fixed (typed `checkout.operation.parked` / `checkout.operation.reconciled` family published in the writer's transaction, one shared bounded vocabulary, `scripts/verify/verify-checkout-operation-event-contract.mjs`) |
| ECOM-ADM-01 | P2 / money | Provider execution guard | `payment_provider_operations_checkout_guard` (`m20260713_000016`) blocks provider execution for `compensation_required`, `compensating`, `reconciliation_required`, `compensated` and `failed` — but not for `completed`, so a provider operation may still enter `executing` after the checkout succeeded, the one terminal status in which no new provider execution can be legitimate. | fixed in the Rust admission contract (level `closed` covers `completed`); the trigger is dropped by `m20261007_000013` and the payment claim gate is the only guard left |
| ECOM-ADM-02 | **P1 / money** | Provider execution guard | The same guard blocks *every* transition into `executing`, including the `cancel`/`refund` operations the compensation pipeline itself must run while the operation is `compensating`. A compensation step that needs the provider is refused by the guard and parked as `manual_reconciliation`, which is why the reconciliation queue fills up. | fixed in the Rust admission contract (extending effects refused while the level is not `open`, unwinding effects stay admitted); the trigger is dropped by `m20261007_000013` and the payment claim gate is the only guard left |
| ECOM-DUP-01 | **P1 / architecture** | Checkout duplication | Production-dead legacy `CheckoutService` + `include!`/`#[path]` legacy stage modules ship in the composed build. | needs owner decision |
| ECOM-COMP-01 | **P1 / money** | Compensation | Checkout compensation bookkeeping failures were invisible (`let _ =` on `cancel_*`, `release_cart_checkout`, `mark_reconciliation_required`). Every site now logs a structured error; the trigger error is still propagated. | fixed (all sites) |
| ECOM-COMP-02 | P2 / money | Compensation | Captured-then-failed checkouts are never auto-refunded; compensation returns `manual_reconciliation` and the sweep could retry it forever. | partially fixed (attempt cap + park + metrics/alerts + park/close events in the writer's transaction, ECOM-EVT-02); the automatic refund policy remains an owner decision |
| ECOM-ZERO-01 | P2 / money | Zero-total carts | 100 %-discount carts compute `total_amount = 0`, but `create_collection` rejects `amount <= 0`, so checkout can never complete; `net_total` also silently clamps over-discounts to zero. | needs owner decision |
| ECOM-DUP-02 | P2 / contract | Analytics | `load_order_stats_snapshot` is duplicated verbatim in `apps/admin`; one metric, two owners. | needs owner decision |
| ECOM-ANALYTICS-01 | P2 / correctness | Analytics | "Revenue" counts `order.placed` (not paid), ignores cancellations/refunds, and sums all currencies into one figure. | needs owner decision |
| ECOM-TENANT-01 | P2 / tenancy | Pricing, inventory | Money-domain queries that are not tenant-filtered (relying on a previously validated parent row). | fixed (remediation row 58: `PricingService::get_price`/`get_variant_prices` take `tenant_id` and resolve the variant through the shared `ensure_variant_tenant` loader; the `price`/`inventory_item`/`region_country_tax_policy` reads that lean on a tenant-checked parent carry `// INVARIANT:` notes naming that parent) |
| ECOM-PANIC-01 | P2 / robustness | Money paths | `expect`/`unwrap` on the money path without a documented invariant (`marketplace_financial_runtime`, `checkout_inventory_order_adoption`, `payment`, UI). | partially fixed |
| ECOM-LOG-01 | P2 / hygiene | Cart | `eprintln!("DEBUG MAP ...")` raw debug prints inside money error mapping. | fixed |
| ECOM-REFUND-01 | P2 / correctness | Refunds | Two refund tables (`refund`, `refund_creation`) with two independent services; cross-table reconciliation is unproven. | needs owner decision |
| ECOM-ERR-01 | P3 / hygiene | Money crates | ~165 swallowed `let _ =` / `.ok()` / `unwrap_or*` results and ~131 `unwrap/expect` in the seven money crates (migrations excluded). | partially fixed |
| ECOM-RECON-01 | P2 / operability | Checkout recovery | `reconciliation_required` was terminal in both directions: absent from the Rust journal model, and the DB guards refused every transition out of it, so a parked operation permanently blocked its cart and no admin surface could close it. | fixed (state machine in Rust + action registry + admin list/actions) |
| ECOM-RECON-02 | P2 / parity | Checkout migration | MySQL carries only the parking trigger: no status/transition guard and no `ux_checkout_operations_active_cart` equivalent (MySQL has no partial indexes), so the one-active-checkout-per-cart invariant is unenforced there. | closed — not applicable (MySQL is not deployed; owner decision 2026-10-07) |
| ECOM-DB-01 | P2 / contract | Checkout guards | The checkout state machine, the parking rewrite, the lease/completion shape and cross-row tenant lookups lived in PL/pgSQL/SQLite/MySQL constraint triggers, which `AGENTS.md` explicitly forbids ("state transition validations, workflow guards ... MUST be owned and validated by typed Rust domain entities and services"; "Do NOT implement ... cross-row business validations inside PL/pgSQL constraint triggers"). This split-brain is what produced ECOM-RECON-01. | fixed (guards dropped by `m20261007_000010`, rules moved to `CheckoutOperationJournal`; decision recorded as ADR `DECISIONS/2026-10-07-checkout-operation-invariants-owned-by-rust.md`) |
| ECOM-UI-01 | P3 / robustness | Storefront UI | `cart.unwrap()` in the cart drawer view (currently unreachable, still a panic path). | reported |
| ECOM-VERIFY-01 | P2 / tooling | Verification | The two `scripts/verify` source verifiers for the admin checkout-operation surface fail on the pristine tree (41 and 89 findings): they encode an older/aspirational design, so the diagnostic-safety contract they claim to verify is not actually verified. The digest-admission verifier had the same class of stale marker (an unpinned `actions/upload-artifact@v7` string in a SHA-pinned workflow) and was fixed in row 27; `verify-api-compatibility-contract.mjs` still fails on its own stale marker. | reported (baseline-relative: 40 and 89 after this change set; digest-admission now green) |

---

## P1 findings

### ECOM-MONEY-01 — `OrderPlaced.total` is silently zeroed and scaled by 100

**Evidence**

- `crates/modules/rustok-order/src/services/order.rs:485`
  `total: decimal_to_minor_units(total_amount).unwrap_or(0)`.
- `crates/modules/rustok-order/src/services/order.rs:1158-1160`
  `fn decimal_to_minor_units(amount: Decimal) -> Option<i64> { (amount.round_dp(2) * Decimal::from(100)).to_i64() }`.
- Consumers: `crates/modules/rustok-order/src/analytics.rs` (admin dashboard revenue, see
  ECOM-ANALYTICS-01), `apps/server/src/graphql/queries.rs:1019`, and every event-bus subscriber of
  `order.placed`.

**Impact**

- The event contract carries *minor units*, but the conversion always multiplies by 100 and rounds
  to 2 decimals. For a 0-decimal currency (JPY/ISK/VUV/…) the emitted value is 100× the true minor
  amount; for a 3-decimal currency (KWD/BHD/…) the value is truncated to 2 decimals.
- If the scaled value does not fit `i64`, `unwrap_or(0)` publishes `total: 0` — a silent
  money-correctness failure in a domain event rather than an error.
- The dashboard "revenue" tile is computed straight from this field, so the number an operator sees
  is wrong for any tenant whose catalog is not EUR/USD-like.

**Fix applied:** conversion is now currency-aware and failure is an error instead of a silent zero
(see remediation log). A canonical owner for the currency table is still required (ECOM-MONEY-02).

### ECOM-MONEY-02 — no canonical owner for currency exponents

**Evidence (four implementations of the same rule)**

| Location | Zero-decimal list | Three-decimal list | Scale note |
|---|---|---|---|
| `rustok-cart/src/services/cart/helpers.rs:1362` `currency_exponent` | BIF CLP DJF GNF **ISK** JPY KMF KRW PYG RWF UGX VND VUV XAF XOF XPF | BHD **IQD** JOD KWD **LYD** OMR TND | used for marketplace snapshot minor units |
| `rustok-payment/src/stripe_provider.rs:723` `currency_exponent` | BIF CLP DJF GNF JPY KMF KRW **MGA** PYG RWF UGX VND VUV XAF XOF XPF | BHD JOD KWD OMR TND | **what is actually sent to Stripe** |
| `rustok-order/src/services/order.rs:1158` `decimal_to_minor_units` | — | — | always 2 decimals, ×100 |
| `rustok-pricing/src/services/pricing.rs:1648` `decimal_to_cents` | — | — | always 2 decimals, `round_dp(0)` |

Divergences between the cart table and the Stripe table: **ISK, VUV, MGA, IQD, LYD**. For any of
those currencies the amount that reaches the PSP is off by a factor of 100 (or loses a decimal)
relative to the platform's own internal accounting. Because there is no canonical money type or
module (`rustok-core` has no money helper; `rustok-commerce-foundation` is the declared shared
e-commerce foundation but is not a dependency of `rustok-order`/`rustok-payment`), each crate keeps
its own copy and nothing detects drift.

**Impact:** customer charged 100× (or 1/100×) the intended amount for affected currencies; hard to
notice in tests that use USD-like fixtures.

**Fix applied:** the order and pricing event conversions were made currency-aware using the
platform table (values copied from `rustok-cart`, values documented at the call site). **Not fixed:**
the provider table divergence — a PSP adapter legitimately follows the PSP's own minor-unit
contract, so the correct remediation is an ADR that declares the canonical owner
(recommended: `rustok-commerce-foundation::money` with `currency_exponent`, `to_minor_units`,
`from_minor_units`, `round_to_currency`) and rewires `rustok-cart`, `rustok-order`, `rustok-pricing`
plus a Stripe-specific mapping with an explicit test per currency.

### ECOM-MONEY-03 — `PriceUpdated` amounts are silently zeroed

**Evidence**

- `crates/modules/rustok-pricing/src/services/pricing.rs:950`
  `let new_cents = decimal_to_cents(amount).unwrap_or(0);` then
  `DomainEvent::PriceUpdated { …, new_amount: new_cents }`.
- Same pattern at `pricing.rs:1066` inside `set_prices`.
- `crates/libs/rustok-events/src/types/domain_event.rs:214-220` — `PriceUpdated` has no semantic
  validation for `old_amount`/`new_amount`.

**Impact:** on conversion failure the platform publishes `price.updated` with `new_amount = 0`
instead of failing the transaction; consumers (index/search projections, analytics, any future
price trigger) see a price drop to zero. Combined with the always-×100 scale, 3-decimal currencies
are also truncated ($0.001 granularity lost).

**Fix applied:** conversion is currency-aware and failure propagates as
`CommerceError::Validation` before the event is built.

### ECOM-EVT-01 — payment lifecycle is not observable through the event backbone

**Evidence**

- `crates/modules/rustok-payment/Cargo.toml` has **no** `rustok-outbox` / `rustok-events`
  dependency.
- `grep -rn "publish_in_tx|\.publish(" crates/modules/rustok-payment/src` → **no matches**; no
  `event_bus` field is threaded through `PaymentService`, `PaymentRefundCreationService`,
  `PaymentProviderEventIngressService`, or the admin refund/collection commands.
- `crates/libs/rustok-events/src/types/domain_event.rs` defines `order.placed`,
  `order.status_changed`, `order.completed`, `order.cancelled`, `price.updated`,
  `inventory.updated`, … but **no** `payment.*` or `refund.*` variants.

**Impact**

- Authorized/captured/cancelled payments and completed/cancelled refunds are only visible by
  polling `payment_collections` / `payments` / `refund*` tables. Notification, marketplace ledger,
  fraud, and analytics consumers cannot subscribe transactionally.
- The checkout pipeline works around this by writing its own journal (`checkout_operation`), which is
  correct for checkout, but leaves every *post-checkout* money transition (admin capture/refund,
  provider webhook reconciliation, return-driven refunds) event-less.
- Anything that must react "atomically with the money state change" (e.g. a ledger entry per refund,
  which the marketplace module needs) has no supported mechanism.

**Required remediation:** add `payment.*`/`refund.*` variants to the canonical event catalog, thread
`TransactionalEventBus` into the payment service (constructor + composition roots in
`apps/server`, `rustok-commerce` runtime), and publish with `publish_in_tx` inside the existing
transactions in `authorize_collection`, `capture_collection`, `cancel_collection`,
`complete_refund`, `cancel_refund`, and the creation/replay path of `PaymentRefundCreationService`.
This is an architecture change and needs an ADR + composition-root updates, so it was **not**
implemented here.

---

## P2 findings

### ECOM-EVT-02 — parked and closed checkouts were invisible, and the park label had two spellings

**Evidence**

- `crates/modules/rustok-commerce/src/services/checkout_operation.rs` wrote
  `checkout_operations.status` (including the `reconciliation_required` park and the operator
  close/retry) without touching the transactional outbox; the parking path was discovered only by the
  sweep or by polling the row.
- The metric label and the payload vocabulary were separate string literals: the journal's local
  `PARK_REASON_MANUAL_RECONCILIATION` was `"manual"`, while the documented label in
  `docs/audits/…` and the ADR is `manual_reconciliation`. A dashboard alerting on the documented
  label could never fire.

**Impact**

- Notification, analytics and operator tooling could not subscribe to "a checkout now needs a human"
  or "a human closed a checkout"; every consumer had to poll, and a parked cart sat silently until
  someone opened the admin list.
- The park alerting contract was unverifiable: the metric value and the event value could not be
  proven equal because one of them did not exist.

**Fix applied (this change set, increment 1)**

- New typed family `crates/libs/rustok-events/src/checkout_operation.rs`:
  `checkout.operation.parked` (`operation_id`, `cart_id`, bounded `reason`) and
  `checkout.operation.reconciled` (`operation_id`, `cart_id`, bounded `outcome`, `operator_id`),
  schema version 1, with `ValidateEvent` (non-nil identifiers, bounded labels) and registration in
  `ContractEventPayload`, `event_schema()` and `event_schemas()`.
- The journal publishes the family with
  `TransactionalEventBus::publish_contract_in_tx` **inside** the transaction that writes the status,
  from all four writers: `release_lease_with_error` (manual-reconciliation park),
  `park_exhausted_compensation` (attempt cap), `resolve_reconciliation_required`
  (`compensated` / `failed`) and `request_compensation_retry` (`compensation_required`). The
  transaction is rolled back on a CAS loss, the event write is part of the same unit of work as the
  status write, and the `rustok_checkout_reconciliation_parked_total` sample is recorded only after
  the commit.
- The bounded park vocabulary now has exactly one definition in `rustok-events` and is imported by
  the journal, so the metric label and the event payload label cannot drift apart again; the
  documented `manual_reconciliation` value is the one that ships.
- `TransactionalEventBus` is threaded through every journal construction site (19 call sites across
  the checkout services, the HTTP runtime and two test fixtures), and
  `scripts/verify/verify-checkout-operation-event-contract.mjs` locks the family registration, the
  in-transaction publication order and the single-vocabulary rule; it is wired into the
  `Ecommerce Hardening` workflow.

**Owner follow-up:** the new family changes the canonical event contract, so
`crates/libs/rustok-events/contracts/event-contract-digests.json` must be regenerated with
`cargo run --locked -p rustok-events --example event_contract_digests -- --write` (the digest gate
belongs to the maintainer because it needs the Rust toolchain).

### ECOM-DUP-01 — a second checkout implementation ships and is dead

**Evidence**

- `rustok-commerce/src/services/checkout.rs` — `CheckoutService::complete_checkout` (1 320 lines):
  exported publicly (`src/lib.rs:75`), constructs the order with
  `create_order_with_channel` (no checkout identity), authorizes/captures straight through the
  provider registry, and compensates via `compensate_order` /
  `compensate_payment_and_order`.
- Both real transports use the *staged* path instead:
  `controllers/store/checkout.rs:345` and `graphql/mutations/checkout.rs:287` call
  `storefront_staged_checkout_runtime::complete_storefront_checkout_input…`, which builds
  `StagedCheckoutService` + `RecoveringStagedCheckoutService`.
- The only remaining callers of `CheckoutService::complete_checkout` are
  `tests/checkout_service_test/*` and `tests/graphql_runtime_parity_test/main.rs`.
- Additional legacy mirrors: `checkout_fulfillment_stages.rs:398` uses
  `include!("checkout_fulfillment_stages_legacy.rs")` inside a `mod legacy { … }` block;
  `checkout_payment_stages.rs:403` uses `#[path = "checkout_payment_stages_legacy.rs"] mod legacy;`;
  `services/mod.rs:18` uses `#[path = "checkout_marketplace_financial_hardened.rs"]`;
  `services/mod.rs:22` keeps `checkout_marketplace_financial_legacy`.

**Impact**

- The composed platform ships two contradictory checkout implementations. The legacy one has weaker
  invariants (no persisted checkout identity, amount taken from the cart snapshot rather than the
  order, compensation that was silently swallowing errors before this audit). Any future caller that
  wires `CheckoutService` (it is a public API of the crate) silently gets the weaker semantics.
- `include!()` file-stitching is explicitly listed as a forbidden anti-pattern in
  `docs/CONTINUOUS_CODE_REVIEW.md` §2.
- Test maintenance cost is paid twice; behavioural drift between the two paths is invisible.

**Required remediation:** delete the legacy `CheckoutService` and the `*_legacy` mirrors, and port
the surviving integration assertions in `tests/checkout_service_test/*` onto the staged runtime (or
delete them with an explicit note in the PR). This is a large, intentional removal, so it is
recorded here for owner decision rather than executed in an audit pass.

### ECOM-COMP-01 — legacy compensation swallows every failure

**Evidence** (`crates/modules/rustok-commerce/src/services/checkout.rs`)

- `983-995` `compensate_order`: `let _ = self.order_service.cancel_order(...)`.
- `996-1020` `compensate_payment_and_order`: `let _ = self.payment_service.cancel_collection(...)`
  **and** `let _ = self.order_service.cancel_order(...)`.
- `231`, `249`, `262`, `646`: `let _ = self.release_cart_checkout(...)` — the cart lock is not
  released if the release itself fails, with no log and no error receipt.

**Impact:** when a checkout fails after order creation and the compensating cancel fails (DB error,
invalid transition, boundary timeout), the platform leaves an order/payment/cart inconsistency with
*no signal at all* — the API still returns the original stage error. The staged path is fine here
(`CheckoutCompensationService` propagates and journals every failure), which makes the legacy path
the odd one out.

**Fix applied:** every swallowed call now logs a structured `tracing::error!` with
tenant/order/collection/reason context:

- legacy `checkout.rs`: `release_cart_checkout` (4 sites via `release_cart_checkout_or_log`),
  `compensate_order` (`cancel_order`), `compensate_payment_and_order` (`cancel_collection` +
  `cancel_order`);
- `rustok-payment`: `admin_collection_command.rs` (4), `admin_refund_command.rs` (3),
  `checkout_execution/provider_helpers.rs` (1) — the best-effort `mark_reconciliation_required`
  that follows a provider-outcome-unknown error;
- `rustok-commerce`: `refund_reconciliation.rs` (3), `journaled_payment_provider.rs` (4),
  `journaled_fulfillment_orchestration.rs` (2), `journaled_create_label_provider.rs` (1).

The trigger error is still returned to the caller (logging does not change control flow), and the
call sites that could safely propagate already did. Removing the legacy path (ECOM-DUP-01) remains
the real fix.

### ECOM-COMP-02 — captured checkouts are never auto-refunded

**Evidence**

- `crates/modules/rustok-commerce/src/services/checkout_compensation.rs:194-201`
  `if stage_rank(operation.stage) >= payment_captured → Err(manual_reconciliation("captured checkout
  state requires refund reconciliation"))`; `249-305` `compensate_payment` returns the same for a
  `captured` collection.
- `staged_checkout.rs` compensation trigger: any non-retryable pipeline failure is marked
  `compensation_required` (`pipeline_failure_disposition`, `staged_checkout.rs:366-410`).

**Impact:** the only stage after capture is `fulfillment_created` and `completed`. If the fulfillment
stage fails non-retryably (or the process dies before the checkpoint), the customer's money is
captured, no refund is issued automatically, and the operation is parked in `compensation_required`.

The active compensation path is otherwise sound: `CheckoutCompensationService::compensate`
(`checkout_compensation_owner_ports.rs:162-214`, compiled from `checkout_compensation_error_safe.rs`
via `include!`) claims the operation, journals every failure
(`mark_compensation_retryable` → back to `compensation_required`), and propagates the error. The
problem is the loop around it:

- `claim_compensation` (`checkout_operation.rs:481-537`) increments `attempt_count` but **no code
  path caps it**; `manual_reconciliation` results therefore re-enter the claimable set forever.
- the sweep is only reachable through the admin HTTP endpoint
  (`controllers/admin/checkout_operations.rs:270`); there is no scheduled worker, no outbox event,
  and no operator alert tied to `manual_reconciliation` — the only signal is the sweep report in
  the admin response (`checkout_compensation_sweep.rs:28-34, 105-132`).
- `mark_compensation_retryable` is called even for `ManualReconciliation`, so the operation never
  leaves `compensation_required` for `failed`/`compensated`.

**Required remediation:** decide and encode the policy: either (a) auto-refund captured payments via
`PaymentRefundCreationService` with a durable reconciliation operation and a maximum attempt count,
or (b) keep manual reconciliation but (i) bound the retries, (ii) emit an operator alert/outbox
event, and (iii) document the runbook. Note that (a) requires ECOM-EVT-01 first.

**Fixed in this change set — option (b)(i) and the alert half of (b)(ii):**

- `MAX_CHECKOUT_COMPENSATION_ATTEMPTS` (8, `checkout_operation.rs`) bounds the loop. Before claiming,
  the sweep checks `attempt_count`; at or above the cap it calls
  `CheckoutOperationJournal::park_exhausted_compensation`, a legal
  `compensation_required | compensating (lease expired) -> reconciliation_required` transition that
  clears the lease, sets `completed_at`, and records
  `checkout.compensation_attempts_exhausted`. A stuck cart therefore becomes a visible,
  resolvable reconciliation case instead of being retried forever.
- `/admin/checkout-operations/compensation-sweep` reports the new `exhausted` counter; a failed park
  (concurrent worker won the CAS) is reported with the bounded code
  `checkout.compensation_exhaustion_park_failed` and logged as `error_kind` only.
- Two Prometheus counters now make parking and operator work observable without a tenant label:
  `rustok_checkout_reconciliation_parked_total{reason}` (reasons `manual_reconciliation`,
  `attempts_exhausted`) and `rustok_checkout_reconciliation_actions_total{action,result}` (results
  `succeeded`, `replayed`, `rejected`, `not_found`, `conflict`, `idempotency_conflict`,
  `payment_owner`, `close_after_money_moved`, `operation_conflict`, `storage_error`). Example alert
  rules are in the ADR (`DECISIONS/2026-10-07-checkout-operation-invariants-owned-by-rust.md`,
  §"Operational signals").
- The runbook half of (b)(iii) was already delivered with the operator action endpoints.

Still open: option (a) itself (no automatic refund) and the transactional outbox events for
park/close, which belong to the event contract of ECOM-EVT-01.

### ECOM-ZERO-01 — zero-total carts can never complete checkout

**Evidence**

- `rustok-cart/src/services/cart/helpers.rs:196-202` `net_total`: if `adjustment_total > subtotal`
  it returns `Decimal::ZERO` (silent clamp).
- `rustok-cart/src/services/cart/helpers.rs:1117-1124`: `total_amount = adjusted_total + shipping_total (+ tax)`.
- `rustok-payment/src/services/payment.rs:65-69` `create_collection` rejects `amount <= Decimal::ZERO`.
- Staged checkout requires a payment collection before capture
  (`checkout_payment_stages_legacy.rs:122-200` prepare → authorize → capture) and asserts
  `captured_amount == order.total_amount`.

**Impact:** a cart fully paid by promotions/shipping discounts (a normal marketing scenario:
"100 % welcome code", "free gift on first order") has `total_amount = 0`, which cannot create a
payment collection, so `complete_checkout` fails with a validation error and the cart stays
`checking_out` until the lock is released. `net_total`'s silent clamp also hides
over-discounting from operators (the raw `adjustment_total > subtotal` condition is rejected in
`set_adjustments`, but `net_total` is the last line of defence and reports success).

**Required remediation:** a zero-amount checkout branch that skips payment creation/authorization and
records the order as paid-by-discount (with a settlement receipt), plus turning the `net_total` clamp
into an explicit error at the call site that can produce it.

### ECOM-DUP-02 / ECOM-ANALYTICS-01 — duplicated and semantically wrong revenue metric

**Evidence**

- Canonical: `crates/modules/rustok-order/src/analytics.rs:22-95` (`load_order_stats_snapshot`).
- Byte-identical copy: `apps/admin/src/features/dashboard/transport/native_server_adapter.rs:228-315`
  (the admin SSR dashboard does **not** call the module function).
- The SQL aggregates `SUM(payload->event->data->total)` over `sys_events WHERE event_type =
  'order.placed'` — i.e. it counts *placed* orders (including orders that are never paid, or are
  cancelled/refunded later), and it has no `currency` grouping, so a tenant selling in USD and EUR
  gets a single meaningless integer.

**Impact:** the operator dashboard revenue figure is wrong for every tenant that (a) has unpaid or
cancelled orders or (b) does not sell in exactly one currency; and the two copies will drift the
first time one of them is fixed.

**Required remediation:** keep one implementation (module-owned), move it behind the module's read
port so the admin app stops duplicating SQL, define the metric (paid orders, refunds subtracted,
grouped by currency), and add a test that asserts a cancelled order does not contribute.

### ECOM-TENANT-01 — money-domain queries without an explicit tenant filter

Scanner output (`scripts/maintenance/scan_codebase.py --target crates/modules/rustok-cart`) reports
36 `SEC-TENANT-01` hits in `rustok-cart`; most are **false positives**: the queries filter by
`cart_id`/`line_item_id` after a tenant-scoped cart load
(`load_cart_in_tx`/`load_cart_for_update_in_tx` at `helpers.rs:660-714` both filter
`CartId … TenantId`). Verified-good examples: `cart.rs:346`, `helpers.rs:1075`, `promotions.rs:44`.

Genuine gaps found by manual tracing (the parent row is tenant-validated first, so they are
defence-in-depth, not exploitable by themselves):

- `rustok-pricing/src/services/pricing.rs:1147-1151` — `price` rows are queried by
  `variant_id + currency_code` with **no tenant filter** (the variant is tenant-checked at 1141-1145).
- `rustok-inventory/src/services/inventory.rs:427-431` and `440-455` — `inventory_item` by
  `variant_id` with no tenant filter (variant is tenant-checked).
- `rustok-cart/src/services/cart/helpers.rs:955` — `region_country_tax_policy` by `region_id` only.

**Impact:** low by itself (UUIDs are tenant-unique in practice), but it breaks the AGENTS.md §7
"every query touching domain data is tenant-scoped" invariant, which is a mechanical rule the
project uses to prevent exactly this class of mistake from becoming exploitable later.

**Required remediation:** canonical tenant-scoped loaders for `price`, `inventory_item`, and
`region_country_tax_policy` (or an explicit `// INVARIANT:` note where the parent-check is the
documented contract).

**Fixed (remediation row 58).** The reads that lean on a tenant-checked parent row carry that
contract at the query (`// INVARIANT:` notes naming the row that owns the boundary), and the two
`PricingService` readers that took **no** `tenant_id` at all — `get_price` and `get_variant_prices`,
the latter also called from the write path inside `set_price_tier_with_channel`, where it was safe
only because the write before it had validated the variant — now take `tenant_id` and resolve the
variant through the shared `ensure_variant_tenant` loader (`product_variant.tenant_id`, the rule
`resolve_variant_price` stated inline and now reuses). A foreign or unknown variant is a
`VariantNotFound`, never a price row; a variant of the caller with no price in the requested currency
still answers `Ok(None)`.

### ECOM-PANIC-01 — undocumented panics on the money path

| Location | Context | Assessment |
|---|---|---|
| `rustok-commerce/src/services/marketplace_financial_runtime.rs:68` | `.expect("MarketplaceFinancialCommandPort must be host-composed for reversal workflows")` | Panics at **request time** when the host did not compose the port. A miscomposition should be a typed startup error or a `PortError`, not a panic in a money workflow. |
| `rustok-commerce/src/services/checkout_inventory_order_adoption.rs:219` | `.expect("reservation set was validated before transaction")` | Guarded by earlier validation, but the check is not visible at the call site and there is no `// INVARIANT:` marker. |
| `rustok-commerce/src/services/fulfillment_orchestration.rs:181` | `.expect("requested items already validated as non-empty")` | Documented by a preceding comment (good) but still a panic in a fulfillment path. |
| `rustok-payment/src/services/payment.rs:107` | `cart_id.expect("cart_id was checked before race recovery")` | Safe by the match guard; the guard-based rewrite is applied in this change set. |
| `rustok-payment/src/services/payment.rs:949`, `rustok-commerce/src/services/collection_owner.rs:393`, `collection_translation.rs:652` | `.expect("…")` | Applied invariants/infallible serialization; acceptable but should carry `// INVARIANT:` comments per standard. |

### ECOM-REFUND-01 — two refund tables, two services, no reconciliation proof

**Evidence**

- `PaymentService::complete_refund` / `cancel_refund`
  (`rustok-payment/src/services/payment.rs:422-489`) mutate `entities::refund` rows.
- `PaymentRefundCreationService::create_or_replay`
  (`rustok-payment/src/services/refund_creation.rs:41-140`) inserts `entities::refund_creation` rows
  with its own `creation_key`/`creation_request_hash` idempotency and its own
  `captured_amount − Σ(pending|refunded)` over-refund guard.
- Both entities exist in `src/entities/` and both are reachable from the admin refund command
  (`admin_refund_command.rs`) depending on the entry point.

**Impact:** refund state can only be reconciled by reading both tables; the over-refund guard is
implemented on `refund_creation` only, so any path that writes `refund` rows directly bypasses it.
Whether the two tables are "legacy + current" or "two legitimate owners" is not documented anywhere
in the crate.

**Required remediation:** declare the owner (recommended: `refund_creation` with its idempotency
contract), migrate/close the other table, and add a single `remaining_refundable_amount` query used
by every refund entry point.

---

## P3 / hygiene findings

**ECOM-ERR-01 — swallowed results and panics in the money crates.**
Static sweep over `rustok-cart`, `rustok-order`, `rustok-payment`, `rustok-inventory`,
`rustok-fulfillment`, `rustok-pricing` (migrations excluded): 165 occurrences of
`let _ = …` / `.ok();` / `unwrap_or_default()` / `unwrap_or(…)`, and 131 `.unwrap()` / `.expect(`
(non-test modules after stripping `#[cfg(test)]`). The money-path subset is now fixed or explicitly
classified: every leftover `let _ =` in the seven crates is either a test module, a tuple/value
discard whose fallible part already propagates with `?`, or the deliberately unused-binding pattern
in the legacy stage shims (`checkout_payment_stages_legacy.rs:535`,
`checkout_fulfillment_stages_legacy.rs:137`). The remainder is normal Rust error-handling noise and
should be triaged by the owning module rather than bulk-rewritten.

**ECOM-LOG-01 — raw debug prints in cart money mapping.**
`rustok-cart/src/atomic_checkout_guard.rs:222` and `rustok-cart/src/marketplace_snapshot.rs:197`
printed `eprintln!("DEBUG MAP … ERROR: {error:?}")`. Replaced with structured `tracing::error!`.

**ECOM-RECON-01 — a parked checkout had no way out (fixed).**
The status is real and is written outside Rust: migrations `m20260713_000016` and
`m20260713_000017` declare it in the `ck_checkout_operations_status` CHECK constraint and in the
partial unique index `ux_checkout_operations_active_cart` (all three DB backends), and install
triggers that rewrite a `compensation_required` row to `reconciliation_required` (setting
`completed_at`) whenever `last_error_code = 'checkout.compensation_manual_reconciliation'` — which is
exactly what `mark_compensation_retryable` writes when compensation ends in
`CheckoutCompensationError::ManualReconciliation` (`compensation_error_code`,
`checkout_compensation_owner_ports.rs:665-680`). `recovering_staged_checkout` reads it
(`find_latest_by_cart` / `find_by_key`) and returns `checkout_reconciliation_required` for a repeated
checkout of the same cart — that direction was always correct.

Three defects made the parked state a dead end:

1. **The Rust journal model did not know the status.** `CheckoutOperationStatus` and
   `active_statuses()` omitted it, so `find_active_by_cart` did not treat a parked operation as
   active: a new `begin` for the same cart with a different idempotency key passed the in-app
   conflict checks and only failed at the insert on the partial unique index, surfacing a raw
   database error instead of the typed `active_cart_conflict`.
2. **The DB guards allowed no exit.** Both the PostgreSQL function
   `enforce_checkout_operation_integrity()` and the SQLite trigger `checkout_operations_guard_update`
   enumerated the legal transitions and stopped at `… -> reconciliation_required`; `OLD.status =
   NEW.status` was the only permitted update for a parked row.
3. **No admin surface existed to close the row.** The checkout-operations router exposed only
   `POST /compensation-sweep`, `GET /{id}` and `POST /{id}/compensate`; `claim_compensation`
   filters `compensation_required | compensating`, so a parked operation could not be listed,
   and no endpoint could move it to a terminal state. Because the partial unique index keeps
   `reconciliation_required` rows in the active set, **the customer's cart stayed blocked forever**
   and the only escape was raw SQL.

**Fix applied — the parked state is now owned by Rust, end to end.**

1. **The state machine moved into the journal.** `CheckoutOperationStatus::allowed_transitions()`
   declares the legal edges, every status write goes through `CheckedTransition::new(...)` (which
   rejects an edge that is not listed, *before* the row is touched), and any mis-wired transition
   fails the transaction with a typed `Conflict` instead of a raw database exception. The parking
   decision is code, not a trigger: `compensation_next_status(...)` parks the operation in
   `reconciliation_required` (with `completed_at`) as soon as the compensation step reports
   `checkout.compensation_manual_reconciliation`, and `requires_completed_at()` keeps the
   `ck_checkout_operations_completion` column constraint satisfied by construction.
2. **The database guards were removed instead of extended.** Migration
   `m20261007_000010_move_checkout_operation_guards_to_rust` drops the PostgreSQL trigger and
   `enforce_checkout_operation_integrity()` function, the SQLite `checkout_operations_guard_insert` /
   `checkout_operations_guard_update` / `checkout_operations_manual_reconciliation` triggers and the
   MySQL parking trigger, then parks any left-over `compensation_required` row carrying the manual
   reconciliation code. Its `down` restores the exact text installed by `m20260713_000017`
   (extracted mechanically, verified snippet by snippet). The database keeps the column-level
   `CHECK` constraints and the `ux_checkout_operations_active_cart` uniqueness index — declarative
   schema constraints, not workflow logic. This supersedes the earlier
   `m20261007_000010_allow_checkout_reconciliation_resolution` approach (adding an edge to the
   PL/pgSQL matrix): the edge is now irrelevant because there is no matrix in the database.
3. **The cross-row tenant lookups became Rust code.** `ensure_cart_tenant`, `ensure_order_tenant`
   and `ensure_payment_collection_tenant` run in the same transaction as the write and use the
   owning crate's entity, so no invariant was lost with the triggers.
4. **An action registry replaced the single resolve endpoint.** New append-only journal
   `checkout_reconciliation_actions` (`m20261007_000011`, unique
   `(tenant_id, checkout_operation_id, idempotency_key)`) and `CheckoutReconciliationService` expose
   the operator actions, each with its own preconditions, permissions and money behaviour:
   `retry_compensation` (unpark, no money), `attest_external` (mandatory `evidence_ref` + outcome,
   no money), `write_off` (mandatory second approver, no money), `refund_full` / `refund_partial`
   (idempotent refund through `PaymentAdminRefundCommandPort`, then close as compensated) and
   `void_authorization` (cancel an uncaptured authorization through
   `PaymentAdminCollectionCommandPort`, then close as failed). Money actions require
   `orders:manage` **and** `payments:update`; the permission set comes from the action itself
   (`CheckoutReconciliationAction::required_permissions()`), so a money action cannot be reached
   through the safe-action path. The idempotency key of the action is handed to the payment owner as
   the refund creation key, and the journal row stores the request fingerprint, so a replay returns
   the stored row instead of moving money twice. `POST /admin/checkout-operations/{id}/resolve` is
   superseded by `POST /admin/checkout-operations/{id}/actions` (it was introduced in this same,
   unreleased change set). Actions are listed with `GET /admin/checkout-operations/{id}/actions`
   (`orders:read`).

**Operator runbook**

1. `GET /admin/checkout-operations?status=reconciliation_required` — list parked operations
   (`orders:read`).
2. `GET /admin/checkout-operations/{id}` — read the stage, the bound order and payment collection.
3. Pick the action that matches the money reality (`Idempotency-Key` header is mandatory):
   - funds must be returned and the platform can do it → `refund_full` (or `refund_partial`) with a
     reason; the refund goes through the payment owner and the checkout closes as `compensated`;
   - funds were already returned outside the platform → `attest_external` with `compensated` and an
     `evidence_ref` (ticket / bank statement / provider dashboard id);
   - nothing was captured, only an authorization is open → `void_authorization`;
   - the money is written off → `write_off` with a second approver different from the caller;
   - the blocking condition is gone and automation should retry → `retry_compensation`.
4. `GET /admin/checkout-operations/{id}/actions` — the append-only record of who decided what, why,
   on what evidence, and which refund (if any) was created.

**Still open (ECOM-COMP-02, owner decision):** the registry is operator-driven, not policy-driven.
Nothing refunds a captured payment automatically, and there is still no transactional outbox event
for park/close (that belongs to the event contract of ECOM-EVT-01). What is no longer open is the
unbounded loop: the sweep now parks an operation whose compensation attempts reach
`MAX_CHECKOUT_COMPENSATION_ATTEMPTS`, the park and every action are counted by the two new Prometheus
counters, and the sweep report exposes the `exhausted` count. Increment 3 in Appendix A
(policy-as-data, default `manual_required`) is the next step for the automatic path; the append-only
journal, the action guards and the bounded retry loop it needs are now in place.

**ECOM-RECON-02 — MySQL guard parity (closed, not applicable).**
`m20260713_000017` installed, for MySQL, only the parking trigger: no status/transition guard, no
`ux_checkout_operations_active_cart` equivalent (MySQL has no partial indexes), and no CHECK
constraint from the create-table path, so on a MySQL deployment the transition matrix and the
one-active-checkout-per-cart invariant were not enforced. **Owner decision (2026-10-07): MySQL is
not a deployment target for this platform, so the finding is closed as not applicable.** It is
recorded as *superseded* rather than *fixed*: since `m20261007_000010_move_checkout_operation_guards_to_rust`
drops the checkout guards on every backend and moves the rules into `CheckoutOperationJournal`, the
remaining MySQL gap is reduced to the missing unique index — the transition matrix no longer lives in
any database. Should MySQL ever be deployed, the required work is one migration adding a
`generated column + unique index` equivalent of the partial index (`CASE WHEN status IN (...)`), and
nothing else.

**ECOM-DB-01 — checkout business rules lived in database triggers (fixed).**
The repository contract is explicit (`AGENTS.md`): schema-level `CHECK` constraints for column-level
value validity are welcome, while "multi-row domain rules, state transition validations, workflow
guards, and cross-aggregate invariants MUST be owned and validated by typed Rust domain entities and
services inside a transaction boundary", and "do NOT implement complex business rules, cascading
side-effects, or cross-row business validations inside PL/pgSQL constraint triggers". The checkout
journal violated that contract in four ways, all installed by `m20260713_000009` /
`m20260713_000017`: the transition matrix, the parking rewrite (`compensation_required` +
`checkout.compensation_manual_reconciliation` → `reconciliation_required`), the lease/completion
shape, and cross-row tenant lookups against `carts`, `orders` and `payment_collections`. The cost was
concrete, not stylistic: the matrix existed in three dialects (PL/pgSQL, SQLite, MySQL) plus a Rust
mirror, and it drifted — the Rust model did not know `reconciliation_required` at all, which is what
made ECOM-RECON-01 a customer-visible dead end. Adding one legal edge required an 18 KB migration that
re-stated the whole matrix verbatim.

Fixed in this change set: `m20261007_000010_move_checkout_operation_guards_to_rust` removes every
checkout guard trigger and function on all three backends, `CheckoutOperationStatus::allowed_transitions`
+ `CheckedTransition` own the transitions, `compensation_next_status` owns the parking decision, the
lease/completion shape follows from the writers, `CheckoutOperationJournal` never touches identity
columns, and `ensure_cart_tenant` / `ensure_order_tenant` / `ensure_payment_collection_tenant` own the
cross-row tenant checks. Adding an action or an edge is now a Rust change plus (at most) a table
migration — no SQL state machine to re-state.

The decision is recorded as
[`DECISIONS/2026-10-07-checkout-operation-invariants-owned-by-rust.md`](../../DECISIONS/2026-10-07-checkout-operation-invariants-owned-by-rust.md)
(`Accepted`, implementation `In progress`, registered in `DECISIONS/README.md` and `docs/index.md`;
`npm run verify:adrs` passes for 110 decisions). The one deliberate remainder named here — the same
rule applied to `payment_provider_operations_checkout_guard` (`m20260713_000016`), to be removed once
the equivalent check exists in `rustok-payment`'s provider execution path — has since been removed
(fourth pass: the typed admission level and the claim gate, `m20261007_000013`), together with the
payment-collection binding guard (`m20260713_000015`, sixth pass), so the remainder is closed.

**ECOM-VERIFY-01 — the admin checkout-operation verifiers are already red.**
`scripts/verify/verify-commerce-admin-checkout-operation-diagnostic-safety.mjs` and
`scripts/verify/verify-commerce-admin-checkout-operation-error-context.mjs` are the repository's own
source contracts for the diagnostic safety of this surface, backed by
`contracts/evidence/admin-checkout-operation-diagnostic-safety-source-review.json`. Run against the
audit base commit (`a53a4dd`, `git archive` into a clean directory), they fail with **41** and **89**
findings respectively — before any change in this audit:

```text
$ node scripts/verify/verify-commerce-admin-checkout-operation-diagnostic-safety.mjs   # a53a4dd
failed: 41 findings
$ node scripts/verify/verify-commerce-admin-checkout-operation-error-context.mjs        # a53a4dd
failed: 89 findings
```

The drift is factual, not cosmetic: the verifiers expect `CheckoutCompensationError::Payment/Order/
PaymentOrchestration` variants that do not exist in the code, read `services/checkout_compensation.rs`
(a file that is only reachable through the `#[path]`-overlay `checkout_compensation_error_safe.rs`),
expect a generic `admin_checkout_operation_http_error<E>(...)` logger that no longer takes the typed
error at all, and expect diagnostic fields named `tenant_id` / `actor_id` where the source now uses
`tenant_state` / `actor_state`. Some of those expectations describe behaviour the code does *not*
have (typed compensation errors), so re-aligning them means deciding, marker by marker, whether the
source or the specification is wrong — a maintainer decision, not a mechanical edit. Until then the
"source-ready / unvalidated" contract is effectively unverified, and any CI gate that runs these
scripts is either red or silently skipped.

This change set touched only two things in those scripts: the expectation its new admin handlers
invalidate (context-aware mapper callsites, 3 → 6, with `reconciliation` added to the recognised
mapper names) and, in the crate doc the verifier reads, a sentence that was wrapped across two lines
and therefore never matched. Measurements: diagnostic-safety verifier **41 findings on the pristine
base commit, 40 now**; error-context verifier **89 before and 89 after** — i.e. this change set adds
no new verifier findings and fixes one. The reviewer should decide whether to finish the realignment
or to retire the scripts in favour of the Rust tests.

**ECOM-UI-01 — `cart.unwrap()` in the storefront cart drawer.**
`crates/modules/rustok-cart/storefront/src/ui/leptos.rs:997-1028`: `items` is derived from `cart`
(`cart.as_ref().map(|c| c.line_items.clone()).unwrap_or_default()`), so `cart.unwrap()` inside the
`else` branch is currently unreachable. It remains a latent panic: any future second source of
`items` (or a refactor that moves the derivation) panics the drawer. Recommended shape: match
`cart` once and render the empty state from the `None` arm.

**ECOM-MONEY-04 — 2-decimal rounding in promotions and pricing.**
`rustok-cart/src/services/cart/promotions.rs` (percentage and fixed, cart- and line-item- and
shipping-scoped previews plus the persisted adjustment) and
`rustok-pricing/src/services/pricing.rs:456, 2117` round with `round_dp(2)` regardless of the
currency. For 3-decimal currencies this truncates a discount/promo to the wrong granularity and the
*order* then carries an amount the provider cannot represent (`stripe.to_minor_units` rejects excess
precision, `stripe_provider.rs:740-744`, failing the checkout). **Cart-side call sites are fixed**
via `round_to_currency(amount, currency_code)` (`helpers.rs:1377`, all five promotion sites and the
persisted adjustment); the pricing price-list rule sites are reported (they need the same treatment
when ECOM-MONEY-02's canonical owner lands). Note the pricing percentage helper at `pricing.rs:2390`
uses `round_dp(2)` for a *percentage*, which is a deliberate presentation choice and is not a money
rounding defect.

---

## Verified-good behaviours (recorded so later reviews do not re-litigate them)

- **Oversell protection is atomic.** `rustok-inventory/src/services/inventory.rs:468-486` performs a
  single conditional `UPDATE … WHERE stocked_quantity − reserved_quantity >= requested` and treats
  `rows_affected != 1` as `InsufficientInventory`; backorder policy opts out explicitly.
- **Reservation identity is durable and replay-safe.**
  `rustok-commerce/src/services/checkout_inventory_reservation_executor.rs:96-215`: reservations are
  planned in a journal, `Reserved` rows are adopted on replay, provider responses are checked against
  the persisted identity (`inventory.reservation_response_mismatch`), and the stage checkpoint is
  CAS-guarded.
- **Payment identity is validated against the order.** `validate_collection`
  (`checkout_payment_stages_legacy.rs:426-465`) asserts tenant, cart, order, customer, currency and
  `collection.amount == order.total_amount`, plus `checkout.operation_id`/`order_plan_hash`
  metadata; capture asserts `captured_amount == order.total_amount`.
- **Journal/lease discipline is sound.** `checkout_operation.rs:228-330`: CAS claim with expired-lease
  takeover, `LeaseExpiresAt > now` guard on every checkpoint and on `renew_lease`, and no
  read-modify-write without a `rows_affected` check.
- **Storefront guest access has REST/GraphQL parity.** Both transports use the same guarded
  crate-root port (`rustok_cart::in_process_cart_storefront_port` =
  `guarded_ports::guarded_cart_storefront_port`, `lib.rs:36-37`): REST controllers
  (`controllers/store/{carts,checkout,products}.rs`), GraphQL mutations/queries
  (`graphql/mutations/{cart,checkout}.rs`, `graphql/query.rs`), and the `safe_query` shims all import
  the crate-root name. Authorization happens inside the port (`guarded_ports.rs:61-80`:
  customer-owned carts pass, guest carts require a token from `PortContext.claims` or the request
  task-local). The middleware that binds the token to the request
  (`rustok_cart::guest_access_http::resolve`) is layered on the fully merged router in
  `compose_application_router` (`apps/server/src/services/app_router.rs:441`), and the GraphQL router
  is merged before that (`apps/server/src/host.rs:288`, `controllers/graphql.rs:581-590`), so
  `POST /api/graphql` resolves `x-cart-access-token`/`rustok-cart-access-token` exactly like the REST
  storefront. The checkout runtime additionally re-checks customer ownership
  (`storefront_staged_checkout_runtime.rs:209-253`).
- **The active compensation path is fail-loud.** `CheckoutCompensationService::compensate`
  (`checkout_compensation_owner_ports.rs:162-214`): claim → `compensate_payment` → `compensate_order`
  → `release_remaining_reservations` → `release_cart`; a failure is journaled as retryable
  (`mark_compensation_retryable`) and if the journal write itself fails the combined
  `CompensationAndJournal` error is returned. No step discards an error.
- **Guest cart capability is correctly implemented.** `rustok-cart/src/guest_access.rs:135-190`:
  SHA-256 of a 32–256 char token stored in cart metadata, constant-time comparison, transient token
  only in the response, `403 cart.guest_access_denied` for guests without the capability;
  customer-owned carts are additionally protected at the transport by
  `ensure_store_cart_access` (`rustok-commerce/src/controllers/store/mod.rs:361-379`), which returns
  401/403 instead of leaking another customer's cart.
- **No SQL injection surface** in the seven money crates: every raw statement uses
  `Statement::from_sql_and_values` with bound parameters (`payment.rs:709,785`,
  `refund_creation.rs:138`, `order.rs:104,129,154`, `analytics.rs:28,57`).
- **No blocking I/O in async money paths:** no `std::fs`, `std::thread::sleep`, `std::sync::Mutex`
  across `await`, `block_on`, or `.blocking_*` in the seven crates.
- **Webhook ingress fails closed:** a provider signature header is mandatory
  (`controllers.rs:503-520`), the inbox row records `signature_verified` (enforced by migrations
  `m20260714_000114…000116`), Stripe verifies HMAC-SHA256 over `timestamp.payload` with a
  300 s tolerance and `< 1 MiB` body limit, and events are claimed with a 30 s lease
  (`provider_event_ingress.rs:19-20`).
- **Promotion guards:** percent ∈ (0, 100], fixed amount > 0, base amount clamped at zero, source id
  bounded to 1–191 chars, and promotion adjustments are replaced (not accumulated) per source
  (`helpers.rs:417-533`, `promotions.rs:409-418`).

---

## Remediation log (changes applied in this audit)

| # | File | Change |
|---|---|---|
| 1 | `crates/modules/rustok-order/src/services/order.rs` | `OrderPlaced.total` now converts with the platform currency exponent and fails the transaction (`OrderError::Validation`) instead of `unwrap_or(0)`. |
| 2 | `crates/modules/rustok-pricing/src/services/pricing.rs` | `PriceUpdated.old_amount`/`new_amount` now convert with the currency exponent and propagate an error instead of `unwrap_or(0)` (`set_price`, `set_prices`). |
| 3 | `crates/modules/rustok-cart/src/atomic_checkout_guard.rs` | `eprintln!("DEBUG MAP …")` → structured `tracing::error!`. |
| 4 | `crates/modules/rustok-cart/src/marketplace_snapshot.rs` | same. |
| 5 | `crates/modules/rustok-payment/src/services/payment.rs` | removed `cart_id.expect(...)` from the unique-constraint race recovery path; no panic, identical behaviour. |
| 6 | `crates/modules/rustok-commerce/src/services/checkout.rs` | legacy compensation no longer discards failures silently: `cancel_collection`, `cancel_order` and all four `release_cart_checkout` sites are logged with full context. |
| 7 | `crates/modules/rustok-cart/src/services/cart/helpers.rs` + `…/cart/promotions.rs` | new `round_to_currency(amount, currency_code)` helper; all five promotion call sites and the persisted adjustment now round with the cart currency exponent instead of a hard-coded 2 decimals. |
| 8 | `crates/modules/rustok-payment/src/{admin_collection_command,admin_refund_command}.rs`, `checkout_execution/provider_helpers.rs` | 8 best-effort `let _ = …mark_reconciliation_required(…)` sites now log the failure instead of dropping it (`if let Err(mark_error) … tracing::error!`). The provider-execution site logs bounded fields only (`operation_id_non_nil`, `provider_operation`, `reconciliation_mark_failed`, a validation bit) because that file's diagnostic-safety contract forbids raw identifiers, raw error text and a fifth `PaymentError` fact extraction; the verifier sweep caught the first draft and it was corrected. |
| 9 | `crates/modules/rustok-commerce/src/services/refund_reconciliation.rs` | same treatment for the 3 refund reconciliation checkpoints. |
| 10 | `crates/modules/rustok-commerce/src/services/journaled_{payment_provider,fulfillment_orchestration,create_label_provider}.rs` | same treatment for the 7 journal-commit reconciliation checkpoints. |
| 11 | `crates/modules/rustok-fulfillment/src/services/fulfillment.rs` | removed the dead `checkout_fulfillment_index` parameter (and its `let _ =` discard) from `validate_checkout_identity`. |
| 12 | `crates/modules/rustok-commerce/src/services/checkout_operation.rs` + `…/recovering_staged_checkout.rs` | added the DB-only `ReconciliationRequired` status to `CheckoutOperationStatus` and to `active_statuses()` (so `begin` returns a typed `active_cart_conflict` for a parked cart instead of a raw unique-index error); removed the duplicated status string constant. |
| 13 | `crates/modules/rustok-commerce/src/services/checkout_operation.rs` | `CheckoutOperationStatus` now owns the state machine: `allowed_transitions()`, `can_transition_to()`, `requires_completed_at()`, `is_parked()`, plus `CheckedTransition` (every status write validates the edge before touching the row) and `compensation_next_status` (parks on `checkout.compensation_manual_reconciliation`). `release_lease_with_error` / `mark_terminal` set `completed_at` from the target status, and the parking path now clears the lease and sets `completed_at` in Rust instead of relying on a trigger. |
| 14 | `crates/modules/rustok-commerce/src/services/checkout_operation.rs` | new `request_compensation_retry` (`reconciliation_required -> compensation_required`, clears `completed_at`, records the operator and reason) and reworked `resolve_reconciliation_required` (outcome → `checked` transition, constant error codes `CHECKOUT_RECONCILIATION_RESOLVED_CODE` / `CHECKOUT_RECONCILIATION_RETRY_REQUESTED_CODE`). |
| 15 | `crates/modules/rustok-commerce/src/services/checkout_operation.rs` | the cross-row tenant lookups the PL/pgSQL guard used to do are back in Rust: `ensure_cart_tenant` (`begin`), `ensure_order_tenant` and `ensure_payment_collection_tenant` (`checkpoint`), each running in the same transaction and using the owning crate's entity. |
| 16 | `crates/modules/rustok-commerce/src/migrations/m20261007_000010_move_checkout_operation_guards_to_rust.rs` (+ `migrations/mod.rs`) | replaces the earlier `…_allow_checkout_reconciliation_resolution` migration: `up` drops the PostgreSQL trigger + `enforce_checkout_operation_integrity()`, the three SQLite triggers and the MySQL trigger and parks stale manual-reconciliation rows; `down` restores the exact `m20260713_000017` guard text (extracted mechanically and verified). Registered with an explicit dependency on `m20260713_000017`. |
| 17 | `crates/modules/rustok-commerce/src/{entities/checkout_reconciliation_action.rs,entities/mod.rs}` + `…/migrations/m20261007_000011_create_checkout_reconciliation_actions.rs` | new append-only action journal table (no `updated_at`) with the unique idempotency index `(tenant_id, checkout_operation_id, idempotency_key)` and operator/operation lookup indexes. |
| 18 | `crates/modules/rustok-commerce/src/services/checkout_reconciliation.rs` (+ `services/mod.rs`, `lib.rs`) | new `CheckoutReconciliationService` action registry: `retry_compensation`, `attest_external`, `write_off`, `refund_full`, `refund_partial`, `void_authorization`; per-action permission set and precondition validation; idempotency by key + request fingerprint; refunds through `PaymentAdminRefundCommandPort` with the action key as the refund creation key; authorization cancel through `PaymentAdminCollectionCommandPort`; every action appends one journal row. A concurrent request that loses the race *after* both sides moved money through the same creation key reads the winner's journal row and returns it as a replay (`CloseAfterMoneyMoved` is only reported when no journal row for that key exists), so a double-submitted action cannot look like a 502 to the second caller. |
| 19 | `crates/modules/rustok-commerce/src/controllers/admin/checkout_operations.rs` (+ `openapi.rs`) | `POST /admin/checkout-operations/{id}/actions` (per-action permissions: `orders:manage` for safe actions, `orders:manage` + `payments:update` for money actions; mandatory `Idempotency-Key`) and `GET /admin/checkout-operations/{id}/actions` (`orders:read`); the single `/{id}/resolve` endpoint introduced earlier in this change set is superseded and removed. New response/input schemas registered in the OpenAPI document. |
| 20 | `crates/utils/rustok-migrations/tests/checkout_reconciliation_smoke.rs` | expectation message updated only ("classified by the database guard" → parked by the journal); the assertions are unchanged and must still pass because the parking decision now runs in `mark_compensation_retryable`. |
| 21 | `DECISIONS/2026-10-07-checkout-operation-invariants-owned-by-rust.md` + `DECISIONS/README.md` + `docs/index.md` | the guard-ownership decision is recorded as an ADR (`Accepted` / `In progress`) and registered in the ADR registry and the central index; it states the allowed/forbidden transitions, the tenant and idempotency boundary, the failure semantics, the migration/cutover plan and the remaining provider-guard follow-up. |
| 22 | `crates/modules/rustok-commerce/src/services/checkout_operation.rs` (+ `services/mod.rs`) | `MAX_CHECKOUT_COMPENSATION_ATTEMPTS = 8`, `CHECKOUT_COMPENSATION_ATTEMPTS_EXHAUSTED_CODE`, and `park_exhausted_compensation` — a `compensation_required \| compensating (lease expired) -> reconciliation_required` CAS park that clears the lease, sets `completed_at`, and counts the park metric. Fixes the unbounded compensation loop (ECOM-COMP-02). |
| 23 | `crates/modules/rustok-commerce/src/services/checkout_compensation_sweep.rs` + `src/controllers/admin/checkout_operations.rs` | the sweep checks the attempt cap before claiming, parks exhausted operations, counts them in the new `exhausted` report/response field, and reports a failed park with the bounded code `checkout.compensation_exhaustion_park_failed`. |
| 24 | `crates/libs/rustok-telemetry/src/metrics.rs` | two bounded-cardinality counters (`rustok_checkout_reconciliation_parked_total`, `rustok_checkout_reconciliation_actions_total`) with `record_checkout_reconciliation_parked` / `record_checkout_reconciliation_action`; registered in `register_all`, and wired into the park paths and the reconciliation service (one sample per action call, `replayed` vs `succeeded` distinguished). |
| 25 | `crates/libs/rustok-events/src/{checkout_operation.rs,contract.rs,lib.rs}` + `crates/modules/rustok-commerce/src/services/checkout_operation.rs` (+ the 19 journal construction sites, `controllers/mod.rs` runtime, `storefront_staged_checkout_runtime.rs`, `journaled_checkout.rs`, two test fixtures) | new `checkout.operation.parked` / `checkout.operation.reconciled` contract family, published with `publish_contract_in_tx` in the same transaction as the status write by all four journal writers; the bounded park vocabulary is imported from `rustok-events` instead of a local literal, so the metric label and the event payload agree on `manual_reconciliation`/`attempts_exhausted`. Fixes ECOM-EVT-02. |
| 26 | `scripts/verify/verify-checkout-operation-event-contract.mjs` + `.github/workflows/ecommerce-hardening.yml` + `package.json` | new verifier for the family registration, the in-transaction publication order and the single-vocabulary rule, wired as the `verify:commerce:checkout-operation-events` npm script and an `Ecommerce Hardening` step. |
| 27 | `scripts/verify/verify-event-contract-digest-admission.mjs` + `.github/workflows/event-contract-digest-admission.yml` | the verifier's `actions/upload-artifact@v7` marker could not match a SHA-pinned workflow, so the digest gate's only verifier always failed; it now requires the pinned `@<sha> # v7` form and forbids the unpinned tag. The workflow gained the `# v7` pin comment used by the other workflows. |
| 28 | `crates/libs/rustok-events/src/{checkout_operation.rs,lib.rs}` + `crates/modules/rustok-commerce/src/{entities/checkout_operation.rs,migrations/m20261007_000012_add_checkout_operation_admission.rs,migrations/mod.rs,services/checkout_operation.rs,controllers/admin/checkout_operations.rs}` + `crates/utils/rustok-migrations/tests/checkout_reconciliation_smoke.rs` + two commerce test fixtures | the checkout-owned half of the provider execution admission cutover (ADR "Remaining cutover"): `execution_admission` (`open`/`settling`/`closed`) and `admission_epoch` on `checkout_operations`, the typed `CheckoutExecutionAdmission` level derived from the status machine, the level and the generation written by all seven status writers inside the same conditional update as the status, `checkout.operation.admission_changed` published in the writer's transaction by `begin`/`release_lease_with_error`/`mark_terminal`/`resolve_reconciliation_required`, the level/generation exposed by the admin response, and `begin` starting a new generation per cart. Fixes ECOM-ADM-01/ECOM-ADM-02 in the Rust contract; the trigger removal is the next pass. |
| 29 | `scripts/verify/verify-checkout-execution-admission-contract.mjs` + `package.json` + `.github/workflows/ecommerce-hardening.yml` | new verifier for the admission record: bounded level vocabulary owned by `rustok-events`, positive generation, the "every status writer writes the level and the generation" count rule, the in-transaction publication order of `checkout.operation.admission_changed`, and no duplicated vocabulary in the journal; wired as `verify:commerce:checkout-execution-admission` and as an `Ecommerce Hardening` step. |
| 30 | `crates/modules/rustok-payment/src/{migrations/m20261007_000122_add_provider_operation_admission.rs,migrations/mod.rs,entities/provider_operation.rs,services/checkout_admission.rs,services/provider_operation.rs,services/mod.rs,checkout_execution*,admin_collection_command.rs,admin_refund_command.rs,checkout_compensation.rs,lib.rs,Cargo.toml}` + `crates/modules/rustok-commerce/src/{services/checkout_execution_admission.rs,services/checkout_operation.rs,services/checkout_payment_stages.rs,services/checkout_payment_stages_legacy.rs,controllers/mod.rs,graphql_runtime/payment_commands.rs,lib.rs}` + `apps/server/src/services/commerce_provider_runtime.rs` + `crates/libs/rustok-telemetry/src/metrics.rs` | the payment half of the same cutover: `payment_provider_operations.admission_epoch` with the bounded refusal columns and their PostgreSQL/MySQL `CHECK`; the narrow owner port `CheckoutExecutionAdmissionPort` (implemented by commerce over a tenant-scoped read — no derived copy in payment, so no drift and no resync path); the claim gate as one conditional write on the level and the generation, with `0` adopted only while `open`; unwinding effects admitted while `settling`/`closed` and never fenced; bounded refusal codes + `rustok_payment_provider_execution_admission_refused_total` + `execution_admission_refusal_error` instead of a silent `None`; park-time generation stamping inside the transition transaction; every production payment-execution construction site wired to the port. Closes ECOM-ADM-01/ECOM-ADM-02 in the Rust contract. |
| 31 | `crates/modules/rustok-commerce/src/migrations/{m20261007_000013_drop_provider_execution_checkout_guard.rs,mod.rs}` + `scripts/verify/verify-checkout-execution-admission-contract.mjs` | removes `payment_provider_operations_checkout_guard` and its PL/pgSQL function (`up` drops, `down` restores the exact `m20260713_000016` text), so no business rule of this contour is left in the database; the verifier now covers both halves, the trigger removal and the port wiring. |
| 32 | `crates/modules/rustok-outbox/src/{migration.rs}` + `crates/utils/rustok-migrations/src/{m20261007_000014_add_sys_events_claim_index.rs,lib.rs}` + `scripts/verify/verify-money-path-query-indexes.mjs` + `package.json` + `.github/workflows/ecommerce-hardening.yml` | the relay claim (`status = 'pending'` + liveness predicates + `ORDER BY created_at LIMIT batch_size`) had no index for its order: the table carried `(status, next_attempt_at)` and `(claimed_at)`, so every relay iteration sorted the filtered pending set. `idx_sys_events_pending_created_at (status, created_at)` is created through the owner helper by both migrators; the new verifier pins the query/index alignment, the registration and the relay's predicate/order. |
| 33 | `crates/modules/rustok-cache/src/{service.rs,redis_status.rs}` | a build without `redis-cache` silently discarded the configured Redis URL, so `health()` reported `redis_configured: false` and readiness stayed green while the deployment served a memory-only cache with no cross-instance invalidation. The URL is now resolved and kept in every build and `redis_status()` reports `configured but not initialized` with a build-aware message, which makes `is_degraded()`/readiness catch the misconfiguration (a memory-only profile that exports `RUSTOK_REDIS_URL` therefore reports degraded, the same rule the feature-enabled build already applies to an unreachable Redis). |
| 34 | `crates/modules/rustok-commerce/src/migrations/{m20261007_000014_add_checkout_operation_admin_list_index.rs,mod.rs}` | the admin reconciliation list (`tenant_id` + optional `status`, newest update first) had no supporting index; `idx_checkout_operations_tenant_status_updated (tenant_id, status, updated_at)` serves the filtered queue read, and the migration is registered with an explicit dependency on the table owner. |
| 35 | `crates/modules/rustok-outbox/src/{retention.rs,lib.rs,Cargo.toml,migration.rs}` | delivered-event retention became an owner subsystem: `OutboxRetention::prune_once` deletes only `status = 'dispatched'` rows whose `dispatched_at` is non-null and older than the window, in bounded batches, reading the oldest delivered rows through the new `idx_sys_events_dispatched_at (status, dispatched_at)` and deleting exactly the selected ids with the same predicates; `pending` (still owed) and `failed` (DLQ) rows are never touched, which a verifier forbids. `OutboxRetentionConfig { retention: 30d, batch_size: 500 }` with `validate()`, `OutboxPruneReport { pruned, batch_exhausted }`, and the `rustok_outbox_pruned_total` / `rustok_outbox_retention_last_run_timestamp_seconds` metrics. Closes the retention owner decision of the tail pass. |
| 36 | `crates/utils/rustok-migrations/src/{m20261007_000015_add_sys_events_retention_index.rs,lib.rs}` | the retention index is append-only for deployed schemas: the migration calls the outbox-owned `create_sys_events_retention_index` and reverses it with `drop_sys_events_retention_index`. `SysEventsMigration` creates the same index for module/test schemas, so both migrators keep one shape. |
| 37 | `apps/server/src/{common/settings.rs,services/event_transport_factory.rs,services/outbox_retention_worker.rs,services/app_lifecycle.rs,services/mod.rs,controllers/health.rs,controllers/metrics.rs}` | runtime wiring for the pruner: `events.outbox_retention.{enabled,retention_days,batch_size,interval_seconds}` (defaults `true`, 30 days, 500 rows, hourly; the batch default comes from the owner constant), resolved into the event runtime only for the outbox profiles and rejected at boot outside 1..=3650 days / 1..=100000 rows; the supervisor worker prunes on its interval, counts failed runs, and stops on the shutdown signal; readiness reports `worker:outbox_retention` only when retention is enabled, and `/api/metrics` exports its state plus `rustok_runtime_worker_failures_total{worker="outbox_retention"}`. |
| 38 | `crates/libs/rustok-telemetry/src/metrics.rs` + `crates/libs/rustok-core/src/events/{bus.rs,handler.rs,consumer.rs}` + `apps/server/src/controllers/metrics.rs` | the `rustok_event_bus_*` family is produced by the runtime again instead of only by the telemetry tests: `record_event_published` on the accepted in-process publish (`EventBus::publish_envelope`), `record_event_dispatched` + `record_event_processing_duration` per handler invocation with the static handler name as the bounded label, `record_event_lag` through `EventConsumerRuntime::record_publish_lag` from the envelope timestamp, and `rustok_event_bus_queue_depth{transport="outbox"}` fed by the same backlog reading that renders `rustok_outbox_backlog_size`. The already-live members (`record_event_error`, consumer restarted/lagged, dispatch latency) are unchanged, and the durable write/relay numbers stay on `rustok_outbox_*`, so nothing is double counted. Closes the metric-family owner decision of the tail pass. |
| 39 | `crates/modules/rustok-outbox/src/migration.rs` + `crates/utils/rustok-migrations/src/{m20260211_000002_create_sys_events.rs,m20261007_000016_drop_sys_events_superseded_indexes.rs,lib.rs}` | `sys_events` now has one schema owner: the table shape lives only in `create_sys_events_schema` / `drop_sys_events_schema`, `SysEventsMigration` composes the schema with the claim/retention indexes and the receipt table, and the platform wrapper `m20260211_000002_create_sys_events` keeps its timeline identity but delegates instead of carrying a second copy of the DDL. The two indexes the claim index superseded (`(status, next_attempt_at)`, `(claimed_at)`) are no longer created by any schema path and are dropped for deployed schemas by the append-only `m20261007_000016_drop_sys_events_superseded_indexes` (reversible `down`). Closes the duplicated-creation owner decision of the tail pass. |
| 40 | `scripts/verify/verify-money-path-query-indexes.mjs` + `scripts/verify/verify-outbox-retention-and-event-metrics.mjs` + `package.json` + `.github/workflows/ecommerce-hardening.yml` | guards for both passes: the money-path verifier also pins the retention prune predicates/order/batch, the retention index and its platform migration, the single schema owner (the platform wrapper must not contain DDL, the schema helper must not create indexes) and the superseded-index drop; the new verifier pins the retention policy plumbing end to end (owner, telemetry pair, settings, factory bounds, supervisor, readiness, metric rendering) and the four production call sites of the event-bus family. Wired as `verify:commerce:money-path-query-indexes` and `verify:outbox:retention-event-metrics` plus an `Ecommerce Hardening` step. |
| 41 | `crates/modules/rustok-outbox/{README.md,CRATE_API.md,docs/README.md,rustok-module.toml}` + `crates/modules/rustok-outbox/src/lib.rs` + `docs/references/outbox/README.md` + `docs/guides/metrics.md` | documentation for the new surface: the retention window and its bounded/delivered-only semantics, the config keys and their defaults, the metric names, the index ownership (claim/retention/superseded) and the single schema owner; `rustok-telemetry` added to the crate's dependency list in `CRATE_API.md`, the module description mentions retention, and the metrics guide documents where each `rustok_event_bus_*` sample now comes from plus the retention counters. |
| 42 | `crates/modules/rustok-commerce/src/services/checkout_operation.rs` | the park-time fence was implemented but never invoked: `invalidate_provider_execution_admitted_by` (the commerce-side call that stamps the collection's non-terminal provider operations with the new admission generation) had no call site, so the generation bump only happened on the payment side, where nothing triggered it for a parked checkout. It is now called from `publish_admission_changed` inside the same transaction as the level write and the `checkout.operation.admission_changed` event, which is what makes the in-flight-claim fence effective; the existing admission verifier pin (`self.invalidate_provider_execution_admitted_by(txn, operation)`) now describes reachable code, and the private method is no longer dead. |
| 43 | `apps/server/src/common/settings.rs` | `impl Default for EventSettings` was not extended with the new `outbox_retention` field, so the server crate did not compile (`E0063`) even though the serde path was complete; the default now comes from `OutboxRetentionSettings::default()` like every other field. |
| 44 | `apps/server/src/services/app_lifecycle.rs` | the `EventRuntime` literal in the relay-worker idempotency test missed the new `outbox_retention` field (same `E0063`, in the test target the CI compiles with `--all-targets`), and the retention handle's `instance_id()` had no caller — a `dead_code` warning under the workflow's `-D warnings`. The test now enables retention and asserts the retention worker is started exactly once across two `connect_runtime_workers` calls; the accessor is the assertion, so the handle has no unused surface. |
| 45 | `crates/utils/rustok-migrations/tests/checkout_reconciliation_smoke.rs` | the smoke test asserted the *removed* trigger behaviour (`claim_execution` returning an error whose text mentions the checkout compensation) and never bound the payment collection to the operation, so it would have failed on two assertions once a toolchain ran it. The gate answers `Ok(None)` and records a bounded refusal; the test now binds the collection through the same `checkpoint` the pipeline writes, wires `checkout_execution_admission_port` into the provider journal (without it the refusal is the fail-closed `unavailable`, not `settling`), asserts the recorded `checkout_admission_settling` code and turns it into the owner error through `execution_admission_refusal_error`. |
| 46 | `crates/modules/rustok-payment/src/{services/checkout_admission.rs,services/provider_operation.rs}` | the refusal metric took its `operation` label from the journal's raw operation string, which contradicts the metric's own contract and is unbounded exactly in the `effect_unknown` case (one time series per foreign string). `refusal_metric_operation_label` now bounds the label to the effect class the gate decided on (`extending` / `unwinding` / `unknown`), the raw string stays in the WARN line, the telemetry doc comment describes the bounded values, and the admission verifier pins the label owner and all three literals. |
| 47 | `crates/modules/rustok-commerce/src/services/checkout_reconciliation.rs` | `validate_request` documented "every other field is rejected when it does not belong to the action" but silently ignored `outcome` on the money actions and a second approver on actions that have none — a decision nobody made would be recorded as taken. The per-action matrix (`reject_unexpected_fields`) rejects them, `evidence_ref` stays the one free-form field every action may carry, and `fields_that_do_not_belong_to_the_action_are_rejected_not_ignored` pins the matrix. |
| 48 | `crates/libs/rustok-events/docs/event-contract-digest-admission.md` + this audit | the release-digest consequence of the new schema family is recorded where the generator lives: three `checkout.operation.*` schemas change the canonical registry digest, `published_event_contract_matches_committed_release_artifact` fails until the maintainer runs the canonical generator once, and the exact command is maintainer action 0 of this pass. No digest value was hand-edited. |

| 49 | `crates/modules/rustok-commerce/src/{services/checkout_operation.rs,services/checkout_reconciliation.rs}` | the operator decision row (`checkout_reconciliation_actions`) was appended in a **second** transaction after the operation was already closed: a crash or a storage error between the two writes left an operation closed with no decision row, and no retry could repair it (the operation had left `reconciliation_required`, so every action answered `Conflict` and `attest_external` was unreachable too — the operator could not record the decision at all). The journal now takes the decision (`CheckoutReconciliationDecision`) and writes it *inside* the closing transaction for both `resolve_reconciliation_required` and `request_compensation_retry`, returning a `ReconciliationResolution`; the reconciliation service only describes the transition (`Settlement`) and no longer writes the table itself. A failed close now leaves the operation parked, so the same request replays (the payment owner deduplicates the refund by creation key) or `attest_external` records the outcome by hand. The event-contract verifier pins the in-transaction append and forbids the service from writing the decision table directly; a negative control (renaming the append call) fails it. |
| 50 | `crates/modules/rustok-payment/src/services/checkout_admission.rs` | the bounded refusal vocabulary was written twice — once in `CheckoutAdmissionRefusal::as_str` (the codes the database `CHECK` and the metric label use) and once in `parse` — so a new refusal could be decodable but unencodable, or the reverse. `CheckoutAdmissionRefusal::ALL` is now the single list both directions read, and the unit test round-trips every entry plus the five literal codes. |
| 51 | `apps/server/src/services/app_lifecycle.rs` + `crates/utils/rustok-migrations/tests/payment_provider_operation_uncertain_outcome.rs` | two test targets the new contract had left stale. The relay/retention idempotency test started the retention worker against a shared test database that applies no migrations (`setup_test_db` is deliberately schema-free), so every prune run would have logged `no such table: sys_events` instead of proving the wiring — the test now installs the outbox schema through `rustok_outbox::SysEventsMigration` (the same call the outbox crate's own retention tests use) before `connect_runtime_workers`, and its import block was put back into the order `cargo fmt --check` enforces. The provider-operation fixture in the migration suite was constructed field-by-field and missed the three admission columns added by `m20261007_000122`; the literal now carries them, so the suite compiles with `--all-targets`. |
| 52 | `crates/modules/rustok-payment/src/admin_refund_command.rs` | a pre-existing AGENTS.md §14 violation on the money path that this pass's forbidden-pattern scan surfaced: `JournaledRefundProviderResult::result` was never read anywhere in the workspace and was silenced with `#[allow(dead_code)]` — the same suppression the repository's own remediation gate refuses for added lines. The dead field and its single-use struct are gone; `execute_refund_provider_operation` returns the provider operation id it exists to hand back, and the idempotent-replay paths keep their behaviour (the `Some(result)` test is folded into `.is_some()` before the early return). |
| 53 | `docs/guides/metrics.md` | the three metric families this work registered — `rustok_checkout_reconciliation_parked_total`, `rustok_checkout_reconciliation_actions_total` and `rustok_payment_provider_execution_admission_refused_total` — were absent from the operator guide, while the guide already documented the EventBus and outbox families. The new "Checkout Operation Metrics" section lists each metric with its bounded label vocabularies and states what an *extending* refusal means versus the by-design admission of unwinding effects. |
| 54 | `crates/modules/rustok-commerce/src/services/checkout_marketplace_financial.rs` | the legacy mount of the marketplace journal still carried a duplicate of the flow the hardened branch owns — its own `CheckoutMarketplaceFinancialError`/`Result` with `retryable()`, its own `post_after_capture_if_present` plus the three validators it called, and the `map_port_error`/`boundary_code` translation (also `LEDGER_DEADLINE`) that existed only for that duplicate. The composed build selects `checkout_marketplace_financial_hardened.rs` as `checkout_marketplace_financial` and mounts the legacy file only for the journal primitives the hardened branch imports (`checkout_marketplace_financial_hardened.rs:14`), so none of the removed items had a reader. The journal (`begin`/`get`/`claim`/`complete_with_ledger`/`mark_retryable_error`/`mark_operator_review`) and the normalizers it calls stay, 285 lines lighter; no verify gate reads the file (`grep -rl checkout_marketplace_financial scripts/` is empty). |
| 55 | `crates/modules/rustok-commerce/src/graphql/safe_query/source.rs` | the scoped `rustok_api` shim kept a five-method `GraphQLError` mirror of `rustok_api::graphql::GraphQLError`, three of which nothing calls — silenced with `#[allow(dead_code)]`, the pattern AGENTS.md §14 refuses, on the commerce GraphQL read boundary. The mirror now exposes exactly what the query implementation calls (`permission_denied`, `unauthenticated`) plus the `require_module_enabled` wrapper. Checked first: zero callers of `internal_error`/`bad_user_input`/`not_found` inside the shim's compilation unit (`graphql/mod.rs:215` uses the real crate trait, not the mirror), and every verifier that reads the file was re-run against the pristine tree with matching results. |
| 56 | `crates/modules/rustok-commerce/src/migrations/{m20261007_000015_drop_payment_collection_binding_trigger.rs,mod.rs}` + `crates/modules/rustok-payment/src/services/provider_operation.rs` + `crates/modules/rustok-commerce/src/services/checkout_operation.rs` + `scripts/verify/verify-checkout-execution-admission-contract.mjs` + `crates/utils/rustok-migrations/tests/checkout_reconciliation_smoke.rs` | the payment-collection binding guard — the last cross-module business rule in database code on the money path — is removed, and the two rules it owned are ported to typed Rust. `m20260713_000015` validated the collection's checkout identity and **wrote** `checkout_operations.payment_collection_id` from a trigger on `payment_collections`, i.e. a second writer of a column the checkout journal owns. Its identity rule was already exceeded by `validate_collection` in the payment stage and its binding write by `checkpoint`; what had kept it alive was an ordering: `invalidate_provider_execution_admitted_by` read `operation.payment_collection_id` and returned early while it was null, and the reachable case is `begin` — a fresh operation for a cart that already has in-flight claims of the previous attempt's collection. The fence now takes the **cart** (`PaymentProviderOperationJournal::stamp_admission_epoch_for_cart`): it resolves the tenant's collections of that cart and stamps their non-terminal (`pending`, `provider_error`) provider operations inside the transition's own transaction, which is exact rather than merely wider — `ux_payment_collections_active_cart` (`m20260713_000106`) admits one active collection per cart and `ux_checkout_operations_active_cart` one live checkout per cart — and the tenant filter keeps foreign operations out. `checkpoint` absorbed the guard's remaining rule, binding immutability: the same conditional write accepts an unbound operation or re-asserts the collection it already carries, and a rebind attempt answers a bounded `Conflict` naming the write-once binding, because provider operations, marketplace financial rows and refunds are keyed by that column (nothing else in Rust or in the schema enforced it). `m20261007_000015_drop_payment_collection_binding_trigger` drops the triggers and the PL/pgSQL function on PostgreSQL, SQLite and MySQL with one statement per `execute_unprepared` call (the SQLite and MySQL drivers prepare one statement at a time), `down()` restores the original migration's text byte-identically, the migration is registered with explicit dependencies on the table owner and on the migration it supersedes, and an applied migration (`m20260713_000015`) is left untouched. The contract verifier pins the cart-scoped signature and its bounds, the fence passing `operation.cart_id` (forbidding the binding-parameter form), the drop/restore statements and the write-once binding; the integration tests `park_time_fence_reaches_provider_operations_before_the_collection_is_bound` (new) and the rebind block in `manual_checkout_reconciliation_is_terminal_and_blocks_provider_execution` pin the runtime behaviour. Recorded rather than changed: a checkout parked between the collection being created and `checkpoint` now leaves that `pending` collection uncancelled — the payment compensation reports `Ok(None)` for a null `collection_id`, no money can be authorized before `checkpoint`, and the next attempt on the cart adopts the collection through `find_reusable_collection_by_cart` + `attach_order_to_collection`, which re-points the collection's metadata link at the new operation; every reader that must be exact past `payment_ready` is served by the binding, which `checkpoint` writes in the same conditional update that advances the stage. |
| 57 | `crates/modules/rustok-commerce/src/migrations/mod.rs` + `scripts/verify/verify-rust-constructor-arity{,.test}.mjs` + `package.json` + `.github/workflows/hardening-gates.yml` | a compile error the deep review itself introduced and no gate caught: the migration registration passed two `::Migration` values to one `Box::new` (`m20261007_000012`, `m20261007_000013`), which cannot compile — `Box::new` takes exactly one argument — so the pushed branch was broken until this pass found it with a source sweep. The registration is now two `push` calls, and the class is guarded: `verify-rust-constructor-arity.mjs` scans every `Box::new`/`Rc::new`/`Arc::new` in `crates`, `apps`, `xtask`, `examples` and `tests`, blanks comments and string bodies, collapses closure parameter lists and generic argument lists into single units and ignores rustfmt's trailing comma, then fails with `file:line` when a constructor receives two or more top-level arguments. `verify-rust-constructor-arity.test.mjs` carries the negative fixture (the exact `Box::new(A, B)` shape) and thirteen tolerated shapes, and both run in the repository-hardening job. A tree-wide sweep with the same rubric found no other occurrence: the remaining multi-comma hits are the five legitimate two-parameter closures (`pages/admin/builder.rs:132`, `fly/web/browser_runtime.rs:111`, `leptos-ui-routing/lib.rs:159`/`:178`, `forms/leptos/inputs.rs:1469`). |
| 58 | `crates/modules/rustok-pricing/src/services/pricing.rs` + `crates/modules/rustok-commerce/tests/pricing_service_test/price_set.rs` | ECOM-TENANT-01 tail: `PricingService::get_price` and `get_variant_prices` were the two money-domain readers that accepted no `tenant_id` at all, so nothing in their call path checked the tenant — `price` rows carry no tenant column and the readers did not even resolve the variant that owns the scope. Both now take `tenant_id` and resolve the variant through the shared `ensure_variant_tenant` loader (`product_variant.tenant_id`), which `resolve_variant_price` also uses now instead of restating the rule inline; a foreign or unknown variant answers `VariantNotFound` instead of `Ok(None)`, the same answer the resolver gives, while a variant of the caller with no row in the requested currency keeps answering `Ok(None)`. The 33 existing call sites (the post-write read in `set_price_tier_with_channel` and the pricing tests) pass the tenant they already operate under, and `test_price_reads_are_tenant_scoped` locks the refusal. Verified against the pristine base: the 17 pricing-related verifiers report identical results on both trees (0 regressions). |

**Not applied (owner decision required):** ECOM-EVT-01 (payment events — ADR + composition roots),
ECOM-DUP-01 (legacy checkout removal — intentional deletion), ECOM-COMP-02 (auto-refund policy for
captured-then-failed checkouts — the manual path, the bounded retry loop and the alerting counters
now exist, the automatic refund and the outbox events do not), ECOM-ZERO-01 (zero-total checkout), ECOM-MONEY-02 (canonical money owner — ADR),
ECOM-DUP-02/ANALYTICS-01 (metric ownership + currency dimension), ECOM-REFUND-01 (refund owner).
The verification pass added one more, and the binding-guard pass resolved it: the
payment-collection binding trigger (`m20260713_000015_bind_checkout_payment_collections`) was a *second*
writer of `checkout_operations.payment_collection_id` and a cross-module business rule inside a trigger —
the same class the ADR moved to Rust for the state machine and the provider-execution guard. Its identity
validation was already replicated (and exceeded) by `validate_collection` in the payment stage, and the
binding write by `checkpoint`, so it was removable; what kept it alive was a load-bearing ordering —
`invalidate_provider_execution_admitted_by` silently skipped stamping while no collection was bound, and
the trigger guaranteed that a checkout with provider operations always had the binding before it could
park. The fence now resolves the **cart** instead of the binding and the guard is dropped by
`m20261007_000015_drop_payment_collection_binding_trigger` with the up-drops / down-restores pattern of
`m20261007_000010` (remediation row 56).
The tail pass added three more owner decisions: outbox retention for delivered events (a prune worker
would delete the event record), the dead `rustok_event_bus_*` metric family (wire it to the relay or
retire it), and the duplicated `sys_events` creation between the platform migrator and the outbox
module — see the tail-pass subsection for the details. All three were then implemented in the ideal
pass (remediation rows 35-41), including the product decision the retention item asked for: the
delivered-event window defaults to 30 days and is configurable up to ten years, while `pending` and
`failed` rows are never pruned (disabling `events.outbox_retention.enabled` restores the unbounded
delivered history).
ECOM-RECON-02 was closed by owner decision (MySQL is not deployed) and ECOM-DB-01 was fixed in the
second pass, see §"P3 / hygiene findings".

The re-verification pass added one more: the storefront line-item compatibility shims — `graphql/mutations/helpers.rs:619` `resolve_storefront_line_item_input`, `:1018` `validate_storefront_line_item_quantity` and `:1039` `validate_storefront_variant_inventory` (the last one has no reference at all outside the dead code), plus the two wrappers at `graphql/mutations/safe_helpers.rs:451` and `:508` that only forward to `typed_line_item_helpers`, and the two `#[allow(dead_code)]` in `graphql/mutations/mod.rs` that exist only to silence them — have no production caller left (the bare calls in `graphql/mutations/cart.rs` resolve to the typed module through the re-export glob), but five repository verifiers pin them *as* the implementation. Removing them is a gate re-pin, not a code deletion, and waits for the same ECOM-VERIFY-01 decision.

## Binding-guard pass (the last database business rule on the money path)

Remediation row 56. The pass closes the tail the verification pass had to leave open — the
payment-collection binding trigger — and, with it, the last database business rule of the checkout
contour.

*What the guard did.* `m20260713_000015_bind_checkout_payment_collections` installed
`bind_checkout_payment_collection()` and `payment_collections_bind_checkout_operation*` triggers that
(a) validated the collection's checkout identity against tenant, cart, order and the existing binding,
and (b) **wrote** `checkout_operations.payment_collection_id` — a column the checkout journal owns.
That is the class of object the ADR moved to Rust twice (the checkout state machine and the
provider-execution admission), and it was the second writer of the binding column.

*Why it survived the verification pass.* Both of its rules looked redundant (`validate_collection`
checks strictly more; `checkpoint` writes the binding), but the park-time invalidation read
`operation.payment_collection_id` and returned early while it was null, and the trigger was what
guaranteed a checkout with claims always had the binding before a park. The concrete reachable case
is `begin`: a new operation for a cart whose previous attempt still has in-flight claims on its own
collection — the fresh row has no binding, so the early return skipped exactly the claims the park
was supposed to invalidate.

*What replaced it.* The fence now takes the **cart**:
`PaymentProviderOperationJournal::stamp_admission_epoch_for_cart` resolves the tenant's collections of
that cart (`payment_collections.tenant_id` + `cart_id`) and stamps their non-terminal
(`pending`, `provider_error`) provider operations in the transition's own transaction. The scope is
exact rather than merely wider: a provider operation is created by the payment stage from a collection
of the checkout's cart, `ux_payment_collections_active_cart` (`m20260713_000106`) admits one active
collection per cart, and `ux_checkout_operations_active_cart` admits one live checkout per cart, so the
cart holds every claim that can still reach this checkout and no other tenant's operations are touched.
`checkpoint` absorbed the guard's remaining rule — the binding is **write-once**: the same conditional
write that stores the collection accepts an unbound operation or re-asserts the collection it already
carries and refuses to re-point it, answering a bounded `Conflict` that names the write-once binding.
Nothing in Rust or in the schema enforced that rule after the guard went away, and provider
operations, marketplace financial rows and refunds are all keyed by the column.

*The migration.* `m20261007_000015` is left untouched (an applied migration is immutable).
`m20261007_000015_drop_payment_collection_binding_trigger` (`rustok-commerce`) drops the trigger and
the PL/pgSQL function on PostgreSQL, SQLite and MySQL with one statement per `execute_unprepared`
call — the SQLite and MySQL drivers prepare a single statement at a time, so a batched text would
silently drop the rest — and its `down()` restores the original text, verified byte-identical by
extracting the three installer blocks of `m20260713_000015` and comparing them with the restored
strings. The registration carries explicit dependencies on the table owner
(`m20260713_000009_create_checkout_operations`) and on the migration it supersedes
(`m20260713_000015_bind_checkout_payment_collections`).

*Parity note, recorded rather than fixed.* The installer of `m20260713_000015` issues its two SQLite
and two MySQL `CREATE TRIGGER` statements through a single `execute_unprepared` call each, while the
drop path is per-statement. The drop path is the one that has to be exact; `down()` deliberately
re-installs the historic text verbatim so a rollback reproduces exactly the objects the original
migration would have produced. MySQL is not deployed for this contour (owner decision on
ECOM-RECON-02), so no production backend can be in the half-installed state today.

*Recorded rather than changed.* With the trigger gone, a checkout parked in the window between the
collection being created and `checkpoint` leaves that `pending` collection uncancelled: the payment
compensation answers `Ok(None)` for a null `collection_id` and the commerce wrapper treats that as
"nothing recorded". It is not a money risk — nothing can be authorized before `checkpoint` writes the
binding, since the stage pipeline refuses to advance past `payment_ready` without it — and the next
attempt on the cart adopts the collection through `find_reusable_collection_by_cart` +
`attach_order_to_collection`, which re-points the collection's metadata link at the new operation. The
alternative (a payment-side resolver plus a request field) was rejected as scope without value while
every decision-bearing reader sits past `payment_ready`, where the stage invariant guarantees the
binding. The admin collection command's response may show a null binding for such a parked operation;
that is display-only.

*Verification for this pass.* `verify-checkout-execution-admission-contract.mjs` passes with the new
pins and fails against the pristine base commit with **130** findings (115 with the revision that
predates this pass, so the fifteen new expectations are exactly the removed trigger, its restore
text, the cart-scoped signature and the write-once binding guard). The 99-gate subset built from every
verifier that mentions the touched files is identical to the pristine base apart from the four gates
this branch already owns plus the new arity gate and its self-test; `verify-docs` and `verify-adrs`
pass. Also verified in this pass: no other migration or gate still installs
`bind_checkout_payment_collection` or reads the dropped function, and the payment journal may not even
name `checkout_operations` (the verifier forbids it), so the payment half of the contract stays
binding-free.

*Also in this pass.* Remediation row 57: a compile error this review had introduced in the
`rustok-commerce` migration registration (two migrations passed to one `Box::new`) is fixed and guarded
by a new repository-wide verifier with a negative-fixture self-test, wired into the
repository-hardening job. Remaining owner decisions from the earlier passes are unchanged
(ECOM-VERIFY-01 gate re-pins, the gate-canon decision); nothing in this pass depends on them.

## Appendix A — target design for reconciliation (proposal, owner decision)

The audit fixed the *exit* from a parked checkout (ECOM-RECON-01) and then moved the whole
reconciliation contour into typed Rust code (ECOM-DB-01). This appendix records the target design it
was measured against. **Increments 0 and 2 are implemented** — the state machine, the append-only
action journal, the action registry and the RBAC split exist in `rustok-commerce` as described in
§"Remediation log" (rows 13–21), and the ownership of that state machine is recorded as an accepted
ADR (`DECISIONS/2026-10-07-checkout-operation-invariants-owned-by-rust.md`); the compensation attempt
cap with its parking path, the park/action metrics and the outbox events for park/close also shipped
(increment 1 is complete, see rows 22–26), so what remains open from this table is increments 3–4.
The admission cutover that this paragraph used to list as open is complete: the provider-execution
guard `m20260713_000016` was removed in the fourth pass (`m20261007_000013`, rows 30–32) and the
payment-collection binding guard `m20260713_000015` in the sixth pass
(`m20261007_000015_drop_payment_collection_binding_trigger`, rows 56–57), so no business rule of this
contour lives in database code any more (§"Remaining cutover: provider execution admission" in the
ADR records both). The parts below that are not implemented yet are marked as such.

### A.1 Separate the three roles now mixed into one status

`checkout_operations.status` currently carries three independent facts: how far execution got, what
the money state is, and whether a human is needed. That is why every new policy question turns into a
status-machine change. The target split:

| Concern | Owner | Today |
|---|---|---|
| Execution progress | `checkout_operations` journal (stage + terminal state) | exists |
| Money state | payment/refund owners (`payment_collections`, `refund*`, provider-operation journals) | exists, but invisible to the checkout journal |
| Operator work | **reconciliation case**: `operation_id`, tenant, reason code, money snapshot, state (`open → triaged → executing → closed / escalated`), policy version, SLA | does not exist; a single status string plays this role |

### A.2 Actions as a registry, not an endpoint

**Delivered.** The single `resolve` call was replaced by declarative actions, each with its own
preconditions, idempotency key, reversibility and permission. The implemented set is
`retry_compensation`, `attest_external`, `write_off`, `refund_full`, `refund_partial`,
`void_authorization`; `release_only` was dropped because `retry_compensation` already covers the
no-money case through the existing compensation pipeline. Every action appends to the append-only
`checkout_reconciliation_actions` row (actor, action, reason, evidence, amount, refund id, request
fingerprint, idempotency key), and closes the case only through the journal's typed transition. The
table below is the design the implementation follows:

| Action | Preconditions | Moves money |
|---|---|---|
| `retry_compensation` | stage < `payment_captured`, money not captured | no |
| `release_only` | nothing captured, only reservations/cart hold the cart | no |
| `refund_full` / `refund_partial` | captured amount − reserved ≥ requested (`PaymentRefundCreationService` already enforces this) | yes |
| `void_authorization` | collection authorized, not captured | no |
| `write_off` | explicit operator decision; two-person approval above a threshold | no (accounting only) |
| `attest_external` | operator reconciled out of band; evidence reference mandatory | no |

Every action appends to an **append-only** action journal (`checkout_reconciliation_actions`:
actor, action, reason, evidence, policy source + version, idempotency key, request hash, result) and
closes the case only when the money snapshot is consistent.

### A.3 Policy as data

Flexibility should come from configuration, never from new statuses. A versioned policy per tenant
(platform default → tenant → channel/region), expressed as ordered rules:

```
when reason = "provider_outcome_unknown"
 and stage >= payment_captured
 and amount_minor <= 100_00
 and attempt_count < 2
then refund_full via <provider>, else manual_required
```

Rules to keep it honest: the default is always `manual_required` (fail-closed, exactly today's
behaviour); automatic execution is opt-in per tenant; the policy version that produced a decision is
persisted on the case and the action row, so any resolution is reproducible; irreversible actions are
never automatic without an explicit rule and a bounded attempt count.

### A.4 Scheduler, limits, escalation

The compensation sweep today is an HTTP endpoint (`POST /admin/checkout-operations/compensation-sweep`)
with no attempt cap, and `manual_reconciliation` results re-enter the claimable set forever. Target: a
worker that claims cases with exponential backoff and `max_attempts` per action, SLA timers
(age → metric → alert → `escalated`), at-most-once semantics for money actions, at-least-once with
idempotency for the rest.

### A.5 Events and audit

Transactional outbox events (`publish_in_tx`, see `docs/architecture/event-flow-contract.md`):
**delivered** for the checkout-owned part — `checkout.operation.parked` and
`checkout.operation.reconciled` are written in the same transaction as the status change by all four
journal writers, with one bounded vocabulary shared with the park metric (ECOM-EVT-02). The remaining
design names — per-phase action events (`checkout.reconciliation_action_started|succeeded|failed`) and
`payment.*`/`refund.*` from the payment owner (ECOM-EVT-01 — today `rustok-payment` publishes
nothing, so refunds and captures are only observable by polling) — are still open. Events are the
alerting and analytics feed; they are not a substitute for the append-only action journal.

### A.6 One contract for every money journal

`ReconciliationPort` with four operations — `list`, `triage`, `execute(action, idempotency_key)`,
`attest` — implemented by checkout operations, payment provider operations, fulfillment provider
operations and return completion. The fulfillment module already proved this shape
(`provider_operation_recovery.rs`: `list_reconciliation_required`, `resolve_unknown_as_failed`,
`resolve_unknown_as_succeeded`); the checkout variant added in this change set is the second data
point. Unifying them later is cheap; inventing a fifth bespoke endpoint is not.

### A.7 RBAC and backend parity

Split permissions into read / execute-safe / execute-money / attest / configure-policy, with
two-person approval for write-offs. **Delivered for increments 0 and 2:** the read path needs
`orders:read`, safe actions `orders:manage`, money actions `orders:manage` + `payments:update`
(derived from `CheckoutReconciliationAction::required_permissions()`, not from route wiring), and
`write_off` requires a second approver different from the caller. The invariants are now enforced by
Rust on every backend (ECOM-DB-01); MySQL parity is closed as not applicable because MySQL is not
deployed (ECOM-RECON-02), leaving the partial unique index as the only MySQL-specific gap.

### A.8 Suggested increments

| # | Increment | Moves money | State |
|---|---|---|---|
| 0 | Exit from the parked state: list, close, audit fields, state machine in Rust | no | **delivered** (state machine + guards removed + list/actions) |
| 1 | Append-only action journal + outbox events for park/close + metrics/alerts + attempt cap in the sweep | no | **delivered**: the journal (increment 2), the attempt cap with parking, the park/action Prometheus counters and the `checkout.operation.parked` / `checkout.operation.reconciled` family published in the writer's transaction (ECOM-EVT-02, rows 25–26) are in. The family was added as its own `rustok-events` contract, so it no longer waits on ECOM-EVT-01 |
| 2 | Action registry: `refund_full` / `refund_partial` (idempotent, through the payment owner's refund port), `void_authorization`, `write_off`, `attest_external`, `retry_compensation` + RBAC split | yes | **delivered**; `release_only` intentionally not implemented — `retry_compensation` covers the no-money case through the existing compensation pipeline |
| 3 | Provider execution admission: level + generation owned by the journal, read by `rustok-payment`, guard dropped | no (the guard only refuses execution; it moves no money) | **delivered** (rows 28–31): the level/epoch record, `checkout.operation.admission_changed` in the writer's transaction, the payment claim gate over the narrow owner port, the bounded refusals with their metric, park-time invalidation, and the trigger removal. Payment keeps no projection by design, so the increment ships without a rebuild/resync path |
| 4 | Policy-as-data per tenant, default `manual_required` | yes | open (needs its own policy-as-data ADR; the 2026-10-07 ADR covers the ownership of the state machine only) |
| 5 | Unified `ReconciliationPort` over checkout/payment/fulfillment/return journals + event-contract doc update | — | open |
| 6 | MySQL guard parity | — | closed as not applicable (MySQL is not deployed) |

Increments 0–2 remove the operational risk and give operators a typed, audited surface; increment 3
moves the last cross-module guard out of the database, and increment 4 changes money behaviour
automatically, so it needs its own ADR and, for `payment.*`/`refund.*` events, ECOM-EVT-01.

## Verification statement

What *was* executed: the repository's Node verifiers — `npm run verify:adrs` (passes for 110
decisions, including the new ADR), `npm run verify:docs` (passes; central links and topology intact)
and a **baseline-relative sweep of all 292 source verifiers** whose names match the crates this change
set touches (`checkout`, `compensation`, `reconciliation`, `refund`, `payment`, `fulfillment`,
`pricing`, `cart`, `order`, `commerce`, `recovery`, `staged`, `telemetry`, `metric`). Each script was
run twice, once in this working tree and once in a clean export of the base commit
(`git archive a53a4dd` → `/tmp`), and the failure counts were compared. Result: every script reports
the same count on both trees except `verify-commerce-admin-checkout-operation-diagnostic-safety.mjs`,
which reports one *fewer* failure here (41 → 40, one content marker fixed by unwrapping a sentence);
the error-context verifier stays at 89 with its `mapper callsites 3 → 6` expectation updated on
purpose (ECOM-VERIFY-01). The sweep also caught two real regressions introduced by an earlier pass of
this change set and they were fixed: a raw `operation_id = %operation_id` log field and a fifth
`PaymentError` fact extraction in
`crates/modules/rustok-payment/src/checkout_execution/provider_helpers.rs`, both rejected by that
file's diagnostic-safety/encoding contracts and both now replaced by bounded fields. Everything else
below was not executed.

No `cargo check`, `cargo clippy`, `scripts/verify/verify-remediation-gate.py`, or test suite was run:
this environment has no Rust toolchain and the repository's own rules reserve test execution for the
maintainer. Every edit in this change set — the currency-scale corrections, the logging passes, the
Rust state machine, the action registry, the entity, the two migrations and the controller surface —
was verified only by reading the diff, by brace/raw-string balance checks and by mechanical
comparison of the SQL moved into `m20261007_000010_move_checkout_operation_guards_to_rust` against
`m20260713_000009`/`m20260713_000017` (the `down` text is byte-identical after whitespace
normalisation for every guard except the superseded `m20260713_000009` `checkout_operations_guard_update`
trigger, which `m20260713_000017` replaced — the migration restores the `000017` version, which is the
state that existed).

Neither migration was executed: there is no `cargo`, no `sea-orm-cli`, and no PostgreSQL/SQLite
instance in this environment, and MySQL is not a deployment target. **The maintainer must:**

1. compile (`cargo check -p rustok-commerce` plus the crates touched by the other rows);
2. run the two migrations `up` and `down` on PostgreSQL and SQLite and confirm that no checkout
   guard trigger survives `up` (`\d checkout_operations`, `SELECT * FROM pg_trigger`,
   `SELECT name FROM sqlite_master WHERE type = 'trigger' AND tbl_name = 'checkout_operations'`);
3. exercise `POST /admin/checkout-operations/{id}/actions` for every action, including a replay with
   the same `Idempotency-Key` (must return the stored row, not a second refund), a replay with a
   different body (must return `409 checkout_reconciliation_idempotency_conflict`) and two concurrent
   requests with the same key (one creates the refund and the journal row, the other must return the
   stored row);
4. run the affected crate tests, the smoke test
   (`crates/utils/rustok-migrations/tests/checkout_reconciliation_smoke.rs`) and the gatekeeper;
5. exercise the compensation sweep with an operation whose `attempt_count` is at the cap: expect
   `exhausted: 1`, a row parked with `last_error_code = checkout.compensation_attempts_exhausted`, and
   one `rustok_checkout_reconciliation_parked_total{reason="attempts_exhausted"}` sample.

Formatting: `rustfmt`/`cargo fmt --check` could not be run (no toolchain). The new code follows the
surrounding style and keeps lines within the crate's usual width, but the maintainer must run
`cargo fmt` before merging — the repository's formatting gate is not proven.

The unit tests added next to the state machine and the registry
(`checkout_state_machine_rejects_unlisted_transitions`, `manual_reconciliation_codes_park_the_operation`,
`reconciliation_actions_round_trip_and_declare_their_guards`,
`attest_external_requires_outcome_and_evidence`, `write_off_rejects_amount_and_conflicting_outcome`,
`fingerprints_differ_when_the_decision_changes`) were written but never executed.

### Third pass — park/close events in the writer's transaction (2026-10-07)

Delivered after the second pass, as increment 1 of Appendix A: the typed `checkout.operation.parked`
/ `checkout.operation.reconciled` family (ECOM-EVT-02), its in-transaction publication from the four
`CheckoutOperationJournal` writers, the unified bounded park vocabulary, the new verifier and its CI
wiring (rows 25–26).

What was executed for this pass:

- `npm run verify:docs` → passes; `npm run verify:adrs` → passes for **110** decisions (the ADR text
  was updated, not replaced); `node scripts/verify/verify-workflow-action-pins.mjs` → passes (the
  `Ecommerce Hardening` step added by this pass keeps the pinned SHAs untouched).
- A baseline-relative sweep of the **226** source verifiers whose names match this change set
  (`checkout`, `commerce`, `event`, `outbox`, `telemetry`, `metric`, `digest`) was run twice: once in
  this working tree, once in a clean detached worktree of the base commit
  (`git worktree add --detach /tmp/pristine-verify a53a4dd`). Every exit code is identical on both
  trees except two: `verify-commerce-admin-checkout-operation-diagnostic-safety.mjs` reports one
  *fewer* finding here (41 → 40) — the same baseline difference the second pass recorded — and
  `verify-event-contract-digest-admission.mjs` now **passes** here while it fails on the base tree
  (row 27). Total failing scripts: 147 here vs 148 on the base. 147 of those 226 scripts fail on both
  trees: they encode renamed
  files (`controllers/admin/fulfillments.rs` and similar) and older surface contracts, which is the
  pre-existing ECOM-VERIFY-01 class and not something this pass changed.
- Negative control for the new verifier: `RUSTOK_VERIFY_REPO_ROOT=/tmp/pristine-verify node
  scripts/verify/verify-checkout-operation-event-contract.mjs` exits 1 with 36 findings (family file
  absent, publication helpers absent, metric constant absent), so the verifier does detect the state
  it claims to forbid instead of passing trivially.
- Row 27 fix: the digest-admission verifier required the literal marker `actions/upload-artifact@v7`,
  which cannot appear in a workflow whose actions are pinned to commit SHAs, so the only source
  verifier of the digest gate could never pass (the same stale-marker class as
  `verify-api-compatibility-contract.mjs`, which is still red and is left as reported). The marker now
  accepts the pinned form (`@<40-hex sha> # v7`) and refuses the unpinned tag form; the workflow
  documents the pin with the same `# v7` comment the other workflows use. This is why the sweep above
  shows one verifier fixed rather than merely not-broken.
- Not executed, and why: `cargo check`/`cargo fmt`/cargo tests and
  `scripts/verify/verify-remediation-gate.py` (it shells out to `cargo metadata`; the sandbox has no
  toolchain) — maintainer's zone, unchanged from the second pass.

Maintainer actions specific to this pass:

1. `cargo fmt` and `cargo check -p rustok-events -p rustok-commerce` (plus the crates whose
   constructors gained the bus argument);
2. `cargo run --locked -p rustok-events --example event_contract_digests -- --write`, then confirm
   the diff only touches `registry` and the two `contract_*` digests and run
   `node scripts/verify/verify-event-contract-digest-admission.mjs`;
3. run the checkout/reconciliation tests, including
   `crates/utils/rustok-migrations/tests/checkout_reconciliation_smoke.rs` (its journal now publishes
   `checkout.operation.parked` through a real `OutboxTransport`, so the test needs the `sys_events`
   table that `SqliteTestMigrator` already creates) and
   `crates/modules/rustok-commerce/tests/checkout_service_test`;
4. `npm run verify:commerce:checkout-operation-events` and the `Ecommerce Hardening` workflow.

### Fourth pass — provider execution admission moved out of the database (2026-10-07)

The last business rule owned by the database was `payment_provider_operations_checkout_guard`
(`m20260713_000016`): a `BEFORE UPDATE OF status` trigger that reads `checkout_operations.status`
through `payment_collections.metadata` and refuses the transition into `executing`. This pass delivers
the admission contract the ADR describes ("Remaining cutover") in both halves — the checkout-owned
record and the payment claim gate — removes the trigger, and fixes two defects the port surfaced.

The guard has two problems the Rust contract fixes on the owner side (ECOM-ADM-01, ECOM-ADM-02):

1. **`completed` is not a blocked status.** The blocked set is `compensation_required`,
   `compensating`, `reconciliation_required`, `compensated` and `failed`. A provider operation may
   therefore still enter `executing` after the checkout finished *successfully* — the one terminal
   status in which no new provider execution can be legitimate. The typed level derives from the whole
   status set, so `completed` closes the admission.
2. **The guard blocks unwinding effects too.** It refuses every transition into `executing`, including
   the `cancel` and `refund` operations the compensation pipeline itself has to run while the
   operation is `compensating`. The compensation step is refused by the guard, the journal parks the
   operation as `manual_reconciliation`, and an operator has to close it by hand. The target contract
   separates *extending* effects (`authorize`, `capture`), refused while the level is not `open`, from
   *unwinding* effects (`cancel`, `refund`), which stay admitted because the compensation needs the
   provider.

Delivered in this pass:

- `checkout_operations.execution_admission` (`open` / `settling` / `closed`) and `admission_epoch`,
  added by `m20261007_000012_add_checkout_operation_admission` with the pre-existing-row backfill by
  status and the PostgreSQL/MySQL `CHECK` shape (the typed writer owns the vocabulary on SQLite, which
  cannot add a constraint to an existing table);
- the typed `CheckoutExecutionAdmission` level in `CheckoutOperationJournal`, derived from the status
  machine by `for_status`, with **all seven status writers** writing the level and the generation
  inside the same conditional update as the status. The generation moves exactly when the level moves
  (delta computed by `AdmissionWrite`), so a write that stays inside one level keeps the generation
  in-flight provider operations were admitted under, and the level is re-asserted on every write;
- `checkout.operation.admission_changed` in the `rustok-events` checkout family (bounded level
  vocabulary, `previous_admission`, `admission_epoch`, status; `admission_epoch >= 1`), published
  inside the writer's transaction by `begin`, `release_lease_with_error`, `mark_terminal` and
  `resolve_reconciliation_required`, so the generation a consumer sees is always the one the status
  belongs to;
- `begin` starts the next generation for the cart (`max(admission_epoch) + 1`), so a provider operation
  created under an earlier checkout attempt can never be admitted against a later one even when the
  payment collection is reused;
- the level and the generation are exposed by `AdminCheckoutOperationResponse`;
- the new verifier `scripts/verify/verify-checkout-execution-admission-contract.mjs`.

Payment half — delivered in the same pass, so the trigger can go:

- `payment_provider_operations.admission_epoch` (`0` = created before the contract) plus the bounded
  `admission_refusal_code` / `admission_refused_at` diagnosis columns, added by
  `m20261007_000122_add_provider_operation_admission` with the PostgreSQL/MySQL `CHECK` on the refusal
  vocabulary (SQLite keeps the typed gate as the enforcer);
- the narrow owner port `rustok_payment::CheckoutExecutionAdmissionPort`, implemented by
  `rustok-commerce` (`services/checkout_execution_admission.rs`) over a tenant-scoped read of
  `checkout_operations`. Payment keeps **no derived copy** of the level, so there is nothing to drift
  and no projection to rebuild — the failure mode that made the database trigger attractive;
- the claim gate in `PaymentProviderOperationJournal::claim_execution`: **one conditional write**
  (`status = 'executing'` only while the checkout reports `open` *and* the stored generation still
  matches the observed one), with a pre-contract row (`admission_epoch = 0`) adopted into the observed
  generation only while the level is `open` — exactly the guard's own behaviour for such rows;
- effect kinds: *extending* (`authorize`, `capture`) is fenced by the level and the generation;
  *unwinding* (`cancel`, `refund`) is admitted while `settling`/`closed` and never even reads the
  admission, so the compensation can no longer be trapped by the guard's blanket refusal (ECOM-ADM-02);
- fail-closed, observable refusals: a refused claim records
  `checkout_admission_settling|closed|unavailable|epoch_mismatch|effect_unknown`, increments
  `rustok_payment_provider_execution_admission_refused_total{operation,reason}` (both labels are
  bounded: `reason` is the refusal vocabulary, `operation` is the effect class
  `extending|unwinding|unknown` produced by `refusal_metric_operation_label`, never the raw journal
  string), logs a bounded
  diagnostic, and stays visible to the caller through `execution_admission_refusal_error`, which the
  four payment claim call sites (admin collection command, admin refund command, checkout
  compensation, checkout capture/authorize) turn into a bounded owner error instead of a silent `None`;
- park-time invalidation instead of a race: every level change stamps the new generation on the
  cart's non-terminal provider operations **inside the transition's own transaction**
  (`invalidate_provider_execution_admitted_by` → `stamp_admission_epoch_for_cart`), so a claim decided
  under the previous generation fails its own conditional write; the operations already `executing`
  keep their generation because their invocation was admitted before the park;
- `m20261007_000013_drop_provider_execution_checkout_guard` drops the trigger and its PL/pgSQL
  function with the same `up`-drops / `down`-restores pattern as `m20261007_000010`, and
  `verify-checkout-execution-admission-contract.mjs` now checks both halves (payment migration,
  entity columns, owner port, claim gate, refusal vocabulary, metric, park-time stamp, trigger
  removal, and the wiring of every production construction site of the payment execution ports).

Closed by decision in the tail pass: the level stays a bounded string in
`AdminCheckoutOperationResponse`, next to `status` and `stage`, which are exposed the same way. The
vocabulary is owned by the typed `CheckoutExecutionAdmission` enum and enforced by the
`ck_checkout_operations_execution_admission` `CHECK`; turning this one field into an OpenAPI enum would
be an API-contract change (the repo diffs every exported schema in
`verify-api-compatibility-contract.mjs`) for a payload the admin UI already receives as a bounded
label. Still open for this contour: the admission contract has to be exercised by the maintainer's
`cargo test` run (the sandbox has no toolchain). No projection or resync action is needed: payment
reads the level from its owner on every claim, so there is no copy that could drift.

What was executed for this pass:

- `node scripts/verify/verify-checkout-execution-admission-contract.mjs` → passes (0 findings) with
  both halves covered. Negative control against the base commit
  (`git worktree add --detach /tmp/pristine-verify a53a4dd`): exits 1 with **104** findings
  (migrations, entity columns, level and refusal vocabularies, event variant, publication order,
  owner port, claim gate, park-time stamp, trigger removal and the port wiring), so the verifier
  detects the state it forbids rather than passing trivially.
- 111-script checkout/payment/reconciliation subset sweep in this tree and in the pristine base
  worktree: 65 failing here vs 67 at base, and the only four deltas are improvements
  (`verify-checkout-execution-admission-contract.mjs` 1 → 0,
  `verify-checkout-operation-event-contract.mjs` 1 → 0,
  `verify-commerce-admin-checkout-operation-diagnostic-safety.mjs` 41 → 40,
  `verify-commerce-admin-checkout-operation-error-context.mjs` 90 → 89) — no script regressed.
- `npm run verify:docs`, `npm run verify:adrs` (110 decisions) and
  `node scripts/verify/verify-workflow-action-pins.mjs` → pass; the new npm script and the new
  `Ecommerce Hardening` step keep the pinned action SHAs and the ADR registry intact.
- `node scripts/verify/verify-checkout-operation-event-contract.mjs` → still passes with the family
  extended by the third event type.
- Not executed, and why: `cargo fmt` / `cargo check` / `cargo test` and
  `scripts/verify/verify-remediation-gate.py` (it shells out to `cargo metadata`; the sandbox has no
  toolchain) — maintainer's zone.

Maintainer actions specific to this pass:

1. `cargo fmt` and `cargo check -p rustok-events -p rustok-commerce -p rustok-payment -p
   rustok-telemetry` (the journal writes two more columns and publishes a third event type, the
   payment journal grew the claim gate, the owner port and the refusal metric, and two commerce test
   fixtures now set the new entity fields);
2. regenerate the event-contract digests (`cargo run --locked -p rustok-events --example
   event_contract_digests -- --write`) and then
   `node scripts/verify/verify-event-contract-digest-admission.mjs`, because the family gained
   `checkout.operation.admission_changed`;
3. run `crates/utils/rustok-migrations/tests/checkout_reconciliation_smoke.rs` (it now also asserts
   the level and the generation after the manual-reconciliation park; the smoke test builds the
   payment journal without an admission port, which is the fail-closed default for collections that
   carry no checkout link) plus `cargo test -p rustok-commerce`, `cargo test -p rustok-payment` and
   `cargo test -p rustok-events`;
4. `npm run verify:commerce:checkout-execution-admission` and the `Ecommerce Hardening` workflow.

### Tail pass — outbox claim index, cache degradation surface, admin list index (2026-10-07)

Three areas were re-checked after the admission cutover: the outbox relay path, the cache layer, and
the indexes the new admin/recovery queries read through. Two defects were fixed, one was fixed by
decision, and three remain owner decisions.

Fixed:

1. **The relay claim read had no index for the order it uses.** `OutboxRelay::claim_batch` reads
   `status = 'pending'` with the claim-liveness predicates on `claimed_at` / `next_attempt_at` and
   `ORDER BY created_at LIMIT batch_size`. `sys_events` carried
   `idx_sys_events_pending_next_attempt (status, next_attempt_at)` and `idx_sys_events_claimed_at
   (claimed_at)`; neither serves the `created_at` order, so every relay iteration (the hot loop, every
   100 ms per worker) filtered the pending set and sorted it. The owner now exposes
   `create_sys_events_claim_index` (`idx_sys_events_pending_created_at (status, created_at)`), called
   by `SysEventsMigration` for module/test schemas and by the append-only platform migration
   `m20261007_000014_add_sys_events_claim_index` for a deployed schema. The liveness columns stay
   residual predicates of the same ordered range, and an abandoned claim is an old row and therefore at
   the head of that range. Pinned by
   `scripts/verify/verify-money-path-query-indexes.mjs` (also checks that the migration is registered
   and that the relay keeps the predicate/order the index is built for).
2. **The admin checkout-operation list had no supporting index.** `CheckoutOperationJournal::
   list_by_status` (added with the reconciliation surface) selects one tenant, optionally one status,
   ordered by `updated_at DESC LIMIT n`; the table's indexes are keyed on `cart_id`, the idempotency
   scope, the compensation lease and the active-cart slot, so the operator list scanned every tenant's
   history and sorted it. `m20261007_000014_add_checkout_operation_admin_list_index` adds
   `idx_checkout_operations_tenant_status_updated (tenant_id, status, updated_at)`; the unfiltered
   variant still sorts within one tenant and is deliberately not indexed a second time.
3. **A cache build without the Redis feature could look healthy while serving memory only.** In
   `rustok-cache`, `CacheService::from_url_with_options` under `#[cfg(not(feature = "redis-cache"))]`
   discarded the requested URL, so `redis_status()` reported `url_present: false` and
   `CacheHealthReport::is_healthy()` returned `true` — a deployment that configured
   `RUSTOK_REDIS_URL`/`settings.rustok.cache.redis_url` but was built without the feature silently lost
   the cache, the cross-instance invalidation channel and the tenant-generation coordination, while
   readiness stayed green. The URL is now resolved and kept in every build, `redis_status()` reports
   `configured but not initialized` with a build-aware message, and `is_degraded()`/readiness surface
   it. The money path itself was verified to hold no cache: `rustok-cart/order/payment/pricing/
   inventory/fulfillment/commerce` do not depend on `rustok-cache`, and the only moka user
   (`rustok-core::cache_atomic`) is used by `rustok-cache`'s own fallback backend and its tests.

Remaining owner decisions (found, not applied):

- **No retention for delivered events.** `sys_events` is both the delivery queue and the durable event
  record; `status = 'dispatched'` rows are never pruned, while the claim, the readiness lag check, the
  `/api/metrics` backlog query and the DLQ list all read the same table. A bounded prune worker
  (dispatch-age window + `dispatched_at` index + metric + settings key) is the canonical fix, but it
  deletes the event record, so the retention window is a product/audit decision.
- **The `rustok_event_bus_*` metric family is produced by nothing but its own tests.**
  `EVENT_BUS_PUBLISHED_TOTAL`, `EVENT_BUS_QUEUE_DEPTH`, `EVENT_BUS_DISPATCHED_TOTAL`,
  `EVENT_BUS_PROCESSING_DURATION_SECONDS`, `EVENT_BUS_ERRORS_TOTAL`, `EVENT_BUS_LAG_SECONDS`,
  `EVENT_CONSUMER_LAGGED_TOTAL`/`_RESTARTED_TOTAL` and `EVENT_DISPATCH_LATENCY_MS` have no production
  caller outside `crates/libs/rustok-telemetry/src/metrics.rs`; the relay's own counters are exported
  through `RelayMetricsSnapshot` → `rustok_outbox_relay_*` and the outbox gauges through
  `rustok_outbox_*`, so a dashboard on the `rustok_event_bus_*` names would read zero forever. Either
  wire the family to the relay or retire it — both touch the telemetry crate's public surface and its
  tests, so it is not a tail-pass change.
- **`sys_events` is created twice.** The platform migration `m20260211_000002_create_sys_events` and
  `rustok-outbox`'s `SysEventsMigration` carry the same table and index shape (the platform copy adds
  `if_not_exists`), and both are loaded by different migrators. The receipt table already shows the
  intended split — the platform migration calls the owner's helper; the `sys_events` pair should follow
  it, otherwise the next column change has to be made twice.

All three items above were implemented in the ideal pass below (remediation rows 35-41): the retention
window is configuration with a delivered-only bounded pruner, the metric family is produced by the
bus/dispatcher/outbox runtime, and `sys_events` has one schema owner with append-only indexes.

Verification for this pass (no cargo in the sandbox, as before):

- `node scripts/verify/verify-money-path-query-indexes.mjs` → passes (0 findings); negative control
  against the base commit (`/tmp/pristine-verify`, detached at `a53a4dd`) exits 1 with **19** findings,
  so the new guard detects the state it forbids.
- Full verifier sweep (355 `scripts/verify/*` scripts matching the money-path/outbox/cache/migration
  areas, the same set in this tree and in the pristine base): **137 failing here vs 140 at base** (the
  ideal pass re-measures the same set at 137 vs 141 after extending the money-path verifier), and
  the three deltas are improvements (`verify-checkout-execution-admission-contract.mjs`,
  `verify-checkout-operation-event-contract.mjs`, `verify-money-path-query-indexes.mjs`); no script
  passes at base and fails here.
- `npm run verify:docs`, `npm run verify:adrs`, `node scripts/verify/verify-workflow-action-pins.mjs`,
  the admission/event-contract/digest gates (see the fourth pass) → pass after this pass.

Maintainer actions specific to this pass:

1. `cargo check -p rustok-outbox -p rustok-cache -p rustok-commerce -p rustok-migrations` and
   `cargo test -p rustok-outbox -p rustok-cache` (the claim index changes a migration, the cache
   change touches both `cfg` branches of `CacheService`, and the commerce index migration is new);
2. `cargo test -p rustok-cache --no-default-features` if a memory-only build is a supported profile —
   that is the branch the cache fix changes;
3. `npm run verify:commerce:money-path-query-indexes` and the `Ecommerce Hardening` workflow.

### Ideal pass — delivered-event retention, event-bus metrics, one `sys_events` owner (2026-10-07)

The tail pass ended with three items it recorded as owner decisions. Each of them had an answer that
already follows from invariants the codebase states, so they were implemented rather than left open,
together with the guards that keep them from decaying (remediation rows 35-41).

**1. Retention for delivered events is a bounded owner subsystem now.** `sys_events` is both the
delivery queue and the durable record of what was delivered, which is why the earlier passes refused
to guess a window. The pruner therefore deletes one generation only: rows that already reached the
transport (`status = 'dispatched'`) whose `dispatched_at` is non-null and older than the configured
window. `pending` rows still owe a delivery and `failed` rows are the DLQ an operator works through;
neither is read by the prune, and the verifier fails the build if the pruner even mentions those
statuses. Each run selects the oldest `batch_size` delivered rows through
`idx_sys_events_dispatched_at (status, dispatched_at)` and deletes exactly the selected ids with the
same status/age predicates, so a row that changed state between the two statements (a DLQ replay)
cannot be deleted by a stale candidate list; a full batch sets `batch_exhausted`, which is how the
table is trimmed over several short runs instead of one long delete. The window is deployment policy:
`events.outbox_retention.{enabled,retention_days,batch_size,interval_seconds}` defaults to enabled,
30 days, 500 rows and one run per hour, the batch default comes from the owner constant, and the
server rejects a window outside 1..=3650 days at boot instead of silently keeping every delivered row
forever. Operators see the pruner through `rustok_outbox_pruned_total`,
`rustok_outbox_retention_last_run_timestamp_seconds`,
`rustok_runtime_worker_state{worker="outbox_retention"}` and
`rustok_runtime_worker_failures_total{worker="outbox_retention"}`, and readiness reports the worker
only when retention is enabled.

**2. The `rustok_event_bus_*` family is produced by the runtime again.** The family looked dead
because four of its helpers had no production caller outside the telemetry crate's own tests. They
now have one, each at the place that owns the meaning: `record_event_published` on the accepted
in-process publish (`EventBus::publish_envelope`, tenant bucketed by the existing bounded FNV
helper), `record_event_dispatched` and `record_event_processing_duration` per handler invocation
inside `EventDispatcher::handle_with_retry` (the `handler` label is the static
`EventHandler::name()`, so cardinality stays bounded), `record_event_lag` through
`EventConsumerRuntime::record_publish_lag` measured from the envelope timestamp at dispatch, and
`rustok_event_bus_queue_depth{transport="outbox"}` fed by the same backlog reading that renders
`rustok_outbox_backlog_size`. The members that were already live (`record_event_error`, consumer
restarted/lagged, `rustok_event_dispatch_latency_ms`) are unchanged, and the durable write and relay
numbers stay on `rustok_outbox_*` / `rustok_outbox_relay_*`, so an event is counted once per stage
instead of twice per hop. A dashboard on the `rustok_event_bus_*` names now reads the in-process bus
and the outbox queue depth instead of zero.

**3. `sys_events` has one schema owner, and every index matches a query.** The table shape lived twice
(the platform migration `m20260211_000002_create_sys_events` and the module's `SysEventsMigration`),
so a column change had to be made twice and the platform copy silently lacked the receipt table and
the claim index. The shape now lives once, in the owner helper `create_sys_events_schema` /
`drop_sys_events_schema`: `SysEventsMigration` composes it with the claim index, the retention index
and the receipt table for module/test schemas, and the platform wrapper keeps its deployed timeline
identity but delegates — a verifier forbids DDL in that wrapper and forbids indexes in the schema
helper. The indexes followed the same rule: the claim index (`m20261007_000014`) and the retention
index (`m20261007_000015`) are append-only, and the two indexes the claim index superseded
(`(status, next_attempt_at)` and `(claimed_at)`, which no query filters on alone, so they were pure
write amplification) are dropped for deployed schemas by the append-only
`m20261007_000016_drop_sys_events_superseded_indexes`, whose `down` restores the pair. Fresh schemas
never create them.

**Planner evidence for the two outbox reads** (SQLite `EXPLAIN QUERY PLAN`, a table of 4 000 rows: a
third unclaimed `pending`, a third `pending` with a fresh claim, a third delivered 40 days ago):

- claim read with only the indexes shipped before the claim index
  (`(status, next_attempt_at)`, `(claimed_at)`): `SEARCH sys_events USING INDEX
  idx_sys_events_pending_next_attempt (status=?)` plus `USE TEMP B-TREE FOR ORDER BY` — every relay
  iteration sorted the pending generation it had just filtered;
- claim read with `idx_sys_events_pending_created_at (status, created_at)` (the tail-pass fix): one
  index search, no temp b-tree — the ordered range the claim asks for;
- retention read without an index for its order: `SEARCH … idx_sys_events_pending_created_at
  (status=?)` plus `USE TEMP B-TREE FOR ORDER BY`, i.e. a sort over the whole delivered generation per
  run;
- retention read with `idx_sys_events_dispatched_at (status, dispatched_at)`: one range
  (`status=? AND dispatched_at>? AND dispatched_at<?`), no sort.

**A dead fence found while wiring the above.** The `sys_events`/metrics work surfaced a
missing-call defect in the admission cutover: the commerce-side invalidation helper
(`invalidate_provider_execution_admitted_by`) that stamps the collection's non-terminal provider
operations with the new admission generation was defined but never called, so the generation bump
relied on a payment-side command that a parked checkout never invokes. The level write and the
invalidation now happen in one transaction with the `checkout.operation.admission_changed` event
(remediation row 42), which is what the fourth pass intended; the admission verifier's pin for that
call site was red without it and is green with it, and the unreachable body was also the kind of dead
private method a `dead_code` warning reports in the maintainer's `cargo check` — which is why the
cargo actions below matter. The pair of runs is recorded as a negative control for the fix
(removing the one call line makes the admission verifier fail with
`invalidation inside the admission publication: missing
self.invalidate_provider_execution_admitted_by(txn, operation)`).

**What this pass deliberately did not do.** Retention is not extended to
`owner_operation_receipts`: a receipt is idempotency evidence for a write, not a delivery queue, and
it needs its own window and its own migration (open owner decision, unchanged by this pass).
Retention also never touches `pending`/`failed` rows, and the platform wrapper keeps the migration
identity a deployed database already recorded (`seaql_migrations` still holds
`m20260211_000002_create_sys_events`), so an existing deployment sees only the two new append-only
migrations.

**Verification for this pass:**

- `node scripts/verify/verify-money-path-query-indexes.mjs` → pass; the same verifier against the base
  commit (`/tmp/pristine-verify`, detached at `a53a4dd`) exits 1 with **46** findings;
- `node scripts/verify/verify-outbox-retention-and-event-metrics.mjs` (new) → pass; against the base
  commit it exits 1 with **51** findings;
- `node scripts/verify/verify-checkout-execution-admission-contract.mjs` → pass after the
  invalidation repair, and exits 1 with
  `invalidation inside the admission publication: missing
  self.invalidate_provider_execution_admitted_by(txn, operation)` when that one call line is removed
  (the negative control for remediation row 42);
- the admission, event-contract and digest-admission gates, `npm run verify:docs`, `npm run verify:adrs`
  and `node scripts/verify/verify-workflow-action-pins.mjs` → pass;
- the 355-script sweep over the money-path/outbox/cache/migration set: **137 failing in this tree vs
  141 at base**, the four deltas are improvements (`verify-checkout-execution-admission-contract.mjs`,
  `verify-checkout-operation-event-contract.mjs`, `verify-event-contract-digest-admission.mjs`,
  `verify-money-path-query-indexes.mjs`) and no script passes at base and fails here;
- not executed, and why: `cargo fmt` / `cargo check` / `cargo test` (no Rust toolchain in this
  sandbox); the SQLite plan measurements above come from a purpose-built harness against the exact
  predicates and index shapes, not from the crate's test suite.

Maintainer actions specific to this pass:

0. regenerate the event-contract release artifact **before** running the test suite: the three new
   `checkout.operation.*` schemas change the canonical registry digest, so
   `cargo run --locked -p rustok-events --example event_contract_digests -- --write` must run once and
   `published_event_contract_matches_committed_release_artifact` fails until it does (see
   `crates/libs/rustok-events/docs/event-contract-digest-admission.md`; the workflow is the manual
   `Event contract digest admission` one, so nothing else is blocked by it);
1. `cargo fmt` and `cargo check -p rustok-outbox -p rustok-core -p rustok-telemetry -p
   rustok-migrations -p rustok-server` (the outbox crate gained a `rustok-telemetry` dependency,
   `rustok-core` records four new metric call sites, `rustok-migrations` gained two migrations, and the
   server gained the retention worker, its settings and its readiness/metrics rendering);
2. `cargo test -p rustok-outbox -p rustok-core -p rustok-server` — the retention prune invariants, the
   supervisor shutdown/failure tests and the dispatcher metric call sites are new tests in those
   crates;
3. run `m20261007_000015_add_sys_events_retention_index` and
   `m20261007_000016_drop_sys_events_superseded_indexes` against a scratch database and confirm the
   rollback restores the superseded pair;
4. `npm run verify:commerce:money-path-query-indexes`, `npm run verify:outbox:retention-event-metrics`
   and the `Ecommerce Hardening` workflow.

### Verification pass — the review of the review (2026-10-07)

The ideal pass closed the three owner decisions; this pass re-read every file that pass touched, on the
assumption that a change nobody has compiled yet contains at least one mistake. It did: eleven defects
(remediation rows 43-53) plus one finding that turns into an increment rather than a patch. Listed by
class:

- **compile errors**: two `E0063`s — the server `EventSettings::default()` and a test's `EventRuntime`
  literal were never extended with the new `outbox_retention` field (rows 43-44) — and one test literal
  of `provider_operation::ActiveModel` in the migration suite, which missed the three admission columns;
  the last one would have failed the test target only, which is exactly why the sweep `--all-targets`
  exists;
- **`-D warnings` material**: the retention handle's `instance_id()` had no caller at all, and the
  reconciliation service kept an `ActiveModelTrait` import that the moved insert made unused (row 44,
  49);
- **tests that asserted the replaced behaviour**: the reconciliation smoke test expected the removed
  trigger's error text and never bound the collection, so it failed on two assertions (row 45);
- **tests that could not observe what they asserted**: the relay/retention test would have exercised the
  retention worker against a database without `sys_events` (row 51), and a migration-suite fixture was
  missing the three admission columns the new contract adds (row 51);
- **contract drift**: an unbounded metric label (row 46), a validation rule the request contract claimed
  but did not implement (row 47), and a bounded vocabulary written twice — `as_str` and `parse` could
  disagree (row 50);
- **a durability hole on the money path**: the decision row was appended *after* the operation was
  closed, so a crash between the two writes left a closed operation with no decision record and no way
  to record one; the close and the record are now one transaction (row 49);
- **a pre-existing suppression on the money path**: a refund-provider struct field had no reader and was
  silenced with `#[allow(dead_code)]`, the pattern AGENTS.md §14 and the repository's own remediation
  gate forbid (row 52);
- **an undocumented surface**: the metric families this work registered were not in the operator guide
  that documents every other family (row 53).

What was checked, and how, without a Rust toolchain in the sandbox:

- **Every struct that gained a field** was diffed field-by-field against every literal construction of
  it in the workspace, which is how the two `E0063`s surfaced (a `Default` impl and a test literal,
  both invisible to the compiler's users until `cargo check` runs);
- **every constructor and journal signature that changed** (`CheckoutOperationJournal::new`,
  `InProcessCheckoutPaymentExecutionPort::{new,with_provider_registry}`,
  `in_process_checkout_payment_execution_port`, `CheckoutPaymentStageExecutor::new`,
  `PaymentProviderOperationJournal::new`) was matched against *all* call sites, production and test:
  the arities agree;
- **every DTO field the new code reads** (`PaymentCollectionResponse::{captured_amount,refunded_amount,
  currency_code}`, `CreateAdminRefundRequest`, `CreateRefundInput`, `PortError::{kind,code,new,
  validation,invariant_violation}`, `EventEnvelope::timestamp`) exists with the type the call site
  needs;
- **migration fidelity**: the guard text restored by `down()` in
  `m20261007_000010_move_checkout_operation_guards_to_rust` and
  `m20261007_000013_drop_provider_execution_checkout_guard` was extracted statement-by-statement and
  compared with the migrations that installed those objects (`m20260713_000009`,
  `m20260713_000017`, `m20260713_000016`): every trigger and function body is **byte-identical**
  after whitespace normalisation, and the index names dropped by
  `m20261007_000016_drop_sys_events_superseded_indexes` match the pair the platform wrapper used to
  create;
- **registration order**: the three platform migrations live in the same `Migrator::migrations` list
  as the wrapper that creates the table, and the list's name sort puts `m20260211_000002` first, so a
  fresh database never creates an index before its table;
- **dead code and forbidden patterns**: an unused-import/unused-private-symbol scan over all changed
  Rust files, plus a scan for `allow(dead_code)`, `allow(unused…)`, `todo!()`, `unimplemented!()`,
  `dbg!`, `print!`/`println!` families. Two hits were actionable and are fixed: the `#[allow(dead_code)]`
  on the dead refund-result field (row 52) and an `ActiveModelTrait` import the moved reconciliation
  insert left unused (row 49). The remaining hits are pre-existing and untouched by this work: an
  `eprintln!` in `rustok-cache` test code, the migration runner's `println!` progress output, and two
  unrelated `#[allow(dead_code)]` in `apps/server` outside the checkout contour;
- **structural literals vs definitions**: every struct whose fields changed was compared against every
  literal construction of it in the workspace — the check that found the missed `EventRuntime` and
  `provider_operation::ActiveModel` fields;
- **migration registration and ordering**: all five commerce migrations and the three platform ones are
  in the module list, the `Migrator::migrations` list and the dependency descriptors, and the
  date-leading names put `m20260713_000016/000017` before `m20261007_000010`, so the guards exist when
  the cutover drops them (and `down()` restores text that was compared statement-by-statement with the
  installing migrations);
- **`cargo fmt`-sensitive edits**: the import blocks this pass touched were re-checked against the
  file-local ordering (CI runs `cargo fmt --all -- --check`);
- **the four Node gates** plus the eight single gates and a fresh 404-script sweep (the money-path
  subset plus every verifier whose name matches the checkout/payment/outbox vocabulary) were re-run
  against a pristine worktree at `a53a4dd`: 204 failing in this tree vs 209 at base, the five extra
  base failures being exactly the five gates this work adds, **zero regressions**, and the verifiers
  gained pins for the bounded refusal label and the in-transaction decision append — both were checked
  with a negative control (mutating the code makes the gate exit non-zero).

Three items this pass deliberately leaves to the owner rather than patching:

1. the payment-collection binding trigger (owner decision above) — its removal is mechanical, but it
   would expose a silent skip in the admission invalidation until the binding moves earlier;
2. four pre-existing money-path gates that are red at `a53a4dd` and fail identically here, for reasons
   that are *not* code defects: `verify-commerce-admin-refund-owner-command-cutover` pins method
   chains as single-line literals (`context.require_policy(PortCallPolicy::write())`,
   `self.provider_registry.execute_refund`) that formatting has since split across lines;
   `verify-commerce-admin-checkout-operation-error-context`,
   `verify-commerce-staged-checkout-payment-retry-disposition` and
   `verify-commerce-staged-checkout-fulfillment-retry-disposition` pin a richer error-policy layer and
   test fixtures that no longer exist in the tree (the compensation error enum they describe lives in
   an unreferenced file; the compiled one is `checkout_compensation_owner_ports.rs` through the
   `checkout_compensation_error_safe.rs` `#[path]` wrapper). Repairing that layer or re-pinning those
   gates is an owner decision, not a review fix;
3. the event-contract digest artifact: it is produced
by a Rust generator (`cargo run --locked -p rustok-events --example event_contract_digests -- --write`)
and the canonical digests include `schemars` output, which cannot be reproduced by hand without
risking a wrong release artifact — the honest move is the exact command as maintainer action 0 rather
than a hand-edited digest. Everything else this pass found is fixed in the tree.

**Re-verification pass.** A second read of the same tree tried to finish the storefront line-item cleanup and re-ran the 33 verifiers that read the files this work touches. The cleanup was withdrawn: the five verifiers listed in the not-applied note pin the legacy bodies as the implementation, and three gates that are already red at the base commit still assert the shims' shape, so deleting them would need those pins re-pointed at `typed_line_item_helpers` first. The two changes this pass did land (rows 54-55) leave the 33-verifier subset byte-identical to the pristine `a53a4dd` worktree: 11 passing and 22 failing on both sides, with the same sets.

## Appendix B — prior art: Medusa v2 (comparison, 2026-10-07)

The reconciliation contour was compared against Medusa v2 (`medusajs/medusa`, `develop`, read
2026-10-07), because it solves the same problem — a cart completion that fails after money moved —
with a generic workflow engine instead of a domain state machine. Facts, from the documentation and
the source:

- **The workflow engine is the saga.** `createStep(name, invoke, compensate)` with
  `new StepResponse(output, compensateInput)`; when a step fails, the compensations of the completed
  steps run in reverse order. Executions are persisted (workflow-engine module:
  `workflow_id`/`transaction_id`/`run_id`/`state`/`execution`/`retention_time`, with indexes that
  collect `done|failed|reverted` executions once `retention_time` passes); step-level idempotency keys
  are `{action: invoke|compensate, transactionId, stepId, workflowId}`; retries are configured per step
  (`maxRetries`, `retryInterval`, `autoRetry: false` → the step waits for a manual `retryStep`).
- **Cart completion is a saga with one money compensation.** `completeCartWorkflow` (`store: true`,
  `idempotent: false`, `retentionTime: THREE_DAYS`) declares `compensatePaymentIfNeededStep` before the
  order steps so that its compensation runs last; that compensation refunds a captured payment through
  `refundPaymentAndRecreatePaymentSessionWorkflow`, wrapped in `try/catch` + `logger.error`, and it
  skips the refund (logging a warning) when another completion attempt already created an order for the
  cart.
- **Ambiguity is resolved by webhooks, not by a journal.** `processPaymentWorkflow` consumes a provider
  `WebhookActionResult` (authorized/captured/canceled/failed/…) and converges the session, then
  completes the cart after payment. The payment module has no persisted "provider outcome unknown"
  record: its models are account-holder, capture, payment-collection, payment-provider, payment-session,
  payment, refund, refund-reason.
- **Human-needed states are flags, not machines.** `OrderStatus.REQUIRES_ACTION` is a single
  order-level flag, alongside `PaymentCollectionStatus`
  (`not_paid|awaiting|authorized|partially_authorized|partially_captured|completed|canceled|failed`) and
  `PaymentSessionStatus` (`pending|requires_more|pending_authorization|authorized|captured|error|canceled`).
- **Concurrency is a distributed lock plus versioning.** `acquireLockStep` (locking module with
  in-memory/Redis/Postgres providers) guards completion, and `Order.version` with
  `order_change`/`order_change_action` and `order.transaction` rows (`reference: capture|refund`) form
  the per-order money ledger.
- **No database business rules, and no HTTP idempotency keys.** Migrations are declarative; the only
  idempotency in v2 is the workflow-level one (the v1 `idempotency_key` table and middleware were not
  carried over), and there is no transactional outbox (the only `outbox` in the repository is the
  telemetry batching store).

What the comparison says about this change set:

- The extra artifacts here — the append-only decision journal, per-action RBAC, the request
  fingerprint, the permanent `checkout_operations` history, `publish_in_tx` events, and tenant scoping
  on every query — are not accidental complexity: each replaces something Medusa delegates to the
  engine, to the PSP webhook, or to an operator, and each is required by this repository's rules
  (permanent money audit, tenant isolation, transactional events).
- Worth borrowing, tracked as increment 3–4 candidates: per-step retry scheduling
  (`retryInterval`/`maxRetries`) instead of a worker plus lease plus attempt counter; one explicit
  "temporary failure" state with a manual retry step (here `retryable_error` plus
  `request_compensation_retry`); a single order-level `requires_action`-style flag as the only
  human-needed state, instead of adding further statuses; and reading the checkout admission through a
  port before the provider claim (the simpler variant of the target cutover in the ADR) if the epoch
  column turns out to be more than this deployment needs.
- Must not be borrowed: best-effort compensation that only logs the failure (that is ECOM-COMP-01/02
  here), garbage-collected workflow logs as the audit trail, and the absence of a transactional event
  contract.
