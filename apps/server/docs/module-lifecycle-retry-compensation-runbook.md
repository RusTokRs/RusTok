# Runbook: uncertain module lifecycle hook outcomes

A lifecycle state change can commit before its post-hook completes. A failed,
timed-out, or orphaned hook does **not** prove that the hook's external work has
stopped, and the committed tenant intent is not automatically rolled back.

## Current safety restriction

New post-hook retries and compensating lifecycle operations are unavailable.
The journal records selected intent and a hook-settings digest, but does not
record sufficient evidence of the original executable identity and termination
of its external effects. Neither lease expiry, cancellation, an absent claim,
a matching settings digest, nor an operator flag proves quiescence. This also
applies to legacy rows whose claims were already cleared by an older version.

Recovery plans therefore report `retryable = false` and recommend no automated
action. A new retry or compensation fails before issuing a hook, with
`hook_identity_and_quiescence_unverified` when the earlier selected-intent and
settings checks pass (or an earlier not-retryable reason if they do not). A
matching historical completed retry may still be replayed without dispatch. Pre-hook and
orphaned operations likewise must not be assumed safe to repeat merely because
their journal state is terminal. Ordinary toggles and older mixed-version
writers are not yet comprehensively fenced against every legacy uncertainty.

There is **no implemented proof-based reconciliation procedure**. Do not clear
claims, change journal rows, repeat toggles, or operate older workers to bypass
the restriction. Escalate to the module and control-plane owners. Any future
reconciliation must establish durable executor/installation identity, account
for downstream effects, prove external quiescence, and serialize its decision
with the other writers before making a new operation possible.

## Diagnostics

1. Stop affected mixed-version writers according to the deployment procedure;
   prevent old workers from reclaiming or clearing claims. Stopping a worker
   alone is not evidence that downstream work stopped.
2. Record tenant, module slug, operation id, actor, trace, correlation,
   timestamps, journal status and error. Inspect the aggregate claim and the
   related executor/downstream systems; do not mutate these records.
3. Inspect `module_operation_override_states` and current `tenant_modules`
   state. A recorded nullable override means inherited intent, not an absent
   evidence row. The historical effective availability is not a substitute
   for selected intent. The settings digest is input evidence only.
4. Preserve the incident evidence and involve the owning teams. Do not present
   automatic retry or compensation as an available remediation until the
   required identity and quiescence protocol is implemented and verified.

Legacy `failed` rows with a `post-hook:` prefix and newer
`committed_hook_failed` rows can both represent committed tenant intent. A
`committed_hook_pending` row with an expired lease has an unknown outcome.
`orphaned` also does not establish that a dispatched hook stopped.

## Related contracts

- `crates/modules/rustok-modules/src/recovery.rs`
- `crates/modules/rustok-modules/src/executor.rs`
- `crates/modules/rustok-modules/src/lifecycle_writer.rs`
- `apps/server/src/services/module_lifecycle.rs`
- `docs/architecture/modules.md`
- `DECISIONS/2026-05-22-module-lifecycle-hook-phases-and-retry-contract.md`
