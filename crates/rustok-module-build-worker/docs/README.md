# Module build worker

The worker requires a mutually authenticated listener configured with the
`RUSTOK_MODULE_BUILD` prefix:

- `RUSTOK_MODULE_BUILD_LISTEN_ADDR`;
- `RUSTOK_MODULE_BUILD_SERVER_CERT_PEM`;
- `RUSTOK_MODULE_BUILD_SERVER_KEY_PEM`;
- `RUSTOK_MODULE_BUILD_CLIENT_CA_PEM`;
- `RUSTOK_MODULE_BUILD_JOB_LAUNCHER` (an absolute non-symlink,
  deployment-owned executable that launches exactly one OCI build job).
- `RUSTOK_MODULE_BUILD_JOB_RUNTIME` (`gvisor` or `kata`; no permissive
  runtime is accepted).
- `RUSTOK_MODULE_BUILD_JOB_IMAGE_DIGEST` (the exact `sha256:<hex>` OCI image
  identity permitted for every launched build job).
- `RUSTOK_MODULE_BUILD_WORKDIR` (an existing absolute image-owned directory).
- `RUSTOK_MODULE_BUILD_SOURCE_ROOT` (an existing absolute read-only CAS archive mount).
- `RUSTOK_MODULE_BUILD_CARGO` (an absolute non-symlink Cargo executable owned by the image).
- `RUSTOK_MODULE_BUILD_CARGO_HOME` (an absolute non-symlink deployment-owned
  Cargo cache directory).
- `RUSTOK_MODULE_BUILD_WASM_TOOLS` (an absolute non-symlink image-owned
  `wasm-tools` executable).
- `RUSTOK_MODULE_BUILD_PUBLICATION_REGISTRY` and
  `RUSTOK_MODULE_BUILD_PUBLICATION_REPOSITORY` (the deployment-owned scoped OCI
  publication destination).
- `RUSTOK_MODULE_BUILD_COSIGN_PROGRAM` (an absolute non-symlink image-owned
  Cosign executable).
- `RUSTOK_MODULE_BUILD_COSIGN_KEY_REFERENCE` (an approved KMS provider URI;
  never a file path or raw private key).
- `RUSTOK_MODULE_BUILD_REGISTRY_CREDENTIAL_BROKER` (an absolute non-symlink
  deployment-owned executable that returns a short-lived scoped registry lease).

The worker never reads registry usernames or passwords from environment. Before
publication it invokes the fixed credential broker with exactly one bounded JSON
request containing `protocol_version: 1`, configured `registry`, `repository`,
and `minimum_ttl_seconds`. The broker returns one bounded JSON lease for that
same repository with username, password, and expiry. Publication/signing uses a
14-minute maximum window and requests a further 30-second lease margin, while
the lease itself remains capped at 15 minutes and is kept only in worker memory.
Broker stdout is limited to 16 KiB; stderr is discarded. The broker runs with a
cleared environment and must use its own deployment-managed workload identity
or local provider channel.

The worker validates the configured publication target at construction even
when it is instantiated without environment loading. It rechecks the acquired
lease immediately before creating the OCI publisher; an expired lease is a
terminal publication failure and is never passed to the registry client.

`scripts/verify/verify-module-build-worker-isolation.mjs` guards this boundary
in source: the worker cannot add tenant-database, platform-storage, or general
secret dependencies, and the untrusted OCI job launcher must remain
environment-cleared without database or credential forwarding. It also prevents `apps/server` from
adding a module-build worker or delivery path and requires the separate
dispatcher to use the mTLS remote worker after readiness verification. The
transport readiness response must use the worker-owned hardened OCI-job probe,
not an unconditional success. The same verifier requires the fixed Cosign
command to clear its environment and receive only the temporary private Docker
configuration.

Cosign receives the verified, digest-pinned artifact reference only after OCI
publication. Its KMS-provider identity is deployment-owned and must be scoped
to the configured publication repository; neither a private key nor a signing
credential enters a build request, runner environment, descriptor, or log.
The worker clears the Cosign process environment and supplies only the private
Docker configuration, so process-wide proxy credentials or `COSIGN_REPOSITORY`
cannot redirect a signature to another repository. It then resolves Cosign's
standard `sha256-<subject>.sig` OCI manifest tag to a digest-pinned receipt. The
tag is only a registry lookup mechanism, never an identity exposed to the
control plane. The same in-memory lease is written only to a private temporary
Docker configuration for the fixed Cosign process and is removed immediately
after it exits.

