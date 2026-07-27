# Customer read port policy context

Status: `source_ready_unvalidated`

## Closed gap

`CustomerReadPort` previously called `PortContext::require_policy` before assigning the
owner operation. A missing or invalid read deadline therefore returned a typed
`PortError` without a customer-specific structured event tying the rejection to the
exact owner method and original call context.

All four read methods now:

1. assign their canonical owner operation first;
2. delegate read admission to one shared helper;
3. log a rejected admission with the complete available `PortContext`;
4. return the original `PortError` unchanged.

The diagnostic event records:

- owner `rustok_customer`;
- correlation and tenant identity;
- actor, channel, locale, causation, traceparent, and deadline context;
- exact owner operation;
- original port code, typed kind, and retryability;
- boundary `customer_read_port`.

## Covered operations

- `read_customer_projection`
- `read_customer_projection_by_user`
- `list_customer_projections`
- `list_profile_enrichment`

This includes the owner method used by the current Commerce authenticated-customer
identity reads.

## Canonical local outcomes

Canonical root construction now adds a separate post-delegation context wrapper for
stable local outcomes. Root `InProcessCustomerReadPort` and
`in_process_customer_read_port` retain the complete delegated context and only safe
operation-specific request facts before calling the unchanged `CustomerService` port
implementation.

The wrapper classifies exact stable context, page, storage, not-found, validation, and
profile-projection envelopes and returns the same delegated `PortError`. It does not
log raw search text, email, names, customer rows, or profile payloads. The complete
contract is documented in [read-local-context.md](./read-local-context.md).

The policy event and local-outcome event have different phases. Policy rejection is
recorded before owner delegation; a covered local outcome is recorded after the owner
implementation returns. Direct callers of the compatibility factory under
`rustok_customer::ports` do not pass through the root wrapper.

## Preserved contracts

The change does not alter:

- the `CustomerReadPort` trait;
- request or response DTOs;
- read deadline requirements;
- tenant parsing or list validation;
- customer service calls;
- existing customer error-to-`PortError` codes, kinds, messages, and retryability;
- GraphQL not-found handling (`unauthenticated` or optional `None`);
- GraphQL fallback envelopes or successful responses;
- FBA or FFA status.

The policy helper rethrows the exact original `PortError` after diagnostics are retained.
The root local-outcome wrapper also returns the exact delegated `PortError` unchanged.

## Still open

This work does not claim full customer transport completion. Remaining work includes:

- direct callers that bypass canonical root construction through `rustok_customer::ports`;
- retaining consumer-side `PortContext` at every non-owner GraphQL, REST, native, and
  operator mapping boundary not already covered by focused slices;
- auditing customer write adapters and profile transports;
- adding runtime/transport evidence and compile validation;
- completing the wider ecommerce correlation-safe mapper cleanup.

No ecommerce FBA/FFA status is promoted.

## Intended verification

```bash
node scripts/verify/verify-customer-read-local-context.mjs
node scripts/verify/verify-customer-read-policy-context.mjs
node scripts/verify/verify-customer-fba-no-compile.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-customer --lib
cargo check -p rustok-commerce --lib
```

These commands were not run for this source wave; validation remains maintainer-owned.
