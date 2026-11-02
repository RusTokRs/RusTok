# `rustok-build-publication` Crate API & Architecture Contract

## 1. Architectural Purpose & Boundaries

`rustok-build-publication` defines the shared, secret-minimizing publication and signing boundary for RusToK packages and OCI container artifacts. It provides isolated workers with digest-pinned credential brokers and cryptographic Cosign signing capabilities.

Key responsibilities:
- Acquires short-lived, scoped OCI registry credentials via external broker binaries (`CommandRegistryCredentialBroker`, `RegistryCredentialBroker`).
- Manages in-memory ephemeral credential leases (`RegistryCredentialLease`) with bounded TTL and safe Docker config generation.
- Executes cryptographic container signing with Cosign (`CosignArtifactSigner`) strictly backed by Cloud KMS key references (AWS KMS, GCP KMS, Azure Key Vault, HashiCorp Vault).
- Normalizes and attaches signed in-toto attestations (`CosignAttestationPredicate`) for SLSA Provenance v1 and CycloneDX SBOMs.
- Orchestrates the hermetic, digest-pinned OCI publication pipeline (`publish_signed_oci_artifact`).

## 2. Bounded Credential Broker & Short-Lived Leases

To eliminate ambient credentials and secret sprawl in worker processes:
1. **Digest-Pinned Broker Execution**:
   - The credential broker program must be an absolute, regular, non-symlink file matching its pre-configured SHA-256 hash (`validate_fixed_program`).
   - Broker execution runs in an isolated subprocess with `env_clear()`, standard streams piped or nulled, and `kill_on_drop(true)`.
2. **Short-Lived Ephemeral Leases**:
   - `RegistryCredentialLease` holds in-memory credentials bounded by `MAX_CREDENTIAL_LEASE_SECONDS` (15 minutes).
   - Generates temporary, single-use `config.json` directories (`0700` permission on Unix) for Cosign authentication, immediately deleted upon sign/attest completion via cleanup guards.
   - `Debug` implementation redacts password strings (`<redacted>`).

## 3. Cosign Signing & In-Toto Attestation Normalization

- **KMS-Only Key Policy**: Rejects local raw private keys or file paths; requires explicit URI schemes (`awskms://`, `azurekms://`, `gcpkms://`, `hashivault://`).
- **Standardized Attestation Predicates**:
  - `SlsaProvenance`: `https://slsa.dev/provenance/v1`
  - `CycloneDx`: `https://cyclonedx.org/bom`
- **Predicate Normalization**:
  - Validates that SLSA evidence contains valid `_type == "https://in-toto.io/Statement/v1"`, matching predicate type, and non-empty subject.
  - Extracts the inner predicate payload so that `cosign attest --predicate` constructs the final canonical in-toto statement over the OCI manifest subject without nested double-enveloping.
  - Temporary predicate files are written with `create_new(true)` and cleaned up immediately after invocation.

## 4. Hermetic OCI Publication Pipeline

`publish_signed_oci_artifact(...)`:
- Verifies target parameters and readiness of credentials and signer before initiating network operations.
- Enforces a global execution deadline (`Instant::now() + execution_timeout`); remaining duration is partitioned across credential acquisition, artifact publication, Cosign signing, SLSA attestation, and CycloneDX attestation.
- Returns `SignedOciArtifactPublicationReceipt` with pinned digests for both the package artifact and the Cosign signature manifest.
- Translates underlying transport and CLI errors into bounded error variants (`Rejected`, `TimedOut`, `Unavailable`) without leaking credential material or subprocess output.

## 5. Universal Logic Dimensions Verification

1. **Preconditions & Bounds**:
   - Program binaries and digests validated prior to execution; rejects symlinks and invalid SHA-256 strings.
   - Credential lease TTL clamped to $[1, 900]$ seconds; broker output bounded to 16 KiB.
   - Timeouts strictly non-zero and tracked via monotonic deadlines.
2. **Branch Completeness & Anti-Swallowing**:
   - Malformed SLSA statements or invalid CycloneDX JSON rejected immediately before calling Cosign.
   - Process spawn, I/O pipe, and exit status failures map to explicit, typed `RegistryCredentialError` or `CosignSigningError`.
3. **State Transition Integrity**:
   - Subprocesses managed with `kill_on_drop(true)` to prevent zombie processes on task cancellation.
   - Temporary auth directories and predicate files reliably unlinked in both success and error paths.
4. **Side-Effect Completeness**:
   - Temporary auth and predicate directories created in system temp with randomized UUIDs and restrictive permissions (`0700`/`0600`).
   - Clean unlinking prevents credential persistence across build jobs.
5. **Algorithmic Correctness**:
   - Strict SHA-256 streaming verification of binaries before launch.
   - Safe deadline remaining calculation preventing negative underflow.
6. **Order of Operations & Concurrency**:
   - Artifact pushed to OCI registry and digest resolved before signing begins.
   - Signature manifest and attestations attached in sequence before returning the verified publication receipt.
