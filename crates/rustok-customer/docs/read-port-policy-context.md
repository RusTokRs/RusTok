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

This includes the owner method used by the three current commerce GraphQL customer
identity reads.

## Preserved contracts

The change does not alter:

- the `CustomerReadPort` trait;
- request or response DTOs;
- the in-process provider factory;
- read deadline requirements;
- tenant parsing or list validation;
- customer service calls;
- existing customer error-to-`PortError` codes, kinds, messages, and retryability;
- GraphQL not-found handling (`unauthenticated` or optional `None`);
- GraphQL fallback envelopes or successful responses.

The policy helper rethrows the exact original `PortError` after diagnostics are retained.

## Still open

This slice does not claim full customer transport completion. Remaining work includes:

- retaining consumer-side `PortContext` at every non-owner GraphQL, REST, native, and
  operator mapping boundary;
- auditing customer write adapters and profile transports;
- adding runtime/transport evidence and compile validation;
- completing the wider ecommerce correlation-safe mapper cleanup.

No ecommerce FBA/FFA status is promoted.

## Intended verification

```bash
node scripts/verify/verify-customer-read-policy-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-customer --lib
cargo check -p rustok-commerce --lib
```

These commands were not run for this source wave; validation remains maintainer-owned.
