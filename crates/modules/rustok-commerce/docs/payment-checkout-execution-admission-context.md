# Payment checkout execution admission context

Status: source-complete / unvalidated

## Scope

This slice hardens admission diagnostics for the payment-owned
`CheckoutPaymentExecutionPort` used by staged Commerce checkout.

Covered operations:

- `prepare_checkout_collection`
- `authorize_checkout_collection`
- `capture_checkout_collection`
- `read_checkout_collection`

The public port signatures, request/response types, read/write policies, write semantics,
tenant and causation validation, owner delegation, provider journal behavior, lifecycle
policy, idempotency, and returned `PortError` values are unchanged.

## Previous gap

The four public owner-port methods called `PortContext::require_policy` directly, and the
three write methods called `require_write_semantics` directly. A rejected policy or
missing write semantic returned before the existing local outcome mapper retained the
owner operation and request context.

The errors were already stable typed `PortError` values, so this was a diagnostic and
correlation gap rather than a public-message sanitization change.

## Source change

Read admission now goes through `require_checkout_payment_read_admission`. Write
admission now goes through `require_checkout_payment_write_admission`, which preserves
both the write policy and non-empty write/idempotency semantics.

Both helpers use `inspect_err`. They log the rejected typed error and return the exact
same `PortError` without constructing or translating a public envelope.

The admission event retains:

- owner and exact owner operation;
- policy versus write-semantics admission stage;
- correlation ID and tenant;
- actor, channel, and effective locale;
- causation ID and traceparent;
- idempotency key and deadline;
- typed error kind, stable code/message, and retryability;
- the `checkout_payment_execution_port` boundary.

Unavailable, timeout, and invariant failures are emitted at error level. Ordinary policy
or validation rejections are emitted at warning level.

No checkout identity payload, currency string, plan hash, provider identity, metadata,
or provider payload is added to the admission logs.

## Preserved delegated outcome behavior

After admission, the existing local-context mapper still retains only safe request facts
for owner validation, storage, lifecycle, provider, and reconciliation outcomes. Unknown
outcomes still pass through unchanged.

## Evidence boundary

The updated focused guard is:

```text
node scripts/verify/verify-payment-checkout-execution-local-context.mjs
```

It now requires contextual admission helpers, forbids direct public-method admission,
checks that helpers use `inspect_err` rather than remapping, and retains the existing
safe delegated-outcome checks.

The verifier, Cargo, tests, formatting, workflows, CI, and runtime failure scenarios were
not executed for this source slice.

## Remaining work

The master ecommerce correlation-safe mapper item remains open. In particular,
`CheckoutPaymentCompensationPort` still performs policy and write-semantics admission
directly and should be handled as a separate focused slice before the payment
execution/compensation mapper work can be considered complete.
