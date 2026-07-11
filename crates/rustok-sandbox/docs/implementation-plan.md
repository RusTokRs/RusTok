# Implementation plan for `rustok-sandbox`

## Scope

Provide one neutral sandbox runtime used equally by Alloy authoring and module
artifact execution.

## Current State

The foundation contracts, default-deny capability broker, executor registry and
execution observer pipeline are implemented. The generic Rhai engine is behind
the contract. The optional Wasmtime component executor has a default-deny v1
ABI with fuel and epoch deadline enforcement. Its sole typed WIT capability
import delegates to `SandboxHost`; WASI and all other imports remain disabled.

## Milestones

1. Connect artifact installation and Alloy draft execution to the same runtime.
2. Add production audit persistence, cancellation and concurrency admission.
3. Add a sidecar executor only after its isolation and protocol contract is
   approved.

## Verification

- Contract tests for executor selection and duplicate registration.
- Default-deny and constrained capability broker tests.
- Equivalent draft/artifact execution evidence tests.
- Rhai and WebAssembly resource-limit parity tests.

## Update Rules

Update this plan, the ADR and consumer plans whenever sandbox ownership,
capability semantics or executor failure taxonomy changes.
