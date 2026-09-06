# Marketplace listing admin native request error safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the shared marketplace-listing admin native request boundary in:

- `crates/rustok-marketplace-listing/admin/src/transport/native_server_adapter.rs`.

The boundary is shared by directory, detail, and command server functions. It resolves the host runtime, owner runtime, authentication, tenant and request contexts, tenant-module availability, and the owner `PortContext`.

## Delivered source contract

Static public `ServerFnError` messages now cover:

- missing `HostRuntimeContext`;
- missing host-composed `MarketplaceListingRuntime`;
- authentication-context extraction failure;
- tenant-context extraction failure;
- request-context extraction failure;
- failure to read tenant module availability.

Internal causes remain only in SSR diagnostics with the available:

- marketplace-listing admin owner and shared native operation;
- requested admin action;
- tenant id;
- request correlation id, channel id, channel slug and locale for module-availability failures;
- stable internal code and boundary;
- original extraction or module-check error.

## Preserved behavior

This slice does not change:

- the directory, detail or command endpoint paths;
- request or response DTOs;
- directory filters, pagination or event limit;
- command variants or command payloads;
- action-to-permission mapping;
- permission-denied messages;
- request identity checks or their message;
- the module-disabled message;
- the 5-second owner-port deadline;
- native PortContext locale, generated correlation id, channel or idempotency-key composition;
- the existing `PortErrorKind` public mapper;
- GraphQL transport or transport selection;
- marketplace-listing FBA/FFA status.

## Static evidence

`scripts/verify/verify-marketplace-listing-admin-native-request-error-safety.mjs` guards:

- SSR tracing feature composition;
- stable owner, operation, action, code and boundary diagnostics;
- static runtime and request-context public envelopes;
- correlation, tenant, channel and locale logging for module-check failures;
- removal of raw host/runtime/context error mapping;
- unchanged endpoint, permission, module-disabled, PortContext and `PortError` contracts;
- source-only validation flags.

## Remaining gaps

Compile, mounted parity, runtime and remote transport evidence remain open. The broader ecommerce mapper-cleanup task also remains open for other owner consumers, compensation/execution adapters, remaining transports and non-`PortError` public envelopes.

This slice does not make the marketplace financial integration an optional Commerce capability and does not change marketplace topology.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-marketplace-listing-admin-native-request-error-safety.mjs
node scripts/verify/verify-marketplace-listing-admin-ffa.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-marketplace-listing-admin --all-features
```
