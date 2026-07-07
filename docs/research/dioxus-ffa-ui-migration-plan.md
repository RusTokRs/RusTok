---
id: doc://docs/research/dioxus-ffa-ui-migration-plan.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# Plan: FFA Refactor of UI Packages and Preparation for Dioxus

## Context

The platform already captures a dual-path transport contract for Leptos UI:

- native `#[server]` functions — preferred internal path in SSR/hydrate runtime;
- GraphQL `/api/graphql` — mandatory parallel contract for headless hosts and fallback.

The goal of this plan is to prepare module-owned UI packages for the FFA pattern
(shared core + transport adapters + host adapters) so that migration to Dioxus can be
incremental, rather than a second full rewrite.

## Goals

1. Preserve the current production contract (`native + GraphQL fallback`) without regression.
2. Decompose Leptos UI packages into framework-agnostic and framework-specific layers.
3. Prepare infrastructure for Dioxus host/adapters without changing domain logic.
4. Maintain parity for headless clients (Next.js/mobile/external).

## Non-goals

- Immediate full migration of all UI packages to Dioxus.
- Removal of GraphQL/REST contracts.
- Changing the ownership model (UI ownership remains with modules).

## Invariants

- GraphQL must not be removed because of the emergence/expansion of native server path.
- UI package must continue to work in SSR/hydrate and standalone CSR compatibility mode.
- Host application remains a mount/wiring/navigation layer, not the owner of domain UI.

## Implementation Phases

## Phase A — Baseline and Inventory (1–2 weeks)

### A1. Pilot Selection

- Pilot 1 (medium complexity): `rustok-pages` or `rustok-blog`.
- Pilot 2 (high complexity): `rustok-search` or `rustok-commerce`/`rustok-cart`.

### A2. Connectivity Map

For each pilot, capture:

- Leptos-specific points (`#[component]`, router hooks, reactive state);
- transport binding points (`#[server]`, GraphQL requests, fallback branches);
- places where UI/state/business logic are mixed.
- Baseline connectivity map for pilots (`rustok-pages`, `rustok-search`) captured in `docs/research/dioxus-ffa-pilot-connectivity-map.md`.

### A3. Contract Freeze

- Capture current GraphQL/native surfaces and smoke scripts.
- Add parity checklist: SSR native path, GraphQL fallback, headless path.
- Baseline checklist established in `docs/verification/ffa-ui-parity-checklist.md` and mandatory for phase-gate evidence.

## Phase B — FFA Decomposition in Pilots (2–4 weeks)

For each pilot UI package, introduce 3 layers:

1. `core.rs` or `core/` (framework-agnostic)
   - use-cases, typed state transitions, view-model mapping;
   - errors and policy results in transport-agnostic form;
   - `core.rs` acceptable for a small slice, `core/` mandatory when multiple subdomains appear (`view_model`, `policy`, `error`, `ports`, `identifiers`).
2. `transport/`
   - `native_server_adapter` (current Leptos native path);
   - `graphql_adapter` (fallback/headless-compatible path);
   - if the slice temporarily has only one adapter, this is captured as a temporary single-adapter state with next-step parity plan.
3. `ui/leptos.rs` or `ui/leptos/`
   - render/bind layer only, without transport/business ownership;
   - `ui/leptos.rs` acceptable for a single adapter file, `ui/leptos/` used when the render adapter layer grows.

Key rule: UI adapter does not call raw GraphQL/native functions directly. It may only access module-owned `transport/` facade; request/command/state construction, validation and business/policy decisions remain in `core` ports/helpers.

### Standard for minimal FFA slice and anti-over-extraction

An FFA slice should reduce coupling, not mechanically move every line from Leptos adapter
into `core`. For all modules, a unified decision gate applies: moving to `core` is allowed if
at least one of the conditions below is met.

**Move to `core`:**

1. request/command construction, normalization and validation that affect transport payload
   or domain semantics;
2. view-model mapping with computable fields, fallback policy, CSS/status class policy,
   route/query intent, pagination/filter/sort state or reusable display rules;
3. transport-agnostic error/policy envelope, if it must be consumed identically by Leptos,
   future Dioxus adapter, Next/mobile/headless host or tests;
4. state transitions, busy/selected/empty/error policy and mutation outcomes, if they could
   diverge between adapters or need to be tested without UI runtime;
5. a recurring pattern already present in at least two surfaces or expected to be
   extracted into shared foundation.

**Keep in `ui/leptos`:**

1. simple i18n label bindings (`t(locale, key, fallback)`) and one-shot success/error copy,
   if they do not change policy and are not needed by other host adapters;
2. DOM layout, classes without state/policy branching, event binding, signals/resources/effects;
3. reset/refresh side effects after mutation, if they depend on specific adapter state;
4. mechanical wrappers over a single formatting line that provide no reuse and increase
   the number of DTO/enum/label structs;
5. code whose extraction increases public surface more than it reduces coupling.

**Size rule:** a small FFA slice is preferable to a large one, but it must have
architectural meaning. If a change adds more boilerplate than it removes coupling,
the slice is rejected or reverted.

