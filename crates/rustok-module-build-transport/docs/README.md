# Module build transport

The protobuf service is a narrow transport boundary for the versioned
`ModuleBuildRequest` and `ModuleBuildResult` contracts. JSON bytes preserve the
Rust-owned protocol while gRPC provides method identity, deadlines,
cancellation, and status semantics.

The crate must not contain build policy, Cargo execution, source/CAS access,
database access, publication credentials, or result persistence. Those belong
respectively to the isolated worker and `rustok-modules` owner services.

Production control-plane callers use `GrpcModuleBuildWorker::connect_with_tls`
with a mounted client identity, trust root, and expected worker domain. The
same mTLS listener exposes `ModuleBuildService/GetReadiness`; a worker reports
ready only after its deployment validates its OCI job runtime and policy.

The transport has no in-process or plaintext fallback.
