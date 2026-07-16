# Implementation Plan for `rustok-sandbox`

## Scope

Provide the single neutral execution contract used equally by Alloy drafts and
installed module artifacts. The sandbox owns execution mechanics and evidence,
not module identity, marketplace state, installation, build, or Alloy source
workspaces.

The cross-component sequence and completion rules are defined by the
[canonical module-platform plan](../../../docs/modules/module-control-plane-consolidation-plan.md).

## Current State

Implemented:

- typed execution subject, context, payload, policy, limits, outcome, metrics,
  status, record, and errors;
- executor registry and default-deny capability broker;
- capability-call identity propagation and pre-broker execution, subject,
  tenant, actor, phase, and trace-context matching;
- typed HTTP, secret-reference, event-topic, and logical-data grant constraints
  enforced before broker invocation;
- typed MCP server/tool-pair constraints enforced before broker invocation;
- exact-name capability broker router for composing owner adapters while
  preserving default-deny behavior for unregistered capabilities;
- fallible observer pipeline for started/succeeded/failed redacted execution
  evidence and correlation context;
- generic Rhai engine/executor with resource and timeout limits;
- request-scoped Rhai host extensions;
- Wasmtime Component Model executor with fuel, epoch deadlines, store limits,
  and no ambient WASI imports;
- typed `rustok:module/host.invoke` WIT import routed through `SandboxHost`;
- installed artifact execution from `rustok-modules`.

Remaining:

- stable Rhai binding and WIT v1 compatibility contract;
- owner-specific durable audit deployment composition; `rustok-modules` now
  provides the artifact-only SeaORM adapter, while this neutral crate keeps no
  storage dependency;
- production compiled-artifact cache policy and metrics;
- richer capability constraints and call budgets;
- sidecar executor after its entry conditions are met.
- a production isolated-worker deployment profile for untrusted Rhai;
- removal of unbounded thread-per-host-call behavior in favor of an
  async-compatible or strictly bounded bridge.

## Local Work Phases

### S1 - Draft/Artifact Runtime Parity

- [x] Add the `AlloyDraft` request path with monotonic revision.
- [x] Move Alloy production execution atomically to the shared runtime.
- [x] Preserve Alloy bindings as Alloy-owned request-scoped extensions.
- Add equivalent draft/published Rhai execution fixtures.

### S2 - Runtime Control and Evidence

- [x] Implement request-scoped cancellation propagation through the runtime,
  Rhai progress callback, Wasmtime epoch watchdog, and capability host.
- [x] Add runtime-scoped global, executor, tenant, and artifact concurrency
  admission controls with automatic permit release.
- [x] Add fallible observer delivery, redaction, and correlation context. The
  artifact owner supplies the durable SeaORM adapter; hosts must attach it when
  durable execution evidence is required.
- Add queue time, execution time, fuel/instruction, memory, output, capability,
  and cache metrics.
- Add one shared in-process/isolated-worker executor placement contract.
- Run untrusted production Rhai in the supervised isolated worker without
  giving it infrastructure clients; keep in-process Rhai an explicit
  local/reviewed profile only.
- [x] Bound synchronous host-call bridging to one native thread per execution.

### S3 - Stable Language/ABI Contracts

- Freeze the Rhai input/output binding used by drafts and artifacts.
- [x] Freeze WIT v1 package/world/entrypoint and JSON/error encoding.
- Define runtime ABI compatibility and cache invalidation.
- Add malformed/untrusted input and component fuzz targets.

### S4 - Capability Hardening

- [x] Enforce subject/tenant/actor consistency on every call before broker
  invocation.
- [x] Add typed HTTP host/method/path-prefix constraints before broker
  invocation.
- [x] Add typed constraints for storage, events, secrets, and MCP.
- [x] Add per-execution capability call-count, input-size, and rolling rate
  budgets before broker invocation.
- Add capability time budgets.
- [x] Emit redacted capability denial evidence through an observer contract.
- [x] Exclude untrusted error text, inputs, outputs, headers, and credentials
  from neutral observer records.
- [x] Ensure adapters receive scoped `SandboxHost` handles without raw
  credentials or platform-global clients.

### S5 - Sidecar Executor

- Start only after audit, cancellation, admission, OCI trust, and WASM paths are
  stable.
- Use a hardened process/container boundary and local versioned RPC.
- Route platform access through the same capability broker.
- Add crash/hang/resource/cancellation/cleanup evidence.

## Local Verification

- Executor registration/selection and stable error-code tests.
- Default-deny and constrained capability tests for every executor.
- Draft/artifact Rhai parity tests.
- Rhai/WASM timeout, fuel, memory, output, cancellation, concurrency, and audit
  tests.
- WIT import/export compatibility and malformed component tests.
- Sidecar process isolation tests when that mode is enabled.
- Untrusted Rhai worker crash/OOM/hang/protocol tests and proof that production
  cannot silently fall back in process.

## Completion Condition

This plan is complete when every enabled untrusted execution mode uses this
runtime, Alloy has no parallel production executor, limits/cancellation/audit
are operational, and the sidecar mode (if enabled) meets the same policy and
evidence contract.

## Update Rules

Update this plan, the neutral sandbox ADR, the central plan, and affected
consumer plans whenever sandbox ownership, ABI, capability, failure, audit,
admission, cancellation, or executor semantics change.
