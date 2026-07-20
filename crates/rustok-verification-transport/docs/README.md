# Verification transport

The protobuf service is intentionally a narrow transport boundary. Its request
and response bodies serialize the versioned Rust-owned trust contracts, while
gRPC supplies method identity, deadlines, cancellation, and status codes.

The crate must not contain admission policy, CAS access, database access, or
verification credentials. Those belong respectively to `rustok-modules` and
`rustok-verification-worker`.

Production callers use `GrpcTrustVerifier::connect_with_tls` with a mounted
client identity, trust root, and expected worker domain. A TLS connection or
certificate failure reaches the owner as a verifier error and rejects admission.
The serialized decision carries independent signature, provenance, SBOM,
license-policy, and vulnerability-policy outcomes. Missing outcome fields are
not defaulted by the transport and therefore fail decoding rather than weakening
owner admission.
