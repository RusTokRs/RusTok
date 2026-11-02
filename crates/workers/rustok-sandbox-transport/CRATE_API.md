# `rustok-sandbox-transport` Crate API & Architecture Contract

## 1. Architectural Purpose & Boundaries

`rustok-sandbox-transport` provides the production-grade gRPC streaming transport between host applications (e.g. `apps/server`, `rustok-artifact-node-agent`) and isolated sandbox workers (`rustok-sandbox-worker`).

### Key Invariants
- **mTLS & Authenticated Channels**: All remote sandbox executions flow across mutually authenticated TLS connections with client certificate pinning.
- **Bi-Directional Streaming**: Executes a single session per stream: the host sends initial `SandboxRequestPayload` and answers `CapabilityRequest`s; the worker executes the script and yields capability callbacks and the final `SandboxOutcome`.
- **Fail-Closed Isolation**: The client enforces strict wall-clock timeouts. If the worker hangs or the stream disconnects unexpectedly, the execution aborts without falling back to in-process execution.
- **Bounded Message Limits**: Max gRPC frame size is bounded at 72 MiB (`SANDBOX_WORKER_MAX_MESSAGE_SIZE`), supporting the 64 MiB payload ceiling plus metadata framing.

---

## 2. Wire Protocol & Streaming State Machine

Defined in `proto/rustok/sandbox/sandbox.proto`:
- `SandboxWorkerService`:
  - `rpc Execute(stream HostFrame) returns (stream WorkerFrame);`
  - `rpc GetReadiness(ReadinessRequest) returns (ReadinessResponse);`

### 2.1 Frames
- `HostFrame`:
  - `protocol_revision`: Exact match required (`SANDBOX_WORKER_PROTOCOL_REVISION == 1`).
  - `execution_id`: Must match the active session UUID.
  - Variants: `request_payload`, `capability_result`, `cancel_execution`.
- `WorkerFrame`:
  - `protocol_revision`: Exact match required (`SANDBOX_WORKER_PROTOCOL_REVISION == 1`).
  - `execution_id`: Must match the active session UUID.
  - Variants: `capability_request`, `outcome_payload`, `error_payload`.

### 2.2 Execution Concurrency
- `SandboxWorkerGrpcService` uses an internal 1-permit semaphore (`execution_permit`) to ensure that a single worker process executes at most one untrusted sandbox script at a time, preventing cross-tenant side-channel leakage.

---

## 3. Worker Identity & Deployment Readiness

`SandboxWorkerIdentity`:
- `executor_binary_digest`: Canonical `sha256:...` digest of the worker binary.
- `engine_build_digest`: Pinned Rhai/WASM engine build digest.
- `worker_image_digest`: Container image digest.
- `engine_config_digest`: Engine limits & configuration digest.
- `target_cpu_contract`: Instruction set architecture contract (e.g., `x86_64-v3`).
- `runtime_abi`: ABI identifier (e.g., `rustok:module/runtime@1`).
- `isolation_runtime`: Execution environment descriptor (e.g., `containerd`, `bwrap`, `nsjail`).

Hosts query `GetReadiness` and verify:
1. `response.ready == true`
2. `response.executor == "rhai"`
3. `response.protocol_revision == 1`
4. Complete non-empty identity evidence for governed pool registration.

---

## 4. Universal Logic Dimensions Verification

1. **Preconditions & Bounds**:
   - Frames exceeding 72 MiB are rejected before decoding.
   - Non-UUID execution IDs, nil UUIDs, or mismatched execution IDs immediately abort the session.
   - Protocol revision mismatches fail fast with `Code::InvalidArgument`.
2. **Branch Completeness & Anti-Swallowing**:
   - Worker disconnects and capability callback drops are propagated as typed `SandboxError::Aborted`.
   - Cancellation requests immediately cancel execution contexts without dropping error responses.
3. **State Transition Integrity**:
   - Session starts with exactly one `request_payload` frame; subsequent frames can only be `capability_result` or `cancel_execution`. Any other frame sequence triggers fail-closed abortion.
   - 1-permit execution semaphore guarantees exclusive attempt lifecycle.
4. **Side-Effect Completeness**:
   - Capability calls are mediated through the host-side `SandboxHost` router, guaranteeing that tenant, actor, and policy constraints are applied before any host infrastructure is invoked.
5. **Algorithmic Correctness**:
   - Monotonic call ID sequencing (`AtomicU64`) prevents callback crosstalk. Overflow at `u64::MAX` returns a structured `LimitExceeded` error.
6. **Order of Operations & Concurrency**:
   - Initial request validation and limits admission occur before spawning the worker execution task.
   - Reader and executor tasks communicate through bounded oneshot/mpsc channels and terminate promptly upon completion or cancellation.
