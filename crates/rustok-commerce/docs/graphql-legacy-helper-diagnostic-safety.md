# GraphQL legacy helper diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens `legacy_graphql_error` in the Commerce GraphQL storefront cart helper facade.

The mapper wraps failures from five compatibility helper calls. Its public message, code, and retryability are already supplied as stable static policy by each caller. Before this change, the diagnostic event emitted the complete `async_graphql::Error`, raw tenant UUID, and optional raw resource UUID.

## Bounded projection

The mapper now projects request identity before the diagnostic event:

- tenant UUID becomes `nil` / `non_nil`;
- optional resource UUID becomes `absent` / `present_nil` / `present_non_nil`.

The original `async_graphql::Error` is consumed by `StorefrontLegacyGraphqlDiagnosticError::from`. The resulting zero-sized diagnostic token has a custom `Debug` implementation that always emits `redacted`. The original GraphQL payload is therefore unavailable to the event.

The event continues to retain only the static Commerce boundary owner, operation, `legacy_graphql_error` kind, public code, public retryability, shared boundary, static event message, and the two closed UUID shapes.

## Preserved GraphQL behavior

This work does not change:

- `enrich_storefront_cart` delegation or its `CART_ENRICHMENT_UNAVAILABLE` envelope;
- `validate_selected_shipping_option` delegation or its `SHIPPING_OPTION_INVALID` envelope;
- compatibility line-item resolution delegation or its existing selected public envelope;
- `reprice_storefront_cart_line_items` delegation or its `CART_REPRICE_FAILED` envelope;
- compatibility quantity validation delegation or its existing selected public envelope;
- error-level diagnostic severity;
- `public_graphql_error` construction;
- the previously hardened customer and Cart/Pricing port mappers;
- private compatibility helper routing.

The compatibility line-item callers still classify legacy error debug text before calling this mapper. Removing that string classification belongs to the separate typed line-item/legacy compatibility cleanup and is not claimed here.

## Verifier correction

The broad cart-helper verifier previously required raw resource UUID logging and still expected the pre-hardening inline Cart/Pricing source-owner expression. It now requires:

- UUID shape helpers;
- consuming conversion of the GraphQL error;
- redacted diagnostic output;
- shape projection before conversion and event emission;
- absence of raw tenant/resource diagnostics;
- the current precomputed Cart/Pricing source-owner contract.

## Remaining helper work

Still open:

- typed line-item diagnostics and remaining compatibility string classification;
- storefront shared and cart-shipping mappers;
- tax, promotion, native transport, and remaining owner-adapter cleanup;
- mounted execution, compile, and runtime evidence.

## Evidence

- `crates/rustok-commerce/contracts/evidence/graphql-legacy-helper-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-graphql-cart-helper-error-safety.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, mounted GraphQL scenarios, workflows, or CI were run. No compile, runtime, FFA, or FBA status is promoted.
