# Implementation Plan for `rustok-sandbox`

## Scope

Provide the single neutral execution contract used equally by Alloy drafts and
installed module artifacts. The sandbox owns execution mechanics and evidence,
not module identity, marketplace state, installation, build, or Alloy authoring
workflow. It owns only the neutral canonical Rhai workspace representation used
at execution boundaries.

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
- canonical bounded Rhai workspace/import resolution, serialized scope records,
  standard functions, and request-scoped broker-backed host extensions;
- Wasmtime Component Model executor with fuel, epoch deadlines, store limits,
  and no ambient WASI imports;
- bounded node-local LRU cache of serialized compiled Components keyed by
  Wasmtime version, host target, admitted runtime ABI, and artifact digest;
- typed `rustok:module/host.invoke` WIT import routed through `SandboxHost`;
- installed artifact execution from `rustok-modules`.
- local authoring/test harness over the same runtime request, policy, execution,
  cancellation, and error contracts with an explicit fixture-only capability
  broker and no infrastructure clients; the bounded scenario envelope binds
  input, typed grants/limits, exact fixture responses, and success/error
  expectations, and local WASM authoring uses the real Component executor.
- mandatory executor placement registration: every caller selects
  `in_process` or `isolated_worker`, duplicate kinds are rejected across
  placements, and runtime readiness exposes the selected placement without a
  fallback.
- generated bidirectional tonic worker transport with exact revision/identity
  framing, broker callbacks, cancellation, deadline, and no local fallback.
- standalone product-neutral Rhai worker with mTLS, one execution per process,
  and startup/readiness/request admission against a digest-pinned gVisor/Kata
  isolation attestation;
- shared cgroup v2 memory observation for Rhai worker readiness and truthful
  request peak-memory outcomes;
- canonical Kubernetes deployment renderer with digest-pinned gVisor/Kata,
  exact mTLS health probes, portable pod hardening, restricted ingress, and
  default-deny egress;
- artifact and Alloy server composition through one shared isolated Rhai worker
  client; Wasmtime remains explicitly in process.

Remaining:

- executor cache-observation metrics;
- richer capability constraints and call budgets;
- sidecar executor after its entry conditions are met.
- retained worker containment/supervisor evidence, including cluster-owned PID
  and file-limit enforcement;

## Local Work Phases

### S1 - Draft/Artifact Runtime Parity

- [x] Add the `AlloyDraft` request path with monotonic revision.
- [x] Move Alloy production execution atomically to the shared runtime.
- [x] Preserve Alloy bindings through neutral serialized constants/records and
  output changes, without loading Alloy code into the worker.
- [x] Execute canonical workspace imports and mutable record changes through
  the same neutral Rhai executor contract.

### S2 - Runtime Control and Evidence

- [x] Implement request-scoped cancellation propagation through the runtime,
  Rhai progress callback, Wasmtime epoch watchdog, and capability host.
- [x] Enforce the configured wall-clock deadline in every enabled executor:
  Rhai uses its progress callback and Wasmtime a request-private epoch
  watchdog, both mapped to the common timeout error.
- [x] Add runtime-scoped global, executor, tenant, and artifact concurrency
  admission controls with automatic permit release.
- [x] Add fallible observer delivery, redaction, and correlation context. The
  artifact owner supplies the durable SeaORM adapter; hosts must attach it when
  durable execution evidence is required.
- Queue time, execution time, Rhai instruction/Wasmtime fuel, output size, and
  policy-admitted capability-call metrics are emitted on terminal records.
  Wasmtime reports observed aggregate non-shared guest linear-memory peak while
  excluding failed growth. The isolated Rhai worker reports its observed cgroup
  peak and fails closed when that evidence is unavailable. Executor
  cache-observation metrics remain pending without substituting configured
  limits for usage.
- [x] Add one shared in-process/isolated-worker executor placement contract.
  Registration is explicit and atomic across all current callers, duplicate
  kinds cannot create a fallback across placements, and runtime readiness
  exposes the selected placement.
- [x] Route every current untrusted production Rhai path through the isolated
  worker without giving it infrastructure clients. Artifact and Alloy
  composition share one readiness-checked mTLS client and have no fallback.
  Hardened deployment and supervisor evidence remain open separately.
- [x] Bound synchronous host-call bridging to one native thread per execution.

### S3 - Stable Language/ABI Contracts

- [x] Freeze the strict `RhaiBindingInput`/`RhaiBindingOutput` v1 JSON envelope
  used by drafts and artifacts. Unknown fields, another version, and raw JSON
  compatibility paths are rejected at the executor boundary.
- [x] Freeze the current external WIT package/world/entrypoint and JSON/error
  encoding. `rustok-module-sdk` owns the canonical file and Bytecode Alliance
  macros generate both guest and host bindings from it.
- [x] Define exact runtime ABI compatibility and bounded compiled-component
  cache invalidation. Serialized Components are keyed by Wasmtime version,
  target, admitted runtime ABI, and artifact digest; capacity uses LRU eviction
  and a corrupted cache value is removed before recompilation.
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

- Current placement slice: 13 neutral runtime-contract tests, seven streaming
  transport tests, and three worker-isolation tests pass. The transport tests
  cover capability callbacks, typed errors, cancellation, hang/deadline,
  serialized Rhai scope input/output, disconnect, readiness loss, and protocol
  mismatch. No server or
  workspace-wide compile claim is made.
- Seven focused Rhai executor tests pass for raw source, canonical workspace
  imports, serialized mutable-record changes, immutable-record denial,
  brokered HTTP allow/default-deny, instruction pressure, and deadline mapping.
  Three focused scope-contract tests cover valid bounded bindings, duplicate
  names, and reserved host bindings. The focused Alloy test command did not finish compiling
  within the bounded 60-second window, so no Alloy compile/test claim is made.
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
