# Fly Browser

SSR-first browser adapter assets and protocol contracts for Fly editors.

## Responsibilities

- Define the typed browser intent protocol.
- Classify revision-protected mutating intents.
- Provide the JavaScript adapter bundle and safe default limits.

## Interactions

Fly editor hosts embed the exported adapter and dispatch validated intents to their owner transport. This crate contains no domain persistence.

## Entry points

- `src/lib.rs` — protocol types, configuration, and embedded adapter assets.

See the [Page Builder documentation](../../modules/rustok-page-builder/docs/README.md) and the [module UI guide](../../../docs/UI/module-package-implementation.md).
