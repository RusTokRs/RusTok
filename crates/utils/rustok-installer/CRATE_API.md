# rustok-installer — Crate API

Shared installer foundation for RusToK: install plan, state machine, secret references, receipts, checksums, and preflight policy.

## Public surface

- Plan: `InstallPlan`, `InstallProfile`, `InstallTopology`, `InstallTopologyMode`, `ModuleSelection`, `InstallDistributionRole`, `InstallDistributionRoleArtifact`.
- Execution: `execute_install_apply`, `execute_seed_profile`, `bind_instance_placement`, `prepare_instance_layout`, `InstancePlacement`.
- Preflight and receipts: `VerifiedInstallBaseDistributionReceipt`, `load_base_distribution_receipt`, `InstallReceipt`, `ReceiptError`, `ReceiptOutcome`, `checksum_json`.
- Secrets and seed: re-exported `SecretRef`/`SecretValue`/`SecretMode` contracts and seed request types.
- Environment: `InstallEnvironment`, `DatabaseConfig`, `DatabaseEngine`.

## Invariants

- The crate contains no HTTP, CLI, or database adapter code; wrappers call these typed contracts instead of reimplementing them.
- Secret material is carried as references; redaction (`redact_install_plan`) is applied before plans leave the process.
- Receipts and checksums are authoritative: apply decisions must be derived from verified receipts, not from filesystem inference.