`RUSTOK_MODULE_BUILD_DEPENDENCY_MATERIALIZER` is optional for deployments that
accept only `network_policy: denied`. When configured, it must be an absolute,
regular, image-owned executable that submits or runs a separate OCI network
sandbox for scoped dependency materialization. The worker never grants that
sandbox's egress to Cargo, the fixed build runner, or publication.

The materializer receives canonical `ModuleBuildRequest` JSON on standard input
and the source directory, fresh Cargo-home output directory, and approved
endpoint array through the `RUSTOK_MODULE_DEPENDENCY_MATERIALIZER_*`
environment. It must write exactly one JSON receipt to standard output:
`protocol_version: 1`, `source_digest`, `dependency_lock_digest`, the exact
ordered `endpoints`, and `outcome` (`materialized` or `endpoint_denied`). It
must not create `config` or `config.toml` in Cargo home, links anywhere in that
tree, or files above the build disk limit. Its OCI sandbox and allowlist proxy
are deployment-owned enforcement points, not source-controlled configuration.

Optional listener limits are `RUSTOK_MODULE_BUILD_REQUEST_TIMEOUT_MS`,
`RUSTOK_MODULE_BUILD_CONCURRENCY_LIMIT`, and
`RUSTOK_MODULE_BUILD_MAX_MESSAGE_SIZE` (at most 1 MiB). Startup fails if the
mounted OCI job launcher, Cargo executable/cache, or `wasm-tools` executable is
absent or invalid, if the hardened OCI runtime is not explicitly selected, or
if the mTLS configuration is incomplete. There is no plaintext,
permissive-runtime, or server-local fallback.

The fixed OCI job launcher receives canonical request JSON on standard input
and must launch one job with the supplied `RUSTOK_MODULE_BUILD_OCI_RUNTIME` and
digest-pinned `RUSTOK_MODULE_BUILD_JOB_IMAGE_DIGEST`. It also receives
`RUSTOK_MODULE_BUILD_REQUEST_DIGEST`: a domain-separated SHA-256 digest of the
exact request JSON bytes on standard input.
The job returns exactly one canonical `ModuleBuildResult` JSON value on the
launcher standard output. The launcher is an image-owned entrypoint, not
request-selected input. The worker clears its environment and supplies only
request-scoped source/output/target/home paths, the fixed
`RUSTOK_MODULE_BUILD_CARGO` executable, a verified `CARGO_HOME`, and
`CARGO_NET_OFFLINE=true`. The launched job must invoke that fixed Cargo path and
must not clear or override offline mode. Cargo homes containing config or
credential files are rejected. The worker uses a fixed image-owned workdir,
caps aggregate stdout/stderr by the request output limit while reading it,
derives execution timeout from both deployment and request limits, kills the
launcher on timeout, and validates the terminal result against the immutable
request before it crosses gRPC. The launcher path cannot be a symlink.

Before the worker accepts any terminal result, the launched job must write the
regular non-symlink `oci-job-receipt.json` file in its output directory. Its
bounded JSON object contains `protocol_version: 2`, `request_id`,
`source_digest`, `attempt`, `dependency_lock_digest`, `toolchain_digest`,
`wit_digest`, `request_digest`, `runtime`, `image_digest`, and a bounded opaque
`job_id`. The worker requires those values to equal the immutable request, its
exact canonical request JSON, and its fixed
gVisor/Kata and image-digest configuration; a missing, oversized, symlinked,
malformed, or mismatched receipt is a transport failure and is never accepted
as build evidence.

For a successful result, the runner must also write its only executable payload
to `RUSTOK_MODULE_BUILD_OUTPUT_DIR/component.wasm`. The worker rejects a
symlink, empty, oversized, non-component, invalid, or digest-mismatched file.
It validates the Component Model bytes with `wasmparser` and requires the
reported imports/exports to equal the root component's inspected interface.
The deployment-owned `wasm-tools` executable extracts WIT from that same fixed
payload. The worker parses the extracted WIT and requires the request's
package, world, version, and complete import/export surface to match exactly.
This rejects undeclared capability imports; the runner cannot supply WIT text
or choose the inspection executable. The JSON result is therefore evidence
about a fixed output, not an unverified substitute for the payload. Publication
and Cosign signing use only that verified output; admission remains a later
stage.

The runner must additionally write
`RUSTOK_MODULE_BUILD_OUTPUT_DIR/module-artifact-descriptor.json`. The worker
parses this regular non-symlink file under the descriptor limit and binds its
slug, version, runtime ABI, WASM payload kind, and payload digest to the
immutable successful build result. It publishes the fixed component, SBOM, and
provenance through the configured scoped OCI destination, then uses fixed
Cosign with the configured KMS reference to sign the digest-pinned artifact.
The returned result then carries only digest-pinned artifact/SBOM/provenance/
signature-manifest references and marks that signature as `build_service`; the
owner rejects any successful result that lacks those publication facts. An
author signature and marketplace approval are separate governance evidence and
cannot be claimed by the build worker.

