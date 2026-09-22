# Isolated Rhai sandbox worker

## Runtime Boundary

The process exposes `SandboxWorkerService` with mutual TLS and composes only
`RhaiExecutor` plus `RhaiCapabilityBridge`. The bridge has a request-scoped
`SandboxHost`; all platform capabilities travel back to the host over the
stream. The worker cannot obtain infrastructure clients through its dependency
graph.

The service admits one execution globally per process. Additional concurrent
requests fail with `resource_exhausted`; deployments scale with isolated
replicas instead of running several untrusted interpreters in one process.

## Required Environment

Listener and client trust configuration uses the `RUSTOK_SANDBOX` prefix:

- `RUSTOK_SANDBOX_LISTEN_ADDR`
- `RUSTOK_SANDBOX_SERVER_CERT_PEM`
- `RUSTOK_SANDBOX_SERVER_KEY_PEM`
- `RUSTOK_SANDBOX_CLIENT_CA_PEM`
- optional `RUSTOK_SANDBOX_REQUEST_TIMEOUT_MS`
- optional `RUSTOK_SANDBOX_CONCURRENCY_LIMIT`
- optional `RUSTOK_SANDBOX_MAX_MESSAGE_SIZE`

Deployment isolation additionally requires:

- `RUSTOK_SANDBOX_RUNTIME`, exactly `gvisor` or `kata`;
- `RUSTOK_SANDBOX_IMAGE_DIGEST`, an exact lowercase SHA-256 digest;
- `RUSTOK_SANDBOX_ISOLATION_ATTESTATION`, an absolute path to a non-symlink
  regular JSON file no larger than 16 KiB.

The strict attestation declares protocol revision `1`, exact runtime and image
digest, non-privileged execution, no host mounts/socket/PID namespace, network
mode `rpc_only`, mTLS gRPC ingress, denied egress, no database or secret access,
a read-only root, and finite CPU, memory, PID, ephemeral-storage, file, and
wall-clock limits. Unknown fields, unbounded values, mismatch, replacement by
symlink, deletion, or malformed JSON make readiness and execution fail closed.

The worker verifies that every request's memory, output, wall-clock, and
concurrency limits fit inside the current attested envelope. The hardened
runtime, not Rust application code, enforces OS/container limits.

The worker also requires the cgroup v2 `memory.current` pseudo-file to be a
non-symlink regular file with a positive numeric value. Startup, every mTLS
readiness check, request admission, and execution fail closed if observation is
unavailable. Because each process admits one execution, a bounded sampler can
write the observed worker-cgroup peak to the neutral `peak_memory_bytes`
metric without presenting the configured memory limit as measured usage.

## Production Manifest Renderer

`scripts/generate/render-sandbox-worker-deployment.mjs` renders the canonical
Kubernetes profile from explicit deployment inputs. It requires a repository
plus exact image digest, selects the `runsc` or `kata` RuntimeClass, creates at
least two replicas, disables service-account credentials and host namespaces,
uses a read-only non-root container with dropped capabilities and runtime
seccomp, bounds CPU/memory/ephemeral storage, mounts only TLS and attestation
inputs, and creates a default-deny egress NetworkPolicy. Ingress is limited to
the server pod selector and worker mTLS port.

The startup, readiness, and liveness probes execute
`rustok-sandbox-worker-probe`. The probe connects to the loopback worker over
mTLS and calls the exact readiness RPC, so Kubernetes does not treat a merely
open TCP port as ready.

The deployment must provide a TLS Secret containing `server.crt`, `server.key`,
`client-ca.crt`, `probe-client.crt`, `probe-client.key`, and `server-ca.crt`.
It must also provide an immutable ConfigMap whose `attestation.json` key matches
the exact image, RuntimeClass policy, and enforced resource envelope. The
renderer deliberately does not fabricate either trust material or enforcement
evidence.

## Deployment Responsibilities

The deployment must launch the digest-pinned image under the attested gVisor or
Kata profile, mount only the read-only certificates and attestation, deny
egress, apply the attested resources, supervise process exit/OOM, use bounded
restart/backoff, and expose readiness to the host. The worker does not claim
these controls merely because configuration is present; retained deployment
evidence is required before the operational plan item closes.

The generated manifest covers the portable Kubernetes controls. Cluster-owned
RuntimeClass configuration must additionally enforce the attested PID and file
limits; a ConfigMap assertion is not proof of those controls. Kill, OOM,
filesystem, egress, cleanup, and replica-capacity recovery must be exercised in
the selected cluster and retained as operational evidence.
