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
  match.
- Production-profile composition and fail-closed missing-keyring evidence are complete:
  the default server feature set selects `ai-translation`, and the composed
  factory resolves to no provider when deployment result keys are absent.
- Durable accounting recovery has deterministic shared-database multi-instance
  evidence: another runtime releases the abandoned provider slot without
  releasing the execution budget reservation, and a restarted worker reclaims
  the queued execution with a new lease.
- Live external-provider execution, failure, and restart evidence remains open.

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
4. The implemented distribution-owned lazy factory is selected by the
   production profile without a server-owned match or either owner importing
   the bridge. The composed missing-keyring path is verified as optional and
   fail-closed.
5. Live failure evidence must cover replay/conflict, invalid output, quota,
   cancellation, restart, fallback, and unavailable/degraded states.

## Verification

- `cargo test -p rustok-ai-translation`
- `cargo test -p rustok-distribution --no-default-features --features ai-translation selected_ai_translation_bridge_publishes_factory_and_stays_optional_without_keyring`
- `node scripts/verify/verify-ai-translation-boundary.mjs`
- `git diff --check`
