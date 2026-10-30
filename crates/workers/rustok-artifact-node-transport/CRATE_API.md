# rustok-artifact-node-transport / CRATE_API

## Role

`rustok-artifact-node-transport` provides typed mutual TLS gRPC client and server adapters for the owner-owned ports:
- `ArtifactNodeService`: Node agent surface for claiming, heartbeating, and reporting assignments.
- `ArtifactNodeReconciliationService`: Operator surface for submitting desired topology snapshots.

The crate contains the protobuf schema definitions (`proto/rustok/artifact_node/artifact_node.proto`), client transport abstractions, server adapters, and strict error status mapping.
It operates strictly as a transport boundary:
- It maintains **no CAS storage mutation capabilities**, **no sandbox execution capabilities**, and **no local build container invocation**.
- It enforces **no plaintext or unauthenticated fallback**; connections require mutual TLS (`connect_with_tls`) with client certificates, CA roots, and worker domain validation.
- It provides strict isolation between node agents (who can only claim and report assignments) and deployment operators (who author desired topology snapshots).

## Public surface

### Public modules

- `proto`: Generated protobuf service and message definitions.
- `client`: `GrpcArtifactNodeAgent` client implementation wrapping tonic gRPC channel with thread-safe `Mutex`.
- `server`: `ArtifactNodeGrpcService` agent-facing gRPC server adapter, authenticator traits, and static registries.
- `reconciliation_server`: `ArtifactNodeReconciliationGrpcService` operator-facing gRPC server adapter and operator authenticators.

### Primary public types, traits, and constants

- `ARTIFACT_NODE_AGENT_PROTOCOL_REVISION`: Constant identifying the current external framing revision (`1`).
- `ARTIFACT_NODE_AGENT_MAX_MESSAGE_SIZE`: Maximum allowable message size for agent payloads (`1 MiB`).
- `ARTIFACT_NODE_RECONCILIATION_MAX_MESSAGE_SIZE`: Maximum allowable message size for reconciliation payloads (`1 MiB`).
- `GrpcArtifactNodeAgent`: Client adapter for node agents claiming assignments, renewing lease heartbeats, and submitting status reports.
- `ModuleArtifactNodeAgentIdentity`: Verified agent identity containing `node_id` (non-nil UUID) and bounded `agent_id`.
- `ModuleArtifactNodeAgentAuthenticator`: Trait for authenticating agent client certificate fingerprints.
- `StaticModuleArtifactNodeAgentAuthenticator`: Immutable in-memory registry mapping certificate fingerprints to agent identities.
- `ArtifactNodeGrpcService<P, A>`: Server adapter exposing `ArtifactNodeService` bounded by `WorkerAdmission`.
- `ModuleArtifactNodeReconciliationIdentity`: Verified operator identity containing audit `actor_id` and bounded `allowed_node_ids`.
- `ModuleArtifactNodeReconciliationAuthenticator`: Trait for authenticating operator client certificate fingerprints.
- `StaticModuleArtifactNodeReconciliationAuthenticator`: Immutable in-memory registry mapping certificate fingerprints to operator identities.
- `ArtifactNodeReconciliationGrpcService<A>`: Server adapter exposing `ArtifactNodeReconciliationService` bounded by `WorkerAdmission`.
- `ClientTlsConfig`: Re-exported from tonic for mTLS endpoint configuration.

## Wire Protocol & Framing

- **Transport:** HTTP/2 over mTLS with ALPN.
- **Framing:** Protobuf message envelopes carrying UTF-8 JSON payloads (`work_item_json`, `report_json`, `topology_json`, `receipt_json`) to retain Rust-owned domain types and structural validation across RPC boundaries.
- **Status Codes:**
  - `InvalidArgument`: Malformed JSON, non-UUID identifiers, or invalid command structures.
  - `PermissionDenied`: Unrecognized certificate fingerprints, mismatched report principals, or node scope violations.
  - `Aborted`: Concurrency conflicts, lease conflicts, or revision conflicts.
  - `Unavailable`: Database or owner service disconnections.
  - `FailedPrecondition`: Ineligible reconciliation state, protocol revision mismatch, or expired assignments.
  - `Internal`: Response serialization failures.

## Minimum Contract Set

### Input DTOs/Commands
- `ClaimAssignmentRequest`: Protocol revision handshake for claiming pending node work.
- `HeartbeatAssignmentRequest`: Protocol revision and claim ID for active assignment lease renewal.
- `ReportAssignmentRequest`: Protocol revision and JSON-encoded `ModuleArtifactNodeAssignmentReport`.
- `ReconcileTopologyRequest`: Desired topology snapshot, policy revision, expected revision, idempotency key, and tracing context.

### Domain Invariants
1. **Strict Mutual TLS Enforcement**: The only public connection constructor is `connect_with_tls(endpoint, tls_config)`; unencrypted transport is strictly prohibited.
2. **Actor and Principal Derivation from Certificates**: Request bodies cannot choose an audit actor or agent identity; principals are derived exclusively from the verified leaf certificate SHA-256 fingerprint.
3. **Strict Protocol Versioning**: Callers must match `ARTIFACT_NODE_AGENT_PROTOCOL_REVISION`; mismatches fail closed with `FailedPrecondition`.
4. **Principal and Node Scope Verification**: Reports must match the authenticated agent principal; topology submissions must only target nodes within the operator's authorized `allowed_node_ids`.
5. **Deterministic Protobuf Generation**: Built via `build.rs` leveraging `protoc-bin-vendored` for hermetic builds across all environments.

### Events / Outbox Side Effects
- Does not emit events directly; delegates to the owner control plane which commits assignments and outbox events transactionally.

### Errors / Failure Codes
- `Status`: gRPC status codes generated by `owner_status` from `ModuleArtifactNodeReconciliationError`.

## Verification & Test Coverage

Verified via:
- Manifest standardization assertions in `src/contract_tests.rs`.
- Contract assertions covering `CRATE_API.md` required sections and debug implementations.
- Unit tests in `src/server.rs`:
  - Static authenticator uniqueness and fingerprint validation.
  - Identity validation (non-nil UUID, bounded length).
  - Protocol revision checks.
  - Report principal mismatch rejection.
- Unit tests in `src/reconciliation_server.rs`:
  - Bounded node scope validation (1 to 1,024 nodes).
  - Operator certificate mapping and authorization.
  - Topology scope enforcement against foreign nodes.
  - Submitted topology resolver and policy fencing.
- Scoped verification: `cargo test -p rustok-artifact-node-transport`.
- Scoped linter: `cargo clippy -p rustok-artifact-node-transport --all-targets`.
- Gatekeeper verification: `python scripts/verify/verify-remediation-gate.py --staged`.
