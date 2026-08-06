# `rustok-index` main delta — 2026-08-06 admission recheck

Active branch: `agent/index-m6-repair-evidence-admission-20260806`.

Stack parent: `agent/index-m6-repair-execution-evidence-20260806@aab4f9418317f965d540adf538ede2e1660785d1` / PR #3083.

Parent dependencies:

- `agent/index-m6-orphan-link-repair-20260806@8c36960608ef387b5e36f2b5904c6ea83ceda752` / PR #3075;
- `agent/index-m6-prepared-repair-recovery-20260806@7ababf39e26de9d2039c864d5092147d52d50d1a` / PR #3067.

Latest rechecked main: `main@fae8e537e9712e63350ab93248cab23a8c1eafaf`.

## Intervening main delta

The complete `main@b564f764fc28b12da55b2a0db0d8b4e94c97b0d4..main@fae8e537e9712e63350ab93248cab23a8c1eafaf`
delta contains ten commits. Changed paths are confined to:

- Commerce, Customer, and Region diagnostic or GraphQL error-boundary hardening;
- Forum/Search canonical route projection, reindex evidence, native-host route evidence, and exact
  storefront route validation;
- Page Builder inline-session DOM hardening;
- Storefront Forum category/topic route validation.

No `crates/rustok-index` source, migration, documentation, contract, integration-test, evidence
runner, or Index verifier path changed. There is no path overlap with the prepared-repair recovery,
orphan-link repair, repair execution harness, or retained-evidence admission slices.

## Stacked PR boundary

This branch deliberately starts from PR #3083 because the locked admission commands execute the
metadata target and both concrete PostgreSQL targets introduced there. PR #3083 is stacked on #3075,
which is stacked on #3067.

A pull request from this branch to `main` therefore includes all parent slices until they land. After
#3067, #3075, and #3083 merge, the remaining diff is the concrete repair retained-evidence admission
packet only.

## Review scope

The new slice adds only:

- one bounded PostgreSQL environment metadata integration target;
- one locked JSON execution contract;
- one clean-commit capture runner with source hashing and credential redaction;
- one pending/executed retained-evidence verifier;
- admission and harness documentation updates;
- live plan, documentation index, static harness guard, and aggregate verifier updates.

No production runtime, public transport, migration, domain, application, or PostgreSQL adapter source
is modified. No runtime evidence packet or retained stdout/stderr log is included before owner
execution.

## Validation boundary

This is a source-level contract and overlap review only. Tests, Node verifiers, formatting, Cargo
checks, migrations, PostgreSQL scenarios, evidence capture, workflows, and CI were not executed by
the implementation agent.
