# rustok-module-build-transport / CRATE_API

## Role

`rustok-module-build-transport` provides typed mTLS gRPC client and server adapters for the owner-owned ports:
- `rustok_modules::ModuleBuildWorker` (untrusted module Rust builds)
- `rustok_modules::ModuleStaticDistributionExecutor` (static storefront distribution packaging)

The crate contains the single current protobuf framing definitions (`proto/rustok/module_build/module_build.proto` and `proto/rustok/static_distribution/static_distribution.proto`) plus control-plane client and worker-side server adapters.
It operates strictly as a transport boundary:
- It maintains **no Cargo binary execution capability**, **no CAS/filesystem access**, and **no database client**.
- It provides **no plaintext or unauthenticated fallback**; connections require mutual TLS (`connect_with_tls`) with client certificates, CA roots, and worker domain validation.
- It exposes readiness probes (`check_readiness`) so callers only dispatch to workers whose runtime policy and credentials are fully verified.

## Public surface

### Public modules

- `client`: `GrpcModuleBuildWorker` client implementation wrapping tonic gRPC channel with thread-safe `Mutex`.
- `distribution_client`: `GrpcStaticDistributionExecutor` client implementation wrapping tonic gRPC channel.
- `server`: `ModuleBuildGrpcService` worker-side gRPC server implementation bounded by `WorkerAdmission`.
- `distribution_server`: `StaticDistributionGrpcService` worker-side gRPC server implementation with work item validation.
- `module_build_proto`: Protobuf definitions and generated tonic client/server for `rustok.module_build`.
- `static_distribution_proto`: Protobuf definitions and generated tonic client/server for `rustok.static_distribution`.

### Primary public types and re-exports

- `GrpcModuleBuildWorker`: Owner-side client adapter implementing `ModuleBuildWorker`.
- `GrpcStaticDistributionExecutor`: Owner-side client adapter implementing `ModuleStaticDistributionExecutor`.
- `ModuleBuildGrpcService<W>`: Worker-side gRPC service adapter implementing `ModuleBuildService` for `W: ModuleBuildWorker + ModuleBuildWorkerReadiness`.
- `StaticDistributionGrpcService<E>`: Worker-side gRPC service adapter implementing `StaticDistributionBuildService` for `E: ModuleStaticDistributionExecutor + ModuleStaticDistributionExecutorReadiness`.
- `ClientTlsConfig`: Re-exported from tonic for mTLS endpoint configuration.

## Wire Protocol & Framing

- **Transport:** HTTP/2 over mTLS with ALPN.
- **Framing:** Protobuf message envelopes carrying UTF-8 JSON payloads (`module_build_request_json`, `module_build_result_json`, `work_item_json`, `outcome_json`) to retain Rust-owned domain types and structural validation across RPC boundaries.
- **Status Codes:**
  - `InvalidArgument`: Malformed JSON or failed domain validation.
  - `FailedPrecondition`: Worker-side build policy violations or compilation rejections.
  - `Unavailable`: Worker transport disconnection or server admission permit starvation.
  - `Internal`: Response serialization failures.

## Minimum Contract Set

### Input DTOs/Commands
- `ExecuteBuildRequest`: Protobuf envelope carrying UTF-8 JSON encoded payload (`module_build_request_json` or `work_item_json`).
- `ReadinessRequest`: Empty probe request triggering runtime readiness checks on the worker daemon.

### Domain Invariants
1. **Strict Mutual TLS Enforcement**: The only public connection constructor is `connect_with_tls(endpoint, tls_config)`; unencrypted or unauthenticated transport is strictly prevented.
2. **Admission and Concurrency Protection**: Server execution handlers acquire `WorkerAdmission` permits prior to invoking underlying build execution.
3. **No In-Process Cargo Compilation**: The transport crate has zero direct access to Cargo, compilers, or child processes; all execution is performed by the worker behind the gRPC boundary.
4. **Preflight Domain Validation**: Static distribution work items are validated (`work_item.validate()`) before dispatching to the executor.
5. **Deterministic Protobuf Generation**: Built via `build.rs` leveraging `protoc-bin-vendored` for hermetic builds across all environments.

### Events / Outbox Side Effects
- None; the transport crate acts as an RPC bridge and does not interact with the transactional outbox or emit domain events.

### Errors / Failure Codes
- `ModuleBuildProtocolError::Transport`: Propagated on transport-level failures, connection drops, or malformed responses.
- `ModuleStaticDistributionExecutorError`: Mapped to gRPC `Status` (`Unavailable` for transport errors, `FailedPrecondition` for policy rejections).
- `tonic::Status`: Standard gRPC error status returned by server implementations.

## Verification & Test Coverage

Verified via:
- Manifest standardization assertions in `src/contract_tests.rs`.
- Wire envelope and protobuf message structure tests.
- Status code mapping tests (`ModuleStaticDistributionExecutorError` -> `tonic::Status`).
- Scoped verification: `cargo test -p rustok-module-build-transport`.
- Scoped linter: `cargo clippy -p rustok-module-build-transport --all-targets`.
- Gatekeeper verification: `python scripts/verify/verify-remediation-gate.py`.
