# Module authoring CLI

This command family is the WASM Component/archive authoring path. It does not
package Rhai: reviewed Rhai releases flow from an exact Alloy revision to one
canonical bounded-workspace object through the generic source-CAS owner, never
through `module package` or a `.tar` wrapper.

## Commands

```text
rustok-cli module init <path> --slug <snake_case> [--name <display name>] [--version <semver>]
rustok-cli module validate <path>
rustok-cli module test <path> [--scenario <project-relative-json>]
rustok-cli module build <path> --tenant-id <uuid> --actor-id <identity> --project-id <identity> --trace-id <identity> --correlation-id <identity> --idempotency-key <identity>
rustok-cli module package <path> --output <archive.tar>
rustok-cli module publish <path> --tenant-id <uuid> --actor-id <identity> --build-request-id <uuid> --trace-id <identity> --correlation-id <identity> --idempotency-key <uuid> --name <display-name> --description <plain-text> --license <spdx-expression> [--default-locale <locale>] [--category <token>] [--tags <comma-separated-tokens>]
rustok-cli module inspect <project-or-archive>
```

`module init --dry-run` validates the identity and reports the exact file set
without changing the filesystem. A real initialization requires a previously
absent target whose parent already exists, writes only renderer-owned relative
paths with create-new semantics, invokes pinned Cargo to generate `Cargo.lock`,
then runs the same validation as `module validate`. Any failure removes only
the newly created target; failure to remove it is reported explicitly.

`module validate` is an authoring preflight, not an admission bypass. The
isolated worker repeats source, dependency, Component, WIT, SBOM, provenance,
and publication checks under its immutable request and hardened job boundary.

`module test` runs format, locked host tests, Component-target Clippy, and a
release `wasm32-wasip2` build with offline Cargo, an exact pinned toolchain, a
sanitized environment, a shared two-MiB output budget, and bounded stage
timeouts. It then rehashes the regular Component and executes it through the
real Wasmtime executor in `LocalSandboxHarness`. The default
`tests/sandbox-scenario.json` (or a safe project-relative override) supplies the
input, exact typed grants/limits, deterministic capability responses, and
expected success output or stable error code. Dry-run validates all project and
scenario contracts without invoking Cargo or mutating `target`.

This local result is author feedback, not trusted build or admission evidence.
Remote build, WIT inspection, SBOM/provenance, signing, and publication remain
worker-owned.

`module build` validates the project, creates a deterministic private archive,
and submits it through the owner-composed `ModuleAuthoringBuildControl`. Every
request carries explicit tenant, actor, project, trace, correlation, and
idempotency identity. The owner rehashes and strictly scans the archive. The
current implementation publishes it atomically as `<sha256-hex>.tar` in the
configured source CAS; the accepted release-safety cutover replaces that
archive-specific writer and layout with the `rustok-modules` preparation
owner's single `SourceObjectStore` while this command remains its
archive-specialized client. No dual lookup survives. The owner selects the
fixed dependency-egress, resource, validation, WIT, ABI, and target
policy, and commits the immutable request plus transactional outbox fact. The
command never calls a worker. The independent dispatcher consumes that fact
and invokes the isolated worker through mTLS.

The operations CLI runtime requires a database and `RUSTOK_INSTANCE_ROOT`.
Module build always uses the canonical `<instance-root>/sources` subtree; there
is no independent source-CAS path setting.

The control-plane process needs write access to this directory. Build jobs see
the same content as a read-only mount through
`RUSTOK_MODULE_BUILD_SOURCE_ROOT`. `module build --dry-run` validates local
inputs without requiring either runtime or creating an archive.

`module publish` accepts only a completed tenant-scoped platform build. It
constructs the canonical bounded JSON metadata bundle from the current
`module-artifact.json` and `Cargo.toml`, while the owner fixes
`platform_built`, `third_party`, `sandboxed`, no native UI packages, and the
actor principal shape. The owner revalidates the bundle, stores it in the
platform object store under its SHA-256 identity, creates an idempotent publish
request, binds the exact completed build/OCI receipt, and queues isolated
registry validation. The bundle checksum is intentionally distinct from the
WASM Component payload digest and OCI manifest digest.

This command submits a governance review; it cannot approve, admit, or finalize
a release. Build-service attestation and platform admission remain reserved for
the production OCI verification path, and marketplace approval remains an
operator review transition. `module publish --dry-run` validates the project,
metadata, UUID identities, and serialized bundle without requiring a database
or object storage.

Non-dry-run publication also requires the platform storage driver configuration
in `RUSTOK_SETTINGS_JSON`. For the local driver, the CLI always derives
`<instance-root>/storage`; `base_dir` is not an independent operator setting:

```json
{
  "storage": {
    "driver": "local",
    "local": {
      "base_url": "/media",
      "fsync": true
    }
  }
}
```

The current dependency profile is fail-closed to checksummed crates.io
packages with no Git dependencies, build scripts, native links, patch/replace,
or path dependencies. The final `module-artifact-descriptor.json` is forbidden
in source because only the worker may create it from the verified Component
digest.

`module package` first applies the complete project validation, requires a new
`.tar` destination outside the source tree, then invokes the shared deterministic
USTAR writer. Root `.git` and `target` directories are omitted; links, special
files, source-local Cargo configuration, a final descriptor, unsafe/non-UTF-8
paths, oversized trees, and concurrent file-size changes fail closed. The
machine-readable result contains the archive SHA-256 digest, matching `cas://`
identity, byte counts, entry count, and validated project provenance. Dry-run
performs validation and destination checks without creating the archive.

`module inspect` validates a source directory with the same project preflight or
hashes and scans a regular non-symlink archive with the same strict parser used
by build workers. Archive inspection is read-only and does not imply upload,
publication, trust admission, or marketplace approval.
