# rustok-artifact-node-controller

This independent operations process serves the authenticated gRPC port for
durable artifact-node assignment claims, lease heartbeats, and observations. It
composes only `ModuleControlPlane::artifact_node_agent()`: the process cannot
select topology, create desired reconciliations, inspect artifact policy, read
CAS bytes, or execute a sandbox.

Each RPC requires mutual TLS. The controller derives the SHA-256 fingerprint
of the verified leaf certificate and maps it to one immutable node/agent
principal. Request bodies never choose a node or agent identity. The controller
is an operations-tool component and must be deployed outside application role
bundles.

See the [local deployment contract](./docs/README.md) and the
[module control-plane plan](../../docs/modules/module-control-plane-consolidation-plan.md).
