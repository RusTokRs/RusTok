# Implementation plan for `rustok-sandbox`

## Scope

Provide one neutral sandbox runtime used equally by Alloy authoring and module
artifact execution.

## Current State

The foundation contracts, default-deny capability broker, executor registry and
execution observer pipeline are implemented. Language-specific executors have not
yet been moved behind the contract.

## Milestones

1. Move the generic Rhai engine and its resource limits from Alloy into a Rhai
   executor adapter while keeping Alloy bridges outside the foundation.
2. Add the Wasmtime Component Model executor with equivalent timeout, memory,
   instruction and capability outcomes.
3. Connect artifact installation and Alloy draft execution to the same runtime.
4. Add production audit persistence, cancellation and concurrency admission.
5. Add a sidecar executor only after its isolation and protocol contract is
   approved.

## Verification

- Contract tests for executor selection and duplicate registration.
- Default-deny and constrained capability broker tests.
- Equivalent draft/artifact execution evidence tests.
- Rhai and WebAssembly resource-limit parity tests.

## Update Rules

Update this plan, the ADR and consumer plans whenever sandbox ownership,
capability semantics or executor failure taxonomy changes.

