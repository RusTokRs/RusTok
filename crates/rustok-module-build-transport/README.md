# rustok-module-build-transport

This support crate provides typed mTLS gRPC adapters for the owner-owned
`rustok_modules::ModuleBuildWorker` and
`rustok_modules::ModuleStaticDistributionExecutor` ports. It contains the
single current protobuf framing plus control-plane client and worker-side
server adapters for both build boundaries.

`rustok-modules` owns request/result validation, durable queueing, leases, and
terminal result persistence. Separately deployed workers own source
materialization, isolated execution, and build tooling. A transport error is
returned to the owner path; it never authorizes server-local Cargo execution or
records a false terminal build result.

See [local documentation](./docs/README.md) and the
[control-plane plan](../../docs/modules/module-control-plane-consolidation-plan.md).
