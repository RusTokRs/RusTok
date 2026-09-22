# M7 Product Storefront deep-page policy

Status: `source_complete_owner_execution_policy_retained`.

## Owner boundary

The modern Product Storefront owner validates `page >= 1` and `1 <= per_page <= 48`, then executes the
requested offset. It does not impose the generic Index offset ceiling.

The generic Product Storefront Index shadow builder remains bounded to offset `10_000`. That bound is an Index
query safety contract; it is not an owner Product validation rule and must not silently narrow owner-visible
Storefront behavior.

## Current policy

`ProductStorefrontIndexShadowExecutor` classifies owner-valid pagination after authoritative owner success and
before Product EAV/schema-read work:

- computed offset `<= 10_000` => `ShadowEligible { offset }`;
- computed offset `> 10_000` => `OwnerNativeDeepPage { offset }`;
- invalid page/per-page or arithmetic overflow => the existing invalid-pagination query-build error.

For an owner-native deep page, the authoritative Product owner list has already completed successfully. The
projected side returns typed `DeepPageOwnerNative { offset }`; no Index page is fabricated and the successful
owner result remains authoritative.

The policy does not clamp page or offset, rewrite the request to cursor pagination, or reinterpret an
owner-valid deep page as invalid input. The pure shadow builder still retains `OffsetTooDeep` as a defensive
fail-closed boundary when called directly.

## Evidence and serving boundary

This is a request-shape policy decision, not proof that deep pages execute acceptably at serving latency. The
mounted Storefront remains owner-native. A future serving router may send only eligible shallow requests to an
admitted Index path; deep pages must continue through the Product owner unless a later generic pagination
contract preserves the owner semantics exactly.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
