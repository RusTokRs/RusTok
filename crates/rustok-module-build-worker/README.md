# rustok-module-build-worker

This executable is the isolated deployment boundary for untrusted module Rust
builds. It exposes the owner-owned module build protocol only over mTLS gRPC,
then delegates each build to a fixed deployment-owned OCI job launcher using a
required gVisor or Kata runtime. Startup and readiness also require a bounded,
deployment-owned isolation attestation matching that runtime and the pinned
image, while deployment evidence must still demonstrate that the launcher
enforces the attested job controls. The server and module runtime never invoke
Cargo through this package.

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
