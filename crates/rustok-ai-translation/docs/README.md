# AI Translation Adapter Contract

`rustok-ai-translation` is an outbound adapter owned jointly at the boundary
between the Translation and AI capabilities. The two owners remain acyclic:
only this support crate imports both public contracts.

The current adapter supports bounded plain text, SEO text, placeholder
templates, and localized scalar values. It validates the Translation request
before billable execution, supplies explicit source and target locales, emits a
typed JSON schema, and validates the complete structured result before
returning it.

The bridge also publishes the exact `AiStructuredTaskDescriptor` used by the
AI task catalog. The descriptor binds owner/task identity, prompt policy,
input/output schema digests, system prompt, allowed classifications and hard
limits, so a caller cannot reuse the task slug with a different contract.

Every accepted result is `review_required`. The adapter cannot approve a
proposal, publish content, call an owner service, or construct an owner patch.

The `server` feature exposes a neutral host-context composition function. The
production server selects the explicit distribution bridge, which publishes
the Translation-owned lazy factory without importing capability types into the
host.
The canonical ledger/accounting foundation and encrypted TTL-bound
terminal-result replay, permission-checked accounting-policy provisioning,
deployment keyring publication, and scheduler recovery/result cleanup now
exist. Composition evidence also verifies the fail-closed missing-keyring state:
the factory resolves to no machine provider while manual Translation workflows
remain available. Live external-provider failure/restart evidence remains
pending.
The adapter resolves and cancels executions through the stable
`(owner, idempotency_key)` AI contract. Cancellation therefore remains durable
when it arrives before AI execution registration, and completed encrypted
results can be recovered without another billable call.
Registering this adapter against chat sessions, direct provider engines, or a
non-durable fallback would violate the machine-translation architecture.

See the [implementation plan](./implementation-plan.md) for the activation
gate.
