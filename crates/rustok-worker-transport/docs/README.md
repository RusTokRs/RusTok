# Worker transport foundation

`MutualTlsListenerConfig` centralizes the deployment listener baseline used by
isolated workers. Given an uppercase prefix, it loads `<PREFIX>_LISTEN_ADDR`,
`<PREFIX>_SERVER_CERT_PEM`, `<PREFIX>_SERVER_KEY_PEM`, and
`<PREFIX>_CLIENT_CA_PEM`, with optional bounded timeout, concurrency, and
message-size settings.

The crate owns no worker-specific protocol, policy, task execution, CAS,
database, or secrets beyond the mounted listener identity and trust material.

`MutualTlsClientConfig` uses the same prefix with `CLIENT_CERT_PEM`,
`CLIENT_KEY_PEM`, `SERVER_CA_PEM`, and `SERVER_DOMAIN` to build a tonic mTLS
client configuration for an external dispatcher or other deployment host.
