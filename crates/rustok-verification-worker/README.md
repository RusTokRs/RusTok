# rustok-verification-worker

## Purpose

Runs artifact trust verification outside the server and module runtime.

## Responsibilities

- enforce mounted signer and policy-revision constraints;
- host Cosign, SLSA, and CycloneDX verification adapters;
- return only typed redacted decisions with independent signature, provenance,
  SBOM, license-policy, and vulnerability-policy outcomes to `rustok-modules`.

## Interactions

`rustok-modules` owns the `TrustVerifier` port and admission decision. The
worker exposes `VerificationGrpcService` through the typed tonic transport;
host deployment supplies the listener and worker credentials. The worker does
not own CAS, database state, outbox writes, or artifact execution.

## Entry points

- `src/lib.rs` — worker policy and verification boundary;
- `src/main.rs` — isolated process entrypoint.

## Documentation

See [local documentation](./docs/README.md).