**Mandatory review after each slice:**

- capture in the local implementation plan which coupling problem the slice solved;
- verify that the UI adapter became thinner specifically in business/policy/transport ownership, not just
   gained more passthrough DTOs;
- if over-extraction is detected, revert it in the same iteration and leave a reusable rule in this plan;
- when modules diverge, first look for a common pattern, then extract it to a shared crate
   rather than copying large module-local structs.

## Phase C — Shared Platform Abstractions (1–2 weeks)

Extract recurring contracts into shared crate(s):

- `RequestMeta`, `EffectiveLocale`, `TenantScope`;
- typed query/filter/pagination contracts;
- unified UI error envelope.

Separately prepare a portability port for route/query plumbing:

- current Leptos implementation remains;
- add transport/framework-agnostic contract for future Dioxus routing adapter;
- shared foundation for first wave extracted into `rustok-api`: `normalize_ui_text`, `parse_ui_csv`, `UiRouteQueryUpdate`, with Leptos adapter applying these intents through `leptos-ui-routing`.

## Phase D — Wave Rollout to Remaining UI Packages (3–6 weeks)

### Wave 1 (low/medium complexity)

- `pages`, `blog`, `region`, `product`.

### Wave 2 (high complexity)

- `search`, `cart`, `commerce`, `workflow`.

For each package, mandatory DoD:

- structural shape captured at minimum as `core_only`, and for phase-gate at least `core_transport_ui`;
- core separated from Leptos runtime (`core.rs` and `core/` do not contain `leptos*` imports);
- native + GraphQL adapters work and are covered by integration tests, or temporary single-adapter state is explicitly noted with next-step parity plan;
- Leptos UI layer became a thin adapter and does not call raw GraphQL/native functions directly;
- module docs and central docs updated when contracts change.

## Parallel Host Track for Admin/Storefront

Admin and frontends are migrated **in parallel, but not as the first layer**:

1. First, module-owned UI packages extract `core/transport/ui` and keep Leptos UI as a thin adapter.
2. Simultaneously, host applications (`apps/admin`, `apps/storefront` and future Dioxus shells) receive only portable host contracts: route/query, locale, auth/session, tenant scope, mount registry and manifest wiring.
3. Host applications do not become owners of domain UI logic; they mount module surfaces through adapters.
4. Dioxus host connects after 1–2 pilot module cores are ready and verifies reuse without removing Leptos or GraphQL/headless paths.

This means changing host wiring requires a separate parity check, but domain logic migration remains in module UI packages.

## Phase E — Dioxus Pilot (2–4 weeks)

1. Set up a minimal Dioxus host shell.
2. Connect 1–2 pilot module UI surfaces through the already extracted core.
3. Implement Dioxus-specific UI adapter + native transport adapter.
4. Confirm parity with Leptos across scenarios and failure modes.

## Verification

For each affected module/wave:

- `cargo xtask module validate <slug>`
- `cargo xtask module test <slug>`

When changing host/UI wiring, additionally:

- `npm run verify:i18n:ui`
- `npm run verify:i18n:contract`
- `npm.cmd run verify:storefront:routes`

## Backlog Execution Principle (One Task Per Iteration)

To avoid accumulating architectural drift and contradictory records, the program is executed
strictly following the **"one task -> all UI surfaces -> double documentation verification"** principle:

1. Take **one specific task** (e.g., extracting `core` for a selected use-case).
2. Apply it **across all relevant UI packages/host surfaces** where this contract must be identical.
3. Update documentation:
   - local module docs;
   - central docs in `docs/`;
   - ADR/decision trail if necessary.
4. Perform **double documentation verification** before moving to the next task:
   - pass #1: verify that new wording fully matches the actual code;
   - pass #2: targeted search and removal/correction of old wording that is misleading
     (outdated "Leptos-only" or conflicting transport descriptions, etc.).
5. Only then close the task and move to the next one.

This mode is mandatory for phases B–E to avoid partial rollout where code and docs diverge
between modules or hosts.

## Verification Script Update Policy

Verification scripts (`scripts/verify/*`) are considered part of the live platform contract and
must be updated together with changes to the rules they check.

Mandatory rules:

1. If a migration task changes transport/UI/doc contract, it **must** include
   updating the corresponding verify scripts in the same PR/iteration.
2. A task is not considered complete if the contract has been changed but verify scripts do not
   reflect the new rules.
3. After each wave (Phase D), a separate review of verify scripts is performed for
   outdated patterns/exceptions and addition of new anti-pattern checks.
4. Before closing a phase-gate, the task owner must attach the output of running the current
   verify scripts as part of the evidence.

`test:verify:ffa:ui:migration` is a batch fast-run of source-level FFA guardrails.
It must include fixture sets for boundary sweep, transport profile and module boundaries
(`channel`, `region`, `blog`, `pages`, `fulfillment`, `product`, `forum`) if the corresponding
verify script already exists.

Minimum revision cadence: at least once every 2–4 weeks and always upon
completion of each rollout wave.

