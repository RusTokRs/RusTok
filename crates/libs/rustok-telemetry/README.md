# rustok-telemetry

## Purpose

`rustok-telemetry` owns the shared observability bootstrap and telemetry helpers for RusToK.

## Responsibilities

- Initialize tracing, metrics, and observability integrations.
- Provide shared telemetry wiring used by server and capability layers.
- Keep observability backends and exporter setup out of business-domain modules.

## Entry points

- `init_tracing`
- `init_metrics`
- telemetry helpers exported from `src/lib.rs`

## Interactions

- Used by `apps/server` for runtime observability bootstrap.
- Used by capability crates such as `rustok-mcp` and `rustok-ai` when they need shared telemetry contracts.
- Depends on foundational runtime crates without taking ownership of domain logic.

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
