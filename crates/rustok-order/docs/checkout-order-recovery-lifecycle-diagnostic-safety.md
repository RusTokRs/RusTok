# Checkout order recovery lifecycle diagnostic safety

Status: **source-ready / unvalidated**

This source slice closes the remaining raw lifecycle diagnostics in the Order checkout recovery adapter.

## Recheck

The merged owner-error, admission, missing-identity read, identity-conflict, and hash/serde slices remain present on `main`. The `Cancelled` and `Unknown` branches in `resume_order` still logged raw tenant, channel, and order UUID values plus a debug-formatted lifecycle enum.

## Source change

Both branches now delegate to `log_checkout_order_recovery_lifecycle_rejection`.

The helper preserves correlation, the stable owner operation/code/boundary, bounded context facts, order UUID non-nil shape, a closed lifecycle state (`cancelled` or `unknown`), and the original severity. It does not record tenant, actor, channel, order UUID, locale, or a debug lifecycle value.

## Preserved behavior

Pending orders are still confirmed and optionally reloaded with locale fallback. Confirmed, paid, shipped, and delivered orders still return unchanged.

Cancelled orders still produce a warning and the non-retryable conflict envelope `order.checkout_order_cancelled` with message `checkout order is already cancelled`.

Unknown orders still produce an error and the non-retryable invariant envelope `order.checkout_order_status_invalid` with message `checkout order has an unsupported lifecycle state`.

## Boundary status

This completes the source-level raw diagnostic cleanup identified in `checkout_order_recovery.rs`. It does not promote Order or Commerce FFA/FBA status and does not close the master ecommerce correlation-safe mapper-cleanup item, which still covers other owner adapters and public envelopes.

## Validation disclosure

No tests, Node verifiers, Cargo commands, formatting, workflows, or CI were executed. The accompanying verifier is retained as a source contract and was not run. No compile or runtime status is promoted.
