# Module Build Worker Transport

- Date: 2026-07-16
- Status: Accepted

## Context

`rustok-modules` already owns immutable module build requests and results, a
tenant-RLS durable queue, and transactional outbox facts. It must dispatch an
untrusted Rust source build without allowing `apps/server` or the runtime
sandbox to invoke Cargo. The worker needs a deployment-ready protocol with
authenticated readiness and no accidental in-process fallback.

## Decision

Use a dedicated `rustok-module-build-transport` support crate. It maps the
owner-owned `ModuleBuildWorker` port onto the single current tonic gRPC service
whose
bodies are canonical JSON serializations of `ModuleBuildRequest` and
`ModuleBuildResult`. `rustok-worker-transport` owns the shared mTLS listener
baseline used by both verification and build workers. Production callers use
mTLS and the same authenticated listener exposes readiness. The transport
contains no build policy, source/CAS access, database access, result
persistence, or Cargo execution.

`rustok-module-build-worker` is the separate process/OCI-job boundary. It
accepts only the immutable request, invokes a fixed image-owned job runner with
a cleared environment, enforces the request deadline/output cap, and returns a
validated terminal result. Before becoming ready, the worker must load a
bounded deployment-owned isolation attestation that matches the selected
runtime and digest-pinned image and records the required unprivileged,
host-isolated, resource-limited, ephemeral-job controls. This attestation is
configuration-review evidence; deployment still has to demonstrate that the
launcher enforces the corresponding OCI controls. `rustok-modules` validates
and persists that result against the queued request. Transport or worker
failure has no server-local fallback.

The delivery host is a separate broker consumer. It owns broker
acknowledgement and the database connection required to call the owner delivery
service; it does not execute Cargo or join the worker process. It invokes the
build worker only through a mutually authenticated client and leaves failed
deliveries unacknowledged for broker retry. The Iggy adapter consumes the
dedicated `module-build` topic through one persistent remote consumer-group
cursor and commits an offset only after the owner result persistence succeeds.
Broker topology provisioning and deployment of the delivery host remain
operational requirements.

## Consequences

- The owner protocol remains independent of tonic and worker runtime details.
- The protobuf package has no generation suffix or compatibility service; a
  contract change replaces callers and workers atomically in this initial
  implementation.
- A worker binary can be deployed and supervised independently of the server.
- Verification and build workers share one mTLS listener implementation rather
  than drifting into separate TLS/limit defaults.
- The delivery host must not compete with the global outbox relay or
  acknowledge another consumer's event stream position.
- The `module-build` topic must be provisioned before the dispatcher starts;
  unexpected or malformed queue payloads remain unacknowledged and require
operator remediation rather than being silently skipped.

The same support crate also maps the owner-owned
`ModuleStaticDistributionExecutor` port onto a separate current-only
`rustok.static_distribution` service. Static-distribution queue ownership,
lease renewal, and terminal persistence remain in `rustok-modules`; the remote
service receives only an already claimed immutable work item and returns one
terminal outcome. Its client exposes only the mTLS connection constructor.
- The legacy server-local `rustok-build` executor cannot be retained as a
  fallback; its removal follows when remote dispatch and the worker deployment
  are wired.
- Build execution and OCI-job isolation remain worker-owned follow-up work;
  readiness attestation is required before deployment evidence can close that
  work.
