# Worker transport foundation

`MutualTlsListenerConfig` centralizes the deployment listener baseline used by
isolated workers. Given an uppercase prefix, it loads `<PREFIX>_LISTEN_ADDR`,
`<PREFIX>_SERVER_CERT_PEM`, `<PREFIX>_SERVER_KEY_PEM`, and
`<PREFIX>_CLIENT_CA_PEM`, with optional bounded timeout, concurrency, and
message-size settings.

`<PREFIX>_ADMISSION_TIMEOUT_MS` controls the maximum wait for the shared global
worker permit and defaults to 250 ms. It must be positive and cannot exceed
`<PREFIX>_REQUEST_TIMEOUT_MS`. Saturated work receives gRPC
`RESOURCE_EXHAUSTED`; a closed admission boundary returns `UNAVAILABLE`.
Readiness RPCs deliberately do not consume permits.

Worker binaries use `shutdown_signal()` with tonic `serve_with_shutdown` so
SIGTERM and Ctrl+C stop new requests and drain bounded in-flight work. Failure
to install the Unix SIGTERM handler stops the worker instead of leaving it
unsupervised.

The caller supplies its protocol-specific maximum to
`MutualTlsListenerConfig::from_env_prefix`. Standard workers use the public
1 MiB ceiling; the sandbox transport uses its bounded 72 MiB artifact ceiling.
Every caller-specific ceiling is still constrained by the shared absolute
128 MiB limit, so an environment variable cannot make the listener unbounded.

The crate owns no worker-specific protocol, policy, task execution, CAS,
database, or secrets beyond the mounted listener identity and trust material.

`peer_certificate_fingerprint` derives the canonical SHA-256 fingerprint of
the verified mTLS leaf certificate attached to a tonic server request. It
rejects a request without peer TLS evidence. Protocol-specific adapters must
map this fingerprint through deployment-owned topology/agent authorization;
they must not trust a node or agent identifier carried in a request payload.

`MutualTlsClientConfig` uses the same prefix with `CLIENT_CERT_PEM`,
`CLIENT_KEY_PEM`, `SERVER_CA_PEM`, and `SERVER_DOMAIN` to build a tonic mTLS
client configuration for an external dispatcher or other deployment host.
