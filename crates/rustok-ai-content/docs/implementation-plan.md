# `rustok-ai-content` — Implementation Plan

## Goal

Make `rustok-ai-content` the owner layer for content AI verticals: content moderation and blog draft generated payload contracts.

## Stages

1. Scaffold crate + docs.
2. Move `content_moderation` direct wiring.
3. Move `blog_draft` task/tool identity and generated payload validation to content-owned support crate.
4. Add policy matrix and approval routing integration. ✅

## Execution checkpoint

- Current phase: blog_contract_static_evidence_added
- Last checkpoint: Added compile-free static verification for the content AI contract and expanded blog draft contract tests to cover full payloads, patch-style empty payloads, and blank-value rejection across every optional generated text field.
- Next step: Add executable targeted verification evidence when compilations are allowed.
- Open blockers: compile/test evidence deferred by explicit iteration constraint: no compilations.
- Hand-off notes for next agent: Do not move executable runtime composition from `rustok-ai`; support crate owns descriptors/policy/validation, host crate only consumes defaults.
- Last updated at (UTC): 2026-06-22T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- Evidence:
  - `admin/src/core.rs`, `admin/src/transport.rs`, and `admin/src/ui/leptos.rs` provide the module-owned admin FFA split.
  - Transport exposes a native-server plus GraphQL fallback placeholder profile; concrete host rendering remains a follow-up.
  - FBA support-consumer metadata is locked in `crates/rustok-ai-content/contracts/ai-content-fba-registry.json` for content moderation/blog draft task identity, `content_ai_policy_matrix` policy ownership and generated-payload validation, including `require_operator_review` and `skip_publish_and_keep_draft_review` degraded modes, mirrored by `crates/rustok-ai-content/contracts/evidence/ai-content-consumer-static-matrix.json` and source-smoke `crates/rustok-ai-content/contracts/evidence/ai-content-runtime-fallback-smoke.json`, and checked by `scripts/verify/verify-ai-fba-baseline.mjs`.
  - Boundary readiness is backed by executable `cargo test -p rustok-ai-content --lib` coverage for content-owned descriptors, policy matrix and generated payload validation.
  - The global readiness board uses the canonical hyphenated module slug `ai-content`.

## Quality backlog

- [x] Domain-owned policy matrix for content moderation/blog draft approval routing.
- [x] Runtime policy integration consumes content-owned sensitive-tool defaults from `rustok-ai`.
- [x] Expand blog generated payload contract with tests.
- [ ] Run `cargo test -p rustok-ai-content --lib` and `cargo test -p rustok-ai --lib` when compilations are allowed.
