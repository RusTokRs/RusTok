# `rustok-worker-transport` Crate API & Architecture Contract

## 1. Architectural Purpose & Boundaries

`rustok-worker-transport` provides the shared mutual TLS (mTLS) transport foundation, cryptographic peer identity resolution, and process-level admission control used across all background execution workers and dispatchers in the RusToK platform fleet.

Key responsibilities:
- Standardizes mutually authenticated gRPC TLS listener configuration (`MutualTlsListenerConfig`) for worker servers (`rustok-module-build-worker`, `rustok-artifact-node-agent`, `rustok-verification-worker`, `rustok-sandbox-worker`).
- Standardizes mutually authenticated gRPC client TLS configuration (`MutualTlsClientConfig`) for dispatchers and controllers connecting to workers.
- Extracts and validates leaf peer certificate fingerprints (`PeerCertificateFingerprint`) to establish unforgeable deployment identities without leaking raw certificate text or relying on unauthenticated application metadata.
- Enforces process-wide worker concurrency admission (`WorkerAdmission`) with bounded queuing timeouts to prevent memory exhaustion and thread pool saturation under heavy fleet load.
- Provides unified signal handling (`shutdown_signal`) for graceful server draining across POSIX and Windows operating systems.

## 2. Mutual TLS (mTLS) Listener & Client Configurations

Every inter-worker RPC channel is strictly authenticated with mutual TLS. Cleartext fallbacks or unauthenticated connections are strictly prohibited.

1. **Listener Configuration (`MutualTlsListenerConfig`)**:
   - Built from uppercase environment variables via `from_env_prefix(prefix, message_size_ceiling)` or explicitly via `new(...)`.
   - Requires valid non-empty PEM-encoded server certificate, private key, and client root CA certificate.
   - Enforces positive request timeouts, positive admission timeouts $\le$ request timeout, positive concurrency limits, and strict message size ceilings (up to 128 MiB absolute limit).
   - Custom `Debug` implementation redacts `private_key_pem` to prevent accidental secret logging.
2. **Client Configuration (`MutualTlsClientConfig`)**:
   - Built from environment variables via `from_env_prefix(prefix)` or explicitly via `new(...)`.
   - Configures `ClientTlsConfig` with client identity and server CA root verification pinned to the expected server domain.
   - Generates a canonical `trust_anchor_digest()` covering the root CA and expected domain to seal configuration identity into controller placement proofs.
   - Custom `Debug` implementation redacts `private_key_pem`.

## 3. Peer Certificate Fingerprint & Deployment Identity

Deployment authentication relies on the cryptographic leaf certificate negotiated during the TLS 1.3 handshake:
- `peer_certificate_fingerprint(request)`: Extracts the peer certificate from the Tonic gRPC request context. If TLS evidence is absent or empty, fails closed immediately with `Status::unauthenticated`.
- `PeerCertificateFingerprint`: Represents a normalized `sha256:<64-hex>` digest over the raw DER bytes of the leaf certificate.
- Enforces strict canonical lowercase formatting. Implements `Display`, `AsRef<str>`, `Ord`, `Eq`, and `Hash` for direct use in identity mapping tables.

## 4. Worker Admission Controller & Concurrency Bounds

`WorkerAdmission` provides process-wide concurrency control across all active connections:
- Backed by an `Arc<tokio::sync::Semaphore>` ensuring that total concurrent request execution across all gRPC streams cannot exceed `concurrency_limit`.
- `acquire()`: Awaits an admission permit up to `admission_timeout`. If timeout elapses, sheds the load immediately with `Status::resource_exhausted("worker concurrency is saturated")`.
- `close()`: Closes the semaphore during shutdown so incoming requests fail fast with `Status::unavailable("worker is shutting down")`.
- `available_permits()`: Returns the number of currently available execution slots for health probes and load telemetry.

## 5. Universal Logic Dimensions Verification

1. **Preconditions & Bounds**:
   - TLS material paths are verified as non-empty regular files $\le$ 10 MiB to prevent memory exhaustion from erroneous file bindings.
   - Concurrency limits, timeouts, and message sizes are strictly non-zero and bounded within platform limits.
   - Fingerprint strings must exactly match `sha256:` followed by 64 lowercase hexadecimal digits.
2. **Branch Completeness & Anti-Swallowing**:
   - Missing peer certificates or invalid certificates return typed `tonic::Status::unauthenticated`.
   - Semaphore acquisition timeouts yield explicit `Status::resource_exhausted`.
   - Shutdown signals log errors if signal handlers fail to install, immediately terminating rather than continuing unsupervised.
3. **State Transition Integrity**:
   - Permits are managed via `OwnedSemaphorePermit` (RAII): permits automatically return to the semaphore pool even on panic or mid-request abortion.
   - Worker shutdown closes admission permits before initiating server drainage.
4. **Side-Effect Completeness**:
   - Client trust anchor digests incorporate domain separation tags and null separators to avoid digest collision across domains.
   - Redacted debug representations prevent private key leakage into log collectors and APM telemetry.
5. **Algorithmic Correctness**:
   - Cryptographic SHA-256 digests over certificate DER bytes provide collision-resistant principal identity.
   - Safe conversion of durations and millisecond integer bounds.
6. **Order of Operations & Concurrency**:
   - TLS handshake and peer authentication complete before any service RPC logic executes.
   - Process-wide admission acquired before stream processing starts.
