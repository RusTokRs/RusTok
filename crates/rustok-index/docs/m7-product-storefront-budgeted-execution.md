# M7 Product Storefront budgeted execution

Status: `source_complete_timeout_evidence_execution_pending`.

## Post-owner only

`ProductStorefrontIndexBudgetedProjectionExecutor` receives an already-successful authoritative Product
Storefront page. It does not call `list_filtered_published_products` and cannot replace the owner result.

The adapter starts only with `ProductStorefrontIndexServingBudgetDecision::Eligible`. Any owner-native budget
decision is rejected before the first timeout or Index operation.

## Bounded phases

The admitted Index phase budget is enforced twice:

- copied into the projected `PortContext.deadline_ms` so Product schema/EAV owner capabilities see the same
  bounded phase contract;
- enforced externally with `tokio::time::timeout` around raw `execute_projected`.

Only a successful raw page reaches Product public placeholder projection and Product tag hydration.

The admitted tag phase budget is likewise copied into the Product tag `PortContext.deadline_ms` and enforced
with an outer `tokio::time::timeout` around Product-owned `hydrate_projected_tags`.

The safety margin is retained in the result for evidence/telemetry but is not spent as another phase.

## Failure separation

The result retains:

- authoritative owner page;
- raw projected page or Index-phase timeout/error;
- optional public title/handle projection;
- optional Product tag hydration or tag-phase timeout/error;
- raw identity/count/page comparison;
- admitted phase/safety budgets.

A raw Index failure skips public projection and tag hydration. A tag failure/timeout leaves the successful raw
and public pages intact. None of these outcomes changes the authoritative Product owner result.

## Retained deterministic timeout evidence — source complete

`ProductStorefrontIndexProjectionPhases` is a crate-private seam around the two post-owner phases. Production
implements it with `ProductStorefrontIndexShadowExecutor`; the budgeted executor stores only this neutral phase
capability. The test-only `from_phases` constructor allows retained evidence to exercise the **same timeout
wrapper** without manufacturing PostgreSQL or `SharedIndexQueryRuntime` setup.

`storefront_budgeted_execution_tests.rs` retains storage-free scenarios for:

- non-eligible budget decisions starting zero projected/tag calls;
- a never-completing projected phase timing out while preserving the authoritative owner page;
- raw projected errors skipping public/tag enrichment;
- a never-completing Product tag phase timing out while raw/public pages and comparison remain intact;
- exact narrowed `PortContext.deadline_ms` values reaching both phase boundaries;
- an eligible fast path retaining Product identity/exact-count/page semantics, public placeholders and Product
  tag projection.

The timeout cases use `std::future::pending` rather than scheduler-sensitive sleeps inside the fake phases. The
outer production `tokio::time::timeout` remains the cancellation boundary under evidence.

## Separation from serving and admission

The existing owner-first shadow executor remains the unbudgeted evidence path and production implementation of
the post-owner phase seam. Mounted Storefront does not call the budgeted adapter.

The retained packet is **source-only** until a maintainer executes it. This source state does not establish a
passing timeout packet, acceptable production latency, external-provider cancellation/deadline propagation or
serving readiness. Those remain admission gates before any traffic switch.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
