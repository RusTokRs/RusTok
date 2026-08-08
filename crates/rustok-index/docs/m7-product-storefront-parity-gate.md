# M7 Product Storefront Index parity gate

Status: `serving_budget_policy_source_complete_timeout_enforcement_pending`.

## Current boundary

Mounted Storefront remains owner-native and continues to execute
`CatalogService::list_published_products_with_query`. No Index traffic switch is part of this state.

Current-key Storefront core/EAV/collation PostgreSQL packets and retained Product packets remain source-only;
maintainer execution/admission is not claimed.

## Request-shape policy — source complete

- trusted non-empty public channel slug + non-nil UUID is shadow-eligible;
- channel-less requests remain typed owner-native because key `4` cannot distinguish unrestricted metadata
  from restricted membership that resolves to every current Channel;
- owner-valid offsets through `10_000` are shadow-eligible;
- deeper owner-valid pages remain typed owner-native;
- no visibility sentinel, page clamp, cursor rewrite or Product key-5 approximation is used.

## Product public projection and tags — source complete

Raw localized Index results remain Product-neutral. `public_projected` maps no-requested/fallback `title` and
`handle` nulls to Product public placeholders only after the raw page is fixed.

`ProductStorefrontTagReadPort` performs bounded post-page Product tag hydration keyed by already-selected
Product IDs, preserving Taxonomy requested->fallback/canonical-key semantics and legacy normalized
`metadata.tags` fallback. Embedded Product runtime selects the capability; external profiles do not receive an
implicit embedded fallback.

Raw `projected`, `public_projected` and `tag_hydration` remain separate results. Tag/public failures cannot
replace the authoritative owner result or change identity/order/count/page/cursor evidence.

## Post-owner serving-budget policy — source complete

`PortContext.deadline_ms` carries the original duration budget; it is not a decreasing remaining deadline. A
future serving router must supply a host-measured `remaining_ms` at the handoff after authoritative Product
owner success.

`ProductStorefrontIndexServingBudget` contains host-selected positive Index-execution and Product-tag-hydration
phase budgets plus a safety margin. The source constructor checked-adds the phases and rejects zero required
phases/overflow. This slice deliberately does **not** hard-code an unevidenced production SLO value.

`classify_product_storefront_index_serving_budget` returns owner-native decisions for:

- absent/zero original request deadline;
- absent configured budget policy;
- absent host-measured remaining budget;
- remaining budget greater than the original deadline (inconsistent observation);
- unavailable Product tag-hydration capability;
- remaining budget below the checked required Index + tag + safety budget.

Only an internally consistent observation with enough remaining time returns `Eligible` and carries the phase
budgets forward.

This is a **classification policy**, not runtime timeout enforcement. It does not start timers, execute Index,
call tag hydration or alter the current non-serving shadow executor. Mounted Storefront does not reference the
policy.

## Search/collation/EAV source state

Product owns the 1022-byte effective Storefront title-search bound compatible with generic 1024-byte
`TextLike`. The retained collation packet observes real owner/default `LIKE` against Index `COLLATE "C"` and
remains execution/admission pending.

Product-owned EAV resolution still supplies neutral typed term expressions to the shadow builder; missing
option identities remain bind-free `Never`. The raw core/EAV PostgreSQL packets remain current-key source
evidence.

## Remaining fail-closed parity/evidence gates

1. Add non-serving execution that actually enforces an admitted Index phase timeout and Product tag-hydration
   phase timeout while preserving the successful owner result.
2. Maintainer execution/review of Storefront core/EAV/collation and actualized retained Product packets.
3. Collation admission per deployment: any owner/default-vs-`C` mismatch keeps eligible Index cutover closed.
4. Retain serving-budget/timeout latency evidence before any mounted traffic switch.
5. Stale locale/readiness/admission/restart cases still require maintainer-executed retained evidence.
6. Any future serving router must preserve typed channel-less and deep-page owner-native branches.

## Next source slice

Add a **non-serving budgeted execution adapter** that accepts only an `Eligible` budget decision and applies the
admitted Index and Product-tag phase timeouts. Timeout/unavailable/error results must remain separate from the
already-successful Product owner result. Do not mount that adapter into Storefront traffic.

## Source guards

- `verify-index-product-storefront-channel-scope-policy.mjs` locks channel-less owner-native policy;
- `verify-index-product-storefront-deep-page-policy.mjs` locks deep-page owner-native policy;
- `verify-index-product-storefront-public-projection.mjs` locks raw/public placeholder separation;
- `verify-index-product-storefront-tag-hydration.mjs` locks bounded Product-owned tag hydration;
- `verify-index-product-storefront-serving-budget-policy.mjs` locks the host-measured remaining-budget contract
  and keeps policy distinct from timeout enforcement/serving;
- `verify-index-product-storefront-shadow-executor.mjs` locks current owner-first evidence execution;
- current Storefront equivalence/EAV/collation/key-4 guards remain retained;
- `verify-index-product-storefront-parity-gate.mjs` keeps mounted Storefront owner-native.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
