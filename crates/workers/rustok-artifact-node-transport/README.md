# rustok-artifact-node-transport

This support crate supplies the typed mTLS gRPC boundaries for the durable
artifact-node reconciliation owner. `ArtifactNodeService` is the narrow
deployment node-agent surface: an agent can claim, heartbeat, and report only
owner-issued assignments. `ArtifactNodeReconciliationService` is a separately
composed deployment-operator surface for submitting a bounded desired
topology.

Both services derive a SHA-256 fingerprint from the verified mTLS leaf
certificate. The agent service resolves it to exactly one node/agent principal.
The reconciliation service resolves it to one audit actor and explicit allowed
node scope; it never accepts a caller-selected actor. The owner validates the
submitted topology and reloads every admitted installation identity under its
own transaction. The crate owns no topology source, CAS access, sandbox
execution, artifact policy, AI, or product behavior.

See [local documentation](./docs/README.md) and the [module control-plane
plan](../../docs/modules/module-control-plane-consolidation-plan.md).
