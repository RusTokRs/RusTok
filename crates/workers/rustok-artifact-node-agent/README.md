# rustok-artifact-node-agent

`rustok-artifact-node-agent` is the independently deployed operations process
that materializes owner-issued dynamic module assignments on one execution
node. It never selects topology, resolves releases, reads the owner database,
or receives tenant identity, policies, grants, secrets, AI, Alloy, or product
infrastructure.

The agent authenticates to `rustok-artifact-node-controller` with mTLS, reads
only the exact admitted payload digest from platform CAS, writes it atomically
under the canonical portable instance cache, and rehashes every cache reuse.
It reports `prepared` after non-executing local payload preparation and
`healthy` only after the exact runtime check succeeds. The control-plane owner
alone promotes all healthy assignments to `active` and advances convergence.

See the [deployment contract](./docs/README.md) and the
[module control-plane plan](../../docs/modules/module-control-plane-consolidation-plan.md).
