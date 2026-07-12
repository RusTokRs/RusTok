# rustok-ai-content implementation plan

## Current state

`rustok-ai-content` owns descriptors, generated-payload validation, and
approval policy for `content_moderation` and `blog_draft`. `rustok-ai`
composes the registered handlers and consumes the sensitive-tool defaults; it
must not own content task identity or policy. The supported field rules are
maintained in the crate and module README.

## FFA/FBA readiness

- FFA status: `in_progress`.
- FBA status: `boundary_ready` (`core_transport_ui`).
- Structural shape: `core_transport_ui`
- The module-owned admin package separates core, selected transport, and
  Leptos UI, but its concrete host rendering remains incomplete.
- `content_ai_policy_matrix` is the canonical source of moderation approval
  defaults and must remain consumed by `rustok-ai` rather than duplicated
  there. Its degraded modes are `require_operator_review` and
  `skip_publish_and_keep_draft_review`.
- Evidence: `crates/rustok-ai-content/contracts/ai-content-fba-registry.json`,
  `crates/rustok-ai-content/contracts/evidence/ai-content-consumer-static-matrix.json`,
  `crates/rustok-ai-content/contracts/evidence/ai-content-runtime-fallback-smoke.json`,
  `scripts/verify/verify-ai-content-contract.mjs`, and
  `scripts/verify/verify-ai-fba-baseline.mjs`.

## Next results

1. **Exercise content policy through the composed AI runtime.** Add an
   executable host/direct-path test for moderation approval routing and the
   `blog_draft` degraded path that preserves review rather than publishing.
   Done when evidence covers the support adapter and its `rustok-ai` consumer.
2. **Render the owned admin surface in its hosts.** Connect the existing
   core/transport/UI package to the appropriate admin route and verify native
   server-function selection with parallel GraphQL/headless parity. Done when
   a host-level test or runtime evidence covers both selected paths.
3. **Add only product-approved content verticals.** Any new task must add a
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
