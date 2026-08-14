# Artifact node reconciler

`rustok-artifact-node-reconciler` is an independently supervised operations
process outside application role bundles. It accepts the artifact-node
topology-authoring gRPC service and terminates if its owner database connection
or listener cannot be initialized; a deployment supervisor restarts it. A
client must retry a lost request using the same idempotency key.

## Required configuration

- `RUSTOK_ARTIFACT_NODE_RECONCILER_DATABASE_URL` identifies the durable
  control-plane database.
- `RUSTOK_ARTIFACT_NODE_RECONCILER_OPERATOR_IDENTITIES_JSON` is a non-empty
  JSON array of `certificate_fingerprint`, `actor_id`, and `allowed_node_ids`
  objects. A fingerprint is canonical lowercase `sha256:<64 hex>`, `actor_id`
  is a non-nil audit principal UUID, and every identity lists one to 1,024
  unique non-nil execution-node UUIDs. Certificate rotations can map multiple
  fingerprints to one identity; duplicate node values or a duplicated
  fingerprint deny startup.

The shared mutually authenticated listener uses the
`RUSTOK_ARTIFACT_NODE_RECONCILER` prefix:

- `RUSTOK_ARTIFACT_NODE_RECONCILER_LISTEN_ADDR`;
- `RUSTOK_ARTIFACT_NODE_RECONCILER_SERVER_CERT_PEM`;
- `RUSTOK_ARTIFACT_NODE_RECONCILER_SERVER_KEY_PEM`;
- `RUSTOK_ARTIFACT_NODE_RECONCILER_CLIENT_CA_PEM`.

Optional shared listener limits are `REQUEST_TIMEOUT_MS`,
`ADMISSION_TIMEOUT_MS`, `CONCURRENCY_LIMIT`, and `MAX_MESSAGE_SIZE`. The
message ceiling is one MiB. There is no plaintext listener, environment
topology payload, client-selected actor, node-agent operation, CAS, sandbox,
release selection, capability broker, tenant/product/AI/Alloy dependency, or
application-server background task.

## Request contract

`ArtifactNodeReconciliationService.ReconcileTopology` carries an expected
reconciliation-state revision, canonical policy revision, idempotency UUID,
and strict JSON `ModuleArtifactNodeTopologySnapshot`. The snapshot contains
only a topology reference/digest and node/installation pairs. The mTLS
certificate determines the audit actor and limits all target node IDs before
the durable owner receives the request. Its canonical topology digest is part
of the owner command's idempotency identity and must match the request-scoped
resolver output, so a replay cannot substitute another target set.

The transport cannot provide a release digest, payload digest, media type,
capability grant, ABI, admission revision, or a live readiness value. The
owner validates the submitted topology, reloads each selected admitted
installation under lock, freezes the complete immutable assignment identity,
and commits the desired set with its outbox event. Node agents remain unable
to author topology because their controller exposes a different service and a
different certificate principal map.
