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
- Register language/runtime executors with an explicit `in_process` or
  `isolated_worker` placement and no implicit fallback between them.
- Expose registry-backed executor readiness so owner policy can distinguish an
  execution port from a registered payload executor and inspect its placement.
- Publish comparable audit evidence for draft and installed executions.
- Provide a bounded local scenario contract with explicit policy grants,
  deterministic capability fixtures, and success/error expectations for
  authoring against the same runtime and broker validation.

## Entry points

- `SandboxRuntime`
- `SandboxExecutor`
- `ExecutorRegistry`
- `SandboxPolicy`
- `CapabilityBroker`
- `ExecutionObserver`
- `RhaiWorkspace` and `RhaiScopeInput`/`RhaiScopeOutput`
- `rhai::RhaiHostExtension` (broker-backed function registration)
- `wasm::WasmComponentExecutor` (feature `wasm-component`)
- `LocalSandboxHarness` and `LocalSandboxScenario`

## Interactions

- Alloy uses the isolated sandbox worker for production draft, test, hook,
  scheduled, and manual execution.
- `rustok-modules` uses it for installed Rhai and WebAssembly artifacts.
- `rustok-module-sdk` owns the canonical WIT file used to generate both guest
  bindings and this crate's Wasmtime host bindings.
- The server supplies host capability implementations through narrow ports.
- Untrusted artifact and Alloy Rhai use `rustok-sandbox-transport` and the
  standalone `rustok-sandbox-worker`; worker failure never selects an
  in-process executor.

Rhai consumer extensions may register broker-backed functions for one request.
They must not retain mutable request state or create a second execution API;
workspace and scope adaptation belong to the neutral executor contract.

See the [local documentation](./docs/README.md).
