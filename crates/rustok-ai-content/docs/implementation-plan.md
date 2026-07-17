# rustok-ai-content implementation plan

## Current state

`rustok-ai-content` owns descriptors, generated-payload validation, and
approval policy for `content_moderation` and `blog_draft`. `rustok-ai`
composes the registered handlers and consumes the sensitive-tool defaults; it
must not own content task identity or policy. The supported field rules are
maintained in the crate and module README.

## FFA/FBA readiness

- FFA status: `not_started` (no standalone support-adapter UI).
- FBA status: `boundary_ready` (`no_ui_boundary`).
- Structural shape: `no_ui_boundary`
- Content adapter controls are composed by the `rustok-ai` owner Leptos and
  Next.js admin surfaces. They must not create a route inside Blog, Forum, or
  this support crate.
- `content_ai_policy_matrix` is the canonical source of moderation approval
  defaults and must remain consumed by `rustok-ai` rather than duplicated
  there. Its typed degraded modes are `require_operator_review` and
  `keep_draft_for_review`; the latter is consumed by the direct Blog adapter
  when it builds owner input. Missing or incompatible draft policy fails closed
  before owner persistence, so generated content cannot auto-publish.
- Evidence: `crates/rustok-ai-content/contracts/ai-content-fba-registry.json`,
  `crates/rustok-ai-content/contracts/evidence/ai-content-consumer-static-matrix.json`,
  `crates/rustok-ai-content/contracts/evidence/ai-content-runtime-fallback-smoke.json`,
  `scripts/verify/verify-ai-content-contract.mjs`, and
  `scripts/verify/verify-ai-fba-baseline.mjs`.

## Completed direct-path evidence

Both content verticals now have composed `rustok-ai` evidence. The direct
`content_moderation` handler verifies validated output, locale propagation,
operator explanation, and its sensitive audit trace. The `blog_draft` path
verifies the adapter-owned keep-draft policy and persists an unpublished draft
through the Blog owner service.

## Next results

1. **Add only product-approved content verticals.** Any new task must add a
   content-owned descriptor, generated-payload validation, approval policy,
   and composed evidence before registration in `rustok-ai`. Done when no
   content task identity or policy is hard-coded by the runtime.

## Verification

- `npm run verify:ai-content:fba`
- `npm run verify:ai:domain-verticals`
- `cargo test -p rustok-ai-content --lib`

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [AI content FBA registry](../contracts/ai-content-fba-registry.json)
