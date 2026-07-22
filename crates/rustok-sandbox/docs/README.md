# rustok-sandbox documentation

## Purpose

This crate is a platform-neutral execution foundation. It does not own module
identity, marketplace governance, installation state or Alloy source workflows.

## Responsibility Zone

The crate owns sandbox requests, executor selection, default-deny capability
enforcement, shared limits, outcomes and execution observers. Executor adapters
may implement Rhai, WebAssembly or sidecar isolation behind the same contract.
It also owns the stable `RHAI_SANDBOX_RUNTIME_ABI` identity consumed by Alloy
publication smoke and admitted Rhai module artifacts; consumers must not
duplicate or reinterpret that ABI string in their own release contracts.

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

The `platform.secrets` grant requires typed non-empty `references` and
`operations` lists. A call must contain exactly one logical `reference` selected
from that allowlist and one declared operation. Resolver aliases, resolver keys,
and secret values are not guest inputs. The module data owner durably binds a
logical reference to a host-authorized `SecretRef`. The owner-provided
`acquire_handle` broker can return only that logical reference and its revision;
value consumption uses a separate host-only exact-revision service with a
fixed-purpose consumer. It is intentionally not a sandbox `get_value` operation
and cannot serialize resolver details, secret values, or arbitrary consumer
output in a capability response.

The `platform.events` grant requires typed non-empty `topics` and `operations`
lists. A call may contain only `topic` and optional `payload`; the topic must
match an exact grant entry or a terminal `.*` wildcard, and the operation must
be declared. A global wildcard, arbitrary broker input fields, and undeclared
event topics are denied before the broker is invoked.

The `platform.data` grant requires typed non-empty `key_prefixes` and
`operations` lists. The tenant/module/data-contract namespace is injected by
the host; guests can name only logical keys under an admitted prefix. The
current contract permits `get`, `put`, and bounded `list` inputs and rejects
physical storage fields such as a table, bucket, path, or namespace. The
owner-provided adapter also permits an explicitly granted `put_batch` of at
most 32 writes. The sandbox validates every batch entry's logical key and UUID
idempotency key, requires both to be distinct across the batch, and keeps every
key inside an admitted prefix before invoking the owner. Listing uses an escaped
prefix query and continuation that remain inside the admitted logical prefix.

The `platform.mcp` grant requires typed non-empty exact server/tool pairs and
the `call` operation. A call may contain only a configured server alias, tool
name, and optional arguments; MCP endpoints, transports, credentials, and
tool-discovery targets are never guest inputs. `rustok-modules` provides the
scoped `ArtifactMcpCapabilityBroker` and its `ArtifactMcpInvoker` owner port;
deployment composition must bind that port to the existing MCP access-policy,
audit, and configured server-alias implementation.

`CapabilityBrokerRouter` composes owner-provided adapters by their exact
capability name. It rejects duplicate owners and denies an unregistered route;
host composition can therefore combine data, secret, and future MCP adapters
without introducing a platform-global fallback or making one owner adapter
responsible for another capability.

The request limits also bound the total number of capability calls, the
serialized input size of each call, and calls in a rolling one-second window.
These limits are shared by all cloned host handles for one execution and are
enforced before broker invocation.

`CapabilityObserver` receives one redacted record for each attempt, including
successful, denied, and failed outcomes. A record carries active execution and
subject identity, request context, capability, operation and stable error code;
it deliberately excludes input, output, credentials and error text.

Execution observer records similarly contain only redacted request identity,
metrics, and stable error codes. The neutral contract has no error-text field,
so the runtime cannot persist or forward untrusted error text. Observers are
fallible: a host that requires durable evidence can reject execution when a
start or terminal record cannot be delivered. `rustok-modules` owns the
artifact-specific SeaORM adapter; this crate remains storage-neutral.

Every terminal execution record includes queue time, executor duration, and the
number of capability calls admitted by the shared policy budget. Rhai reports
its consumed instructions, Wasmtime reports consumed fuel through the same
field, and both executors report serialized output size. Wasmtime also reports
the observed aggregate peak of non-shared guest linear memory through its
resource limiter; failed allocations are excluded and configured limits are not
reported as usage. Rhai leaves peak memory absent until the isolated-worker
profile can measure it truthfully.

