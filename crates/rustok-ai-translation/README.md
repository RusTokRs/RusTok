# rustok-ai-translation

## Purpose

`rustok-ai-translation` is the stateless bridge between the Translation-owned
`MachineTranslationPort` and the AI-owned `AiStructuredTaskPort`.

It owns the `machine_translation` task identity, prompt-policy digest, typed
input/output schemas and their digests, registered system prompt and limits,
bounded request mapping, and deterministic validation of AI output. It never
stores translation workflow state and never mutates owner-owned content.
It also maps Translation status, result recovery, and cancellation to the
AI execution's stable owner/idempotency identity, so a caller does not need to
observe the generated execution UUID before a timeout or restart. Before it
accepts an executed or recovered result, the bridge compares the AI
content-free request binding with the exact bounded batch it submitted.

## Responsibilities

- Map bounded Translation batches to provider-neutral structured AI tasks.
- Map non-billable conservative estimates from the same request, tenant
  routing, attempt bounds, and immutable AI price snapshots used by execution.
- Preserve source/target locale, unit identity, source revision/hash, field
  semantics, protected tokens, exact-digest-bound glossary/Translation Memory
  context, and content-safe evidence. Every packet is at least
  `tenant_private`: it carries tenant-scoped resource identity and may carry
  glossary, memory, style, or evidence context. Personal and sensitive units
  raise that classification. The external structured payload uses canonical
  camel-case JSON names.
- Reject stale prompt policy, mismatched execution bindings,
  missing/extra/duplicate units, changed protected-token multiplicity or
  required whitespace shape, unknown output fields, owner length violations,
  invalid usage, and missing attempt evidence.
- Return proposal-only, human-review-required results with execution, model,
fallback, token, price, and cost evidence.
The estimate path never registers an execution, reserves budget, or calls a
provider.

## Interactions

- Depends on `rustok-translation` for the machine-translation provider SPI.
- Depends on `rustok-ai` for structured execution.
- Uses `rustok-translation-targets` only for neutral field profile and data
  classification contracts.
- Does not depend on owner modules, provider SDKs, GraphQL, server hosts, or
  persistence crates.

The production server profile selects the distribution-owned `ai-translation`
feature. `rustok-ai` provides the content-free execution/attempt/accounting
schema, idempotent ledger, leases, cancellation receipts, budget reservations,
exact task catalog, and a private executor with ordered inference/fallback,
cancellation/deadline observation, and authenticated encrypted TTL-bound
terminal-result replay without duplicate billing. Runtime tenant
accounting-policy provisioning, deployment keyring publication, and scheduler
recovery/result cleanup exist. Composition evidence verifies that the
Translation-owned lazy factory is published without host capability imports
and resolves to no machine provider when the deployment keyring is absent.
Deterministic composed runtime evidence covers ordered provider fallback,
fail-closed JSON Schema enforcement, sanitized failure recording, exact attempt
usage/cost settlement, request-hash conflict rejection, in-flight cancellation
with reservation release, quota rejection before provider execution, and
encrypted restart replay without another provider call or bill.
Configuration-level provider unavailability also produces typed degraded
health. Live external-provider runtime failure/restart evidence remains open;
`rustok-ai` now provides an
ignored operator-only durable structured-runtime probe for collecting it.
Separate-process file-backed evidence already covers expired-attempt recovery,
reservation preservation, and reclaim with a new lease.

The bridge requests `tenant_private` health because that is the minimum egress
classification for every machine-translation packet. AI policy evaluates the
same classification at health, routing, estimate, reservation, and provider
attempt boundaries, so a tenant-private denial fails before an external call.

## Entry points

- `AiMachineTranslationAdapter`
- `machine_translation_descriptor`
- `machine_translation_policy_digest`
- `machine_translation_task_descriptor`
- `machine_translation_input_schema_digest`
- `machine_translation_output_schema_digest`
- `machine_translation_port_from_context` (`server` feature)
- `AiMachineTranslationPortFactory` (`server` feature)
- `MACHINE_TRANSLATION_TASK_SLUG`

## Documentation

- [Adapter contract](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Translation architecture plan](../../docs/modules/translation-implementation-plan.md)
