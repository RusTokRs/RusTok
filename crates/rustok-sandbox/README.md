# rustok-sandbox

## Purpose

`rustok-sandbox` provides the neutral isolated-execution foundation shared by
Alloy drafts and installed module artifacts.

## Responsibilities

- Define the common execution envelope, policy, limits and outcome taxonomy.
- Enforce default-deny host capability access through a scoped broker, including
  active execution, subject, tenant, actor, phase and trace-context matching.
- Enforce typed HTTP host, method, and path-prefix grants before broker dispatch.
- Bound capability-call count, serialized input size, and one-second rate per
  sandbox execution.
- Emit redacted capability-attempt evidence with identity, operation, outcome,
  and stable error code only.
- Exclude untrusted error text from neutral execution evidence; observers receive
  the stable error code only.
- Provide one cooperative cancellation handle per execution, checked before
  executor work and every brokered capability dispatch.
- Bound synchronous Rhai/WIT broker bridging to one native thread per execution.
- Admit executions through shared global, executor, tenant, and artifact gates
  with automatic permit release on every terminal path.
- Register language/runtime executors without depending on their consumers.
- Expose registry-backed executor readiness so owner policy can distinguish an
  execution port from a registered payload executor.
- Publish comparable audit evidence for draft and installed executions.

## Entry points

- `SandboxRuntime`
- `SandboxExecutor`
- `ExecutorRegistry`
- `SandboxPolicy`
- `CapabilityBroker`
- `ExecutionObserver`
- `rhai::RhaiHostExtension` (request-scoped scope/output adaptation)
- `wasm::WasmComponentExecutor` (feature `wasm-component`)

## Interactions

- Alloy uses the sandbox for draft, test, hook and manual execution.
- `rustok-modules` uses it for installed Rhai and WebAssembly artifacts.
- The server supplies host capability implementations through narrow ports.

Rhai consumer extensions may register broker-backed functions and adapt only a
single request's scope and successful output binding. They must not retain
mutable request state or create a second execution API.

See the [local documentation](./docs/README.md).
