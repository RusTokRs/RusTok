# rustok-verification-worker documentation

The verification worker is an isolated operational process. It receives a
typed request from the module control plane, verifies Cosign/Sigstore evidence,
SLSA provenance, and CycloneDX SBOM policy through injected adapters, then
returns a redacted decision. It never publishes CAS blobs, writes admission
rows, executes module payloads, or exposes trust credentials to the server.

## Rollout

1. Complete: typed tonic gRPC client/server adapters in
   `rustok-verification-transport`.
2. Complete: fail-closed `ModuleInstaller` wiring before CAS publication and
   atomic persistence of the redacted verification decision.
3. Next: inject concrete Cosign, SLSA, and CycloneDX adapters into the worker
   process and bind its gRPC listener in the deployment host.
