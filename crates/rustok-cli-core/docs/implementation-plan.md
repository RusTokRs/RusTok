# Implementation plan for `rustok-cli-core`

## Current state

`rustok-cli-core` owns stable command-provider contracts for the platform CLI:
descriptors, normalized requests, machine-readable outcomes, asynchronous
`CommandProvider` execution, and shared CLI errors. It is neither a terminal
parser nor a central command implementation catalog.

The `rustok-cli` binary owns parser/output UX and aggregates module-local
provider adapters through generated distribution registries. Domain crates and
the production server do not depend on CLI contracts.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This tooling contract crate has no module-owned UI or FBA provider port.

## Open results

1. **Connect the first runtime-aware module-local CLI provider.** Implement a
   provider adapter that needs database, storage, or a remote port through the
   generated factory contract, without adding command bodies to domain crates.
   **Depends on:** a module-local `cli/` adapter and runtime composition.
   **Done when:** the provider exposes discovery and typed async execution from
   `CommandRequest.args` without importing the server runtime.

2. **Harden generated registry and distribution coverage.** Add contract tests
   for provider discovery, duplicate rejection, selected distribution output,
   and registry freshness before distribution-aware builds broaden.
   **Depends on:** generated registry metadata and provider fixtures.
   **Done when:** `verify:cli-registry` and typed provider tests prove a
   deterministic, machine-readable command inventory.

3. **Migrate task documentation with command ownership.** Move operational
   command guidance to the platform CLI as providers land, documenting namespace,
   arguments, outcomes, and replacement/removal of prior task entry points.
   **Depends on:** the change-owning module provider.
   **Done when:** users can discover and execute a command without a server or
   domain-crate-specific maintenance path.

## Verification

- `npm run verify:cli-registry`
- `npm run verify:api:surface-contract`
- Targeted typed provider, argument-decoding, registry, and inventory-output
  tests.

## Change rules

1. Keep command bodies in module-local `cli/` adapters, not domain crates.
2. Keep parser, terminal output, and exit handling in `rustok-cli`.
3. Update CLI and module task documentation with any provider contract change.
