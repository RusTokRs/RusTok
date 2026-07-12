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
- observer pipeline for started/succeeded/failed execution evidence;
- generic Rhai engine/executor with resource and timeout limits;
- request-scoped Rhai host extensions;
- Wasmtime Component Model executor with fuel, epoch deadlines, store limits,
  and no ambient WASI imports;
- typed `rustok:module/host.invoke` WIT import routed through `SandboxHost`;
- installed artifact execution from `rustok-modules`.

Remaining:

- explicit Alloy draft execution through `SandboxRuntime`;
- stable Rhai binding and WIT v1 compatibility contract;
- cancellation and per-tenant/artifact/global concurrency admission;
- durable audit observer and redaction policy;
- production compiled-artifact cache policy and metrics;
- richer capability constraints and call budgets;
- sidecar executor after its entry conditions are met.
- a production isolated-worker deployment profile for untrusted Rhai;
- removal of unbounded thread-per-host-call behavior in favor of an
  async-compatible or strictly bounded bridge.

## Local Work Phases

### S1 - Draft/Artifact Runtime Parity

- Add the `AlloyDraft` request path with monotonic revision.
- Move Alloy production execution atomically to the shared runtime.
- Preserve Alloy bindings as Alloy-owned request-scoped extensions.
- Add equivalent draft/published Rhai execution fixtures.

### S2 - Runtime Control and Evidence

- Implement cancellation/deadline propagation.
- Add per-tenant, artifact, executor, and global admission controls.
- Add durable observer adapter, redaction, and correlation.
- Add queue time, execution time, fuel/instruction, memory, output, capability,
  and cache metrics.
- Add one shared in-process/isolated-worker executor placement contract.
- Run untrusted production Rhai in the supervised isolated worker without
  giving it infrastructure clients; keep in-process Rhai an explicit
  local/reviewed profile only.
- Bound or replace synchronous host-call bridging so guest calls cannot exhaust
  native threads.

### S3 - Stable Language/ABI Contracts

- Freeze the Rhai input/output binding used by drafts and artifacts.
- Freeze WIT v1 package/world/entrypoint and JSON/error encoding.
- Define runtime ABI compatibility and cache invalidation.
- Add malformed/untrusted input and component fuzz targets.

### S4 - Capability Hardening

- Enforce subject/tenant/actor consistency on every call.
- Add typed constraints for HTTP, storage, events, secrets, and MCP.
- Add capability call count, size, rate, and time budgets.
- Ensure adapters receive scoped handles without raw credentials.

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
