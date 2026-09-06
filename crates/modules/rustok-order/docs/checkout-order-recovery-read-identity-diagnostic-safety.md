# Checkout order recovery read diagnostic safety

Status: **source-ready / unvalidated**

This source slice closes the diagnostic gap in the missing checkout identity path of `CheckoutOrderRecoveryAdapter::read_checkout_order`.

## Recheck

The previously merged checkout recovery owner-error and admission slices remain present on `main`. The recovery projection read still had one separate warning event that recorded the complete request tenant identifier, optional channel value, and checkout operation UUID.

## Source change

The read path now delegates the warning to `log_checkout_order_recovery_identity_not_found`. The event retains the stable owner, operation, code, boundary, warning severity, correlation identifier, bounded request-context facts, checkout operation non-nil shape, and locale/fallback-locale presence and length facts.

It no longer records tenant, actor, channel, checkout operation UUID, locale, or fallback-locale values.

## Preserved behavior

Identity lookup order, admission, tenant parsing, projection loading, locale fallback, owner service calls, and the public not-found envelope are unchanged.

The preserved envelope remains `order.checkout_order_not_found`, not-found, non-retryable, with message `checkout order was not found for the requested operation`.

## Deliberately open

This slice does not change identity-conflict, hash/serde, lifecycle, admission, or owner-error diagnostics. Commerce orchestration and Order/Commerce FFA/FBA status are unchanged. The master ecommerce correlation-safe mapper-cleanup item remains open.

## Validation disclosure

No tests, Node verifiers, Cargo commands, formatting, workflows, or CI were executed. The accompanying verifier is retained as a source contract and was not run. No compile or runtime status is promoted.
