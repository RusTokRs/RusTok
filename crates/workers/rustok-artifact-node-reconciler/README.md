# rustok-artifact-node-reconciler

This independent operations process accepts authenticated desired-topology
requests for the durable artifact-node reconciliation ledger. It is the only
deployment composition that serves the topology-authoring RPC; the separate
`rustok-artifact-node-controller` continues to serve only node-agent
claim/heartbeat/report operations.

Each request is mutually authenticated. The reconciler derives the SHA-256
fingerprint of the verified client leaf certificate, maps it to an immutable
audit actor and explicit allowed-node scope, and never accepts an actor from
the RPC body. The durable owner reloads every selected installation and its
admitted release/payload identity under transaction before it writes a desired
set. This process has no CAS, sandbox, AI, Alloy, product, tenant, or
application-server dependency.

See the [local deployment contract](./docs/README.md) and the
[module control-plane plan](../../docs/modules/module-control-plane-consolidation-plan.md).
