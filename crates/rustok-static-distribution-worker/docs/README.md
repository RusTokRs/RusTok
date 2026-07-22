# Static distribution worker

The worker is a trusted native-distribution CI boundary. It is intentionally a
different process from `rustok-module-build-worker`, which compiles untrusted
module source into sandbox artifacts.

## Runtime contract

Startup requires the following fixed deployment configuration:

- `RUSTOK_STATIC_DISTRIBUTION_JOB_LAUNCHER` and its
  `RUSTOK_STATIC_DISTRIBUTION_JOB_LAUNCHER_DIGEST`;
- `RUSTOK_STATIC_DISTRIBUTION_JOB_CONFIG` and its
  `RUSTOK_STATIC_DISTRIBUTION_JOB_CONFIG_DIGEST`;
- `RUSTOK_STATIC_DISTRIBUTION_WORK_ROOT`;
- `RUSTOK_STATIC_DISTRIBUTION_TOOLCHAIN_DIGEST`;
- `RUSTOK_STATIC_DISTRIBUTION_BUILD_TARGET`;
- the `RUSTOK_STATIC_DISTRIBUTION_*` mTLS listener settings owned by
  `rustok-worker-transport`.

The launcher and configuration must be absolute non-symlink regular files and
must match their lowercase SHA-256 digests at startup, readiness, and execution.
The work root must be an absolute non-symlink directory. Readiness fails closed
when any of these identities changes.

The strict job-config JSON also pins the CAS root, Cargo and Rustc executables,
Cargo home, evidence publisher and its configuration, toolchain identity,
build target, archive/extraction/entry bounds, aggregate extracted-byte cap,
and command timeout. The worker parses that config and re-hashes every fixed
file during readiness; unknown JSON fields fail closed. The per-command
deadline may not exceed the worker's aggregate execution deadline.

For each immutable owner claim, the worker creates one stable attempt directory
and writes bounded create-only inputs. An exact retry verifies and reuses those
bytes; conflicting content fails closed. The fixed launcher receives only fixed
argument names and paths, runs with a cleared environment, has no standard
input/output channels, and is killed when its bounded execution future is
dropped or times out.

A fixed `rustok-static-distribution-job` launcher is included in this crate. It
materializes only the exact CAS identities from the job request through
`rustok-build-source`, applies the generated dependency fragment and registry
source inside that materialized platform workspace, and executes only these
Cargo operations with the digest-pinned executable and Rustc:

1. `cargo generate-lockfile --offline`;
2. `cargo test --locked --offline --workspace --all-targets --target <target>`;
3. `cargo build --locked --offline --workspace --release --target <target>`.

The final workspace `Cargo.lock` is required to be a bounded regular file. Its
raw digest is bound into the test evidence, publisher request, and publisher
receipt so the merged composition graph remains an auditable release input.
Cargo receives a cleared environment, a fixed configuration-free Cargo home,
a job-local home and target directory, closed standard streams, and the
job-config deadline.

After successful tests and build, the launcher invokes only the digest-pinned
evidence publisher with fixed request, workspace, test-evidence, config, and
receipt arguments. The publisher owns artifact, SBOM, provenance, signature,
and test-evidence publication. Its create-only receipt must bind the exact
publisher-request digest, job request, generated output, composition, resolved
lock, and test evidence. Publication must be idempotent by publisher-request
digest because an owner reclaim can repeat the call after an infrastructure
interruption. A valid existing publisher receipt is verified and reused; it is
never overwritten.

The launcher writes a terminal job receipt only for immutable source-policy,
lock-resolution, test, or build outcomes, or after a valid publication receipt.
A missing, malformed, mismatched, oversized, or symlink receipt is a transport
failure and remains reclaimable by the owner lease; it is never converted into
a successful or terminal build fact.

The launcher regenerates every input byte from the immutable work item,
materializes the platform and reviewed sources into a new job-local workspace,
enforces per-source and aggregate limits, verifies each promoted
`Cargo.toml` package/version and raw `Cargo.lock` digest, rejects dependency
alias collisions, and writes the generated Cargo graph, registry source, and
composition manifest only in that isolated workspace. A reclaim removes only
the known job-owned derived workspace and regenerates it from the unchanged
create-only inputs; it never mutates the source archives or owner job bundle.

Attempt directories are durable retry/evidence inputs and are not removed by
the request path. Deployment retention tooling may collect them only after the
owner attempt is terminal and the evidence retention policy permits deletion.

## Verification

Target verification includes crate compilation and receipt/launcher integration
tests. During the current shared-worktree implementation only the explicitly
allowed formatting, diff, and metadata checks are run.
