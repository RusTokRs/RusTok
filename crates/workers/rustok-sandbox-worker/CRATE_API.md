# `rustok-sandbox-worker` Crate API & Architecture Contract

## 1. Architectural Purpose & Boundaries

`rustok-sandbox-worker` is the dedicated worker binary and deployment runtime crate responsible for executing isolated, guest-submitted Rhai sandbox scripts within RusToK's execution fleet.

Key responsibilities:
- Serves as the binary entrypoint (`rustok-sandbox-worker`) and health check probe (`rustok-sandbox-worker-probe`) for isolated sandbox execution.
- Enforces strict process isolation policies backed by microVM/sandboxed runtimes (gVisor runsc or Kata Containers).
- Verifies deployment attestations at boot and during continuous readiness checks, ensuring denied network egress, read-only root filesystems, and strict bounded CPU, memory, PID, and storage limits.
- Samples host cgroup v2 memory consumption throughout request lifetimes to enforce quota tracking and return verifiable peak memory metrics.
- Exposes worker readiness, limits admission, and worker identity facts through `rustok-sandbox-transport` and `rustok-worker-transport` gRPC mTLS endpoints.

## 2. Hardened Runtime Isolation & Attestation

The crate strictly forbids operating in untrusted or unverified container environments.

1. **Isolation Runtime**: Requires either `gvisor` or `kata` as the specified container runtime via `RUSTOK_SANDBOX_RUNTIME`.
2. **Image Digest Binding**: Verifies that `RUSTOK_SANDBOX_IMAGE_DIGEST` matches the canonical `sha256:...` digest of the container image.
3. **Mounted Attestation Proof**:
   - Reads an absolute path pointed to by `RUSTOK_SANDBOX_ISOLATION_ATTESTATION`.
   - Rejects symlinks, directories, empty files, or files exceeding `MAX_ATTESTATION_BYTES` (16 KiB).
   - Validates JSON format denying unknown fields (`#[serde(deny_unknown_fields)]`).
   - Asserts:
     - `protocol_revision == 1`
     - `privileged == false`
     - `host_mounts == false`
     - `container_socket == false`
     - `host_pid == false`
     - `host_network == false`
     - `network_mode == "rpc_only"`
     - `ingress_mode == "mtls_grpc"`
     - `egress_denied == true`
     - `database_access == false`
     - `secret_access == false`
     - `read_only_root == true`
     - Bounded resource envelope: CPU (100–4000 ms), Memory (64 MiB–512 MiB), PIDs (4–64), Ephemeral Storage (64 MiB–256 MiB), Max File Size (1–64 MiB), Wall-clock timeout (1–30,000 ms).

## 3. Cgroup v2 Memory Observation & Peak Sampling

`WorkerMemoryObserver` continuously samples `/sys/fs/cgroup/memory.current` during execution.
- **Fail-Closed Observation**: If cgroup memory reading is unavailable or produces an error, both readiness probes and in-flight executions fail closed with an internal error rather than returning unmetered execution metrics.
- **Concurrent Polling**: Spawns a dedicated asynchronous background task sampling every 1ms while guest execution runs.
- **Peak Tracking**: Atomic compare-and-exchange loop (`AtomicU64`) ensures that observed peak memory is monotonically increasing without thread contention races.
- **Observed Execution Wrapper**: `ObservedRhaiExecutor<E>` decorates any underlying `SandboxExecutor`, injecting measured peak memory bytes into `SandboxOutcome.metrics.peak_memory_bytes`.

## 4. Readiness Probing & Pool Composition

Worker health and capacity are exposed via the `SandboxWorkerReadiness` trait:
- `check_readiness()`: Validates that deployment isolation attestation is intact and readable, and cgroup v2 memory observation is live.
- `admit_limits(limits)`: Verifies that requested execution limits do not exceed the deployment's attested ceiling.
- `worker_identity()`: Computes deterministic SHA-256 digests over:
  - Worker binary executable (`std::env::current_exe()`).
  - Container image digest.
  - Attestation envelope configuration digest.
  - Target CPU architecture and runtime ABI.
This enables host-side controller pools (`rustok-sandbox`) to reliably detect worker configuration drift and recycle stale workers.

## 5. Universal Logic Dimensions Verification

1. **Preconditions & Bounds**:
   - Attestation file paths must be absolute regular non-symlink files $\le$ 16 KiB.
   - Resource limits strictly bounded within hard numeric ranges (CPU, memory, PIDs, storage, wall-clock ms).
   - Request limits admitted only if non-zero, within attestation limits, and `max_concurrency == 1`.
2. **Branch Completeness & Anti-Swallowing**:
   - Missing attestation, corrupted JSON, or non-matching digests propagate explicit errors.
   - Cgroup probe errors immediately signal `failed` atomic flag and abort with `SandboxError::Internal`.
   - Probe binary exits with non-zero code on readiness failure.
3. **State Transition Integrity**:
   - Memory sampling task cleanly terminated on completion via `stop` atomic flag and `sampler.await`.
   - Single-permit worker model prevents inter-request state leakage or metric pollution.
4. **Side-Effect Completeness**:
   - Memory metrics accurately populated in execution outcomes.
   - Probe binary cleanly drops temp files and handles termination signals.
5. **Algorithmic Correctness**:
   - Lock-free atomic peak memory tracking with `compare_exchange_weak`.
   - Secure hex-encoded SHA-256 digest validation.
6. **Order of Operations & Concurrency**:
   - Isolation policy and cgroup observation verified before any guest payload execution.
   - Read-only root and denied egress verified before gRPC listener accepts requests.
