# rustok-ai-translation implementation plan

## Current state

- Status: `in_progress`.
- Structural shape: `no_ui_boundary`.
- `machine_translation` task identity and proposal-only prompt policy exist.
- The adapter implements `MachineTranslationPort` over
  `AiStructuredTaskPort`.
- Typed camel-case input/output schemas and deterministic
  unit/token/whitespace/length validation exist; output schemas and
  deserialization reject unknown fields. Bounded glossary and Translation
  Memory context must be exact-digest-bound, with no empty or dangling context
  binding.
- Every structured machine-translation packet is classified at least
  `tenant_private`, because it contains tenant-scoped resource identity and
  may contain glossary, memory, style, or evidence context. Personal and
  sensitive source units raise that packet classification. The bridge requests
  tenant-private health; AI checks the same classification before routing,
  reservation, and provider egress.
- Stable-key execution status, completed-result recovery, and cancellation are
  mapped to AI without exposing AI persistence to Translation. Live and
  recovered completions must match the exact content-free request binding
  recreated from the original bounded batch.
- Provider-neutral conservative cost estimation uses the exact structured
  request, tenant routing, attempt limits, and immutable price policies used
  by execution without registering work, reserving budget, or invoking a
  provider.
- The bridge publishes an exact registered-task descriptor containing its
  owner, task identity, immutable prompt policy, input/output schema digests,
  system prompt, classification policy, and hard execution limits.
- The optional `server` feature composes the descriptor and AI port from the
  neutral host context without exposing the concrete AI runtime to the host.
- Tests cover request and estimate mapping, review-required output, stale
  policy, missing units, protected-token multiplicity and whitespace drift,
  unknown output fields, and live/recovered execution-binding mismatch.
- The optional distribution feature `ai-translation` publishes the
  Translation-owned lazy runtime factory without a server-owned capability
  match.
- Production-profile composition and fail-closed missing-keyring evidence are complete:
  the default server feature set selects `ai-translation`, and the composed
  factory resolves to no provider when deployment result keys are absent.
- Real separate-process recovery evidence uses a file-backed database: a second
  process releases the abandoned provider slot without releasing the execution
  budget reservation, preserves immutable failure evidence, and reclaims the
  queued execution with a new lease. Production-database multi-replica
  concurrency remains open.
- Deterministic composed runtime evidence covers ordered provider fallback,
  fail-closed JSON Schema enforcement, sanitized failure recording, exact
  per-attempt token/price/cost settlement, request-hash conflict rejection,
  in-flight cancellation with reservation release, quota rejection before
  provider execution, and authenticated encrypted restart replay without
  another provider call or bill.
- Configuration-level provider unavailability produces typed degraded health.
- An ignored operator-only `rustok-ai` probe executes a deployment-owned
  provider config through the durable structured runtime and restart-replay
  path; retained output from an approved billable run is not yet collected.
- Live external-provider execution, runtime failure, and restart evidence
  remains open.

## Activation gate

Before production activation:

1. `rustok-ai` must finish the structured executor over the now-present
   content-free execution/attempt schema, idempotent ledger, task catalog,
   budget reservation, immutable provider price and egress-classification
   policy, concurrency slots and
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
5. Live external-provider evidence must corroborate the deterministic
   replay/conflict, fallback, invalid-output, cancellation, and quota paths and
   cover runtime outage/degradation and restart in deployment.

## Verification

- `cargo test -p rustok-ai-translation`
- `cargo test -p rustok-ai --features server -- --ignored executes_declared_live_provider_through_durable_structured_runtime`
- `cargo test -p rustok-ai --features server separate_process_recovers_and_reclaims_an_expired_execution -- --nocapture`
- `cargo test -p rustok-distribution --no-default-features --features ai-translation selected_ai_translation_bridge_publishes_factory_and_stays_optional_without_keyring`
- `node scripts/verify/verify-ai-translation-boundary.mjs`
- `git diff --check`
