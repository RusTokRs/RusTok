# rustok-static-distribution-worker

## Purpose

`rustok-static-distribution-worker` is the separately deployed trusted CI
executor boundary for reviewed native module promotions.

## Responsibilities

- Expose the current `rustok.static_distribution` mTLS service.
- Validate the complete owner-claimed immutable distribution work item.
- Require deployment-pinned launcher, job configuration, toolchain, and target
  identities.
- Stage an idempotent bounded job bundle containing deterministic generated
  composition output.
- Share the strict digest-addressed source archive parser from
  `rustok-build-source` with the untrusted module-build worker.
- Invoke only the fixed launcher with an empty environment and bounded timeout.
- Accept only a terminal receipt bound to the exact claim, composition, input,
  generated output, launcher, job configuration, toolchain, and target.
- Parse one strict digest-pinned job configuration and validate the fixed CAS,
  Cargo, Rustc, publisher, publisher configuration, toolchain, target, and
  resource identities during worker readiness.
- Materialize the platform and every promoted source, verify each promoted
  Cargo package/version/lock, and apply generated dependencies, registry source,
  and composition manifest only inside the isolated workspace.
- Regenerate the final workspace lock offline, run only the fixed locked test
  and release-build commands, and bind the resolved lock digest to evidence.
- Invoke only the digest-pinned evidence publisher and accept its receipt only
  when it binds the exact publisher request and all required evidence digests.
- Publish the fixed native executable plus CycloneDX SBOM, SLSA provenance,
  and test-evidence OCI referrers, then sign the exact artifact digest through
  the shared KMS-only Cosign boundary.

## Non-responsibilities

- It does not own the control-plane queue, lease, release ledger, or activation.
- It does not accept caller-selected commands, executables, source paths,
  credentials, registries, or build targets.
- It does not share the untrusted WASM module-build worker process.
- It does not provide a plaintext, in-process, or generation-suffixed service.

## Interactions and entry points

- `src/main.rs` configures the mutually authenticated listener.
- `src/bin/rustok-static-distribution-job.rs` is the fixed native CI launcher
  invoked by the worker.
- `src/bin/rustok-static-distribution-publisher.rs` is the fixed production OCI
  evidence publisher invoked by that launcher.
- `StaticDistributionWorker` implements the `rustok-modules` executor and
  readiness ports.
- The launcher materializes the CAS sources, applies the generated Cargo and
  Rust files inside the isolated platform workspace, runs the trusted CI
  pipeline, calls the deployment-owned evidence publisher, and writes the
  bounded receipt.
- See [local documentation](docs/README.md) and the
  [module control-plane plan](../../docs/modules/module-control-plane-consolidation-plan.md).
