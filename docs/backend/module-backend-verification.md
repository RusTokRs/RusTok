# Backend Module Verification Guide

This guide lists verification paths for backend module work. Prefer the smallest check that
covers the changed boundary, but do not skip guardrails that protect the architecture.

## Fast Static Guardrails

Run these for Axum/FBA/API surface work:

```bash
node scripts/verify/verify-api-surface-contract.mjs
node scripts/verify/verify-axum-runtime.mjs
git diff --check
```

Use `verify-axum-runtime.mjs` to confirm that the server and standalone CLI
entrypoints retain their explicit target boundaries. If a file is intentionally
removed or split, update the marker list instead of preserving stale checks.

## Canonical source-layout verification

For every new native module or major native source refactor, run the
per-module structural lint:

```bash
cargo xtask module lint <slug>
node scripts/verify/verify-module-source-layout.mjs --module <slug>
```

The direct Node invocation is the same runner for local debugging. The default
Blog reference gate runs the shared lint and Blog-specific layout checks.

```bash
npm run verify:module-source-layout
```

Both Blog entrypoints include the strict Blog profile and its retired-path
evidence. The shared lint checks canonical file responsibility and module wiring,
thin crate facades, optional `runtime.rs` / `error.rs` wiring, private
entities, source size, transport-neutral DTO/port contracts, persistence and
service dependency direction, framework-independent FFA model/core, transport
facade boundaries, support-sub-crate identity, declaration-to-package
alignment, symlink safety, and suspicious scaffold placeholders. The crate facade
can export a public Leptos component composed in `ui/mod.rs` or an adapter
component wired through `ui::leptos`. Test-only dependency exemptions apply only
to test directories or Rust modules proven
to be declared under `#[cfg(test)]`; a filename suffix alone is not enough.
It also checks structural Next.js package identity, host-module registration,
and route-page imports for live UI surfaces. It cannot prove functional
Next.js/Leptos parity for data, authorization, locale, or transport behavior,
live FBA transport, or publish registration readiness.

Existing unconverted native modules may fail the canonical lint; findings are
migration tasks, not opt-in exceptions. The canonical `npm run verify:module-source-layout` command also runs the
shared validator fixtures with `node --test scripts/verify/native-module-layout.test.mjs`.
This keeps generic native lint behavior under the same CI gate as the Blog reference profile.

## Post-scaffold verification

The [native scaffold owner plan](../../xtask/docs/implementation-plan.md)
lists the ordered production gates and retained local evidence. Required planned
registration is checked even with `--skip-validate`. Scoped `xtask` tests cover
partial writes, process contention, interruption and restart, conflicting edits,
and redirected paths. The current Bash matrix must run in a disposable checkout;
it mutates its checkout and does not verify distribution registry execution.

A non-dry-run `cargo xtask scaffold` registers the module, then runs static
architecture lint before publish-metadata validation. It prints
`SCAFFOLDED` only after both checks pass and the transaction commits. If architecture lint or publish
validation fails, the command returns a non-zero error and does not claim the
module is verified.

`--skip-validate` is an explicit development escape hatch: registration may
still have been written, but the command reports `REGISTERED BUT UNVERIFIED`
and prints the two checks the owner must run. `--dry-run` shows unified diffs
for all planned source and registration edits and lists their apply
preconditions. It leaves workspace files untouched and runs the same plan
preflight before skipping post-apply validation.

