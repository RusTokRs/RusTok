# Commerce GraphQL storefront-region error safety

Status: `source_closed_unvalidated`

## Scope

This source wave closes the currently identified Commerce GraphQL storefront-region error-envelope gap and the remaining raw correlation payload in the Region owner read diagnostics.

The public resolver source in `crates/rustok-commerce/src/graphql/query.rs` remains unchanged. It still:

- resolves the current tenant and effective locale;
- constructs the existing three-second Region read `PortContext`;
- delegates to `RegionReadPort::list_regions_for_tenant` through `RegionService`;
- maps owner projections to the existing `GqlRegion` list.

No GraphQL field, argument, DTO, locale fallback, owner call, deadline, tenant selection, module gate, or successful response changes.

## Typed safe-query cutover

The compatibility resolver formats `error.code` and `error.message` before constructing a GraphQL error. The mounted safe-query source now intercepts only that exact expression and passes the complete typed `PortError` to `RegionGraphqlMessage`.

All other `format!` calls retain standard Rust behavior. The focused verifier requires exactly one matching Region expression in `query.rs`; source drift therefore fails closed instead of silently broadening the interception.

The transport mapper classifies only `PortErrorKind`:

| Owner kind | Public code | Public message | Retryable |
| --- | --- | --- | --- |
| Validation | `REGION_REQUEST_INVALID` | `Region query is invalid` | false |
| NotFound | `REGION_RESOURCE_NOT_FOUND` | `Region data was not found` | false |
| Conflict | `REGION_STATE_CONFLICT` | `Region state conflicts with this query` | false |
| Forbidden | `REGION_ACCESS_DENIED` | `Region query is not permitted` | false |
| Unavailable / Timeout | `REGION_TEMPORARILY_UNAVAILABLE` | `Region data is temporarily unavailable` | true |
| InvariantViolation | `REGION_OPERATION_FAILED` | `Region query could not be completed safely` | false |

Owner `code` and `message` are not copied into the GraphQL response and are not parsed for control flow.

## Bounded diagnostics

Commerce retains:

- a zero-sized diagnostic token whose `Debug` output is always `redacted`;
- the closed owner kind;
- stable owner code;
- owner retryability;
- owner-message presence and character length;
- selected public code and retryability;
- error severity for unavailable, timeout, and invariant failures;
- warning severity for ordinary validation, not-found, conflict, and forbidden outcomes.

Commerce does not log the complete `PortError` or owner-message content.

The Region owner boundary now records correlation-id character length rather than the raw correlation id. Tenant, actor, locale, channel, causation, trace, idempotency, request selector, UUID, country-code, and owner-error payloads remain represented only by the existing bounded facts.

## Preserved contracts

- `RegionReadPort` and both Region owner read operations are unchanged.
- Region public `PortError` codes, kinds, messages, and retryability are unchanged.
- `query.rs` is unchanged and remains the compatibility resolver source.
- Generic Commerce query dynamic-message redaction remains available for other residual paths.
- Region FFA/FBA status is unchanged.
- No runtime, mounted GraphQL, compile, remote-adapter, or parity evidence is claimed.

## Still open

- Execute the focused source verifiers and Cargo checks.
- Retain mounted storefront-region GraphQL success and each typed failure class.
- Retain deadline, restart, and remote Region adapter evidence.
- Continue customer, tax, promotion, inventory, remaining adapter, and non-`PortError` ecommerce envelope cleanup.

## Intended checks

```bash
node scripts/verify/verify-commerce-graphql-query-region-error-safety.mjs
node scripts/verify/verify-region-owner-port-error-safety.mjs
cargo check -p rustok-commerce --lib
cargo check -p rustok-region --lib
```

No tests, Node verifiers, Cargo commands, formatting, mounted GraphQL scenarios, workflows, or CI were executed.
