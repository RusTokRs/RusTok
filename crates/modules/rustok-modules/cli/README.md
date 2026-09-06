# rustok-modules-cli

## Purpose

`rustok-modules-cli` owns the archive/WASM Component local authoring command
adapters for standalone module components.

## Responsibilities

- Expose `module init`, `module validate`, `module test`, `module build`,
  `module package`, `module publish`, and `module inspect` through
  `rustok-cli-core`.
- Create projects with create-new filesystem semantics and remove an incomplete
  newly created root when initialization fails.
- Generate `Cargo.lock` through the pinned project toolchain.
- Validate source identity, descriptor declaration, toolchain, SDK/template
  provenance, dependency policy, lock graph, and forbidden final descriptor
  output before a source archive is submitted.
- Create a deterministic bounded USTAR source archive through the shared
  `rustok-build-source` writer and return its digest-addressed identity.
- Inspect either a source project or a standalone archive through the same
  validation/parser contracts used by packaging and workers.
- Build with a sanitized, bounded, offline Cargo profile and execute the real
  Component through the neutral local sandbox scenario contract.
- Queue production builds through the owner control, which republishes the
  deterministic archive into source CAS and commits the immutable queue/outbox
  state without invoking a worker in the CLI process.
- Stage a completed build through the owner publication control. The owner
  validates and content-addresses the current metadata bundle, creates the
  immutable governance request, binds the completed OCI build receipt, and
  queues registry validation without granting approval or admission.
- Do not package Rhai workspaces. Reviewed Rhai releases follow the Alloy
  revision -> canonical bounded-workspace source object -> generic source-CAS
  owner path and are never wrapped as `.tar`.

## Interactions

The provider consumes the pure `rustok-module-template` renderer, canonical
`rustok-modules` source-manifest and authoring-control contracts, the shared
`rustok-build-source` archive boundary, and the credential-free
`rustok-sandbox` authoring harness. Local commands need no platform runtime.
The remote build and publication commands compose or reuse owner services but
contain no SQL, worker transport, OCI client, signing client, publication
credentials, production sandbox worker, AI, or Alloy dependency.

The current remote build control republishes an archive through the
archive-specific `CasArchivePublisher`. Under the accepted release-safety
cutover it becomes an archive-specialized client of the preparation owner's
single `SourceObjectStore`; the direct `<digest>.tar` CAS layout is removed
with every caller and is not retained as a fallback.

## Entry Points

- `command_provider`
- `ModuleCommandProvider`

The selected distribution registers this provider through
`crates/rustok-modules/rustok-module.toml`.
