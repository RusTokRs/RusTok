# rustok-verification-worker documentation

The verification worker is an isolated operational process. It receives a
typed request from the module control plane, verifies Cosign/Sigstore evidence,
SLSA provenance, and CycloneDX SBOM policy through injected adapters, then
returns a redacted decision. It never publishes CAS blobs, writes admission
rows, executes module payloads, or exposes trust credentials to the server.

## Rollout

1. Typed tonic gRPC listener/client.
2. Fail-closed `ModuleInstaller` wiring before CAS publication.
3. Cosign, SLSA, and CycloneDX adapters injected into the worker process.
