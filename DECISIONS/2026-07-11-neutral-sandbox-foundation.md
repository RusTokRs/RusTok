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

- The generic Rhai engine, limits and error mapping move from `alloy` into the
  Rhai adapter of `rustok-sandbox`; Alloy-specific bridges remain in Alloy.
- WebAssembly uses the same execution context, broker, audit and outcome contract
  rather than creating a marketplace-only runtime API.
- Capability grants are evaluated before executor invocation and cannot be
  expanded by Rhai helpers, WebAssembly imports or module UI.
- Draft and installed executions are distinguishable by typed subject metadata
  while remaining comparable in observability and policy evidence.
- Existing native modules do not need immediate conversion; they remain trusted
  static promotions until deliberately ported to a sandboxed artifact.

