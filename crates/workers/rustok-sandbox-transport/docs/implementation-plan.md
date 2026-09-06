# Implementation Plan for `rustok-sandbox-transport`

## Scope

Own only the current typed host/worker wire adapter for neutral sandbox
execution. Runtime policy belongs to `rustok-sandbox`; deployment isolation
belongs to `rustok-sandbox-worker`; host capabilities remain owner-composed.

## Current State

- [x] Bidirectional generated tonic/prost protocol with exact revision checks.
- [x] Native artifact-byte framing without JSON/base64 expansion.
- [x] Host capability callback through the original `SandboxHost`.
- [x] Typed outcome/error mapping, deadline, and cancellation propagation.
- [x] mTLS-only public client constructor and readiness handshake.
- [x] Fail-closed execution readiness and isolation-limit admission hooks.
- [x] Single-execution worker admission and no local executor fallback.
- [x] Loopback evidence for callback, typed error preservation, cancellation,
  hang/deadline, disconnect, readiness loss, and protocol mismatch.
- [ ] Add retained deployment evidence for process kill/OOM and replica
  restart/backoff once the hardened worker deployment is composed.

## Completion Condition

The transport is complete when retained hardened-deployment evidence proves
that worker process termination and OOM cannot terminate or exhaust the host,
and the supervisor restores capacity without weakening audit or isolation.
