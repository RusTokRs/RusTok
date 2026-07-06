# Implementation plan for `rustok-test-utils`

## Execution checkpoint

- Current phase: plan_sync
- Last checkpoint: Initial bootstrap by registry workflow.
- Next step: Synchronize the plan with the current code and select the first incomplete item.
- Open blockers: None.
- Hand-off notes for next agent: After each increment, update this block.
- Last updated at (UTC): 2026-05-20T00:00:00Z

## Current status

- Status: **active baseline**.
- Summary: crate already covers basic scenarios (db setup, mock event bus, fixtures, helpers),
  but formalization of a coverage matrix by module and unified quality gates for the test tooling are missing.

## Gap analysis

### What is already done

- Working modules `db`, `events`, `fixtures`, `helpers` exist.
- Public re-export entry points are available for quick inclusion in tests.
- Crate is used as a common reuse layer in platform tests.

### What is missing

- A formal conformance map: which module test scenarios are covered by which utilities.
- A set of golden/contract tests for the test utilities themselves (especially for mock event behavior).
- Standardized examples for multi-tenant/RBAC/integration edge-cases.
- Versioning policy for test APIs (changes to builders/fixtures without unnecessary breakage).

## Work stages

### Stage 1 — Inventory and standardization

- Lock the utility catalog by test type (unit/integration/contract).
- Clarify recommended entry points and usage anti-patterns.
- Synchronize documentation with the central testing guide.

### Stage 2 — Coverage expansion

- Add missing fixtures for key-domain scenarios.
- Strengthen mock event utilities with publication order/idempotency checks.
- Introduce ready helper patterns for tenancy/RBAC test contexts.

### Stage 3 — Stability and release gates

- Add self-tests for public test-utils API.
- Introduce quality gates: a smoke test set to verify critical helpers.
- Lock deprecation policy for changes to test-fixtures API.

## Readiness criteria

- A documented matrix of "test scenario → recommended helper/fixture" exists.
- Public APIs of `rustok-test-utils` are covered by their own regression tests.
- Standardized reusable fixtures exist for tenancy/RBAC/event flows.
- Changes to test-utils API are accompanied by migration notes for consumers.

## Verification metrics

- **Scenario coverage:** share of priority test scenarios that have a recommended helper (target: ≥ 90%).
- **Utility stability:** percentage of green self-tests for `rustok-test-utils` in CI (target: 100%).
- **Adoption consistency:** share of new tests using shared helpers instead of local duplication (target: MoM growth).
- **Migration safety:** number of regressions in consumers after test-utils API changes (target: 0 critical regressions).

## Checklist

- [x] contract tests cover all public use-cases.



## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and accuracy of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
