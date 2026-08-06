# Commerce GraphQL channel error safety

Status: `source_closed_unvalidated`

## Scope

This source wave closes the currently identified non-`PortError` public-envelope gap in the mounted Commerce GraphQL storefront pricing-channel query.

The compatibility resolver source in `crates/rustok-commerce/src/graphql/query.rs` remains unchanged. `storefront_pricing_channels` continues to:

- require the Commerce module and storefront channel gate;
- enforce the current-tenant boundary;
- construct the canonical `rustok_channel::ChannelService` from the mounted database connection;
- call `list_channels(tenant_id, 1, 250)` exactly once;
- project the same successful `ChannelResponse` values into `GqlPricingChannelOption`.

## Typed facade

The mounted safe-query source aliases only the Channel dependency used by the unchanged compatibility query. The facade wraps the canonical `rustok_channel::ChannelService` and delegates the same `list_channels` operation with the same tenant, page, and page-size arguments.

The original resolver constructs a GraphQL error from `err.to_string()`. The facade preserves that source expression through an inherent typed conversion. It does not format the Channel owner error into a public string. The complete `ChannelError` remains typed until the transport mapper.

## Public envelopes

Transport responses are classified structurally by `ChannelError` variant.

| Owner variant | Public code | Public message | Retryable |
| --- | --- | --- | --- |
| Invalid target/policy input | `CHANNEL_REQUEST_INVALID` | `Channel query is invalid` | false |
| Not found | `CHANNEL_RESOURCE_NOT_FOUND` | `Channel data was not found` | false |
| Inactive/duplicate state | `CHANNEL_STATE_CONFLICT` | `Channel state conflicts with this query` | false |
| Database | `CHANNEL_TEMPORARILY_UNAVAILABLE` | `Channel data is temporarily unavailable` | true |
| Serialization | `CHANNEL_OPERATION_FAILED` | `Channel query could not be completed safely` | false |

The complete owner error, database/serialization cause, UUID values, target values, slugs, and policy text are not copied into the GraphQL response.

## Bounded diagnostics

The transport boundary retains:

- a diagnostic token whose `Debug` output is always `redacted`;
- structural error kind;
- closed owner-detail shape and aggregate character length where applicable;
- selected public code and retryability;
- error severity for database and serialization failures;
- warning severity for validation, not-found, and conflict rejections.

The complete `ChannelError` and owner-detail content are not logged.

## Preserved contracts

- `rustok-channel` remains the owner of channel persistence and listing behavior.
- The original owner service, operation, and arguments are unchanged.
- The query source, GraphQL field, tenant policy, result ordering, limit, and DTO conversion are unchanged.
- Commerce and Channel FFA/FBA status is unchanged.
- The broad ecommerce mapper and public-envelope cleanup remains open.
- No compile, runtime, mounted GraphQL, remote-adapter, or parity evidence is claimed.

## Evidence

- `crates/rustok-commerce/contracts/evidence/graphql-query-channel-error-safety-source-review.json`
- `scripts/verify/verify-commerce-graphql-query-channel-error-safety.mjs`

## Still open

- Execute the focused source verifier and Cargo checks.
- Retain mounted success and database-failure GraphQL evidence.
- Continue cart, tax, promotion, inventory, remaining adapter, write-side, and non-`PortError` ecommerce envelope cleanup.

## Intended checks

```bash
node scripts/verify/verify-commerce-graphql-query-channel-error-safety.mjs
cargo check -p rustok-commerce --lib
cargo check -p rustok-channel --lib
```

No tests, Node verifiers, Cargo commands, formatting, mounted GraphQL scenarios, workflows, or CI were executed.
