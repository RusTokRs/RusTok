# План реализации `rustok-ai-content`

## Цель

Сделать `rustok-ai-content` owner-слоем для content AI verticals: content moderation и blog draft generated payload contracts.

## Этапы

1. Scaffold crate + docs.
2. Перенести `content_moderation` direct wiring.
3. Перенести `blog_draft` task/tool identity и generated payload validation в content-owned support crate.
4. Добавить policy matrix и approval routing integration. ✅

## Execution checkpoint

- Current phase: blog_contract_static_evidence_added
- Last checkpoint: Added compile-free static verification for the content AI contract and expanded blog draft contract tests to cover full payloads, patch-style empty payloads, and blank-value rejection across every optional generated text field.
- Next step: Добавить executable targeted verification evidence при разрешённых компиляциях.
- Open blockers: compile/test evidence отложен по явному ограничению итерации: без компиляций.
- Hand-off notes for next agent: Не переносить executable runtime composition из `rustok-ai`; support crate владеет descriptors/policy/validation, host crate только consumes defaults.
- Last updated at (UTC): 2026-06-22T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - `admin/src/core.rs`, `admin/src/transport.rs`, and `admin/src/ui/leptos.rs` provide the module-owned admin FFA split.
  - Transport exposes a native-server plus GraphQL fallback placeholder profile; concrete host rendering remains a follow-up.
  - FBA support-consumer metadata is locked in `crates/rustok-ai-content/contracts/ai-content-fba-registry.json` for content moderation/blog draft task identity, policy matrix and generated-payload validation, including `require_operator_review` and `skip_publish_and_keep_draft_review` degraded modes, mirrored by `crates/rustok-ai-content/contracts/evidence/ai-content-consumer-static-matrix.json` and source-smoke `crates/rustok-ai-content/contracts/evidence/ai-content-runtime-fallback-smoke.json`, and checked by `scripts/verify/verify-ai-fba-baseline.mjs`.
  - The global readiness board uses the canonical hyphenated module slug `ai-content`.

## Quality backlog

- [x] Domain-owned policy matrix for content moderation/blog draft approval routing.
- [x] Runtime policy integration consumes content-owned sensitive-tool defaults from `rustok-ai`.
- [x] Расширить blog generated payload contract тестами.
- [ ] Запустить `cargo test -p rustok-ai-content --lib` и `cargo test -p rustok-ai --lib` при разрешённых компиляциях.
