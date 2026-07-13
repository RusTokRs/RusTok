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
3. Complete: the worker starts a tonic listener and executes only fixed Cosign
   verification commands. It requires a complete allow-list policy, validates
   the signed in-toto subject digest, SLSA builder/build type/source, and
   CycloneDX JSON version plus every declared component license and vulnerability
   rating. When configured, Cosign uses offline verification to require bundled
   transparency evidence.
4. Next: add deployment mTLS/authentication, per-request limits, private KMS
   trust-root mode, and targeted adapter fixtures before production rollout.
