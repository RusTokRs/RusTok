# Artifact node agent

`rustok-artifact-node-agent` is an independently supervised operations-tool
process outside application role bundles. It requires an already prepared
`RUSTOK_INSTANCE_ROOT`; it never claims or initializes a filesystem root.

## Required configuration

- `RUSTOK_ARTIFACT_NODE_AGENT_CONTROLLER_ENDPOINT` — HTTPS endpoint for the
  artifact-node controller.
- `RUSTOK_ARTIFACT_NODE_AGENT_SANDBOX_ENDPOINT` — HTTPS endpoint for the
  isolated Rhai sandbox worker.
- `RUSTOK_ARTIFACT_NODE_AGENT_NODE_ID` — non-nil UUID bound by the
  controller's verified mTLS certificate identity map.
- `RUSTOK_ARTIFACT_NODE_AGENT_ID` — exact non-empty agent identifier bound by
  the same map.
- `RUSTOK_ARTIFACT_NODE_AGENT_STORAGE_CONFIG_JSON` — one `StorageConfig` JSON
  object for the shared durable artifact CAS. Local storage is rebound to the
  instance root's canonical `storage` directory; S3-compatible configuration
  may use workload credentials rather than embedding credentials in JSON.

The optional `RUSTOK_ARTIFACT_NODE_AGENT_POLL_INTERVAL_MS` is bounded from 100
to 60,000 milliseconds and defaults to 1,000 milliseconds. The optional
`RUSTOK_ARTIFACT_NODE_AGENT_HEARTBEAT_INTERVAL_MS` defaults to 30 seconds and
is constrained to less than half the owner-issued assignment lease.

Controller mTLS client material uses the
`RUSTOK_ARTIFACT_NODE_AGENT_CONTROLLER` prefix:

- `CLIENT_CERT_PEM`
- `CLIENT_KEY_PEM`
- `SERVER_CA_PEM`
- `SERVER_DOMAIN`

Rhai sandbox mTLS client material uses the separate
`RUSTOK_ARTIFACT_NODE_AGENT_SANDBOX` prefix with the same suffixes. A
controller credential is never reused as sandbox-worker authority by default.

## Materialization and readiness

For every claimed assignment the agent reads the exact durable CAS digest and
then atomically materializes it at
`<instance-root>/cache/module-runtime/payload/sha256/<shard>/<digest>`. Existing
files are rehashed. Corrupt regular files are replaced from CAS; links,
junctions, and other unsafe filesystem entries fail closed.

The agent validates Rhai source or workspace bytes without executing guest code
and verifies the remote isolated Rhai worker's authenticated readiness before
reporting `healthy`. It compiles an exact Wasmtime Component locally without
instantiating or executing the guest before reporting `healthy`. Local prepared
markers are keyed by the runtime fingerprint and payload digest, and are never
treated as owner convergence evidence by themselves.

Static-promotion and sidecar assignments are reported as terminal failures:
the former belongs to the distinct static distribution aggregate and the latter
has no implemented dynamic runtime. CAS, filesystem, and sandbox transport
outages remain retryable and never produce false `healthy` or terminal
`failed` reports.

The agent does not provide a plaintext transport, in-process controller
fallback, tag resolution, OCI fallback, topology input, policy lookup,
capability broker, database access, sandbox execution of untrusted health
payloads, or application-server background task.
