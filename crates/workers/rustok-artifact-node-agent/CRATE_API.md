# rustok-artifact-node-agent / CRATE_API

## Role

`rustok-artifact-node-agent` is the independent node-local materialization, readiness, and execution-slot supervisor for dynamic RusToK module artifacts.
It connects to the central cluster control plane via mTLS (`rustok-artifact-node-transport`), claims assigned artifact deployments for its designated node, retrieves verified payloads from content-addressed storage (CAS), stages them into local runtime cache directories, and runs comprehensive readiness gates.

The agent operates under strict architectural isolation boundaries:
- It maintains **no database client**, **no secret-store access**, **no tenant topology mutation capability**, and **no server-side business logic**.
- It communicates with the node controller exclusively via mTLS gRPC over `rustok-artifact-node-transport`.
- It executes untrusted Rhai scripts only via an out-of-process sandbox (`rustok-sandbox-transport`) and validates WebAssembly components via `wasm-tools`/wasmtime runtime checks.
- It supervises side-by-side execution slots (`Slot A` and `Slot B`) enabling zero-downtime traffic switching with retained hot-standby predecessor slots.
- It runs an out-of-process node watchdog (`NodeWatchdog`) providing automatic, single-attempt rollback upon candidate crashes or probe threshold failures.
- It coordinates fenced worker generation handoffs (`FencedWorkerGenerationCoordinator`) preventing concurrent job claims during rollouts.

## Public surface

### Public modules

- `config`: Environment configuration parsing and bounds validation (`ArtifactNodeAgentConfig`).
- `slot_supervisor`: Side-by-side execution slot supervisor, HTTP/SSR cutover coordinator, and fenced worker generation coordinator.
- `watchdog`: Outside-candidate observation watchdog, health probe tracking, and automatic crash recovery receipts.

### Primary public types and traits

- `ArtifactNodeAgent<C, M>`: Main daemon loop claiming assignments, maintaining active leases via background heartbeats, and reporting reconciliation phases.
- `ArtifactNodeAgentConfig`: Environment-derived configuration for controller and sandbox endpoints, node identity, storage runtime, and polling intervals.
- `ArtifactNodeAssignmentController`: Async trait abstracting assignment claiming, heartbeat lease renewals, and report submissions.
- `ArtifactNodeMaterializer`: Async trait abstracting payload download, local materialization, and runtime readiness verification.
- `StorageArtifactNodeMaterializer`: Production materializer combining CAS blob storage, instance filesystem layouts, and sandbox execution workers.
- `NodeArtifactPreparation`: Verified artifact preparation descriptor containing immutable SHA-256 `runtime_fingerprint`.
- `ArtifactNodeMaterializationError`: Typed error distinguishing retryable infrastructure errors from terminal payload/policy failures.
- `ArtifactNodeAgentError`: Typed top-level agent lifecycle and communication errors.
- `DeploymentSlot`: Enum representing node-local execution slots (`SlotA`, `SlotB`).
- `SlotState`: Lifecycle state of an execution slot (`Empty`, `PreStaged`, `Serving`, `Standby`).
- `SlotSupervisor`: State machine managing transitions and traffic switching across Slot A and Slot B.
- `HttpSsrSwitchingCoordinator`: Proxy router coordinator enforcing pre-switch safety, atomic cutover, and bounded rollback.
- `FencedWorkerGenerationCoordinator`: Multi-generation coordinator guaranteeing mutually exclusive worker execution.
- `NodeWatchdog`: Active health monitor observing candidate execution windows and triggering recovery on failure.
- `WatchdogConfig`, `WatchdogStatus`, `WatchdogRecoveryReceipt`: Configuration and outcome receipts for watchdog monitoring.

## Configuration & Environment

