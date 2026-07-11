# rustok-sandbox documentation

## Purpose

This crate is a platform-neutral execution foundation. It does not own module
identity, marketplace governance, installation state or Alloy source workflows.

## Responsibility Zone

The crate owns sandbox requests, executor selection, default-deny capability
enforcement, shared limits, outcomes and execution observers. Executor adapters
may implement Rhai, WebAssembly or sidecar isolation behind the same contract.

## Integration

Consumers construct a `SandboxRequest` with a typed subject and admitted policy.
`SandboxRuntime` resolves the executor, creates a scoped `SandboxHost`, records
started/terminal evidence and returns a typed outcome. The crate has no dependency
on Alloy, `rustok-modules`, server hosts or domain modules.

The optional `wasm-component` feature provides `WasmComponentExecutor`. Its
v1 ABI calls the artifact entrypoint as `(string) -> result<string, string>`
with JSON input/output. It grants neither WASI nor ambient imports. The only
accepted WIT import is `rustok:module/host.invoke(capability, operation, json)`;
it is converted to a `SandboxHost` capability call and still requires an
explicit policy grant.

## Verification

- `cargo test -p rustok-sandbox`
- `cargo check -p rustok-sandbox`
- `cargo check -p rustok-sandbox --features wasm-component`

## Related Documents

- [Implementation plan](./implementation-plan.md)
- [Neutral sandbox ADR](../../../DECISIONS/2026-07-11-neutral-sandbox-foundation.md)
- [Documentation map](../../../docs/index.md)
