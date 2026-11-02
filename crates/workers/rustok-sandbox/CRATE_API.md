# `rustok-sandbox` Crate API & Architecture Contract

## 1. Architectural Purpose & Boundaries

`rustok-sandbox` provides the neutral, multi-tenant sandbox execution engine for untrusted guest scripts across RusToK. It serves as the common runtime foundation for:
1. **Alloy Dynamic Extensions & Drafts**: In-process or isolated Rhai scripting with fine-grained capability checks.
2. **Untrusted Wasm Components**: Wasmtime Component Model runtime (`rustok:module@1.0.0` WIT world) executing untrusted module artifacts under strict linear memory and instruction limits.
3. **Execution Fleet Workers**: Isolated execution supervised through `rustok-sandbox-transport` and `rustok-sandbox-worker`.

### Sovereignty & Non-Interference
- **Default-Deny Capability Policy**: The guest environment has no direct access to network, filesystem, process execution, environment variables, or credentials.
- **Controlled Host Bridging**: All external interactions (`platform.http`, `platform.secrets`, `platform.events`, `platform.data`, `platform.data.objects`, `platform.mcp`) require explicit, typed `CapabilityGrant` entries.
- **Fail-Closed Resource Metering**: Both wall-clock deadlines and execution budgets (instruction fuel / operations) are strictly monitored via progress hooks and watchdog threads.
- **Cooperative Cancellation**: Supports `SandboxCancellation` propagated down to script progress callbacks and watchdog interrupts.

---

## 2. Feature Flags

| Feature | Dependencies | Purpose |
|---|---|---|
| `rhai` | `rhai` (sync, metadata), `parking_lot`, `email_address` | Rhai scripting engine with workspace module imports, scope bindings, and helper stdlib. |
| `wasm-component` | `wasmtime` (runtime, component-model, cranelift) | Wasmtime Component Model runtime with WIT host imports, fuel metering, and linear-memory tracking. |

Default build has minimal dependencies for host contracts, types, and capability definitions.

---

## 3. Core Types & Invariants

### 3.1 Subject & Request Invariants
- `SandboxSubject`: Either `AlloyDraft { draft_id: Uuid, revision: u64 }` or `ModuleArtifact { installation_id: Uuid, slug: String, version: String, digest: String }`.
- `installation_id`: Must not be nil (`00000000-0000-0000-0000-000000000000`).
- `digest`: Expected in lowercase `sha256:<64-hex>` format.
- `runtime_abi`: Stable ABI identifier (e.g. `rustok:module/runtime@1` or `v1`).
- `entrypoint`: Declared entrypoint in the payload (e.g., `main` or `run`).

### 3.2 Admission & Concurrency Limits
`SandboxAdmissionLimits`:
- `global`: Default 64 concurrent executions.
- `per_executor`: Default 32 concurrent executions.
- `per_tenant`: Default 16 concurrent executions.
- `per_artifact`: Default 8 concurrent executions.

Each admission acquires an RAII `AdmissionPermit` which automatically decrements and clears state counts on drop.

### 3.3 Resource Limits (`SandboxLimits`)
- `wall_clock_ms`: Default 100 ms (max 10,000 ms in authoring profile).
- `instruction_budget`: Default 50,000 instructions/fuel.
- `max_memory_bytes`: Default 64 MB (max 256 MB in authoring profile).
- `max_output_bytes`: Default 1 MB (max 4 MB in authoring profile).
- `max_capability_calls`: Default 16 calls per execution.
- `max_capability_input_bytes`: Default 64 KB.
- `max_capability_calls_per_second`: Default 16 calls/sec.

---

## 4. Capability System & Policy Enforcement

### 4.1 Capability Grants & Names
- `CapabilityName`: Bounded string (<= 96 bytes) containing only lowercase ASCII letters, digits, underscores, dots, and colons.
- `CapabilityCall`: Enforces context equality before routing. The execution ID, subject, and call context (phase, tenant ID, actor ID) must exactly match the admitted request context.

### 4.2 Platform Capabilities
1. `platform.http`:
   - Enforces exact host matching, uppercase method matching, path prefix filtering, and rejection of userinfo in URLs.
   - Requires HTTPS by default; plain HTTP is allowed only if explicitly enabled in grant.
2. `platform.secrets`:
   - Names allowed logical reference handles and operations (e.g., `acquire_handle`).
   - Secret values are never returned to the sandbox.
3. `platform.events`:
   - Enforces permitted publication topics (exact or terminal wildcard `.*`). Global wildcard `*` is prohibited.
4. `platform.data`:
   - Restricts operations to key prefixes ending with `/`.
   - Operations allowed: `get`, `put`, `put_batch`, `delete`, `list`, `query_index`.
5. `platform.data.objects`:
   - Restricts object operations to logical prefixes ending with `/`.
   - Never exposes physical storage buckets, URLs, or filesystem paths.
6. `platform.mcp`:
   - Validates discrete `(server, tool)` pairs. Arbitrary discovery or endpoint selection is prohibited.

---

## 5. Universal Logic Dimensions Verification

1. **Preconditions & Bounds**:
   - Zero-values, nil UUIDs, empty media types, and empty digests are rejected at request validation.
   - Resource limits and admission quotas fail fast before invoking executor code.
2. **Branch Completeness & Anti-Swallowing**:
   - No silent suppression via `let _ = ...` in capability dispatch.
   - All errors are mapped into strongly-typed `SandboxError` variants with stable machine-readable codes.
3. **State Transition Integrity**:
   - `AdmissionPermit` guarantees monotonic and leak-free counter decrement even during executor panics or drops.
   - Watchdogs safely signal completion through atomic flags before epoch interruption.
4. **Side-Effect Completeness**:
   - Every capability attempt emits redacted `CapabilityAuditRecord` (denied or succeeded) to registered observers without exposing secrets or guest payloads.
   - Execution records capture queue time, wall-clock duration, instructions consumed, peak linear memory, and capability calls.
5. **Algorithmic Correctness**:
   - Rate limiting uses sliding-window timestamp queues without integer overflow or clock drift exploitation.
   - Memory tracking handles failed allocations gracefully by reverting pending growth counters.
6. **Order of Operations & Concurrency**:
   - Pre-broker admission checks (call budget, rate limit, context match) execute before any IO or external broker invocation.
   - Blocking native bridges are strictly restricted to 1 active thread per execution context via atomic compare-and-swap.
