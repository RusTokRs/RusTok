# rustok-verification-worker documentation

The verification worker is an isolated operational process. It receives a
typed request from the module control plane, verifies Cosign/Sigstore evidence,
SLSA provenance, and CycloneDX SBOM policy through injected adapters, then
returns a redacted decision. The decision reports signature, provenance, SBOM,
license-policy, and vulnerability-policy outcomes independently; the owner
requires every outcome and binds them into the immutable admission fingerprint.
It never publishes CAS blobs, writes admission rows, executes module payloads,
or exposes trust credentials to the server.

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

The same mTLS listener exposes `VerificationService/GetReadiness`. It returns
ready only after the process has loaded and validated its listener material and
typed verification policy; an invalid or incomplete startup configuration exits
the process instead. Platform supervision must use this authenticated RPC (via
`GrpcTrustVerifier::check_readiness`) and process liveness, never a plaintext
health port.

## Rollout

1. Complete: typed tonic gRPC client/server adapters in
   `rustok-verification-transport`.
2. Complete: fail-closed `ModuleInstaller` wiring before CAS publication and
   atomic persistence of the redacted verification decision.
3. Complete: the worker starts a tonic listener and executes only fixed Cosign
   verification commands. It requires a complete allow-list policy, validates
   the signed in-toto subject digest, SLSA builder/build type/source/ref, and
   CycloneDX JSON version plus every declared component license and vulnerability
   rating. When configured, Cosign uses offline verification to require bundled
   transparency evidence.
4. Complete: the listener requires server certificate, private key, and client
   CA material. It refuses plaintext startup, requires mutually authenticated
   TLS, and bounds per-connection concurrency, request duration, and protobuf
   message size through mounted deployment configuration.
5. Complete: the mounted policy has one active trust root and may name one
   explicitly retiring root with a hard `retire_after_unix_seconds` deadline.
   Both roots use their own keyless-Sigstore identity/OIDC allow-lists or KMS
   key reference and signer identity; a retiring root is ignored at and after
   its deadline. There is no unbounded or implicit fallback between root modes.
6. Complete: fixture-backed tests cover accepted SLSA/CycloneDX statements;
   exact subject, builder, build type, source, and ref binding; required SBOM
   schema, component-license, and vulnerability-rating fields; denied license
   and severity policy; malformed or empty Cosign envelopes; and keyless/KMS
   policy cases. Owner fixtures separately prove that omitted license or
   vulnerability outcomes reject admission and change its evidence fingerprint.
7. Complete: the worker exposes a mTLS-protected gRPC readiness RPC on the
   verification listener. It becomes available only after fail-closed startup
   validation; no unauthenticated operational endpoint is bound.
