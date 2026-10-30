# `rustok-verification-worker` API & Architecture Contract

## Role
Isolated execution boundary for OCI artifact trust verification. Operates as an independent background worker outside `apps/server` and runtime modules. Verifies Cosign signatures, SLSA provenance attestations, CycloneDX Software Bills of Materials (SBOM), and license/vulnerability policies against platform-defined trust roots before artifact admission.

## Public surface
- `CosignTrustVerifier`: Concrete adapter for running bounded Cosign verification commands against OCI artifacts and parsing in-toto statements.
- `VerificationPolicy`: Strongly typed verification policy specifying active and retiring trust roots, allowed builders, repositories, refs, build types, licenses, CycloneDX versions, and vulnerability severity ceilings.
- `VerificationTrustRoots`, `VerificationTrustRoot`, `VerificationRetiringTrustRoot`: Active trust root specification (Keyless Sigstore with OIDC issuers or KMS key reference) with optional time-bounded retiring root for zero-downtime key rotation.
- `VerificationWorker<V>`: Generic verification orchestrator enforcing request policy revisions, allowed signers, and full evidence admission preconditions before delegating to inner verifier.
- `VerificationWorkerError`: Strongly typed failure reasons (`PolicyRevisionMismatch`, `SignerDenied`, `RequiredEvidenceMissing`).
- `VerificationGrpcService`: Re-exported from `rustok-verification-transport` for gRPC listener composition.
- `MutualTlsListenerConfig`: Re-exported from `rustok-worker-transport` for strict mTLS-only socket binding.

## Wire Protocol & Operational Framing
- Communicates exclusively over mTLS with the control plane (`rustok-verification-transport`).
- Strict fail-closed verification: process arguments are passed directly via `tokio::process::Command` without shell interpretation.
- Child process lifecycle bounded with 30s timeout and `kill_on_drop(true)` ensuring orphaned processes are cleaned up upon cancellation or client disconnect.

## Minimum Contract Set

### Input DTOs/Commands
- `TrustVerificationRequest`: Carries `OciArtifactReference`, `trust_policy_revision`, `capability_policy_revision`, and optional `expected_alloy_workspace_provenance`.
- `VerificationPolicy`: Mounted via `RUSTOK_VERIFICATION_POLICY_JSON` environment configuration and strictly validated at worker startup.

### Domain Invariants
- **Digest Pinning**: The OCI artifact reference MUST contain a valid lowercase SHA-256 digest (`sha256:<64 hex>`) matching the attestation subject.
- **Fail-Closed Admission**: In-toto statements MUST match required predicate types (`https://slsa.dev/provenance/v1`, `https://cyclonedx.org/bom`).
- **Policy Revision Match**: Incoming request policy revisions MUST strictly match the worker's mounted policy revisions.
- **Signer Authorization**: The verified signer identity MUST match the active trust root or an unexpired retiring trust root.
- **Transitive License Hygiene**: All declared CycloneDX component licenses MUST be members of `allowed_licenses`.
- **Vulnerability Threshold**: No reported vulnerability rating may exceed `maximum_vulnerability_severity`.
- **Alloy Provenance Binding**: When `expected_alloy_workspace_provenance` is present, SLSA external parameters MUST match tenant ID, script ID, source revision, source digest, review digest, descriptor digest, and entrypoint.

### Events / Outbox Side Effects
- The verification worker is strictly stateless and emits no events or direct database mutations; it returns typed verification decisions and cryptographic evidence digests to the orchestrator.

### Errors / Failure Codes
- `TrustVerificationError::InvalidRequest`: Policy revision mismatch, unauthorized signer, or missing mandatory evidence.
- `TrustVerificationError::Unavailable`: Process timeout, failure to start Cosign, or missing/rejected trust root.

## Verification & Test Coverage
- Unit and contract tests verify SLSA fixture validation, CycloneDX license/vulnerability enforcement, Alloy workspace receipt binding, expiring trust root cutoff, malformed payload rejection, and Cargo/API documentation compliance.
