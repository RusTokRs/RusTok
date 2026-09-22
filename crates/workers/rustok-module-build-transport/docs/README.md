# Build-worker transport

The protobuf services are narrow transport boundaries for the single current
module-build and static-distribution contracts. JSON bytes preserve the
Rust-owned types while gRPC provides method identity, deadlines, cancellation,
and status semantics. Neither package has a generation suffix or compatibility
service.

The crate must not contain build policy, Cargo execution, source/CAS access,
database access, publication credentials, or result persistence. Those belong
respectively to the isolated worker and `rustok-modules` owner services.

Control-plane callers use `GrpcModuleBuildWorker::connect_with_tls` or
`GrpcStaticDistributionExecutor::connect_with_tls` with a mounted client
identity, trust root, and expected worker domain. These are the only connection
constructors. Each mTLS service exposes authenticated readiness; a worker
reports ready only after its deployment validates its runtime, policy, pinned
toolchain, and required credentials.

The transport has no in-process or plaintext fallback.

The production server adapter for `rustok.static_distribution` is hosted only
by the separately deployed `rustok-static-distribution-worker`. That process
validates its fixed CI launcher and immutable job receipt; the transport crate
still owns no job execution or evidence policy.