## Documentation and Governance

For platform-level changes:

1. update local docs of affected modules;
2. update central docs in `docs/`;
3. keep `docs/index.md` up to date;
4. create an ADR in `DECISIONS/` if the platform transport/UI contract changes.

## Risks and Mitigation

1. **Risk:** core layer remains coupled to Leptos types.
   - **Mitigation:** CI check that forbids `leptos*` dependencies in `core` crates.

2. **Risk:** fallback path stops being actually verified.
   - **Mitigation:** mandatory parity integration suites for native and GraphQL adapters.

3. **Risk:** divergence between Leptos and Dioxus behavior.
   - **Mitigation:** contract tests at the shared use-case level + snapshot tests of key state transitions.

## Program Completion Criteria

The program is considered complete when:

- at least 2 complex modules have undergone FFA decomposition and parity verification;
- Dioxus pilot confirms reuse of shared core without duplicating domain logic;
- headless contracts have not degraded;
- documentation and ADR reflect the new target state.

## Reconciliation with Current Code (as of 2026-05-23)

Below captures the alignment of the plan with the current repository state.

### 1) Actual dual-path contract already captured in docs

- `docs/UI/graphql-architecture.md` captures the model: native `#[server]` preferred + GraphQL as mandatory parallel contract.
- `apps/storefront/docs/README.md` captures native-first in SSR/hydrate and mandatory GraphQL fallback for storefront surfaces.

### 2) UI packages in code are currently Leptos-specific

- Basic shared UI crates depend on Leptos:
  - `crates/leptos-ui/Cargo.toml`
  - `crates/leptos-ui-routing/Cargo.toml`
  - `crates/rustok-graphql/Cargo.toml`
  - `crates/leptos-auth/Cargo.toml`
- Module-owned UI packages actively use `leptos::*`, `#[component]`, `leptos_router` and Leptos hooks (example: `rustok-search`, `rustok-workflow`, `rustok-commerce`, `rustok-cart`).

### 3) Data already flows through native/GraphQL hybrid

- In `crates/rustok-*/storefront/src/api.rs` and `crates/rustok-*/admin/src/api.rs`, GraphQL adapters (`rustok_graphql`) and `#[cfg(feature = "ssr")]` branches for native SSR paths are visible.
- This means the plan does not invent a new model but formalizes an already existing runtime split and converts it into an FFA structure.

### 4) Pilot candidates confirmed by current complexity

- `rustok-pages`/`rustok-blog`: smaller UI state volume and simpler CRUD/read scenarios.
- `rustok-search` and `rustok-commerce`/`rustok-cart`: pronounced complexity in state/fallback flows and SSR branches.

### 5) Verification commands used to update this document

```bash
rg -n "Dioxus|Leptos|headless|server functions|UI packages|GraphQL" docs crates apps
rg -n "^use leptos|#\[component\]|#\[server\]|leptos =|leptos_router|leptos_ui_routing|cfg\(feature = "ssr"\)" crates/rustok-*/admin crates/rustok-*/storefront crates/leptos-* --glob "*.rs" --glob "Cargo.toml"
nl -ba docs/UI/graphql-architecture.md
nl -ba apps/storefront/docs/README.md
npm run verify:ffa:ui:migration
```

### 6) Implication for plan execution

The plan is executed **without changing the product contract**: first, package structure refactor (core/transport/ui), then Dioxus adapter pilot. GraphQL/REST remain mandatory contracts for headless parity at every stage.

## Phase-Gate Criteria (mandatory transitions between phases)

- **A -> B**: pilot connectivity map completed, current native/GraphQL surfaces captured, parity checklist compiled.
- **B -> C**: `core/transport/ui` actually extracted in pilots, UI does not go directly to transport, parity tests pass in pilot scope.
- **C -> D**: shared abstractions agreed with module owners, portability port for route/query accepted as contract.
- **D -> E**: at least one wave completed without doc drift, double documentation verification performed for all affected modules.
- **E -> Program done**: Dioxus pilot passes parity/KPI checks and does not violate headless contracts.

## KPI Parity (measurable thresholds)

- Functional parity: all mandatory scenarios of pilot checklist pass in both native path and GraphQL fallback path.
- Error parity: error-classification divergence rate between adapters = 0 for mandatory scenarios.
- Performance guard: p95 latency of new adapter paths does not degrade more than 15% relative to pilot baseline.
- Contract guard: 0 cases of removal/weakening of headless GraphQL/REST contract in migration PRs.
- Docs guard: 0 known conflicting/outdated transport wordings after double verification.

## RACI (who approves phase-gates)

- **Responsible (R):** owner of the specific module UI package + migration task executor.
- **Accountable (A):** platform foundation team (final gate on transport/UI contract).
- **Consulted (C):** owners of `apps/admin`, `apps/storefront`, `apps/next-admin`, `apps/next-frontend` on host parity.
- **Informed (I):** adjacent module owners and observability/QA owners.

A phase-gate is not considered passed without explicit confirmation from `A` and a note on double documentation verification.
