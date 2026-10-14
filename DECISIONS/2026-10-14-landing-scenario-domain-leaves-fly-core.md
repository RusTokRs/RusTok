# Landing and runtime-scenario domain leaves the Fly core for the Page Builder module

- Date: 2026-10-14
- Decision status: Accepted
- Implementation status: Implemented
- Owners: `rustok-page-builder` (receiving module), `fly` (core engine)
- Extends: 2026-10-14-fly-rich-text-embed-action-runtime-and-adapters.md
- Supersedes: None
- Superseded by: None

## Context

The deep audit (P2-13 in `docs/audits/fly-page-builder-deep-audit-2026-10-14.md`) found the
`fly` core to be a god-crate with RusTok domain leakage: next to the framework-neutral AST
and GrapesJS codec sit `landing_contract`, `landing_property`, `landing_readiness`,
`context_scenario`, and the `runtime_scenario_*` modules. Landing contracts and release
scenarios are RusTok product domain, not engine; their presence blurs the boundary the
standalone extraction exists for. A lexical dependency pass over the core shows the movable
set is closed: the seven domain modules plus `runtime_gate` and
`interaction_capability_gate`, which consume landing readiness and context scenarios and
therefore cannot stay behind without inverting the dependency. No other core module
references this closure, and every external consumer of its 70 root-exported names already
lives in `rustok-page-builder` (its `src`, `admin`, and `tests` trees).

## Decision

Move the nine modules — `landing_contract`, `landing_property`, `landing_readiness`,
`context_scenario`, `runtime_scenario_release`, `runtime_scenario_render`,
`runtime_scenario_snapshot`, `runtime_gate`, `interaction_capability_gate` — out of
`crates/ui/fly/src` into `crates/modules/rustok-page-builder/src/domain/`, behind a
`domain` facade with explicit re-exports. `rustok-page-builder` re-exports the 70 public
names at its crate root, so existing `rustok_page_builder::…` paths (including the
`runtime_scenario_*` DTO shim modules, which keep their paths and now resolve the
implementation locally) are unchanged. Inside the moved code, references to domain items
stay `crate::…`; references to the engine become `fly::…`. The `fly` core root surface
shrinks from 376 to 306 names and keeps only the framework-neutral engine. This is a pure
relocation: no behavior, type shape, or serialization changes.

## Sources of truth and ownership

- Canonical owner of the landing/scenario domain: `rustok-page-builder` (the module that
  owns the builder), in `src/domain/`.
- Canonical owner of the engine (AST, codec, commands, rendering, context machinery):
  the `fly` core.
- Dependency direction: `rustok-page-builder` → `fly` (unchanged). Nothing in the Fly
  stack may depend on `rustok-page-builder`.
- The DTO shim modules (`runtime_scenario_release`, `runtime_scenario_render`,
  `runtime_scenario_snapshot` in `rustok-page-builder/src`) remain transport/host
  projections over the domain, now in the same crate as the implementation.

## Invariants

- `fly`, `fly-ui`, and `fly-web` never name landing contracts, readiness, or scenario
  release concepts.
- Every name that was reachable as `fly::<Name>` for the moved domain is reachable as
  `rustok_page_builder::<Name>`; no other public path changes.
- The moved code compiles against the same `fly` public surface consumers use; no
  `pub(crate)` core internals cross the new crate boundary.

### Allowed states

- Domain code calls the `fly` core through its public API (`fly::…`).
- Page Builder host code (admin, storefront, tests) imports domain names from
  `rustok_page_builder` (root or `domain::…`).

### Forbidden states

- Any `rustok-*` dependency or domain type inside the Fly stack crates.
- A re-export shim left in the `fly` core pointing at the moved code (zero legacy paths).
- Cyclic crate dependencies: the core must never need the Page Builder module.

## Non-goals

- Changing any domain behavior, DTO shape, or persistence format.
- Moving the generic context/runtime machinery (`context_*`, `runtime_pipeline`,
  `runtime_render`, `runtime_validation`, `locale_*`, `translation`): it is
  framework-neutral engine and stays in the core.
- The standalone extraction itself; this decision only removes the blocker named by
  P2-13 ahead of it.

## Data, transaction, and concurrency boundary

Not applicable: the moved code is pure, stateless document evaluation (readiness,
scenario render/snapshot/release) with no persisted state, transactions, or concurrency
of its own. Hosts keep owning document revision and publish CAS behavior as before.

## Context dimensions

Not applicable: no tenant, channel, locale, principal, or policy context changes hands;
the moved functions already receive every context explicitly from their callers.

## Events and projections

Not applicable: no events or projections are produced by the moved code; publish flow
composition in `rustok-pages` consumes the same `rustok_page_builder` shim paths as
before.

## Failure semantics

Unchanged: the moved evaluators keep their fail-closed behavior (invalid documents and
scenario regressions are rejected, never silently published). The relocation introduces
no new failure mode; a missing import after the move is a compile error, not a runtime
degradation.

## Migration and cutover

Single-pass cutover, zero legacy: the nine modules are moved with `git mv`, imports are
rewired (`crate::` → `fly::` for engine names), the `fly` core drops the module
declarations and re-exports, and all 23 consuming files in `rustok-page-builder` are
updated in the same change. Verify gates that pinned the old file locations are rebound
to the new owner in the same change (never weakened). No compatibility shims are left in
the core.

## Alternatives considered

- **New `fly-landing` crate next to the core.** Rejected: it would keep RusTok product
  domain inside the Fly stack's extraction boundary and add a crate whose only consumer
  is the Page Builder module.
- **Move only the seven domain modules and leave the gates behind.** Rejected:
  `runtime_gate` and `interaction_capability_gate` consume readiness/scenario items, so
  the core would depend on the Page Builder module — a dependency cycle.
- **Keep the domain in the core behind a feature flag.** Rejected: it preserves the
  blurred boundary and the inflated API that P2-13 records; the extraction needs the
  code physically out.

## Verification

- Lexical surface comparison: the `fly` core root surface after the move equals the
  previous surface minus exactly the 70 moved names (306 remain, identities unchanged);
  the `rustok-page-builder` root surface gains exactly those names.
- Source gates: `scripts/fly-check.sh gates` (20/20) and the Page Builder module verify
  scripts, with path pins rebound to `src/domain/`.
- Maintainer-run `cargo test` / `cargo clippy` over `fly` and `rustok-page-builder`
  (the authoring sandbox has no Rust toolchain; compile verification is delegated to the
  maintainer, as recorded in the backlog).

## Consequences

- The Fly core is one step from standalone extraction: only framework-neutral engine
  remains, so the `standalone-Cargo.toml` template no longer drags RusTok domain along.
- Page Builder's DTO shims and the domain implementation now live in one crate, closing
  the audit's "where does the logic live" complaint for the scenario stack.
- Follow-up: none for this item; audit P2-13 and backlog item 8 close with this change.
