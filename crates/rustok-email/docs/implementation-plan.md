# Implementation plan for `rustok-email`

## Current state

`rustok-email` is a capability-only core module. It owns SMTP delivery,
template rendering, typed delivery requests and receipts, and the
`EmailDeliveryPort`; authorization remains with the calling module or host.
The disabled-provider path returns a typed noop receipt, and the port requires
deadline and write-idempotency semantics.

Targeted runtime evidence has established the current delivery policy and
validation behavior. The module has no module-owned UI.

## FFA/FBA status block

- FFA status: `not_started`
- FBA status: `transport_verified`
- Structural shape: `no_ui_boundary`
- FBA provider contract: `EmailDeliveryPort` / `email.delivery.v1` in
  `crates/rustok-email/contracts/email-fba-registry.json`.
- Runtime and fallback evidence:
  `crates/rustok-email/contracts/evidence/email-contract-test-static-matrix.json`
  and `crates/rustok-email/contracts/evidence/email-runtime-fallback-smoke.json`.
- `npm run verify:email:fba` and `npm run verify:foundation:fba-runtime-smoke`
  lock provider metadata, policy semantics, typed validation, and fallback
  behavior.

## Open results

1. **Extend typed delivery payloads only with an owned contract.** Add a new
   template, delivery field, or receipt behavior together with module docs and
   host integration tests.
   **Depends on:** the consuming module's public delivery requirement.
   **Done when:** request validation, idempotency, template-error retry policy,
   and disabled-provider behavior are covered by targeted tests.

2. **Keep SMTP and rendering ownership out of the server host.** New host
   wiring must consume the published delivery port rather than reimplementing
   provider mode, rendering, or delivery policy.
   **Depends on:** host runtime composition.
   **Done when:** no server-local email business path exists and the module docs
   and manifest match the exposed delivery contract.

3. **Document a new delivery flow before publishing it.** Record the calling
   contract, recovery behavior, and operational owner whenever a flow becomes
   available to a host or domain module.
   **Depends on:** the change-owning delivery consumer.
   **Done when:** module and consumer documentation describe the same failure
   and retry semantics.

## Verification

- `npm run verify:email:fba`
- `npm run verify:foundation:fba-runtime-smoke`
- `cargo xtask module validate email`
- `cargo xtask module test email`
- `cargo test -p rustok-email ports::tests`
- Targeted host integration tests when runtime wiring changes.

## Change rules

1. Keep delivery policy, rendering, and provider behavior in this module.
2. Update the root README, local docs, and `rustok-module.toml` with a public
   delivery contract change.
3. Update this status block and `docs/modules/registry.md` with an FBA boundary
   change.
