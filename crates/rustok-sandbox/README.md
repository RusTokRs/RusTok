# rustok-sandbox

## Purpose

`rustok-sandbox` provides the neutral isolated-execution foundation shared by
Alloy drafts and installed module artifacts.

## Responsibilities

- Define the common execution envelope, policy, limits and outcome taxonomy.
- Enforce default-deny host capability access through a scoped broker.
- Register language/runtime executors without depending on their consumers.
- Publish comparable audit evidence for draft and installed executions.

## Entry points

- `SandboxRuntime`
- `SandboxExecutor`
- `ExecutorRegistry`
- `SandboxPolicy`
- `CapabilityBroker`
- `ExecutionObserver`
- `wasm::WasmComponentExecutor` (feature `wasm-component`)

## Interactions

- Alloy uses the sandbox for draft, test, hook and manual execution.
- `rustok-modules` uses it for installed Rhai and WebAssembly artifacts.
- The server supplies host capability implementations through narrow ports.

See the [local documentation](./docs/README.md).
