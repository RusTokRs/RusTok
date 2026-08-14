# Artifact node controller

`rustok-artifact-node-controller` is the deployment composition for the
current artifact-node mTLS transport. It requires the following controller
configuration:

- `RUSTOK_ARTIFACT_NODE_CONTROLLER_DATABASE_URL` for the owner database;
- `RUSTOK_ARTIFACT_NODE_CONTROLLER_AGENT_IDENTITIES_JSON`, a non-empty JSON
  array of `certificate_fingerprint`, `node_id`, and `agent_id` objects.

Each fingerprint is canonical lowercase `sha256:<64 hex>` and can occur only
once. Multiple rotating certificates may map to the same immutable node/agent
principal. Unknown fields, an empty map, invalid node UUID, invalid agent ID,
or duplicate certificate identities deny startup.

The shared listener uses the `RUSTOK_ARTIFACT_NODE_CONTROLLER` prefix:

- `RUSTOK_ARTIFACT_NODE_CONTROLLER_LISTEN_ADDR`;
- `RUSTOK_ARTIFACT_NODE_CONTROLLER_SERVER_CERT_PEM`;
- `RUSTOK_ARTIFACT_NODE_CONTROLLER_SERVER_KEY_PEM`;
- `RUSTOK_ARTIFACT_NODE_CONTROLLER_CLIENT_CA_PEM`.

Optional shared listener limits are `REQUEST_TIMEOUT_MS`,
`ADMISSION_TIMEOUT_MS`, `CONCURRENCY_LIMIT`, and `MAX_MESSAGE_SIZE`, using the
same prefix. The maximum message size is one MiB. There is no plaintext
listener, client-selected principal, topology configuration, in-process
fallback, artifact materialization, sandbox connection, or application-server
background task in this component.
