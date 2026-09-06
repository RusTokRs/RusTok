# RusToK MCP Admin

Leptos admin package for the MCP control plane.

## Responsibilities

- Expose MCP-owned operator views.
- Preserve SSR and hydration parity.
- Delegate mutable behavior to MCP owner services.

## Interactions

The package consumes `rustok-mcp` and shared platform contracts. The admin host supplies authentication, tenant, route, and locale context.

## Entry points

- `src/lib.rs` — public route and component exports.
- `src/core.rs` — framework-neutral presentation contracts.

See the [MCP documentation](../docs/README.md) and the [module UI guide](../../../../docs/UI/module-package-implementation.md).
