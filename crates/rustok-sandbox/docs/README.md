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
started/terminal evidence and returns a typed outcome. Before evaluating a grant
or calling a broker, the host verifies that the call's execution ID, subject,
phase, tenant, actor and trace context match its active request. The crate has no
dependency on Alloy, `rustok-modules`, server hosts or domain modules.

The `platform.http` grant requires typed `hosts`, `methods`, and
`path_prefixes` constraints. All three lists must be non-empty; host and method
matching is exact, while paths use an explicit allowed prefix. The host enforces
them before it calls the capability broker.

The request limits also bound the total number of capability calls, the
serialized input size of each call, and calls in a rolling one-second window.
These limits are shared by all cloned host handles for one execution and are
enforced before broker invocation.

`CapabilityObserver` receives one redacted record for each attempt, including
successful, denied, and failed outcomes. A record carries active execution and
subject identity, request context, capability, operation and stable error code;
it deliberately excludes input, output, credentials and error text.

Execution observer records similarly contain metrics and stable error codes only;
the neutral contract has no error-text field, so the runtime cannot persist or
forward untrusted error text.

`SandboxRuntime::execute_with_cancellation` accepts the same request contract
plus a request-scoped `SandboxCancellation` handle. The runtime rejects a
pre-cancelled request, Rhai checks the handle during progress callbacks,
Wasmtime interrupts its request-private epoch watchdog, and `SandboxHost` checks
it before every capability dispatch.

`SandboxAdmissionLimits` is deployment composition policy, not artifact policy.
It gates concurrent executions globally and by executor, tenant, and admitted
artifact digest; an internal permit releases automatically when execution exits.

`RhaiHostExtension` may register request-scoped functions, populate the Rhai
scope after the neutral envelope is present, and adapt a successful value into
the consumer's typed output binding. The extension receives no global runtime
state and must not introduce another executor or bypass the capability broker.

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
