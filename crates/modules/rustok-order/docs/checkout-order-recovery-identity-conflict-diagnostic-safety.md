# Checkout order recovery identity-conflict diagnostic safety

Status: **source-ready / unvalidated**

This source slice closes the diagnostic gap in `validate_identity` within the Order checkout recovery adapter.

## Recheck

The merged owner-error, admission, and missing-identity read slices remain present on `main`. The identity-conflict event still recorded the complete request tenant and channel plus request-owned and owner-owned checkout, cart, payment, shipping, order, and tenant UUID values.

## Source change

`validate_identity` now keeps the five component equality checks explicit and delegates failures to `log_checkout_order_recovery_identity_conflict`.

The event retains correlation, the stable owner operation/code/boundary, bounded context facts, UUID presence/non-nil shape, identity hash presence/length shape, the five component match results, aggregate `base_matches`, and owner/legacy hash-match results.

It no longer records any tenant, actor, channel, request UUID, identity UUID, or hash value.

## Preserved behavior

The accepted identity condition is unchanged: all five base identity comparisons must pass and either the owner hashes or legacy hashes must match. Identity read/adoption order, recovery flow, error severity, and the public conflict envelope are unchanged.

The preserved envelope remains `order.checkout_request_conflict`, conflict, non-retryable, with message `checkout operation is already bound to a different completion request`.

## Deliberately open

This slice does not change hash normalization/serde diagnostics, cancelled/unknown lifecycle diagnostics, admission, read-not-found diagnostics, or the owner-error mapper. Commerce orchestration and Order/Commerce FFA/FBA status are unchanged. The master ecommerce correlation-safe mapper-cleanup item remains open.

## Validation disclosure

No tests, Node verifiers, Cargo commands, formatting, workflows, or CI were executed. The accompanying verifier is retained as a source contract and was not run. No compile or runtime status is promoted.
