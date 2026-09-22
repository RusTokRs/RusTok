# ADR: Single Alloy Capability Module

## Status

Accepted

## Date

2026-03-29

## Context

Alloy is a single platform capability. It must have one canonical crate name, one public module identity, and one documentation surface across the repository.

The platform host must stay module-agnostic:

- `apps/server` is a composition root and does not own module-specific transport shims;
- module GraphQL and HTTP entry points must live in the module crate itself;
- platform registries and docs must describe Alloy as one capability, not as multiple surfaces.

## Decision

Use only `alloy` as the canonical Alloy capability crate and module name.

- `alloy` owns the runtime, storage, scheduler, GraphQL, and HTTP surfaces;
- `apps/server` connects Alloy only through generated manifest wiring;
- Alloy remains outside the `Core/Optional` tenant module taxonomy.

## Consequences

- repository documentation refers only to `alloy`;
- module manifests and generated server wiring point to `alloy`;
- host applications use Alloy through its public crate surface without server-owned shims.
