# Inventory admin native error safety

Status: source-ready, runtime-unvalidated.

## Scope

This source slice covers the mounted Inventory admin native server-function
boundary:

- bootstrap context extraction;
- product list and product detail reads;
- set and adjust quantity writes;
- reserve and release reservation writes;
- availability checks;
- host `TransactionalEventBus` resolution.

The eight endpoint names, request and response DTOs, effective permissions,
tenant matching, locale and filter normalization, inventory policy, owner calls,
and result mapping remain unchanged.

## Confirmed residual gap

The public Inventory Admin envelope was already operation-specific and static.
The private diagnostic helpers, however, still formatted complete errors with
`error = ?error`:

- the shared auth, tenant, and required request-context extraction helper;
- the optional write request-context path;
- the generic inventory owner-error mapper used by product reads and inventory
  mutations.

Those complete payloads could contain framework extraction internals, database
or event-publication details, and validation or invariant text.

## Public boundary

Framework extraction failures, missing host runtime dependencies, database/read
failures, event-publication failures, validation internals, and owner mutation
causes are not serialized through `ServerFnError`.

The transport retains its static operation-specific messages. Input errors that
are already transport-owned, such as invalid UUIDs, invalid product status,
permission denial, or requested/effective tenant mismatch, retain their existing
public contract.

## Correlation-safe diagnostics

Every mounted call creates a separate transport correlation id. Diagnostics
retain the owner and consumer, operation, context kind, tenant, actor, subject,
stable code, boundary, and request tenant/channel/locale when available.

For framework extraction and generic owner failures, only the static Rust error
type is recorded. The complete framework or owner error payload is not logged.
No database error text, event-publication error, validation detail, invariant
message, request body, search text, quantity input, or identifier-bearing owner
payload is added to structured tracing.

Write endpoints continue to treat the additional `RequestContext` as diagnostic
only, so its absence does not change write admission.

## Preserved behavior

This slice does not change:

- mounted endpoint paths or exports;
- request or response DTOs;
- permissions or tenant matching;
- locale, search, status, or pagination behavior;
- `TransactionalEventBus` resolution;
- product read service calls;
- quantity, reservation, availability, or release owner calls;
- result mapping or public messages;
- retry, fallback, workflow, or FFA/FBA status.

## Evidence boundary

The retained source evidence is:

`crates/rustok-inventory/contracts/evidence/admin-native-error-safety-source.json`

The fail-closed source guard is:

`scripts/verify/verify-inventory-admin-native-error-safety.mjs`

No test, verifier, Cargo, formatting, workflow, CI, or runtime pass was executed
for this source slice. Evidence remains deliberately unvalidated.

Suggested maintainer execution:

```bash
node scripts/verify/verify-inventory-admin-native-error-safety.mjs
node scripts/verify/verify-inventory-admin-client-transport-error-safety.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-inventory-admin
cargo check -p rustok-inventory-admin --features hydrate
cargo check -p rustok-inventory-admin --features ssr
```

The broader ecommerce correlation-safe mapper cleanup remains open for the
remaining owners and non-`PortError` public or diagnostic envelopes.
