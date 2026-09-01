# rustok-module-build-worker

This executable is the isolated deployment boundary for untrusted module Rust
builds. It exposes the owner-owned module build protocol only over mTLS gRPC,
then delegates each build to a fixed deployment-owned OCI job launcher using a
required gVisor or Kata runtime. Startup and readiness also require a bounded,
deployment-owned isolation attestation matching that runtime and the pinned
image. The launcher itself is pinned by SHA-256 and rehashed at construction,
readiness, and immediately before invocation, while deployment evidence must
still demonstrate that it enforces the attested job controls. The server and
module runtime never invoke Cargo through this package.

Build RPCs use the shared process-wide bounded admission policy; readiness stays
available while saturated. SIGTERM/Ctrl+C initiates tonic graceful shutdown,
and cancellation kills worker-owned subprocesses instead of orphaning builds.

The isolation attestation is a strict, unknown-field-rejecting contract. It
includes positive PID and open-file ceilings in addition to its resource and
isolation facts. The
worker has no attestation-free constructor and reloads the deployment-owned file
through its readiness gate before every build, so a caller cannot bypass or
outlive revoked configuration evidence. It binds the exact launcher digest and
explicitly denies tenant database and general platform secret access.

The worker has no database or CAS service client. It receives immutable request
facts, materializes only the exact archive from a deployment-mounted read-only
CAS root through shared `rustok-build-source`, and returns a typed terminal
result; `rustok-modules` validates and persists that result against the queued
request.

Registry credential acquisition and KMS-backed Cosign execution use the shared
current-only `rustok-build-publication` boundary. The worker pins and re-hashes
both deployment executables and does not own a second broker or signer path.

See [local documentation](./docs/README.md) and the
[control-plane plan](../../docs/modules/module-control-plane-consolidation-plan.md).