`SandboxRuntime::execute_with_cancellation` accepts the same request contract
plus a request-scoped `SandboxCancellation` handle. The runtime rejects a
pre-cancelled request, Rhai checks the handle during progress callbacks,
Wasmtime interrupts its request-private epoch watchdog, and `SandboxHost` checks
it before every capability dispatch.

The same enabled executor paths enforce `SandboxLimits::wall_clock_ms`: Rhai
terminates through its progress callback, while Wasmtime increments only the
private engine epoch for the timed-out request. Both return the common timeout
error; no enabled executor may continue after its deadline.

`SandboxAdmissionLimits` is deployment composition policy, not artifact policy.
It gates concurrent executions globally and by executor, tenant, and admitted
artifact digest; an internal permit releases automatically when execution exits.

`RhaiHostExtension` may register request-scoped functions, populate the Rhai
scope after the neutral envelope is present, and adapt a successful value into
the consumer's typed output binding. The extension receives no global runtime
state and must not introduce another executor or bypass the capability broker.
Its only platform handle is the scoped `SandboxHost`; broker implementations,
infrastructure clients, and credentials are never exposed to an adapter.
`RhaiCapabilityBridge` is the standard neutral extension for installed
artifacts: it exposes only `capability_call(name, operation, input)` and returns
`{ ok, output }` on success or `{ ok: false, error_code }` on denial/failure.
The named call remains subject to the same policy, identity, limit, audit, and
cancellation checks as a WIT capability invocation.
The synchronous Rhai/WIT bridge admits only one native thread per execution, so
guest code cannot create an unbounded number of blocking broker threads.

Every Rhai request uses `RhaiBindingInput` v1 and every successful execution
returns `RhaiBindingOutput` v1. Both envelopes are strict and reject another
version, unknown fields, and raw JSON compatibility inputs or outputs. The
guest sees the subject-owned `input` value inside the neutral envelope; subject
owners decode the result before interpreting their own payload.

The optional `wasm-component` feature provides `WasmComponentExecutor`. Its
frozen v1 ABI is package `rustok:module@1.0.0`, world `module-runtime`, and
entrypoint `run`: `(string) -> result<string, string>`. Input and successful
output use `application/json`; a guest error is the WIT result's string error
and maps to the stable sandbox trap outcome. It grants neither WASI nor ambient
imports. The only accepted WIT import is
`rustok:module/host.invoke(capability, operation, json)`; it is converted to a
`SandboxHost` capability call and still requires an explicit policy grant.
Compatibility is exact within v1: a package, world, entrypoint, or wire-encoding
change requires a new ABI version rather than a permissive fallback.

`LocalSandboxHarness` is the authoring/test entry point for the same
`SandboxRequest`, policy validation, runtime execution, cancellation, and error
contracts used in deployment. Its `FixtureCapabilityBroker` returns only
caller-supplied deterministic JSON by exact capability/operation; an
unregistered fixture is denied. The harness never reads deployment
configuration or exposes network, filesystem, database, secret, MCP, or other
production clients.

The Wasmtime executor keeps a bounded node-local LRU cache of serialized
compiled Components. Its key includes the pinned Wasmtime engine version, host
target, admitted runtime ABI, and artifact digest. A cache hit deserializes into
a new request-private engine and store, so it cannot retain host handles, tenant
state, credentials, or a prior execution's epoch cancellation state. Corrupt
cached bytes are evicted and recompiled from the admitted payload; oversized
serialized components are executed but not cached.

## Verification

- `cargo test -p rustok-sandbox`
- `cargo check -p rustok-sandbox`
- `cargo check -p rustok-sandbox --features wasm-component`

## Related Documents

- [Implementation plan](./implementation-plan.md)
- [Neutral sandbox ADR](../../../DECISIONS/2026-07-11-neutral-sandbox-foundation.md)
- [Documentation map](../../../docs/index.md)
