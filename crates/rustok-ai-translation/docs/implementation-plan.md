# rustok-ai-translation implementation plan

## Current state

- Status: `in_progress`.
- Structural shape: `no_ui_boundary`.
- `machine_translation` task identity and proposal-only prompt policy exist.
- The adapter implements `MachineTranslationPort` over
  `AiStructuredTaskPort`.
- Typed input/output schemas and deterministic unit/token/length validation
  exist.
- The bridge publishes an exact registered-task descriptor containing its
  owner, task identity, immutable prompt policy, input/output schema digests,
  system prompt, classification policy, and hard execution limits.
- Tests cover request mapping, review-required output, stale policy, missing
  units, and protected-token drift.
- Live runtime registration is intentionally absent.

## Activation gate

Before registration:

1. `rustok-ai` must finish the structured executor over the now-present
   content-free execution/attempt schema, idempotent ledger, task catalog,
   budget reservation, immutable provider price policy, concurrency slots and
   actual attempt token/cost evidence: ordered inference/fallback, atomic
   accounting-policy provisioning, recovery scheduling, terminal-result
   replay semantics, and runtime publication remain.
2. The runtime must keep machine translation out of chat-session and
   agent-stage persistence and retain content-safe evidence only.
3. Translation must project a revision-bound glossary subset and bounded
   Translation Memory suggestions into the request.
4. Runtime contribution composition must register the adapter without a
   server-owned match or either owner importing the bridge.
5. Live failure evidence must cover replay/conflict, invalid output, quota,
   cancellation, restart, fallback, and unavailable/degraded states.

## Verification

- `cargo test -p rustok-ai-translation`
- `node scripts/verify/verify-ai-translation-boundary.mjs`
- `git diff --check`
