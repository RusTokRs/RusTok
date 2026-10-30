# rustok-artifact-node-reconciler / CRATE_API

## Role

`rustok-artifact-node-reconciler` is the dedicated operations daemon hosting the mutual TLS gRPC listener for artifact-node topology authoring and reconciliation requests.
It accepts operator-issued desired-topology snapshots and delegates them to the durable `rustok-modules` control-plane database under strict transactional validation.

The reconciler process operates under isolated boundary constraints:
- It maintains **no sandbox execution capability**, **no CAS storage mutation rights**, **no node-agent claim/heartbeat endpoints**, and **no application-server business logic**.
- It authenticates operators exclusively via mutual TLS: each request's peer client certificate fingerprint (SHA-256) is mapped to an authorized audit actor (`actor_id`) and an explicit allowed node set (`allowed_node_ids`).
- It binds request concurrency and enforces strict request deadlines via `WorkerAdmission`.
- It executes strictly outside application-server bundles as an independent operational control-plane process.

## Public surface

### Public modules

- `config`: Configuration loading and certificate fingerprint mapping (`ArtifactNodeReconcilerConfig`).

### Primary public types

- `ArtifactNodeReconcilerConfig`: Deployment configuration holding the sanitized owner database URL and the authenticated static operator identity registry.

## Configuration & Environment

The reconciler binary (`rustok-artifact-node-reconciler`) requires the following environment variables:
- `RUSTOK_ARTIFACT_NODE_RECONCILER_DATABASE_URL`: Connection URL for the owner-side database.
- `RUSTOK_ARTIFACT_NODE_RECONCILER_OPERATOR_IDENTITIES_JSON`: Non-empty JSON array mapping peer certificate fingerprints to actor identities and allowed node scopes:
  ```json
  [
    {
      "certificate_fingerprint": "sha256:<64 hex>",
      "actor_id": "<uuid>",
      "allowed_node_ids": ["<uuid>", "<uuid>"]
    }
  ]
  ```
- Listener mTLS variables (under prefix `RUSTOK_ARTIFACT_NODE_RECONCILER`):
  - `_LISTEN_ADDR`: Socket bind address (`ip:port`).
  - `_SERVER_CERT_PEM`: PEM-encoded server certificate chain.
  - `_SERVER_KEY_PEM`: PEM-encoded server private key.
  - `_CLIENT_CA_PEM`: PEM-encoded client CA trust bundle.
  - Optional: `_CONCURRENCY_LIMIT`, `_REQUEST_TIMEOUT_MS`, `_ADMISSION_TIMEOUT_MS`, `_MAX_MESSAGE_SIZE`.

## Minimum Contract Set

### Input DTOs/Commands
- `OperatorIdentityConfig`: JSON structure containing `certificate_fingerprint`, `actor_id`, and `allowed_node_ids` with `deny_unknown_fields`.

### Domain Invariants
1. **Strict Mutual TLS Authentication**: Plaintext RPC is prohibited; all incoming operator requests are authenticated by validating the peer certificate's SHA-256 fingerprint.
2. **Canonical Fingerprint Hygiene**: Certificate fingerprints must be lowercase `sha256:<64 hex>` strings; duplicate or malformed fingerprints fail startup.
3. **Actor and Node Scope Pinning**: Requests cannot self-declare an audit actor in the RPC payload; target nodes must be a subset of the operator's configured `allowed_node_ids`.
4. **Credential Redaction**: `Debug` representation of `ArtifactNodeReconcilerConfig` redacts sensitive database connection strings.
5. **Bounded Admission & Timeouts**: Database connection timeout is set to 30s (`DATABASE_CONNECT_TIMEOUT`); requests are bounded by `WorkerAdmission` limits.

### Events / Outbox Side Effects
- Does not publish domain events directly; topology reconciliations, desired assignment sets, and outbox events are transactionally committed by `ModuleControlPlane`.

### Errors / Failure Codes
- Configuration parse errors returned as descriptive `String` errors.
- gRPC errors mapped to tonic `Status` via `ArtifactNodeReconciliationGrpcService`.

## Verification & Test Coverage

Verified via:
- Manifest standardization assertions in `src/contract_tests.rs`.
- Contract assertions covering `CRATE_API.md` required sections and debug representations.
- Unit tests in `src/config.rs`:
  - Canonical certificate fingerprint validation.
  - Rejection of duplicate fingerprints, empty node scopes, and unknown fields.
  - Actor non-nil UUID requirements.
- Scoped verification: `cargo test -p rustok-artifact-node-reconciler`.
- Scoped linter: `cargo clippy -p rustok-artifact-node-reconciler --all-targets`.
- Gatekeeper verification: `python scripts/verify/verify-remediation-gate.py --staged`.