The agent binary (`rustok-artifact-node-agent`) requires the following environment variables:
- `RUSTOK_ARTIFACT_NODE_AGENT_CONTROLLER_ENDPOINT`: HTTPS endpoint (`https://...`) of the node controller.
- `RUSTOK_ARTIFACT_NODE_AGENT_SANDBOX_ENDPOINT`: HTTPS endpoint (`https://...`) of the sandbox worker daemon.
- `RUSTOK_ARTIFACT_NODE_AGENT_NODE_ID`: UUID uniquely identifying the physical or virtual host node.
- `RUSTOK_ARTIFACT_NODE_AGENT_ID`: Unique string identifier for the running agent instance.
- `RUSTOK_ARTIFACT_NODE_AGENT_STORAGE_CONFIG_JSON`: Valid JSON configuration for the CAS storage backend.
- `RUSTOK_ARTIFACT_NODE_AGENT_POLL_INTERVAL_MS`: Optional polling interval (default: 1,000 ms, min: 100 ms, max: 60,000 ms).
- `RUSTOK_ARTIFACT_NODE_AGENT_HEARTBEAT_INTERVAL_MS`: Optional lease heartbeat interval (default: 30,000 ms, min: 1,000 ms, bounded by assignment lease).
- Client mTLS variables: `RUSTOK_ARTIFACT_NODE_AGENT_CONTROLLER_*` and `RUSTOK_ARTIFACT_NODE_AGENT_SANDBOX_*` for mutual TLS certificates and trust anchors.

## Minimum Contract Set

### Input DTOs/Commands
- `ModuleArtifactNodeAssignmentWorkItem`: Dispatched unit of work containing assignment specification, claim ID, and reconciliation metadata.
- `ModuleArtifactNodeAssignmentReport`: Terminal or progress report submitted back to the controller.

### Domain Invariants
1. **Node Identity Pinning**: Agents reject assignments whose `node_id` does not strictly match their provisioned `ModuleArtifactNodeAgentIdentity`.
2. **Zero-Attempt Pre-Switch Protection**: Candidate failures occurring during pre-staging or isolated boot consume 0 recovery attempts; predecessor serving capacity remains 100% untouched.
3. **Bounded Post-Switch Rollback**: Automated traffic or worker generation rollback consumes exactly 1 attempt; subsequent rollback requests fail closed.
4. **Fenced Worker Exclusivity**: Active worker generations must be fenced before candidate generation authorization, preventing duplicate job consumption.
5. **Masked Configuration Secrets**: Storage backend credentials and connection strings are redacted in `Debug` representations.

### Events / Outbox Side Effects
- Does not publish domain events directly; submits typed `ModuleArtifactNodeAssignmentReport` DTOs to the controller over mTLS gRPC.

### Errors / Failure Codes
- `ArtifactNodeAgentError`: `InvalidConfiguration`, `Controller`, `AssignmentIdentityMismatch`, `UnexpectedAssignmentPhase`, `Retryable`, `EvidenceEncoding`.
- `SlotSupervisorError`: `CandidateNotReady`, `PredecessorNotAvailable`, `InvalidTransition`.
- `ArtifactNodeMaterializationError`: `Retryable { detail }`, `Terminal { code, detail }`.

## Verification & Test Coverage

Verified via:
- Manifest standardization assertions in `src/contract_tests.rs`.
- Contract assertions covering `CRATE_API.md` required sections and debug implementations.
- Unit tests covering configuration parsing, interval limits, and HTTPS scheme enforcement.
- Unit tests covering CAS validation, payload media-type checks, and unsupported runtime rejection.
- End-to-end integration tests in `tests/slot_supervisor_and_watchdog_tests.rs`:
  - Full side-by-side rollout with health promotion.
  - Watchdog crash recovery with immutable receipt generation.
  - Pre-switch failure verification (0 recovery attempts consumed).
  - Post-switch recovery and single-attempt rollback limits.
  - Fenced worker generation handoff and symmetric rollback.
- Scoped verification: `cargo test -p rustok-artifact-node-agent`.
- Scoped linter: `cargo clippy -p rustok-artifact-node-agent --all-targets`.
- Gatekeeper verification: `python scripts/verify/verify-remediation-gate.py`.
