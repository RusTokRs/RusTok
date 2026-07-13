# rustok-verification-worker documentation

The verification worker is an isolated operational process. It receives a
typed request from the module control plane, verifies Cosign/Sigstore evidence,
SLSA provenance, and CycloneDX SBOM policy through injected adapters, then
returns a redacted decision. It never publishes CAS blobs, writes admission
rows, executes module payloads, or exposes trust credentials to the server.

## Deployment contract

The worker reads `RUSTOK_VERIFICATION_POLICY_JSON` and fails startup when its
allow-lists or trust root are incomplete. It requires all of the following
listener variables; there is no plaintext fallback:

- `RUSTOK_VERIFICATION_LISTEN_ADDR`;
- `RUSTOK_VERIFICATION_SERVER_CERT_PEM`;
- `RUSTOK_VERIFICATION_SERVER_KEY_PEM`;
- `RUSTOK_VERIFICATION_CLIENT_CA_PEM`.

Optional bounded overrides are `RUSTOK_VERIFICATION_REQUEST_TIMEOUT_MS`,
`RUSTOK_VERIFICATION_CONCURRENCY_LIMIT`, and
`RUSTOK_VERIFICATION_MAX_MESSAGE_SIZE` (at most 1 MiB). The owner transport
must use `GrpcTrustVerifier::connect_with_tls` with its client identity, trust
root, and expected worker domain.

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
4. Complete: the listener requires server certificate, private key, and client
   CA material. It refuses plaintext startup, requires mutually authenticated
   TLS, and bounds per-connection concurrency, request duration, and protobuf
   message size through mounted deployment configuration.
5. Complete: the mounted policy selects exactly one trust root. Keyless Sigstore
   uses certificate identity and OIDC issuer allow-lists; first-party KMS uses
   a configured key reference and signer identity. There is no fallback between
   the two modes.
6. Complete: fixture-backed tests cover accepted SLSA/CycloneDX statements and
   denied digest, license, vulnerability, keyless policy, and KMS policy cases.
