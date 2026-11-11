# Fly Browser

For full public interface and protocol definitions, see [CRATE_API.md](CRATE_API.md).

SSR-first browser adapter assets and protocol contracts for Fly editors.

## Responsibilities

- Define the typed browser intent protocol.
- Classify revision-protected mutating intents.
- Provide the JavaScript adapter bundle and safe default limits.
- Bind the server-rendered editor controls (`bindSsrControls`): palette insert and drag, layer
  selection, toolbar intents, and route view state such as the previewed viewport. The
  attributes are a two-sided contract with the views that render them; see
  [the decision](../../../../DECISIONS/2026-11-11-fly-ssr-control-contract-and-route-view-state.md).

## Interactions

Fly editor hosts embed the exported adapter and dispatch validated intents to their owner transport. This crate contains no domain persistence.

## Entry points

- `src/lib.rs` — protocol types, configuration, and embedded adapter assets.

See the [Page Builder documentation](../../modules/rustok-page-builder/docs/README.md) and the [module UI guide](../../../docs/UI/module-package-implementation.md).
