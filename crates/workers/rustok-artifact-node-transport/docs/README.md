# Artifact node transport

`rustok-artifact-node-transport` supplies two independently composed current
mTLS gRPC services for the owner reconciliation port. `ArtifactNodeService`
is the node-agent framing and carries a protocol revision plus JSON-encoded
Rust-owned assignment, heartbeat receipt, and report contracts.

Every RPC extracts PeerCertificateFingerprint from the verified mTLS leaf
certificate. ModuleArtifactNodeAgentAuthenticator maps that fingerprint to one
deployment-owned node and agent identity. Claim commands are created from that
identity, while heartbeat commands use its agent identifier. Reports must echo
both the mapped node and agent identity before the owner sees them. Payload
fields cannot select a node, agent, installation, release, or policy.

`ArtifactNodeReconciliationService.ReconcileTopology` is the operator-only
surface. Its body has an expected durable-state revision, policy revision,
idempotency UUID, and strict JSON `ModuleArtifactNodeTopologySnapshot`; it
has no actor, release, payload, capability, tenant, or readiness field. The
service derives the audit actor from its mTLS certificate, enforces that every
target node belongs to the immutable certificate scope, and gives the snapshot
to a request-scoped resolver. The transport copies the snapshot's canonical
topology digest into the owner command, so it is included in the idempotency
identity and must equal the resolver output. The owner then reloads the full
admitted installation identity under transaction before it creates any
assignment.

Both services use `WorkerAdmission` around owner operations. Authentication,
fingerprint mapping, malformed JSON, owner validation, conflicts, and
unavailable state map to gRPC statuses without exposing storage errors.
`ArtifactNodeGrpcService::into_tonic_service` and
`ArtifactNodeReconciliationGrpcService::into_tonic_service` each enforce a
one MiB message ceiling. The former is composed only by
`rustok-artifact-node-controller`; the latter only by
`rustok-artifact-node-reconciler`.

The transport intentionally does not materialize an assignment or execute a
sandbox. A node-agent process must separately consume its one work item,
perform only the allowed materialization and health action, and return its
owner-fenced observation through the agent service. Neither transport service
is a topology source or an application-server background task.
