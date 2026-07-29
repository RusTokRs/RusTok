# rustok-ai-translation implementation plan

## Current state

- Status: `in_progress`.
- Structural shape: `no_ui_boundary`.
- `machine_translation` task identity and proposal-only prompt policy exist.
- The adapter implements `MachineTranslationPort` over
  `AiStructuredTaskPort`.
- Typed input/output schemas and deterministic unit/token/length validation
  exist.
- Stable-key execution status, completed-result recovery, and cancellation are
  mapped to AI without exposing AI persistence to Translation.
- The bridge publishes an exact registered-task descriptor containing its
  owner, task identity, immutable prompt policy, input/output schema digests,
  system prompt, classification policy, and hard execution limits.
- The optional `server` feature composes the descriptor and AI port from the
  neutral host context without exposing the concrete AI runtime to the host.
- Tests cover request mapping, review-required output, stale policy, missing
  units, and protected-token drift.
- The optional distribution feature `ai-translation` publishes the
  Translation-owned lazy runtime factory without a server-owned capability
  match. Production-profile enablement and live execution evidence remain
  intentionally absent.

## Activation gate

Before production activation:

1. `rustok-ai` must finish the structured executor over the now-present
   content-free execution/attempt schema, idempotent ledger, task catalog,
   budget reservation, immutable provider price policy, concurrency slots and
   actual attempt token/cost evidence. Ordered inference/fallback, atomic
   encrypted terminal-result handoff/replay, recovery safety, operator
   accounting-policy provisioning, deployment keyring publication,
   scheduler-owned recovery/result cleanup, and optional lazy runtime
   composition now exist.
2. The runtime must keep machine translation out of chat-session and
   agent-stage persistence and retain content-safe evidence only.
3. Translation must project a revision-bound glossary subset and bounded
   Translation Memory suggestions into the request.
4. Verify the implemented distribution-owned lazy factory composition in the
   production profile without adding a server-owned match or either owner
   importing the bridge.
5. Live failure evidence must cover replay/conflict, invalid output, quota,
   cancellation, restart, fallback, and unavailable/degraded states.

## Verification

- `cargo test -p rustok-ai-translation`
- `node scripts/verify/verify-ai-translation-boundary.mjs`
- `git diff --check`
