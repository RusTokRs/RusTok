# Module trust-verification transport

- Date: 2026-07-13
- Status: Accepted

## Context

Artifact admission requires signature, provenance, and SBOM verification, but
the control plane must not execute security tools or receive trust-root
credentials. The owner still needs an explicit, typed, fail-closed boundary
before artifact bytes enter durable CAS.

## Decision

`rustok-modules` owns the `TrustVerifier` port, selected policy revisions, and
the admission decision. `rustok-verification-worker` owns verification tool
execution and credentials. `rustok-verification-transport` maps the port to a
tonic gRPC service and is the only shared transport dependency.

`ModuleInstaller` receives a verifier and policy revisions explicitly. It calls
the verifier before CAS staging, rejects every verifier/transport/protocol or
policy-revision failure, and persists the successful redacted decision with the
installation, admission, dependency lock, and outbox event in one database
transaction. No in-process, host-local, or legacy fallback is permitted.

## Consequences

Hosts construct `GrpcTrustVerifier` and inject it into `ModuleInstaller`; they
do not implement verification. Concrete Cosign, SLSA, and CycloneDX adapters
remain a worker-only follow-up. The transport carries versioned owner contract
serialization inside protobuf framing and must not acquire database, CAS, or
policy ownership.
