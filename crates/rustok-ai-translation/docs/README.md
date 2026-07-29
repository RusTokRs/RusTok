# AI Translation Adapter Contract

`rustok-ai-translation` is an outbound adapter owned jointly at the boundary
between the Translation and AI capabilities. The two owners remain acyclic:
only this support crate imports both public contracts.

The current adapter supports bounded plain text, SEO text, placeholder
templates, and localized scalar values. It validates the Translation request
before billable execution, supplies explicit source and target locales, emits a
typed JSON schema, and validates the complete structured result before
returning it.

Every accepted result is `review_required`. The adapter cannot approve a
proposal, publish content, call an owner service, or construct an owner patch.

The adapter intentionally has no runtime registration in the current slice.
`AiStructuredTaskPort` is now published as a contract, but the canonical
durable implementation is still pending. Registering this adapter against chat
sessions, direct provider engines, or a non-durable fallback would violate the
machine-translation architecture.

See the [implementation plan](./implementation-plan.md) for the activation
gate.