Successful builds must also emit `sbom.cdx.json` and `provenance.intoto.json`
in the same output directory. The worker rehashes both regular non-symlink
files against the result digests and parses them before accepting success. The
SBOM must be bounded CycloneDX JSON with a metadata component. Provenance must
be a bounded SLSA in-toto Statement whose subject carries the component SHA-256
and whose `predicate.buildDefinition.externalParameters.rustok` object binds
the immutable source, dependency-lock, toolchain, and WIT digests plus exact
independently versioned author SDK and template inputs, expected module
slug/version, runtime ABI, build attempt, and exact ordered validation-profile
identities and outcomes. A successful JSON result must report every requested
profile as `passed`; `validation_failed` must identify an ordered requested
profile with outcome `failed`. This checks production evidence before the
worker publishes OCI referrers and signs the artifact; admission trust policy
remains a separate stage.

For v1, source reference must exactly be `cas://sha256/<hex>` and its matching
`<hex>.tar` file must exist in `RUSTOK_MODULE_BUILD_SOURCE_ROOT`. The worker
rehashes the archive before use, accepts only checksummed USTAR regular
files/directories, rejects links, devices, and escaping paths, and enforces the
request disk limit while extracting,
and removes the request-scoped materialized directory after the runner exits.
The runner receives the materialized paths through
`RUSTOK_MODULE_BUILD_SOURCE_DIR` and `RUSTOK_MODULE_BUILD_OUTPUT_DIR`.
Digest mismatch, unsafe archive, and extraction-limit violations return a
validated terminal build result (`source_digest_mismatch`, `unsafe_archive`, or
`resource_limit_exceeded`) with `revise_source`; they are not retried as broker
transport failures. Worker I/O faults remain unacknowledged for retry.

Before starting the runner, the worker also requires a `Cargo.lock` and binds
its raw bytes to `dependency_policy.lock_digest` (`sha256:<hex>`). It rejects
Cargo config files from the source tree and every parent work directory,
patches/replacements, path dependencies, non-allowlisted registry sources, Git
dependencies when forbidden, and manifests that request build scripts or native
`links` while those permissions are denied. It parses
the resolved lock graph without executing Cargo, bounds package/dependency
counts, requires checksums for registry packages, requires a pinned revision
for allowed Git packages, and rejects credentials in source URLs. The allowlist
accepts a canonical crates.io index when the request specifies
`https://crates.io`. These are terminal policy facts (`dependency_policy_denied`,
`build_script_denied`, or `native_link_denied`), not broker retries. The runner
must still run `cargo metadata --locked` after controlled dependency
materialization to produce resolved-graph evidence; this preflight does not
replace that pipeline stage.

The worker now performs that metadata step itself before the fixed OCI job. It
clears Cargo's environment, uses only the image-owned Cargo executable and
deployment-owned cache, forces `--locked --offline`, caps combined metadata
stdout/stderr with the request output limit, applies the request deadline, and
checks every resolved package, dependency source, custom build target, native
link declaration, workspace path, and resolve-node identity. For scoped
dependency policy, the worker first invokes only the fixed materializer adapter
with the exact endpoint list and a fresh job-local Cargo home. The materializer
must return a receipt binding the source digest, raw lock digest, and ordered
endpoint list; the worker rejects symlinks plus Cargo config and credential
files in that cache before again forcing Cargo offline. Missing materializer
configuration, a receipt mismatch, or endpoint denial becomes
`network_policy_denied`. The OCI materializer deployment must enforce egress
with its own network namespace and allowlist proxy; this worker never broadens
its own egress.

Production deployment supplies the OCI job launcher and `wasm-tools` in an
unprivileged OCI image through the absolute, regular, non-symlink
`RUSTOK_MODULE_BUILD_JOB_LAUNCHER` and `RUSTOK_MODULE_BUILD_WASM_TOOLS` paths.
The launcher must create an ephemeral OCI job with the selected gVisor or Kata
runtime, no host mounts or container socket, request-scoped source/target/cache
volumes, and the resource/network controls specified in the module
control-plane plan. It also provides the deployment-owned
`RUSTOK_MODULE_BUILD_CARGO` and `RUSTOK_MODULE_BUILD_CARGO_HOME` paths. The
job pipeline still needs deployment evidence for hardened-job isolation and the
later admission trust stage.
