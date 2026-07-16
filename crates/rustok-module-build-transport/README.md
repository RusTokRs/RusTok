# rustok-module-build-transport

This support crate provides the typed mTLS gRPC adapter for the owner-owned
`rustok_modules::ModuleBuildWorker` port. It contains protobuf framing, the
control-plane client adapter, and the worker-side server adapter.

`rustok-modules` owns request/result validation, durable queueing, and terminal
result persistence. The separately deployed worker owns source materialization,
isolated OCI-job execution, and build tooling. A transport error is returned to
the owner path; it never authorizes server-local Cargo execution.

See [local documentation](./docs/README.md) and the
[control-plane plan](../../docs/modules/module-control-plane-consolidation-plan.md).