The generator retains originals before apply and restores them after validation
failure or uncommitted process interruption when recovery finds no conflicts.
Run `cargo xtask scaffold --recover` or use the
[retained recovery CLI](../../xtask/README.md#recovering-an-interrupted-scaffold).
The generated-crate compilation/test matrix remains a separate gate.


## Native scaffold golden and generated-crate matrix

The scaffold renderer has golden snapshots for the complete planned file tree
in headless, admin-only, storefront-only and dual-surface modes, plus focused
content snapshots for the native module manifest and the generated Leptos UI
package contracts. Run the unit tests with:

```bash
cargo test -p xtask
```

The dedicated CI workflow performs actual generation in a disposable checkout.
It requires each generated module to pass architecture lint and publish
metadata validation, then compiles and runs tests for all generated backend
and UI crates. It also checks the admin and storefront adapters with CSR,
hydrate and SSR feature selections:

```bash
bash scripts/ci/verify-scaffold-matrix.sh
```

The matrix covers generated structure and compilation only. It does not
establish domain behavior, production runtime integration, or functional
Next.js/Leptos parity. Database generation remains intentionally unsupported
until a real owner schema and migration contract exist.

## Canonical reference-contract verification

For a module that is used as a migration/reference template, run both guards:

```bash
npm run verify:module-source-layout
npm run verify:module-reference-contract
```

The first guard checks physical ownership and file/layer boundaries. The second
checks semantic reference invariants such as truthful locale writes, explicit
nullable patches, predecessor CAS, tenant authority, lifecycle enforcement,
derived-revision isolation, safe public errors, private persistence entities,
owner-service integration seams, and FBA write command identity.

A clean layout with a failing reference-contract guard is not a canonical module.

### Applicable reference evidence

Reference readiness is assessed by owner behavior and executed scenarios, not
by test counts or the presence of test directories. The module owner records
each applicable family below in its existing implementation plan, with scenario
names, exact commands, revision, profile/database, prerequisites and results.
An inapplicable family requires an owner-contract reason. Missing infrastructure
leaves applicable evidence pending; it does not make the family inapplicable.

| Test family | Required behavior when applicable |
| --- | --- |
| Domain unit and property | Bounds, invalid inputs, complete lifecycle graph, explicit Patch semantics, calculations, deterministic ordering and serialization round trips |
| Persistence and migrations | Real owner migrations, fresh install and upgrade, schema/entity parity, tenant/composite constraints, persisted-value decoding, supported rollback or explicit safe refusal |
| Database portability | The same owner invariant scenarios on PostgreSQL and SQLite within their supported deployment profiles; dialect-specific constraints preserve the same business meaning |
| Concurrency and retry | Stale CAS, simultaneous writes, duplicate/reordered delivery, principal-bound idempotency and recovery after partial failure |
| Security and context | Tenant/principal/channel isolation, permission denial, anonymous public-read authority, missing/invalid context and safe public errors |
| Events and derived state | Durable mutation/outbox commit and rollback, worker redelivery/restart, cache/search invalidation, independent projection revisions and stale-event handling |
| Transport contracts | Real native/GraphQL and any declared REST/port entrypoints share commands, authorization, context, nullability, pagination and failure semantics |
| FBA composition | Selected consumer/provider/worker is actually invoked, missing provider fails explicitly, timeouts/cancellation/retry identity and any documented degraded mode behave as contracted |
| FFA core and bindings | Pure command/view-model/validation tests without a UI runtime; component loading, empty/error, retry, conflict and success behavior through the owner transport facade |
| UI and content localization | Host effective locale, Fluent key/variable/plural behavior and formatting; requested/effective/available content locales, explicit localized writes and read-only fallback |
| Browser and client parity | Critical owner workflows in both declared Leptos and Next.js admin/storefront surfaces, canonical routes/navigation, locale switching, permission and conflict/error handling |
| Accessibility and design system | Keyboard/focus/labels and error announcements for interactive workflows; token/primitive parity and relevant UI Workbench examples when shared primitives change |
| Performance and robustness | Representative bounded-query/pagination and N+1 checks; measured budgets for performance-sensitive operations; fuzzing or fault injection for applicable parsers, untrusted inputs and recovery boundaries |
| Generator and publication | Deterministic preview/goldens, registration, supported feature profiles, rejected requests, interruption/recovery and truthful readiness metadata |

Reuse the [canonical backend foundations](./module-backend-architecture.md#foundation-crate-responsibilities),
[FFA packages](../UI/module-package-architecture.md),
[UI verification](../UI/module-package-verification.md) and
[i18n contract](../architecture/i18n.md). Add library dependencies only when an
implemented owner operation consumes them. A stateless identity does not need
fabricated database, event, port or browser tests. Property/fuzz/performance
techniques are selected for concrete invariants and risks rather than added as
empty universal suites.

### Database-portability evidence

Database independence means transport/framework-independent domain rules and
equivalent owner invariants on supported database profiles. The current
[deployment contract](../architecture/database.md#installer-storage) requires
PostgreSQL for production; SQLite is local/demo/test. It does not imply support
for arbitrary engines or SQLite production deployments.

For a persistent reference capability:

- Run shared owner-service scenarios against both databases, creating schema
  through real owner migrations and declared prerequisites. A manually recreated
  table or a harness that skips prerequisite migrations is narrower evidence.
- Check tenant uniqueness/foreign keys, null/Patch/CAS behavior, cursor ordering,
  UUID/time/JSON/Unicode boundaries and durable mutation/outbox atomicity.
- Verify PostgreSQL locking/isolation and any declared RLS separately. SQLite
  cannot establish PostgreSQL row-lock behavior.
- Exercise fresh install, upgrade with existing data, supported rollback/refusal
  and restart. Preserve the platform rule against narrowing widened locale
  columns during rollback.
- A required database execution gate must fail when its prerequisite is absent.
  Optional local nonexecution must be reported as skipped/unverified. An early
  successful return without connecting is not database execution evidence.

Reuse SeaORM, `MigrationSource` and existing `rustok-test-utils` facilities.
Dialect SQL belongs in owner persistence/migration adapters; do not introduce
repository interfaces or a database-provider registry without a real consumer.

## Database Schema and Migration Parity Verification

When creating, altering, or consolidating module migrations, use the existing
source scanner as a bounded diagnostic alongside scoped Rust and PostgreSQL
verification.

```bash
# Verify schema parity for a specific module:
npm run verify:schema-parity -- --module <module-slug>

# Verify across all modules:
npm run verify:schema-parity:all

# Run the test suite for the parity engine:
npm run test:verify:schema-parity
```

The scanner recognizes only a subset of table/column/nullability declarations
and registry/reference patterns. It misses nested layouts and several schema
properties. A successful scan does not certify complete parity or data safety.
Published stable migrations remain immutable. Consolidating unpublished steps
requires real schema/data comparison and preservation of dependency, activation
and recovery boundaries. The proposed admission mechanism is unimplemented.
See the [canonical guide](../guides/safe-database-migrations-and-squashing.md)
for current defects, the required protocol and implementation gates.

## Module Validation

For module manifest or ownership changes:

```bash
cargo xtask module validate <slug>
```

For FFA/FBA status changes, update both:

- module-local `docs/implementation-plan.md`;
- central readiness board in `docs/modules/registry.md`.

For backend layout changes, also inspect the physical placement:

```bash
rg -n "clap|std::process::exit|println!|eprintln!" crates/modules/rustok-MODULE/src -g "*.rs"
rg -n "apps::server|ServerRuntimeContext|HostRuntimeContext" crates/modules/rustok-MODULE -g "*.rs"
```

Expected result:

- domain/application code is in `src/`;
- evidence artifacts are in `contracts/`;
- local FFA/FBA status is in `docs/implementation-plan.md`;
- CLI command adapters, when present, are in module-local `cli/`;
- `apps/server` only mounts and composes owner-owned entrypoints.

## Targeted Rust Checks

Use targeted checks when code changed:

```bash
cargo check -p <module-crate>
cargo test -p <module-crate> --lib
cargo check -p rustok-server --no-default-features
```

For heavy server profiles, prefer `--no-default-features` first. Full server test profiles may
compile unrelated UI/admin crates; run them only when the changed boundary requires that
evidence.

## Transport Contract Checks

For GraphQL/REST/`#[server]` changes:

```bash
node scripts/verify/export-reference-artifacts.mjs artifacts/reference
node scripts/verify/verify-reference-artifacts.mjs artifacts/reference
```

When only a static boundary changed, document why reference artifacts were not regenerated.

## FBA Evidence

FBA slices need evidence appropriate to the status:

- source-level descriptors and registry JSON for planned/in-progress;
- static matrix and fallback smoke for boundary-ready candidates;
- compiled runtime smoke before transport-verified or parity-verified promotion.

Use module-local verifier scripts when they exist, for example:

```bash
npm run verify:workflow:fba
npm run verify:foundation:fba-runtime-smoke
```

Do not promote FBA readiness only because a type exists. Readiness requires consumer/provider
metadata, error mapping, fallback policy and verification evidence.

## Documentation Checks

Before completing a backend documentation or contract change:

```bash
rg -n "rustok-web|rustok-runtime|rustok-fba|rustok-cli-core|Axum" docs apps crates -g "*.md"
```

Confirm that active docs point to the target architecture and describe only
current contracts.
