# Inventory admin native error safety

Status: source-ready, runtime-unvalidated.

## Scope

This slice hardens the mounted Inventory admin native server-function boundary:

- bootstrap context extraction;
- product list and product detail reads;
- set and adjust quantity writes;
- reserve and release reservation writes;
- availability checks;
- host `TransactionalEventBus` resolution.

Endpoint names, request and response DTOs, effective permissions, tenant matching,
locale fallback, inventory policy, and owner service behavior are unchanged.

## Public boundary

Framework extraction failures, missing host runtime dependencies, database/read
failures, event publication failures, validation internals, and owner mutation
causes are not serialized through `ServerFnError`.

The transport returns static operation-specific messages. Input errors that are
already transport-owned, such as invalid UUIDs, invalid product status, permission
denial, or requested/effective tenant mismatch, retain their existing public
contract.

## Private diagnostics

Every mounted call creates a separate transport correlation id. Original typed
causes are retained only in structured logs together with the owner operation,
tenant, actor, subject identity, stable error code, and transport boundary.
Request tenant, channel, and locale are included when `RequestContext` is
available. Write endpoints treat that additional request context as diagnostic
only so its absence does not change write admission.

## Evidence boundary

The retained source evidence is:

`crates/rustok-inventory/contracts/evidence/admin-native-error-safety-source.json`

The fail-closed source guard is:

`scripts/verify/verify-inventory-admin-native-error-safety.mjs`

No test, verifier, Cargo, formatting, workflow, CI, or runtime pass is claimed by
this document. FBA and FFA state are unchanged.
