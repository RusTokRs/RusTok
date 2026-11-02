# rustok-module-build-worker / CRATE_API

## Role

`rustok-module-build-worker` is the dedicated isolated execution daemon for untrusted module Rust builds in the RusToK platform.
It terminates the mTLS gRPC worker transport contract (`ModuleBuildService`), performs comprehensive source policy preflight, validates offline Cargo metadata, and executes builds inside a hardened OCI container runtime (gVisor or Kata) with pinned immutable digests and strict resource ceilings.

The build worker process has **no database client**, **no CAS storage mutation capabilities**, and **no Cargo network egress permissions**.
It materializes source from read-only CAS tarballs, verifies dependency graphs against an immutable `Cargo.lock` SHA-256 digest, validates WebAssembly component interfaces against declared WIT contracts, verifies CycloneDX SBOM and SLSA in-toto provenance evidence, and coordinates with `rustok-build-publication` for KMS-backed Cosign signing.

## Public surface

### Public modules

- `artifact`: Component output verification, SBOM/SLSA provenance inspection, WIT contract validation, and publication bundle collection.
- `materializer`: Out-of-process OCI dependency materialization adapter for offline Cargo caching.
- `policy`: Bounded `Cargo.lock` dependency graph inspection, offline `cargo metadata` validation, and build-script / native-link policy preflight.
- `runner`: `OciJobBuildWorker` daemon runtime implementing `ModuleBuildWorker` and `ModuleBuildWorkerReadiness`.
- `source`: Read-only CAS archive materialization into ephemeral, job-scoped workspaces.

### Primary public types and traits

- `OciJobBuildWorker`: Main build execution daemon binding gRPC transport, OCI launcher, signer, and readiness gates.
- `OciJobRuntime`: Hardened execution runtime enum (`Gvisor`, `Kata`).
- `SourceMaterializer`: Read-only CAS archive materializer ensuring digest binding and symlink safety.
- `MaterializedSource`: RAII guard managing ephemeral job workspace directory lifecycle.
- `CargoMetadataInspector`: Offline `cargo metadata` inspector enforcing locked dependency graphs.
- `SourcePolicyPreflight`: Static preflight analyzer inspecting `Cargo.lock` digests and manifests.
- `OciScopedDependencyMaterializer`: Isolated external dependency materializer enforcing network policies.
- `ComponentArtifactInspector`: WebAssembly component parser and interface extractor.
- `BuildEvidenceInspector`: CycloneDX SBOM and SLSA provenance schema and predicate validator.
- `WitContractInspector`: `wasm-tools` wrapper verifying compiled WebAssembly component exports/imports against WIT.
- `PublicationBundleCollector`: Assembler of signed OCI artifact publication bundles.
- `ArtifactDescriptorFinalizer`: Source manifest loader and descriptor finalizer binding component digest.

## Configuration & Environment

The worker initializes exclusively via `OciJobBuildWorker::from_env(request_timeout)` requiring:
- `RUSTOK_MODULE_BUILD_JOB_LAUNCHER`: Absolute path to the verified OCI launcher binary.
- `RUSTOK_MODULE_BUILD_JOB_LAUNCHER_DIGEST`: Expected SHA-256 digest of the launcher binary.
- `RUSTOK_MODULE_BUILD_JOB_RUNTIME`: Container runtime (`gvisor` or `kata`).
- `RUSTOK_MODULE_BUILD_JOB_IMAGE_DIGEST`: Pinned SHA-256 digest of the hardened build container image.
- `RUSTOK_MODULE_BUILD_WORKDIR`: Absolute path to ephemeral workspace root directory.
- `RUSTOK_MODULE_BUILD_SOURCE_ROOT`: Absolute path to read-only CAS storage mount.
- `RUSTOK_MODULE_BUILD_CARGO`: Absolute path to image-owned Cargo binary.
- `RUSTOK_MODULE_BUILD_CARGO_HOME`: Offline Cargo cache root directory.
- `RUSTOK_MODULE_BUILD_WASM_TOOLS`: Absolute path to `wasm-tools` binary.
- `RUSTOK_MODULE_BUILD_ISOLATION_ATTESTATION`: Path to strict, signed JSON attestation file.
- `RUSTOK_MODULE_BUILD_PUBLICATION_REGISTRY` & `RUSTOK_MODULE_BUILD_PUBLICATION_REPOSITORY`: OCI registry targets.
- `RUSTOK_MODULE_BUILD_REGISTRY_CREDENTIAL_BROKER` & `..._DIGEST`: Credential broker executable and pinned hash.
- `RUSTOK_MODULE_BUILD_COSIGN_PROGRAM`, `..._DIGEST`, `..._KEY_REFERENCE`: Cosign binary, pinned hash, and KMS key URI.

