# rustok-sandbox-worker

## Purpose

`rustok-sandbox-worker` is the separately deployable process for untrusted Rhai
execution.

## Responsibilities

- Host the neutral Rhai executor and broker-only capability bridge.
- Serve the sandbox streaming contract over deployment-provided mTLS.
- Fail startup and readiness without exact hardened-runtime attestation.
- Revalidate deployment isolation and request resource limits before execution.
- Fail readiness and execution when the cgroup v2 memory observer is unavailable.
- Report the observed worker-cgroup peak for each admitted execution.
- Admit one untrusted execution at a time per worker process.

## Interactions

The worker depends only on the neutral sandbox, sandbox transport, shared mTLS
foundation, serialization, and async runtime crates. It has no AI, Alloy,
module-control-plane, database, object-storage, secret, network-client, MCP, or
server dependency. Scale and restart are deployment-owned worker-replica
concerns.

## Entry Points

- Binary `rustok-sandbox-worker`
- Binary `rustok-sandbox-worker-probe`
- `IsolationPolicy`
- `WorkerMemoryObserver`
- Deployment renderer `scripts/generate/render-sandbox-worker-deployment.mjs`

See the [local documentation](./docs/README.md) and
[implementation plan](./docs/implementation-plan.md).
