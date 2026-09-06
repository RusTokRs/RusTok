# Implementation Plan for `rustok-sandbox-worker`

## Scope

Provide the independent, product-neutral Rhai worker process. It must remain
usable without AI, Alloy, marketplace, MCP, database, storage, or server
infrastructure.

## Current State

- [x] Separate worker binary with mTLS-only generated gRPC service.
- [x] Neutral Rhai executor with broker-only capability callback.
- [x] One untrusted execution per process and bounded message framing.
- [x] Exact gVisor/Kata, image-digest, access-denial, read-only-root, and
  resource-limit attestation validation.
- [x] Startup, readiness, and per-request isolation-envelope revalidation.
- [x] Shared cgroup v2 memory observation in startup/readiness/admission and
  request outcomes, without reporting a configured limit as usage.
- [x] Unit evidence for valid revalidation, unbounded attestation rejection,
  and request limits above the attested envelope.
- [x] Repository guard rejects product/infrastructure dependencies, plaintext
  public construction, server embedding, and loss of the remote composition.
- [x] Add the canonical Kubernetes deployment renderer for a digest-pinned
  gVisor/Kata RuntimeClass, mTLS-only probes, multi-replica rolling supervision,
  non-root read-only execution, bounded portable resources, restricted ingress,
  and default-deny egress.
- [ ] Retain kill, OOM, filesystem, egress, and multi-replica restart/backoff
  evidence from that deployment.
- [ ] Retain proof that the selected cluster RuntimeClass enforces the attested
  PID and file limits that portable Kubernetes pod fields cannot express.

## Completion Condition

The worker plan is complete when the hardened deployment and supervisor are
composed and retained evidence proves containment, cleanup, capacity recovery,
and measured process-resource reporting.
