# rustok-artifact-node-controller / CRATE_API

## Role

`rustok-artifact-node-controller` is the dedicated operations daemon hosting the mutual TLS gRPC listener for artifact-node assignment claims, heartbeats, and status reports.
It bridges the node agents (`rustok-artifact-node-agent`) to the owner-side control plane (`ModuleControlPlane::artifact_node_agent()`).

The controller operates under strict boundary guarantees:
- It maintains **no sandbox execution capability**, **no CAS storage mutation rights**, **no topology mutation API**, and **no application-server business logic**.
- It authenticates agents exclusively via mutual TLS: each incoming request's peer client certificate fingerprint (SHA-256) is mapped to an authorized `ModuleArtifactNodeAgentIdentity` (`node_id`, `agent_id`).
- It binds request concurrency per connection and enforces strict request deadlines via `WorkerAdmission`.
- It executes strictly outside application-server bundles as an independent operational control-plane process.

## Public surface

### Public modules

- `config`: Configuration loading and certificate fingerprint mapping (`ArtifactNodeControllerConfig`).

### Primary public types

- `ArtifactNodeControllerConfig`: Deployment configuration holding the sanitized owner database URL and the authenticated static agent identity registry.

## Configuration & Environment

The controller binary (`rustok-artifact-node-controller`) requires the following environment variables:
- `RUSTOK_ARTIFACT_NODE_CONTROLLER_DATABASE_URL`: Connection URL for the owner-side database.
- `RUSTOK_ARTIFACT_NODE_CONTROLLER_AGENT_IDENTITIES_JSON`: Non-empty JSON array mapping peer certificate fingerprints to node and agent identities:
  ```json
  [
    {
      "certificate_fingerprint": "sha256:<64 hex>",
      "node_id": "<uuid>",
      "agent_id": "<string>"
    }
  ]
  ```
- Listener mTLS variables (under prefix `RUSTOK_ARTIFACT_NODE_CONTROLLER`):
  - `_LISTEN_ADDR`: Socket bind address (`ip:port`).
  - `_SERVER_CERT_PEM`: PEM-encoded server certificate chain.
  - `_SERVER_KEY_PEM`: PEM-encoded server private key.
  - `_CLIENT_CA_PEM`: PEM-encoded client CA trust bundle.
  - Optional: `_CONCURRENCY_LIMIT`, `_REQUEST_TIMEOUT_MS`, `_ADMISSION_TIMEOUT_MS`, `_MAX_MESSAGE_SIZE`.

## Minimum Contract Set

### Input DTOs/Commands
- `AgentIdentityConfig`: JSON structure containing `certificate_fingerprint`, `node_id`, and `agent_id` with `deny_unknown_fields`.

### Domain Invariants
1. **Strict Mutual TLS Authentication**: Plaintext RPC is prohibited; all incoming agent calls are authenticated by validating the peer certificate's SHA-256 fingerprint.
2. **Canonical Fingerprint Hygiene**: Certificate fingerprints must be lowercase `sha256:<64 hex>` strings; duplicate or malformed fingerprints fail startup.
3. **Non-Nil Principal Binding**: Node IDs must be valid non-nil UUIDs; empty or whitespace agent IDs are rejected at startup.
4. **Credential Redaction**: `Debug` representation of `ArtifactNodeControllerConfig` redacts sensitive database connection strings.
5. **Bounded Admission & Timeouts**: Database connection timeout is set to 30s (`DATABASE_CONNECT_TIMEOUT`); requests are bounded by `WorkerAdmission` limits.

### Events / Outbox Side Effects
- Does not publish domain events directly; assignment updates, leases, and reconciliation statuses are persisted to the owner database via `ModuleControlPlane`.

### Errors / Failure Codes
- Configuration parse errors returned as descriptive `String` errors.
- gRPC errors mapped to tonic `Status` via `ArtifactNodeGrpcService`.

## Verification & Test Coverage

Verified via:
- Manifest standardization assertions in `src/contract_tests.rs`.
- Contract assertions covering `CRATE_API.md` required sections and debug representations.
- Unit tests in `src/config.rs`:
  - Canonical certificate fingerprint validation.
  - Rejection of duplicate fingerprints and unknown fields.
  - Non-empty and non-nil identity requirements.
- Scoped verification: `cargo test -p rustok-artifact-node-controller`.
- Scoped linter: `cargo clippy -p rustok-artifact-node-controller --all-targets`.
- Gatekeeper verification: `python scripts/verify/verify-remediation-gate.py --staged`.
