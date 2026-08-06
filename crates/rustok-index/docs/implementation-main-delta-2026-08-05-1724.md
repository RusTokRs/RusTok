# `rustok-index` main delta — 2026-08-05 17:24 UTC

Latest checked default branch: `main@aa49a0d9dec64838364055749e063114968338d3`.

The branch merge base remains `9cfc43cf72284e16261f788070a47367613bf2e2`. Twelve commits have landed on
`main` after that merge base while PR #2986 continued. Their changed files are limited to:

- Commerce checkout-compensation diagnostics and source evidence;
- Forum reply-range GraphQL transport inside `crates/rustok-forum`;
- Inventory availability diagnostics;
- Order checkout-recovery diagnostics;
- Pages/Page Builder native storefront evidence and harnesses.

No intervening commit changes:

- `crates/rustok-index`;
- `crates/rustok-distribution` Product Index composition;
- `apps/server/src/graphql/schema.rs`;
- `apps/server/src/graphql/index_drift_diagnosis.rs`;
- `apps/server/src/services/index_drift_diagnosis_operator.rs`;
- the Index verification scripts changed by PR #2986.

The Forum GraphQL transport is module-owned and is mounted through generated optional-module
composition. The Index diagnosis transport is server-owned and is mounted directly into the server
root mutation. They do not replace, duplicate, or edit the same source file.

At this check, the Index branch remains scoped to exact-entity diagnosis, Product locale absence,
and their source-ready evidence. Tests, verifiers, formatting, Cargo checks, PostgreSQL or GraphQL
execution, workflows, and CI were not run by the implementation agent.
