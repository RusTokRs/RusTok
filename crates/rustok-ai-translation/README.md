# rustok-ai-translation

## Purpose

`rustok-ai-translation` is the stateless bridge between the Translation-owned
`MachineTranslationPort` and the AI-owned `AiStructuredTaskPort`.

It owns the `machine_translation` task identity, prompt-policy digest, typed
input/output schemas, bounded request mapping, and deterministic validation of
AI output. It never stores translation workflow state and never mutates
owner-owned content.

## Responsibilities

- Map bounded Translation batches to provider-neutral structured AI tasks.
- Preserve source/target locale, unit identity, source revision/hash, field
  semantics, protected tokens, glossary/Translation Memory context, and
  content-safe evidence.
- Reject stale prompt policy, missing/extra/duplicate units, changed protected
  tokens, owner length violations, invalid usage, and missing attempt evidence.
- Return proposal-only, human-review-required results with execution, model,
  fallback, token, price, and cost evidence.

## Interactions

- Depends on `rustok-translation` for the machine-translation provider SPI.
- Depends on `rustok-ai` for structured execution.
- Uses `rustok-translation-targets` only for neutral field profile and data
  classification contracts.
- Does not depend on owner modules, provider SDKs, GraphQL, server hosts, or
  persistence crates.

The bridge is not live-registered yet. Runtime activation is blocked until
`rustok-ai` implements the durable structured execution/attempt/usage/cost
ledger, idempotent replay, budgets, fallback, cancellation, and restart
recovery required by the Translation plan.

## Entry points

- `AiMachineTranslationAdapter`
- `machine_translation_descriptor`
- `machine_translation_policy_digest`
- `MACHINE_TRANSLATION_TASK_SLUG`

## Documentation

- [Adapter contract](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Translation architecture plan](../../docs/modules/translation-implementation-plan.md)
