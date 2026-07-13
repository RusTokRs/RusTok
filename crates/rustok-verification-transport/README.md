# rustok-verification-transport

This support crate provides the typed gRPC adapter for the owner-owned
`rustok_modules::TrustVerifier` port. It contains protobuf framing, the
host-side client adapter, and the worker-side server adapter.

`rustok-modules` owns admission and the policy decision. The verification
worker owns Cosign, provenance, SBOM tooling, and its credentials. A transport
failure returns an error through the port; module installation therefore fails
closed and never falls back to local verification.

See [docs](docs/README.md) and the [control-plane plan](../../docs/modules/module-control-plane-consolidation-plan.md).
