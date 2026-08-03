# rustok-sandbox-transport

## Purpose

`rustok-sandbox-transport` provides the typed streaming boundary between a
RusToK host and the separately deployed Rhai sandbox worker.

## Responsibilities

- Generate the current protobuf/tonic service contract.
- Require an exact external protocol revision on every stream frame.
- Carry artifact bytes without JSON/base64 expansion.
- Route worker capability requests back through the request-scoped host broker.
- Propagate cancellation and deadlines without selecting an in-process fallback.
- Expose authenticated readiness for the exact Rhai executor contract.

## Interactions

The server uses `GrpcRhaiExecutor` as a `rustok-sandbox::SandboxExecutor`. The
worker uses `SandboxWorkerGrpcService` around the neutral Rhai executor. TLS
identity and trust material come from `rustok-worker-transport`; this crate
does not own certificates, capability implementations, databases, storage,
secrets, module identity, or deployment isolation policy.

## Entry Points

- `GrpcRhaiExecutor`
- `SandboxWorkerGrpcService`
- `SandboxWorkerReadiness`
- `SANDBOX_WORKER_PROTOCOL_REVISION`
- `SANDBOX_WORKER_MAX_MESSAGE_SIZE`

See the [local documentation](./docs/README.md) and
[implementation plan](./docs/implementation-plan.md).

