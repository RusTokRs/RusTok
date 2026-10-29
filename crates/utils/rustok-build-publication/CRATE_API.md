# rustok-build-publication / CRATE_API

## Public surface

### Primary Adapters & Types
- `CommandRegistryCredentialBroker`: acquires scoped repository credentials by invoking digest-pinned broker programs.
- `RegistryCredentialBroker`: trait for scoped credential acquisition.
- `RegistryCredentialLease`: in-memory short-lived OCI credential lease.
- `CosignArtifactSigner`: cryptographic artifact signer invoking digest-pinned Cosign with KMS references.
- `CosignAttestationPredicate`: attestation predicate descriptors (SLSA statement, CycloneDX SBOM).
- `SignedOciArtifactPublicationReceipt`: publication confirmation containing OCI digest, signature digest, and attestation refs.
- `publish_signed_oci_artifact(...)`: orchestrates publication and Cosign signing of an OCI artifact.

### Helper & Validation Functions
- `validate_fixed_program(path: &Path, expected_sha256: &str) -> Result<PathBuf, RegistryCredentialError>`: verifies programs are non-symlink regular files matching exact expected SHA-256 hashes.

## Invariants

- **Safe Code Boundary**: Forbids unsafe code (`#![forbid(unsafe_code)]`).
- **Digest-Pinned Program Verification**: Signer and broker executables are re-hashed before every execution against explicit expected SHA-256 digests.
- **Short-Lived In-Memory Credentials**: Credentials reside exclusively in memory with lease-bounded lifespans; no raw private keys or file-backed key material permitted.
- **Hermetic Process Environment**: Subprocesses are executed with cleared environments, closed standard streams, and bounded timeouts.
