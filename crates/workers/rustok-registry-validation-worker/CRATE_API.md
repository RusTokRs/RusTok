# `rustok-registry-validation-worker` API & Architecture Contract

## Role
Independent durable worker for origin-aware registry artifact validation. Operates outside `apps/server` as an autonomous worker process polling validation queues, verifying artifact payloads,Deriving OCI bundles, enforcing supply-chain policies, and writing terminal audit outcomes.

## Public surface
- `RegistryValidationWorker`: Worker process runtime handling queue polling, artifact fetching, and result persistence.
- `RegistryValidationPublicationPolicy`: Strongly typed supply chain policy configuration covering registry ID, trust/capability policy revisions, and build service issuer/revision identities.
- `RegistryValidationAlloyPublication`: Deployment-owned adapter bundle for turning reviewed Rhai workspaces into signed OCI packages.
- `CredentialedOciRegistryProvider`: Short-lived credential broker adapter providing authenticated registry clients.

## Operational Framing & Execution Lifecycle
1. **Queue Claiming**: Worker polls `claim_next_validation_job` and binds claims to its authenticated service principal.
2. **Bounded Artifact Retrieval**: Fetches artifact from configured storage runtime using a bounded retry schedule (`[1s, 3s, 5s]`). Upon exhaustion, terminalizes the job failure in the governance store without throwing unhandled exceptions.
3. **Payload Verification**: Enforces exact byte-length checks and SHA-256 digest matching before parsing.
4. **Origin-Aware Route Enforcement**:
   - `PlatformBuilt`: Reloads build record, acquires registry credential lease, verifies SLSA and SBOM via mTLS-gated isolated verifier (`rustok-verification-worker`), and records build-service attestation.
   - `AlloyAuthored`: Re-canonicalizes Rhai workspace, derives CycloneDX and SLSA provenance, publishes to OCI registry, signs via KMS Cosign, and admits through independent platform verification.
   - `ExternalPrebuilt`: Verifies package contracts without build service attestation.
5. **Durable Result Application**: Applies terminal results via `apply_validation_job_result` with structured automated checks, deduplicated warnings, and error reasons.

## Minimum Contract Set

### Input DTOs/Commands
- `ModuleValidationJobWorkItem`: Immutable job description carrying artifact size, checksum, origin, and request revision.
- `RegistryValidationPublicationPolicy`: Pinned configuration mounted from environment variables.

### Domain Invariants
- **Checksum & Size Pinning**: Uploaded artifacts must match the claimed work item size and SHA-256 digest exactly; mismatches reject validation.
- **Fail-Closed Retries**: Artifact fetch failures are retried up to 3 times before failing the validation job permanently (`ArtifactLoadOutcome::Terminalized`).
- **No Uncontrolled Execution**: Untrusted WASM or Rhai scripts are never executed inside the validation worker.
- **Credential Ephemerality**: Registry credential leases require positive TTL and are never leaked to external callers.

### Events / Outbox Side Effects
- Updates validation job states in `rustok-modules` database tables via `SeaOrmModuleGovernanceService`.

### Errors / Failure Codes
- `RegistryCredentialError`: Credential broker failure (rejected, timed out, or unavailable).
- `SignedOciArtifactPublicationError`: OCI publication failure (rejected, timed out, or unavailable).
- `ModulePublicationRegistryError`: Invalid request or registry interaction errors.

## Verification & Test Coverage
- Unit tests verify Cargo manifest standardization, API contract completeness, publication policy validation, check structure generation, and debug formatting.
