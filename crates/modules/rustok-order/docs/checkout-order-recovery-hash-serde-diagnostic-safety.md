# Checkout order recovery hash/serde diagnostic safety

Status: **source-ready / unvalidated**

This source slice closes the remaining hash and serialization diagnostic gap in the Order checkout recovery adapter.

## Recheck

The merged owner-error, admission, missing-identity read, and identity-conflict slices remain present on `main`. Three separate paths still retained raw diagnostic payloads: serialization of the complete checkout request, serialization of canonical checkout JSON, and rejection of normalized hash evidence.

The two serialization events recorded the complete `serde_json::Error` plus raw tenant and channel context. The hash rejection event did not record the hash value, but still recorded raw tenant and channel context.

## Source change

Serialization failures now delegate to `log_checkout_order_recovery_encoding_failure`. The event retains correlation, stable owner operation/code/boundary, bounded context facts, error severity, and a closed serialization target (`checkout_completion_request` or `canonical_checkout_json`). It no longer records the serializer error or raw context values.

Hash rejection now computes the same length and ASCII-hex acceptance facts explicitly and delegates to `log_checkout_order_recovery_hash_rejection`. The warning retains field name, value length, configured bounds, length-match and ASCII-hex shape, plus bounded context facts. It never records the hash value.

## Preserved behavior

Request and snapshot JSON shape, recursive object-key canonicalization, array ordering, SHA-256 digesting, lowercase trimming, inclusive length bounds, ASCII hexadecimal validation, public error codes/kinds/messages/retryability, and recovery ordering are unchanged.

The preserved envelopes remain:

- `order.checkout_request_encoding_failed`, invariant violation, non-retryable, `checkout completion request could not be encoded`;
- `order.checkout_hash_invalid`, validation, non-retryable, `checkout hash evidence is invalid`.

## Deliberately open

This slice does not change cancelled/unknown lifecycle diagnostics, admission, read-not-found, identity-conflict, or owner-error diagnostics. Commerce orchestration and Order/Commerce FFA/FBA status are unchanged. The master ecommerce correlation-safe mapper-cleanup item remains open.

## Validation disclosure

No tests, Node verifiers, Cargo commands, formatting, workflows, or CI were executed. The accompanying verifier is retained as a source contract and was not run. No compile or runtime status is promoted.
