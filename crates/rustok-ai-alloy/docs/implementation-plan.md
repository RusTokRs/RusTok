# rustok-ai-alloy implementation plan

## Current state

`rustok-ai-alloy` owns the `alloy_code` descriptor, runtime-payload validation,
and Alloy script execution policy. `rustok-ai` consumes its registration API
and remains the runtime and transport composition owner. The supported
operations and payload rules are documented in the crate README and policy
registry.

## FFA/FBA readiness

- FFA status: `not_started` — this support adapter owns no UI surface.
- FBA status: `in_progress` (`domain_support_adapter`).
- `alloy_script_execution_policy` records `allowed_operations`,
  `runtime_operation`, and the current remote transport status. It must remain
  domain-owned; provider routing and execution transport remain in
  `rustok-ai`.
- Evidence: `crates/rustok-ai-alloy/contracts/ai-alloy-policy-registry.json`,
  `crates/rustok-ai-alloy/contracts/evidence/ai-alloy-policy-static-matrix.json`,
  and `scripts/verify/verify-ai-alloy-policy.mjs`.

## Next results

1. **Exercise the policy through the composed direct-execution path.** Add a
   targeted integration test that proves `rustok-ai` consumes the registered
   descriptor, rejects invalid payloads, and admits only policy operations.
   Done when the test covers the composed boundary rather than source markers
   alone.
2. **Specify the remote Alloy transport only when its product owner selects
   it.** Define authentication, operation mapping, failures, and evidence
   before changing `remote_transport` from `not_started`. Done when the
   transport contract has a named owner and no alternate transport path is
   implied.

## Verification

- `npm run verify:ai-alloy:policy`
- `npm run verify:ai:domain-verticals`
- `cargo test -p rustok-ai-alloy --lib`

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [AI FBA registry](../contracts/ai-alloy-policy-registry.json)
