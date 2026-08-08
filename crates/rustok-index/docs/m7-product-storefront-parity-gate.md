# M7 Product Storefront Index parity gate

Status: `budgeted_timeout_evidence_source_complete_execution_pending`.

## Current boundary

Mounted Storefront remains owner-native and continues to execute
`CatalogService::list_published_products_with_query`. No Index traffic switch is part of this state.

Current-key Storefront core/EAV/collation PostgreSQL packets and retained Product packets remain source-only;
maintainer execution/admission is not claimed.

## Request-shape policy — source complete

- trusted non-empty public channel slug + non-nil UUID is Index-eligible;
- channel-less requests remain typed owner-native on Product key `4`;
- owner-valid offsets through `10_000` are Index-eligible;
- deeper valid pages remain typed owner-native;
- no visibility sentinel, page clamp, cursor rewrite or Product key-5 approximation is used.

## Post-page Product projection — source complete

Raw `projected` remains the generic Index page used for identity/order/count/page evidence.
`public_projected` derives Product title/handle placeholders only after raw page completion.
`tag_hydration` is Product-owned, bounded to the selected page identities, and preserves Taxonomy
requested->fallback/canonical-key semantics plus legacy `metadata.tags` fallback.

These layers remain separate; post-page errors cannot replace the authoritative owner result or mutate the raw
Index page.

## Serving-budget policy and timeout enforcement — source complete

`PortContext.deadline_ms` is the original duration budget, not remaining time. The eligibility classifier
therefore requires a host-measured post-owner `remaining_ms`, a configured positive Index/tag phase budget,
safety margin and selected tag capability. Missing/inconsistent/insufficient budget remains owner-native.

`ProductStorefrontIndexBudgetedProjectionExecutor` is a separate **post-owner, non-serving** adapter. It accepts
the already-successful `StorefrontProductList` and only an `Eligible` decision; it does not repeat the owner
read.

For an eligible handoff it:

1. sets projected phase `PortContext.deadline_ms` to the admitted Index budget;
2. applies an outer `tokio::time::timeout` to raw projected Index/EAV execution;
3. applies Product public placeholders only after a successful raw page;
4. sets Product tag phase `PortContext.deadline_ms` to the admitted tag budget;
5. applies an outer `tokio::time::timeout` to Product-owned tag hydration;
6. keeps timeout/error outcomes separate while preserving the authoritative owner page.

The ordinary `ProductStorefrontIndexShadowExecutor::execute` remains the unbudgeted owner-first evidence path.
It implements the crate-private `ProductStorefrontIndexProjectionPhases` seam consumed by the budgeted adapter.
Mounted Storefront references neither budget policy nor budgeted execution.

## Deterministic timeout evidence — source complete, execution pending

The retained storage-free packet exercises the real budgeted executor through fake implementations of only the
post-owner phase seam. It covers:

- non-eligible decisions starting no projected/tag work;
- projected timeout preserving the authoritative owner result and skipping enrichment;
- projected error skipping public/tag enrichment;
- tag timeout preserving successful raw/public pages and comparison;
- admitted phase `deadline_ms` values reaching both phase boundaries;
- a fast eligible path preserving Product identity/exact-count/page semantics, public placeholders and Product
  tag projection.

Never-completing phases use `std::future::pending`; timeout cancellation is still performed by the production
`tokio::time::timeout` wrapper. The packet has not been executed by the implementation agent and therefore is
not admitted runtime/latency evidence yet.

## Search/collation/EAV source state

Product owns the 1022-byte effective Storefront title-search bound compatible with generic 1024-byte
`TextLike`. The retained collation packet observes actual owner/default `LIKE` against Index `COLLATE "C"` and
remains execution/admission pending.

Product-owned EAV resolution supplies neutral typed term expressions; missing option identities remain
bind-free `Never`. Current Product Index remains one 15-field schema on routing key `4`.

## Remaining fail-closed parity/evidence gates

1. Maintainer-execute/admit the deterministic budgeted timeout packet and record acceptable latency/cancellation
   behavior for the selected runtime profile.
2. Maintainer execution/review of Storefront core/EAV/collation and actualized retained Product packets.
3. Collation admission per deployment: any owner/default-vs-`C` mismatch keeps eligible cutover closed.
4. Stale locale/readiness/admission/restart evidence remains maintainer-run.
5. Stage/rebuild/promote current Product key `4` and admit replacement evidence.
6. Any serving router must preserve channel-less/deep-page owner-native branches and traffic switch remains last.

## Next source boundary

Do not move mounted Storefront traffic from source inspection alone. Further serving composition depends on
maintainer execution/admission of timeout, Storefront parity/collation, stale-readiness and replacement evidence.
Independent source work may continue only on one of those retained evidence boundaries without weakening this
gate.

## Source guards

- `verify-index-product-storefront-serving-budget-policy.mjs` locks host-measured budget classification and
  separation from enforcement;
- `verify-index-product-storefront-budgeted-execution.mjs` locks post-owner phase timeout enforcement;
- `verify-index-product-storefront-budgeted-execution-evidence.mjs` locks the deterministic retained timeout
  packet and test-only phase seam;
- `verify-index-product-storefront-shadow-executor.mjs` keeps the ordinary evidence executor separate;
- request-shape, public-projection, tag-hydration, collation and key-4 guards remain retained;
- `verify-index-product-storefront-parity-gate.mjs` keeps mounted Storefront owner-native.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
