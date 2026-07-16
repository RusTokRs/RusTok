# rustok-module-build-worker

This executable is the isolated deployment boundary for untrusted module Rust
builds. It exposes the owner-owned module build protocol only over mTLS gRPC,
then invokes a fixed deployment-owned job runner inside the hardened worker
image. The server and module runtime never invoke Cargo through this package.

The worker has no database or CAS dependency. It receives immutable request
facts and returns a typed terminal result; `rustok-modules` validates and
persists that result against the queued request.

See [local documentation](./docs/README.md) and the
[control-plane plan](../../docs/modules/module-control-plane-consolidation-plan.md).
