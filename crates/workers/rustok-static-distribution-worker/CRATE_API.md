# `rustok-static-distribution-worker` API & Architecture Contract

## Role
Trusted CI executor and evidence publisher boundary for reviewed native module promotions. Compiles native static distributions, generates CycloneDX SBOMs and in-toto SLSA provenance attestations, signs native artifacts via KMS-backed Cosign, and publishes release bundles to the OCI registry.

## Public surface
- `StaticDistributionWorker`: Core gRPC-backed worker orchestrating job staging, launcher execution, and terminal receipt validation.
- `StaticDistributionJobRequest`, `StaticDistributionJobReceipt`: Canonical schemas for launcher invocation and terminal completion reporting.
- `StaticDistributionJobConfig`, `StaticDistributionJobError`, `StaticDistributionJobPaths`: Job execution configuration, error types, and argument paths for `rustok-static-distribution-job`.
- `StaticDistributionPublisherConfig`, `StaticDistributionPublisherError`, `StaticDistributionPublisherPaths`: Publisher configuration, error types, and argument paths for `rustok-static-distribution-publisher`.
- `StaticDistributionTestEvidence`, `StaticDistributionPublisherRequest`, `StaticDistributionPublicationReceipt`: Schemas for test results, publication requests, and final cryptographic publication receipts.
- `materialize_static_distribution_workspace`, `run_static_distribution_job`, `run_static_distribution_publisher`: Functional entrypoints executing workspace materialization, cargo pipelines, and evidence publication.

## Operational Framing & Execution Lifecycle
1. **Admission & Identity Verification**: Verifies that work item toolchain, build target, launcher digest, job config digest, and active deployment identities match.
2. **Deterministic Workspace Materialization**: Extracts CAS source archives with strict size and entry bounds; applies generated dependency fragments, registry sources, and composition manifests within an isolated attempt directory.
3. **Offline Fixed Cargo Pipeline**:
   - `cargo generate-lockfile --offline`
   - `cargo test --locked --offline --workspace --all-targets --target <target>`
   - `cargo build --locked --offline --workspace --release --target <target>`
4. **Evidence Generation & OCI Publication**:
   - Generates CycloneDX 1.6 SBOM from the resolved `Cargo.lock` with sorted component `bom-ref`s.
   - Generates SLSA Provenance v1 in-toto statement binding the builder, buildType, and parameter digests.
   - Publishes native binary and evidence referrers to the configured OCI registry with short-lived repository credentials.
   - Signs the published artifact digest using KMS-only Cosign without local raw keys.
5. **Terminal Receipt & Cleanup**: Produces an immutable, digest-bound job receipt and promptly cleans up intermediate build workspaces.

## Minimum Contract Set

### Input DTOs/Commands
- `StaticDistributionJobRequest`: Inputs passed from worker to launcher.
- `StaticDistributionJobConfig`: Strict JSON configuration pinning CAS root, toolchains, and timeouts.
- `StaticDistributionPublisherConfig`: Strict JSON configuration pinning OCI target and KMS key reference.

### Domain Invariants
- **Digest Pinning & Anti-Tamper**: Launcher, job configuration, toolchain, cargo, rustc, and publisher binaries must match their lowercase SHA-256 digests (`verify_file_digest`).
- **Offline & Locked Pipeline**: Cargo commands must run with `--locked --offline` and closed standard streams with a cleared environment.
- **Single Job Concurrency per Attempt**: `ActiveJobGuard` enforces in-process attempt exclusivity preventing concurrent execution of identical build IDs and attempt numbers.
- **Fail-Closed Verification**: Any mismatch in generated file content, receipt digests, or timeout results in terminal error without retrying corrupted state.
- **Workspace Isolation**: Derived workspaces are isolated per attempt and removed upon error or successful terminal publication.

### Events / Outbox Side Effects
- Emits no direct database transactions; returns terminal receipts containing published OCI artifact and evidence digests to the orchestrator.

### Errors / Failure Codes
- `ModuleStaticDistributionExecutorError::Rejected`: Invalid work item, toolchain mismatch, or input size exceeding limits.
- `ModuleStaticDistributionExecutorError::Transport`: Process launch failure, timeout, or missing/corrupted receipt.
- `StaticDistributionJobError`: Config validation, source extraction, I/O, or command execution failures.
- `StaticDistributionPublisherError`: Registry publication, SBOM generation, or Cosign signing errors.

## Verification & Test Coverage
- Unit tests verify Cargo manifest standards, API contract completeness, job/publisher configuration validation invariants, debug formatting, and CycloneDX/SLSA generation logic.
