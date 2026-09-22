# ADR: Alloy transport crate is named `alloy`

## Status

Superseded by [Single Alloy Capability Module](./2026-03-29-single-alloy-capability-module.md)

## Context

The transport shell around Alloy had been named `rustok-alloy`, which made Alloy look like a
RusToK domain module instead of a standalone capability/runtime layer.

This naming was misleading because:

- Alloy is not a tenant runtime module in `ModuleRegistry`;
- RusToK integrates with Alloy, rather than owning Alloy as a sub-module;
- `alloy` already established the intended naming model for the capability itself.

## Decision

We rename the transport crate from `rustok-alloy` to `alloy` and keep the naming split:

- `alloy` — runtime/engine capability;
- `alloy` — GraphQL/REST transport shell for Alloy;
- `rustok-mcp` — governed AI-to-platform interface that can expose Alloy capabilities.

## Consequences

Positive:

- Alloy is presented as a first-class capability, not a RusToK domain crate;
- documentation and architecture language become more consistent;
- crate boundaries better match the intended platform model.

Constraint:

- the bare crate name `alloy` must now be treated carefully in modules that already have a local
  `alloy` namespace; explicit absolute paths like `::alloy::...` are preferred there.

## Next steps

1. Keep future Alloy capability crates under the `alloy-*` naming family where possible.
2. Avoid introducing new `rustok-*` names for Alloy-owned layers unless they are truly
   RusToK-specific adapters.
