# `rustok-verification-transport` API & Architecture Contract

## Role
Typed gRPC transport adapter for the RusToK trust verification port (`rustok_modules::TrustVerifier`). Bridges the module control plane with the isolated verification worker process over secure mTLS.

## Public surface
- `GrpcTrustVerifier`: Control-plane client adapter implementing `rustok_modules::TrustVerifier` and `check_readiness()` probe.
- `VerificationGrpcService<V>`: Worker-side gRPC service adapter implementing `VerificationService` with `WorkerAdmission` rate limiting.
- `ClientTlsConfig`: Re-exported from `tonic::transport::ClientTlsConfig` for strict mTLS client connections.
- `proto`: Generated protobuf code for `rustok.verification.v1` (`VerificationServiceClient`, `VerificationServiceServer`, `VerifyRequest`, `VerifyResponse`, `ReadinessRequest`, `ReadinessResponse`).

## Wire Protocol & Framing
- Protobuf schema `proto/rustok/verification/v1/verification.proto`:
  - `Verify(VerifyRequest) returns (VerifyResponse)`
  - `GetReadiness(ReadinessRequest) returns (ReadinessResponse)`
- The request body contains canonical UTF-8 JSON serialization of `TrustVerificationRequest`.
- The response body contains canonical UTF-8 JSON serialization of `TrustVerificationDecision`.
- Strict mTLS: The only production connection constructor requires `ClientTlsConfig` (`connect_with_tls`), ensuring bidirectional identity authentication.
- Bounded admission: The server enforces process-wide concurrent request ceilings via `WorkerAdmission::acquire()`.

## Minimum Contract Set

### Input DTOs/Commands
- `VerifyRequest`: Protobuf wrapper containing `trust_verification_request_json: bytes`.
- `ReadinessRequest`: Empty request triggering validation readiness verification.

### Domain Invariants
- **Fail-Closed Verification**: Transport network or deserialization failures never fall back to local admission; any RPC error results in `TrustVerificationError::Unavailable` or `TrustVerificationError::InvalidRequest`.
- **Readiness Invariant**: Readiness probe returns `ready = true` only when the worker process has loaded TLS materials, validated policies, and started listening.
- **Payload Framing**: JSON serialization preserves all typed fields of `TrustVerificationRequest` and `TrustVerificationDecision` without silent defaulting.

### Events / Outbox Side Effects
- The transport layer does not emit events or modify database state.

### Errors / Failure Codes
- `Status::invalid_argument`: Deserialization failure of `TrustVerificationRequest` from request payload.
- `Status::failed_precondition`: Inner verifier rejected artifact verification (e.g., untrusted signature or failing policy).
- `Status::resource_exhausted`: Worker admission concurrency or timeout reached.
- `Status::internal`: Serialization failure of `TrustVerificationDecision` to response payload.

## Verification & Test Coverage
- Unit tests verify readiness probe, successful verification pass-through, and precondition rejection handling.
- Contract tests verify Cargo manifest standards, API documentation completeness, and Debug formatting.