## Minimum Contract Set

### Input DTOs/Commands
- `ModuleBuildRequest`: Complete build specification containing source CAS ref, lockfile digest, toolchain requirements, WIT contract, validation profiles, and resource limits.
- `ModuleBuildLimits`: Strict CPU, memory, disk, process, output, and wall-clock execution limits.
- `ModuleBuildNetworkPolicy`: `Denied` (strict offline) or `ScopedDependencyMaterialization` (controlled endpoint list).

### Domain Invariants
1. **Zero Attestation-Free Initialization**: Worker constructor and readiness gate reload and verify the isolation attestation before every build request.
2. **Immutable Digest Pinning**: All executables (launcher, cosign, credential broker) and container images must match their SHA-256 digests.
3. **No Direct Cargo Network Egress**: Cargo operates exclusively with `--offline` and `CARGO_NET_OFFLINE=true`.
4. **Strict Job Workspace Isolation**: Workspaces are created under UUID subdirectories with parent-boundary enforcement and auto-deleted via RAII `Drop`.
5. **No Secret or Database Leakage**: Worker attestation strictly validates `!tenant_database_access` and `!general_platform_secret_access`.
6. **Strict Output Budgeting**: Process output streams (stdout, stderr) are bounded by atomic token budgets to prevent memory exhaustion attacks.

### Events / Outbox Side Effects
- Emits no domain outbox events directly; worker returns typed terminal `ModuleBuildResult` over gRPC to `rustok-modules`, which owns transactional persistence and event emission.

### Errors / Failure Codes
- `SourceMaterializationError`: `DigestMismatch`, `UnsafeArchive`, `ResourceLimit`, `Internal`.
- `DependencyMaterializationError`: `EndpointDenied`, `ResourceLimit`, `Internal`.
- `SourcePolicyError`: `DependencyPolicyDenied`, `BuildScriptDenied`, `NativeLinkDenied`, `Internal`.
- `CargoMetadataError`: `DependencyPolicyDenied`, `BuildScriptDenied`, `NativeLinkDenied`, `ResourceLimit`, `NetworkPolicyDenied`, `Internal`.
- `ComponentArtifactError`: `InspectionFailed`, `ResourceLimit`, `Internal`.
- `BuildEvidenceError`: `SbomInvalid`, `ProvenanceInvalid`, `ResourceLimit`, `Internal`.
- `WitContractError`: `Mismatch`, `ResourceLimit`, `Internal`.
- `PublicationBundleError`: `Invalid`, `ResourceLimit`, `Internal`.
- `ArtifactDescriptorError`: `Invalid`, `ResourceLimit`, `Internal`.
- `ModuleBuildFailureCode`: Strongly-typed diagnostic codes returned in `ModuleBuildResult`.

## Verification & Test Coverage

Verified via:
- Comprehensive unit tests across `policy`, `artifact`, `runner`, and `contract_tests`.
- `cargo check -p rustok-module-build-worker`.
- `cargo test -p rustok-module-build-worker`.
- `cargo clippy -p rustok-module-build-worker --all-targets`.
- Gatekeeper validation via `python scripts/verify/verify-remediation-gate.py`.
