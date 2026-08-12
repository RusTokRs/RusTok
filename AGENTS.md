# AGENTS

This file defines rules and ownership for all contributors and AI agents working in the RusToK repository.

## How to engage

- Always start by reading [`docs/index.md`](docs/index.md) — the canonical documentation map.
- Before creating or renaming modules, crates, packages, folders, files, public types, query keys, config keys, or documentation, follow the [`Naming Contract`](docs/standards/coding.md#naming-contract).
- For new modules or major module refactors, use [`docs/modules/module-authoring.md`](docs/modules/module-authoring.md) as the primary entry guide before diving into local component docs.
- Review domain module documentation before making changes.
- Use module owners (or the platform team) for approvals when cross-cutting concerns are involved.
- For architecture changes, capture decisions in `DECISIONS/` using an ADR.

## Ownership map

- **Platform foundation**: `crates/rustok-core`, `apps/server`, shared infra.
- **Domain modules**: `crates/rustok-*` (content, commerce, pages, blog, forum, index, etc.).
- **Frontends**: `apps/admin`, `apps/storefront`, `apps/next-admin`, `apps/next-frontend`.
- **MCP server**: `crates/rustok-mcp`.
- **Operational tooling**: `scripts/`, `docker-compose*.yml`, `grafana/`, `prometheus/`.

Detailed module ownership and responsibilities are captured in [`docs/modules/registry.md`](docs/modules/registry.md).

## Documentation policy

### Language

- All documentation is written in **English**.
- `README.ru.md` is the only file allowed in **Russian** (localized translation of the main README).
- Mixed language within a single document is not allowed — choose one language per file.

### Placement

- Platform-wide documentation lives in `docs/`.
- Per-module/per-app documentation lives inside the component: `apps/<name>/docs/` or `crates/<name>/docs/`.
- Every app and crate must have a root `README.md` with: purpose, responsibilities, interactions, entry points, and a link to `docs/`.
- `docs/modules/_index.md` links to all per-module documentation folders.

### Keeping docs up to date

When changing **architecture, API, events, modules, tenancy, routing, UI contracts, or observability**:

1. Update the relevant local docs in the changed component (`apps/*` or `crates/*`).
2. Update the related central docs in `docs/`.
3. Update [`docs/index.md`](docs/index.md) so the map remains accurate.
4. If a module or application was added or renamed, update [`docs/modules/registry.md`](docs/modules/registry.md).
5. Delete superseded implementation documentation once the replacement is canonical. Archive only externally relevant historical decisions or evidence, with an explicit reason and a link to the current contract.

Do not create a new document if a suitable one already exists — extend the existing one.

## Ideal platform implementation standard

Treat every change as work on an ideal, long-lived, production-grade platform.
Deliver the ideal coherent target design, not merely the smallest patch that
makes the immediate symptom or test pass.

- Prefer clear, maintainable, secure, and observable designs that fit the
  platform architecture and established contracts.
- Investigate and address the root cause. Do not introduce shortcuts, hidden
  coupling, special-case branches, fragile workarounds, or technical debt in
  place of a proper implementation.
- Choose the scope required for a complete solution. A small diff is valuable
  only when it fully preserves the intended design; it must not leave known
  inconsistencies, incomplete integration, or avoidable follow-up work.
- Keep code, tests, schemas, callers, configuration, and documentation
  consistent with the resulting contract. Verify observable behavior at the
  appropriate boundary before considering the work complete.

## Initial implementation and zero-legacy policy

RusToK is a pre-release initial implementation. Internal repository code has no
legacy consumers that justify compatibility layers. Implement the canonical
target architecture directly.

- A replacement is atomic: update every repository-owned caller, transport,
  schema, fixture, test, seed, script, and current document, then delete the
  superseded implementation in the same change.
- Do not add or retain compatibility wrappers, deprecated aliases, dual
  read/write paths, fallback-to-legacy behavior, old/new format adapters, or
  parallel implementations of the same internal contract.
- Do not create `legacy`, `old`, `new`, `v2`, or version-suffixed internal names
  to avoid completing a rename or cutover. The surviving implementation uses
  the canonical unversioned name.
- Existing legacy found within the task scope must be removed, not extended,
  documented as current, or hidden behind another facade.
- For unreleased schema work, amend or consolidate pending migrations instead
  of stacking corrective migrations solely to preserve repository history.
  An explicitly preserved external migration history is the only exception.
- Before declaring completion, search the repository for the removed names and
  verify that any remaining occurrences are explicitly preserved historical
  evidence, not executable code or current guidance.

An agent may not invent an exception. A temporary bridge is allowed only when
the user explicitly requires staged compatibility for an external consumer. It
must have a named removal owner, deadline, and deletion task in the owning
module plan.

### No internal API or code version families

Do not create repository-internal `/v1` or `/v2` routes, `FooV2` types,
`new_*` modules, versioned GraphQL fields, versioned storage envelopes, or
parallel package exports for repository-owned callers. Change the canonical
contract in place, migrate its data, update every caller atomically, and keep
one unversioned implementation.

Unreleased tokens, cursors, envelopes, and other repository-owned serialized
formats are explicitly covered by this rule. When their shape changes, replace
the canonical encoder and decoder in place and update all repository-owned
callers, tests, fixtures, scripts, and current documentation in the same change.
Delete prior-format readers, writers, version bytes/tags, compatibility fixtures,
and fallback decoders. Do not introduce `V1`/`V2` claim structs or retain an old
format reader "temporarily" unless the user explicitly approves an external
compatibility bridge under the exception above.

**Why:** an internal version does not remove migration work; it postpones it
while multiplying code paths, security fixes, test matrices, documentation,
and operational states. Two "temporary" versions quickly become two plausible
sources of truth, so later contributors and agents copy the obsolete one and
create further variants. Before the first external release, this cost buys no
compatibility value.

Version identifiers are allowed only where independent external deployment
actually requires them: published package releases, immutable migration order,
or a wire/event/public API consumed outside this repository. Keep that version
at the boundary, map it immediately to the single canonical internal model, and
do not fork domain logic by version. The explicit external-bridge approval and
removal requirements above still apply.

### No stubs or lint suppression as implementation

A warning, unused symbol, failing test, or missing caller is evidence to
investigate, not something to silence so a task appears complete.

- Do not add `#[allow(dead_code)]`, `#![allow(dead_code)]`, broad
  `allow(unused_*)`, or equivalent lint suppression in repository-owned code.
- Do not use `todo!()`, `unimplemented!()`, placeholder return values, no-op
  handlers, unconditional success, fake adapters, dummy data, or an
  in-memory fallback as a substitute for required functionality.
- When an unused implementation represents required target behavior, connect
  it through the real owner, transport, composition, and UI path and add tests
  of observable behavior. A source-marker or compile-only test does not prove
  that the runtime functionality exists.
- When it is not part of the target architecture, delete the symbol and its
  stale tests, fixtures, configuration, and documentation instead of preserving
  it for hypothetical future use.
- When completing the functionality would require authority or scope not
  granted by the user, report the concrete gap and leave the task incomplete;
  do not hide it behind a stub or suppression.
- Any existing suppression encountered in files touched by the task must be
  audited and removed by wiring or deletion. Do not copy it to new code.
- Tests, evidence, and verifiers must never require a suppression, stub,
  placeholder implementation, or legacy path as a positive marker. They must
  verify real behavior or record the capability as incomplete and fail closed.
  Compile-only and source-marker checks may supplement runtime tests, but they
  cannot substitute for missing functionality or justify completion status.

The only narrow exceptions are generated code and externally invoked symbols
that static analysis genuinely cannot observe, such as a required FFI export.
Use the smallest possible `#[expect(..., reason = "...")]`, never a module- or
crate-wide `allow`, and retain a test or verifier proving the external entry
path. Feature- or target-specific code must use correct `#[cfg(...)]` structure,
not lint suppression.

## AI Agent rules

Rules mandatory for all automated agents operating in this repository:

1. Always start by reading [`docs/index.md`](docs/index.md).
2. **Before modifying any frontend code, read the `AI_AGENT_RULES.md` file in that frontend's root directory** (`apps/admin/AI_AGENT_RULES.md`, `apps/storefront/AI_AGENT_RULES.md`, `apps/next-admin/AI_AGENT_RULES.md`, `apps/next-frontend/AI_AGENT_RULES.md`). These files contain critical rules about internal libraries, FFA structure, i18n, and forbidden patterns.
3. For new modules or major module refactors, read [`docs/modules/module-authoring.md`](docs/modules/module-authoring.md) before changing code.
4. Do not create a new document when an existing one is suitable — extend it instead.
5. Documentation must reflect the actual state of the code.
6. Never bypass or disable pre-commit/pre-push hooks. Fix the root cause of failures.
7. Do not edit CI/CD workflow files unless explicitly requested.
8. Do not modify other branches — only work on the assigned task branch.
9. For Leptos apps and module-owned Leptos UI packages, use native `#[server]` functions as the default internal data layer and keep GraphQL in parallel for public/headless-capable surfaces. Documented native-only operator/bootstrap exceptions are allowed only when no GraphQL/REST contract exists yet and the module plan records the parity or exemption decision.
10. Do not invent package-local i18n contracts. Server locale selection is canonical; module-owned UI packages must consume the host-provided effective locale (`UiRouteContext.locale` for Leptos, host/runtime locale providers for Next) instead of introducing their own query/header/cookie fallback chains.
11. For modules with UI and/or transport boundary changes, keep FFA/FBA documentation in sync: update the module-local `docs/implementation-plan.md` FFA/FBA status block and the central registry entry in `docs/modules/registry.md` within the same change.
12. If a module's UI is planned but not implemented yet, keep a `not_started` FFA/FBA status block in the module plan and a matching `not_started` row in the central readiness board; when UI first appears, update both local and central statuses in the same PR with initial verification evidence.
13. Module-owned UI packages may expose only their owning module or capability surface. Do not place MCP, Alloy, commerce, catalog, AI, or other module/operator screens inside an unrelated module UI package. Cross-module workflows must be composed by the host from separate owner-owned entrypoints, not merged into another module. If a UI needs another module's data, consume that module's public transport contract only and document the dependency in the owner module plan.
14. When adding UI for a module or capability, keep the required surfaces in parity: Next/admin where applicable and the Leptos FFA version with native `#[server]` functions plus the target parallel GraphQL/REST contract. Do not ship a Next-only operator surface when the module's admin UI contract requires Leptos FFA parity; document any native-only operator/bootstrap exception in the module plan.
15. Follow the repository-wide [initial implementation and zero-legacy policy](#initial-implementation-and-zero-legacy-policy). Required current platform contracts, such as parallel GraphQL support, are intentional target surfaces and are not legacy compatibility paths.
16. All repository artifacts, including code, documentation, commit messages, comments, examples, and generated files, must be written in **English only**. The sole exception is `README.ru.md` (localized Russian translation of the main README). Direct conversation with the user should follow the user's preferred language.
17. **DO NOT duplicate code across modules.** If a pattern appears in 2+ modules or 2+ hosts, extract it into a shared library:
    - UI primitives → `crates/leptos-ui/`
    - Framework-agnostic UI route/query/input/busy contracts -> `crates/rustok-ui-core/`
    - Routing/query helpers → `crates/leptos-ui-routing/`
    - Framework-agnostic UI i18n → `crates/rustok-ui-i18n/`
    - Framework-agnostic GraphQL client → `crates/rustok-graphql/`
    - Leptos GraphQL hooks adapter → `crates/rustok-graphql-leptos/`
    - Framework-agnostic UI transport path/error/result evidence -> `crates/rustok-ui-transport/`
    - Framework-agnostic contracts → `crates/rustok-api/`
    - Domain-specific cross-module UI → `crates/rustok-<capability>-<surface>-support/`
    - Before writing reusable code, check existing libraries in `crates/leptos-*` and `crates/rustok-*/`. See [Module UI Package Implementation Guide](docs/UI/module-package-implementation.md#when-to-extract-shared-libraries) for extraction decision matrix.
18. When diagnosing a failed GitHub Actions run, first execute `powershell -ExecutionPolicy Bypass -File scripts/ci/download-failed-logs.ps1` and inspect the refreshed local `errors/` directory. Do not rely on stale logs from an earlier run.
