# Neutral sandbox foundation for Alloy and module artifacts

- Date: 2026-07-11
- Status: Accepted

## Context

RusToK has two code-evolution paths that require isolated execution:

- Alloy authors, tests, repairs and evolves Rhai-based behavior;
- the module platform installs versioned marketplace artifacts, including Rhai
  sources and Rust implementations compiled to WebAssembly components.

If either `alloy` or `rustok-modules` owns the sandbox, the other becomes a
secondary adapter and the platform acquires different execution semantics based
on code origin. Separate sandboxes would duplicate capability policy, tenant
isolation, limits, audit evidence and failure mapping.

## Decision

Introduce `rustok-sandbox` as a platform-neutral support crate. Neither Alloy nor
the module marketplace owns it. Both consume the same public execution contract.

`rustok-sandbox` owns:

- the execution request, subject, context, result and error taxonomy;
- sandbox policy and resource-limit profiles;
- the capability broker contract with default-deny behavior;
- executor registration and selection;
- the common audit envelope and execution-observer ports;
- Rhai and WebAssembly executor adapters, with a sidecar adapter reserved for a
  later implementation.

`rustok-sandbox` must not depend on `alloy`, `rustok-modules`, `apps/server` or a
domain module. Host capabilities are supplied through narrow ports.

Untrusted production Rhai crosses the separately deployed
`rustok-sandbox-transport` bidirectional tonic boundary into
`rustok-sandbox-worker`. Every external frame carries one exact protocol
revision and execution identity. Artifact bytes remain a native protobuf field;
the current Rust request metadata and capability values use JSON and map
immediately to the single unversioned internal sandbox model. Worker capability
requests return to the original scoped `SandboxHost`; the worker has no
database, storage, secret, MCP, network-client, Alloy, AI, or module-control-plane
dependency. Public host construction requires mTLS and has no in-process
fallback.

The worker admits one untrusted execution per process. Startup, readiness, and
each execution require a matching digest-pinned gVisor/Kata isolation
attestation with RPC-only mTLS ingress, denied egress and infrastructure access,
a read-only root, and finite OS resource limits. The deployment runtime enforces
those limits and the worker rejects a request whose sandbox limits exceed the
attested envelope. One shared cgroup v2 observer participates in startup,
readiness, request admission, and execution; it records the observed worker
cgroup peak and fails closed when measurement is unavailable. The canonical
Kubernetes renderer selects the digest-pinned gVisor/Kata RuntimeClass, exact
mTLS RPC probes, restricted ingress, default-deny egress, portable pod
hardening, and multi-replica rolling supervision. RuntimeClass-specific PID and
file enforcement plus restart/backoff and containment evidence remain
deployment responsibilities.

`rustok-modules` owns module identity, immutable releases, dependency resolution,
installation, activation, tenant enablement, capability grants, marketplace
governance, rollback and the mapping from an installed artifact to a sandbox
request.

Alloy owns prompts, source workspaces, revisions, draft/review workflows,
testing, repair, optimization and release creation. Alloy draft execution calls
the same sandbox used by installed artifacts. Alloy does not maintain a second
production execution engine or independent capability policy.

Artifact origin is lineage metadata, not an execution contract. A Rhai module, a
ported Rust module compiled to WebAssembly and an untrusted marketplace module
all enter the platform through the same module descriptor and isolated sandbox
API. Executor choice is declared by the immutable artifact and admitted by
policy.

Marketplace releases are immutable. Continuing development of a published Rhai
module through Alloy imports or forks its source lineage and produces a new
semantic version and digest. Installed artifact bytes are never edited in place.

Trusted static promotion remains an explicit distribution mode. It shares module
identity and lifecycle contracts but does not claim sandbox isolation because it
runs native code in process.

## Consequences

- The generic Rhai engine, canonical workspace/import resolver, serialized
  scope records, standard functions, brokered HTTP helpers, limits, and error
  mapping live in `rustok-sandbox`. Alloy owns only domain-data adaptation before
  and after the neutral request.
- `rustok-module-sdk` owns the canonical external WIT file. Bytecode Alliance
  tooling generates both the author-facing guest API and Wasmtime host binding
  from that source; the host does not retain an inline ABI copy.
- WebAssembly uses the same execution context, broker, audit and outcome contract
  rather than creating a marketplace-only runtime API.
- Local module authoring uses a bounded `LocalSandboxScenario` over that same
  request, policy, typed capability validation, fixture broker, outcome, and
  error taxonomy. The local Component executor is an explicit authoring
  placement only; it is not a production fallback and receives no deployment
  credentials or infrastructure clients.
- Capability grants are evaluated before executor invocation and cannot be
  expanded by Rhai helpers, WebAssembly imports or module UI.
- Draft and installed executions are distinguishable by typed subject metadata
  while remaining comparable in observability and policy evidence.
- Executor placement is mandatory registration metadata. A caller must select
  `in_process` or `isolated_worker`; duplicate kinds are rejected across
  placements, placement is observable for readiness, and an unavailable worker
  never selects an in-process executor implicitly.
- Worker loss, cancellation, deadline, readiness loss, protocol mismatch, or
  malformed framing terminates the request without changing executor placement.
- A deployment attestation is a fail-closed admission input, not a replacement
  for retained gVisor/Kata kill, OOM, egress, filesystem, and restart evidence.
- Existing native modules do not need immediate conversion; they remain trusted
  static promotions until deliberately ported to a sandboxed artifact.
