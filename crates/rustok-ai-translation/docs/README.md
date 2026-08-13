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

The external structured-task payload uses canonical camel-case JSON field
names. Its glossary and Translation Memory context are bounded and accepted
only when their declared digests exactly match the projected values; an empty
subset carries no binding. This makes retry/recovery deterministic and rejects
context substitution before any provider call.

Every machine-translation packet has a minimum egress classification of
`tenant_private`, even when all source units are public. The packet carries
tenant-scoped resource identity and may carry tenant-owned glossary, memory,
style, or evidence context; a public source unit must not downgrade that
complete payload. Personal and sensitive source units raise the packet
classification. The bridge asks AI health for `tenant_private`, and the AI
provider policy enforces the same classification before routing, reservation,
and every external provider attempt.

The AI execution carries a content-free request binding: owner/task identity,
policy and schema digests, input and evidence digests, classification, and
limits. The adapter independently recreates that binding from its bounded
Translation batch and requires exact equality before accepting either a live
result or a recovered completed execution. This prevents stable-key recovery
from accepting an execution for another request without retaining source text
in AI persistence.

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
remain available. Deterministic composed runtime evidence covers ordered
provider fallback, fail-closed JSON Schema enforcement, sanitized failure
recording, exact attempt usage/cost settlement, request-hash conflict rejection,
in-flight cancellation with reservation release, quota rejection before
provider execution, and encrypted restart replay without another provider call
or bill. Configuration-level provider unavailability also produces typed
degraded health. Live external-provider runtime failure/restart evidence remains
pending. An ignored operator-only `rustok-ai` structured-runtime probe is ready
to collect the approved billable deployment evidence without creating another
adapter path.
Real separate-process file-backed evidence also covers expired-attempt
recovery, reservation preservation, immutable failure recording, and reclaim
with a new lease; production-database multi-replica concurrency remains open.
The adapter resolves and cancels executions through the stable
`(owner, idempotency_key)` AI contract. Cancellation therefore remains durable
when it arrives before AI execution registration, and completed encrypted
results can be recovered without another billable call only after the exact
request binding has been revalidated.
Registering this adapter against chat sessions, direct provider engines, or a
non-durable fallback would violate the machine-translation architecture.

See the [implementation plan](./implementation-plan.md) for the activation
gate.
