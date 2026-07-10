# rustok-ai-alloy

## Purpose

`rustok-ai-alloy` owns the Alloy-specific descriptor and policy used by the
AI orchestrator. It is a domain support adapter, not an AI runtime or an Alloy
transport implementation.

## Responsibilities

- Define the stable identity for the `alloy_code` AI vertical.
- Define the allowed Alloy script operations and payload shape.
- Validate the optional runtime payload before the orchestrator executes the
  registered direct handler.

## Interactions

`rustok-ai` owns runtime composition and transport. This crate supplies its
descriptor through `register_alloy_ai_vertical_handlers`; it must not own
provider routing, MCP wiring, or host UI.

## Entry points

- `register_alloy_ai_vertical_handlers`
- `alloy_script_execution_policy`
- `validate_runtime_payload`

## Documentation

- [Module documentation](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform documentation map](../../docs/index.md)
