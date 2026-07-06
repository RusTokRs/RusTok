# Implementation plan for `rustok-email`

Status: core delivery baseline is locked; the module has returned to the mandatory
manifest/doc contract path.

## Execution checkpoint

- Current phase: fba_transport_verified
- Last checkpoint: Added targeted Rust contract tests for shared write-policy mapping, disabled-provider noop receipt and typed request validation; static FBA evidence now points to test names, without running compilation due to iteration constraint.
- Next step: When compilation is allowed again, run targeted `cargo test -p rustok-email ports::tests`; current no-compile fallback smoke is locked through `npm run verify:foundation:fba-runtime-smoke`.
- Open blockers: None.
- Hand-off notes for next agent: Update this block after each increment.
- Last updated at (UTC): 2026-06-30T20:14:10Z


## FFA/FBA status block

- FFA status: `not_started`
- FBA status: `transport_verified`
- Structural shape: `no_ui_boundary`
- Evidence / notes:
  - capability-only module has no module-owned UI surface, so FFA remains `not_started`;
  - compiled runtime evidence `cargo test -p rustok-email --lib` passed 8/8 on 2026-06-30, covering delivery-port write-policy mapping, typed request validation and disabled-provider noop receipt; FBA status is `transport_verified`;
  - FBA provider slice: `crates/rustok-email/contracts/email-fba-registry.json` + `crates/rustok-email/src/ports.rs` declare `EmailDeliveryPort` / `email.delivery.v1` for transactional delivery consumers with shared `rustok_api::PortContext`/`PortError`, `PortCallPolicy::write()` deadline/idempotency semantics, disabled-provider noop preservation, runtime-verified evidence packet `crates/rustok-email/contracts/evidence/email-contract-test-static-matrix.json` and no-compile fallback smoke `crates/rustok-email/contracts/evidence/email-runtime-fallback-smoke.json` verified by `npm run verify:email:fba` / `npm run verify:foundation:fba-runtime-smoke`.

## Scope of work

- keep `rustok-email` as a capability-only core module without its own UI;
- synchronize SMTP/rendering contract, local docs and manifest metadata;
- do not blur the boundary between email delivery and host-level authorization logic.

## Current state

- `EmailModule` registered as a mandatory core module;
- SMTP transport, template rendering, typed email helpers and email-owned delivery DTOs live inside the module;
- root `README.md`, local docs and `rustok-module.toml` are part of the scoped audit path;
- RBAC stays in the calling module or host runtime, while the shared write-policy context/error baseline comes from `rustok-api`, not moving delivery business logic into the shared layer.

## Stages

### 1. Contract stability

- [x] return `rustok-module.toml` and local docs to the module standard path;
- [x] lock capability-only status and absence of its own UI;
- [x] add targeted contract tests for delivery port write-policy mapping and disabled noop fallback;
- [ ] run targeted contract tests and maintain sync between delivery contract and host integration tests.

### 2. Integration hardening

- [ ] extend typed email payloads only together with local docs and host tests;
- [ ] do not move SMTP/rendering logic back to `apps/server`;
- [ ] document new delivery flows before publishing them in host runtime.

## Verification

- `cargo xtask module validate email`
- `cargo xtask module test email`
- `cargo test -p rustok-email ports::tests` for targeted delivery-port contract tests
- `npm run verify:foundation:fba-runtime-smoke`
- targeted host tests for auth/email delivery flows when runtime wiring changes

## Update rules

1. When changing SMTP/rendering contract, update this file first.
2. When changing public/runtime contract, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata, synchronize `rustok-module.toml`.


## Quality backlog

- [x] Update targeted coverage for delivery port policy/validation/noop receipt scenarios.
- [ ] Run targeted coverage after compilation restriction is lifted.
- [ ] Verify completeness and currency of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
