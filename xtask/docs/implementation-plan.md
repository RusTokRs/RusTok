# Native module scaffolding implementation plan

Status: `in_progress`. Owner: platform tooling (`xtask`). Review date: 2026-11-03.
Base `main`: `0993260f923fbe63f829bdefdc377adcc1f7b1d2`.

## Scope and source of truth

This is the single delivery plan for `cargo xtask scaffold` and
`cargo xtask module new`. The accepted generation contract remains in the
[backend implementation guide](../../docs/backend/module-backend-implementation.md#scaffold-profiles-and-generation-contract).
Native source layout follows the
[accepted ADR](../../DECISIONS/2026-09-18-canonical-native-module-source-layout.md).
The plan sequences that contract; it does not define a competing module model.

`xtask/src/scaffold` owns native generation and registration. `modules.toml`
owns platform composition, each module's `rustok-module.toml` owns its published
identity/surfaces, and local owner plans own readiness. Documentation indexes
are projections. Server/distribution files compose native runtime entrypoints.

Alloy MCP drafts are a separate reviewed design-assistance flow owned by
[`rustok-mcp`](../../crates/modules/rustok-mcp/docs/implementation-plan.md).
Standalone Component Model guests remain owned by
[`rustok-module-template`](../../crates/utils/rustok-module-template/docs/implementation-plan.md).
Neither is a second implementation of native registration; this plan does not
silently change their apply, authorization or packaging contracts.

No repository-wide audit protocol or audit ledger is activated by this review.
No database, outbox, tenant policy, API or live client contract is added by the
current stateless identity generator.

## Current implementation and review findings

### Blog reference transfer assessment

The intended result is a reusable native-module engineering standard derived
from Blog. A minimal compiling identity is only its starting point. Generator
unit counts and generated-crate compilation do not establish transfer of Blog's
domain, persistence, localization or FBA guarantees.

The semantic source contract is the
[native reference ADR](../../DECISIONS/2026-09-18-canonical-native-module-reference-contract.md).
Blog itself remains a bounded source reference with pending execution evidence,
as stated by its [current cursor](../../crates/modules/rustok-blog/docs/implementation-plan-current.md).
Its comments-consumer registry is `boundary_ready`, not `transport_verified`.

| Reference dimension | Transferred today | Required before claiming the reusable standard |
| --- | --- | --- |
| Responsibility layout and ownership | Thin facade, runtime module, internal tests, optional pure core/model/transport/UI; unused slots omitted | Keep private persistence and owner boundaries checked as actual responsibilities are added |
| Semantic invariants | Documented platform rules; the semantic verifier is still Blog-specific | Apply tenant authority, explicit Patch, mandatory edit CAS, lifecycle, safe errors, deterministic pagination and derived-revision rules to each applicable owner operation |
| FFA | Source separation, explicit transport selection, executable unavailable-path tests | Real command/view-model tests, functional owner transports, mounted routes and Leptos/Next capability parity |
| UI localization | Shared Fluent declaration and generated catalog/variable-parity tests | Host effective locale, parameter-bearing message behavior, no hardcoded language branches and actual localized UI evidence |
| Content localization | No owner schema or fabricated translation tables | Owner-defined requested/effective/available locale provenance; fallback is read-only, base identity remains locale-neutral, tenant content languages are independent of bundled interface catalogs |
| FBA | Readiness is honestly `not_started`; no speculative port/runtime stack | A real consumer/provider contract, canonical context/errors, runtime composition, retry identity and explicit degraded-mode evidence |
| Test coverage | Generator fault tests, goldens, metadata/docs, catalog and unavailable-facade tests | Applicable domain/property, SQL/migration, tenant/security, concurrency, atomic-event, transport, runtime and browser/client tests with retained execution evidence |

Do not mechanically copy Blog's posts, categories, permission names, migration
history or Comments integrations. Extract the invariant and test contract into
the existing tooling/owner boundaries, then instantiate it for real module
requirements. Empty layers and passing placeholder tests remain forbidden.

The bounded reference recheck on 2026-11-03 found:

- Both Blog layout entrypoints run the same generic lint and strict reference
  profile. The generic lint recognizes the public root composed in `ui/mod.rs`.
  Its fixture suite passed 55 tests with two Windows symlink-privilege skips.
- The reference and FBA verifiers share the Comments projection wiring check.
  It verifies that registration injects the transactional outbox bus and that
  restart and replay create a bus against the correct connection. This replaced
  stale markers for the one-argument constructor. `verify:module-reference-contract`
  and the `verify:blog:fba` static chain passed.
- Some Blog PostgreSQL tests return without exercising PostgreSQL when
  `RUSTOK_BLOG_TEST_DATABASE_URL` is absent. Green ordinary test output must not
  be recorded as configured database/concurrency/restart execution evidence.
- Blog has nonreference examples that must not propagate: hardcoded Russian/
  English grid labels in `admin/src/core/presentation.rs`, hardcoded Next form
  labels, and Search page/filter/offset construction in
  `storefront/src/ui/leptos.rs` instead of the pure core/transport boundary.
- Fallback-text editor hydration retains the requested locale in the inspected
  UI source chain. A focused native/GraphQL/client regression is required to
  determine whether saving can relabel fallback text; this review did not
  reproduce a database/browser failure and does not treat the risk as proven.

This comparison records static checks and verifier fixtures. The Blog and
reference-tooling owners track execution of the Rust, database and browser
suites and the remaining semantic gaps.

The same source-layout lint was run against the existing `pages` module.
Publish metadata validation passed. Layout lint reported 41 diagnostics,
including follow-on errors from missing files. The main gaps were exposed
SeaORM entities consumed by `rustok-starter`, noncanonical backend source slots,
Leptos admin composition in the crate facade, legacy contract test wiring, and
missing `next_package` declarations for existing Next.js packages. The Pages
owner's locale, revision and restart harnesses provide a second use case for
the planned capability-based semantic checks. The Next.js owner suites passed
nine admin tests and eight storefront tests. Database and browser evidence
remain separate verification gates.

### Native generator implementation

| Boundary | Current implementation | Remaining gap |
| --- | --- | --- |
| Input | Native profile; normalized slug; unknown, repeated and conflicting flags rejected; metadata escaped; typed composition identity and Cargo key collisions rejected | Extend systematic boundary/size and identity collision cases to the remaining registration surfaces |
| Backend | Minimal `RusToKModule`/`MigrationSource`, contract tests and owner docs; no invented domain operations | Generated source and selected composition must compile in isolated verification |
| Persistence | `--with-db` rejected before writes; `--no-db` emits no schema | An actual module owner must first specify its data model, migration safety, transaction and event contracts |
| UI | Optional unmounted Leptos core/model/transport/render packages; public owner transport facade; both adapters explicitly unavailable with executable failure tests | Effective host locale, owner operations and live Leptos/Next.js parity are not implemented |
| Planning | Prepare all edits before writing, insert by parsed TOML key without rewriting existing bytes, check required registration, and show unified diffs with apply preconditions in dry-run | Markdown and Rust registry insertion still depends on current formatting |
| Apply | Workspace-wide lock, versioned journal, original backups before apply, and no-clobber source writes. Recovery verified on Windows and Linux. | External writers can still race |
| Recovery | New process resumes rollback or finishes committed cleanup; changed targets and foreign files are preserved; a retained CLI runs without Cargo | Ambiguous claims and conflicting edits require manual reconciliation |
| Governance | Crate rustdoc includes owner API; plans declare FFA/FBA and release/data readiness; central plan and readiness indexes updated | Indexes must remain tested as projections; a new identity has no verified release/rollback execution |
| Verification | Unit/negative tests, renderer and four file-tree goldens; four isolated generated modes and eight default-profile crate test suites pass; generated facade tests | Matrix is Bash/disposable-checkout dependent; no selected distribution feature execution; see exact profile evidence below |

The initial Windows baseline was 175 passing and three failing `xtask` tests.
All failures were LF renderer output compared with CRLF golden files.
Golden files now have an explicit LF Git attribute. This is a portability fix,
not evidence that generated business functionality exists.

This change also fixes suffix/comment module-key collisions, missing registration
anchors/files returning false success, omitted owner documentation/indexes,
stale-plan overwrites, unconditional recursive rollback deletion, and restoration
after an injected partial write. The shared docs validator also parses padded
table headers/alignment as presentation rather than duplicate module identities.
Source and test evidence do not establish full
production readiness.

The obsolete host-UI mount helpers and their three helper-only tests were removed:
they had no runtime caller and contradicted the accepted unmounted-draft contract.
Current tests exercise the actual planner, registration and rollback boundaries.

## Ordered work and acceptance gates

Complete each gate with retained commands/results before promoting its status.
Each increment must preserve one native generator and leave generated identities
explicitly incomplete until their owner functionality is implemented.

### 1. Safe filesystem mutation — P0, verified

Owner: platform tooling. Prerequisite: the current preflight/rollback fixes.

- [x] Capture the exact original registration bytes in the plan and reject stale targets.
- [x] Claim the module directory exclusively and create source files with create-new semantics.
- [x] Restore only our writes; preserve external edits and files, report incomplete rollback.
- [x] Keep injected partial writes private and verify that recovery cleans staging.
- [x] Define the [workspace lock and process-interruption contract](../../DECISIONS/2026-11-03-native-scaffold-recovery.md).
- [x] Implement workspace-wide exclusion, versioned recovery state, and standalone
  recovery. Verify contention across slugs, interrupted apply/validation/rollback/
  commit, conflict preservation, read-only targets, junction rejection, and retry
  after Windows denies replacement of an open manifest.
- [x] Run the process recovery suite on Linux and verify apply and recovery on a
  filesystem that runs out of space during the operation.

Exit: an interrupted or repeated operation either preserves/restores all original
bytes or leaves a durable actionable recovery state. No foreign file is deleted
and no incomplete attempt reports success. This gate blocks production approval.

### 2. Complete, reviewable registration — P1, in progress

Owner: platform tooling with server/distribution composition owners.

- [x] Resolve existing module identities through parsed TOML keys.
- [x] Require server/distribution inputs when default composition is requested.
- [x] Check planned workspace membership/dependency, module identity,
  server/distribution features and registry entry even with `--skip-validate`.
- [x] Generate required owner readiness blocks and both central readiness/plan projections.
- [x] Replace TOML anchors with parser-backed insertions. Support quoted keys,
  dotted and inline tables, reordered sections, mixed line endings, EOF tables
  and comments without rewriting existing bytes. Reject occupied feature and
  dependency keys, conflicting composition identities and excluded workspace
  paths. Verify exact planned host feature edges before apply.
- Windows TOML evidence. The `xtask` suite passed 212 tests with one subprocess
  entrypoint ignored by the top-level runner. The regression fixtures compare
  exact output bytes across all four edited TOML files and verify that rejected
  plans preserve every registration input. A disposable Cargo workspace verified
  literal exclusions, explicit member precedence and case-sensitive glob matching
  with `cargo metadata --no-deps`.
  Headless and dual CLI previews were repeatable, matched the original file
  hashes and passed `git apply --check`. Tests and CLI used cached locked
  dependencies because a concurrent Cargo run held the shared build lock.
- [ ] Replace fragile insertion anchors in the Markdown projections and the
  distribution Rust registry using their owning contracts.
- [x] Preview actual content changes and preconditions. Verify repeated preview
  stability and byte-for-byte workspace immutability.
- Windows evidence for preview. All 207 `xtask` tests passed. Repeated headless
  and dual CLI previews produced identical bytes and passed `git apply --check`.
  Each reported SHA-256 matched its current registration file. The preview
  left source files and scaffold control state untouched.
- [ ] Extend negative tests to every registration surface, name/path collision,
  malformed input, requested UI mode and `--skip-server-wire` behavior.

Exit: either every requested registration is present exactly once and matches
the owner identity, or planning fails without modifying repository files.
Neither skip flag may turn an incomplete plan into successful registration.

### 3. Reproducible generated-source verification — P1, in progress

Owner: platform tooling. Prerequisites: gates 1 and 2 for production sign-off;
read-only and disposable-fixture verification can proceed earlier.

- [ ] Close Blog reference-verifier drift and retain actual reference execution
  evidence before certifying it for propagation. Do not preserve obsolete source
  markers or copy confirmed FFA/localization defects into generated modules.
- [ ] Generalize the existing reference validation by module and implemented
  responsibility. For every invariant, record applicability or an explicit
  reason it is absent, its negative behavior test, runtime preconditions and
  evidence status. A new blueprint input/schema requires an owning contract
  decision before implementation; this plan does not invent a parallel registry.
- [ ] Prove transfer with one second real module/use case. Require zero Blog
  names/permissions/schema assumptions and the same applicable invariant gates.
  Generated code and test bodies must follow the actual owner operation.

- [ ] Provide one Windows/Linux verification entrypoint through `xtask`, using a
  disposable checkout and canonical dependency catalog. Keep the current matrix
  as the existing CI consumer until that replacement is authorized and verified.
- [ ] Automate headless, admin, storefront and dual identities on Windows/Linux; prove default
  lint/metadata checks pass, rejection writes nothing, and second generation
  refuses existing targets without altering them.
- [x] Run the four modes in an isolated Windows checkout, with their default
  lint/metadata checks, eight generated crate suites, and explicit rejected
  persistence/duplicate-generation immutability checks. An automated cross-OS
  retained matrix remains the preceding task's deliverable.
- [ ] Test every generated backend/UI crate and supported default/CSR/hydrate/SSR
  profile. Retain resolved dependency/lockfile evidence and docs.rs build evidence.
- [ ] Compile the selected `rustok-distribution` features with defaults disabled
  and assert each generated module is registered exactly once. Do not compile
  `rustok-server` merely to verify an isolated generated identity.
- [x] Generate executable facade failure tests for both explicit paths and the
  selected build path. Expose the owner facade as the UI package's deliberate
  public transport boundary, instead of leaving private unreachable adapters.
- [ ] Retain generated transport test execution for each supported build
  feature profile. Tests cover both explicit paths and each
  feature-selected path: unavailable errors remain typed, with no fallback or
  false success; enum-label assertions alone are insufficient.
- [x] Execute both dual UI packages under CSR, hydrate and SSR on the native
  Windows target: twelve tests per profile, including selected-path failures.
  Browser/WASM execution and the single-surface packages' full feature matrix
  remain separate evidence.

Exit: repeatable scoped compile, unit, static and composition results for all four
modes, with clean source checkout and explicit evidence of any missing dependency
or environment requirement. CI workflow edits require the repository's explicit
CI authorization; this review makes none.

The reusable reference test contract must cover the following dimensions when
their owning behavior exists:

| Test family | Required acceptance evidence |
| --- | --- |
| Pure domain and property tests | Bounds, complete transition graph, Patch semantics, calculations and ordering; pure UI command/view-model tests without a framework runtime |
| Persistence and migrations | Real owner schema on supported databases, tenant/composite constraints, migration safety and persisted-state decoding failures |
| Concurrency and side effects | Stale CAS, duplicate/reordered retries, principal-bound identity, rollback, atomic outbox and independent derived revisions |
| Security and locale provenance | Cross-tenant/principal denial, effective context, missing authority, missing locale, read fallback never silently becoming a localized write |
| Transport contracts | Same owner command through native/GraphQL and any declared REST/port; schema/nullability/pagination and safe typed public errors |
| Runtime and recovery | Selected registry/provider/worker actually invoked, restart/redelivery, missing provider and explicitly supported degraded modes |
| Client and browser parity | Real routes/actions in both hosts, canonical locale and navigation, no UI-owned domain policy; retained browser execution |
| Generator behavior | Complete immutable preview, registration, fault restoration, source goldens, supported feature profiles and evidence of rejected requests |

A module without a persistent owner state does not need invented migration tests;
one without a state machine does not need fake transition tests. Once a capability
is implemented, missing execution prerequisites must leave its evidence pending,
never silently promote it through empty success or source markers.

#### Pilot for localized read and write provenance

Blog and Pages exercise the same invariant against their owner services on
SQLite. Start with content stored only in the source locale. Read a missing
locale with fallback and assert the requested, effective, and available locales.
Try to write to the requested locale with its required localized data missing.
Require the owner's validation error, unchanged source content and revision,
and no new locale. Then supply the exact-locale data and verify that the new
locale can be read independently.

Blog applies this contract to a post update with missing title or body. Pages
applies it to saving a document before its exact-locale metadata exists. The
Pages test also checks that the rejected write creates no body revision.

```powershell
cargo test -p rustok-blog --locked --offline --test integration test_post_read_paths_normalize_requested_and_fallback_locale -- --exact
cargo test -p rustok-pages --locked --offline --test page_language_agnostic_contract document_locale_requires_matching_translation -- --exact
```

On 2026-11-03, Windows execution passed the exact Blog scenario and all nine
tests in the Pages `page_locale_fallback` and `page_language_agnostic_contract`
targets. The checks ran against isolated in-memory SQLite databases.

The pilot covers owner service and storage behavior. UI form hydration,
transport parity, and PostgreSQL are separate evidence for the same contract.

### 4. Owner implementation and optional publication — P2, per module

Owner: the generated module owner; tooling owns reusable generation defects only.
Prerequisites: owner service/context/error contracts before transports or live UI.

- [ ] Select a real module capability and specify preconditions, transitions,
  concurrency/idempotency, tenant/auth/channel/locale context and failure behavior.
- [ ] Add only required platform capability bindings. If persistence is needed,
  specify the actual schema, tenant constraints, transaction/outbox/cache effects,
  migration safety and release/recovery tests before generating persistence code.
- [ ] Implement native/GraphQL owner adapters and verify equivalent authorization,
  nullability, pagination, errors and mutations through the real entrypoints.
- [ ] Before publishing UI, bind the canonical effective locale and implement
  functional Leptos and Next.js routes, navigation and interaction parity.
  Draft locale files and `t(None, ...)` are not live locale integration.
- [ ] Promote manifest surfaces and FBA/FFA/release board statuses only from
  retained owner evidence; remove `MissingOwnerContract` from any live adapter.

Exit: the owner capability is observably functional and its applicable runtime,
data/release and client parity gates pass. A generic stateless scaffold can be
production-quality tooling without claiming to implement arbitrary businesses.

## Verification and evidence

Run scoped generator checks from the repository root:

```powershell
cargo test -p xtask --offline
cargo check -p xtask --offline
rustfmt --edition 2021 --check xtask/src/scaffold/mod.rs xtask/src/scaffold/registration.rs xtask/src/scaffold/backend.rs xtask/src/scaffold/ui.rs xtask/src/docs_registry_contracts.rs xtask/src/tests/commands.rs
python scripts/verify/verify-remediation-gate.py --crate xtask --files xtask/src/scaffold/mod.rs xtask/src/scaffold/registration.rs xtask/src/scaffold/backend.rs xtask/src/scaffold/ui.rs xtask/src/docs_registry_contracts.rs xtask/src/tests/commands.rs
git diff --check
```

`--offline` requires cached dependencies. Use the normal command when dependency
download is required; do not weaken TLS verification to obtain a green result.

On Linux, run the storage-failure test on a dedicated 4 MiB `tmpfs`. The test
fills that mount and refuses to run on an ordinary workspace directory.

```bash
(
  set -e
  storage=$(mktemp -d)
  sudo mount -t tmpfs -o "size=4m,uid=$(id -u),gid=$(id -g),mode=0700" tmpfs "$storage"
  trap 'sudo umount "$storage" && rmdir "$storage"' EXIT
  RUSTOK_SCAFFOLD_TEST_STORAGE="$storage" cargo test -p xtask --locked -- \
    --ignored --exact scaffold::transaction::tests::full_filesystem_preserves_scaffold_recovery
)
```

Run the existing `scripts/ci/verify-scaffold-matrix.sh` only in a disposable
checkout: it changes manifests/docs and creates fixture crates. It does not
provide local isolation or cleanup itself. Full workspace suites and long E2E
remain CI/maintainer responsibilities.

Local evidence on 2026-11-03 (Windows, Rust 1.97.1):

| Check | Result |
| --- | --- |
| `cargo test -p xtask --offline --quiet` | 187 passed, zero failed after the final renderer changes |
| `cargo check -p xtask --offline` | Passed; final source also passes the gatekeeper's all-target Clippy check |
| Scoped `rustfmt --check`, `git diff --check`, plan-link resolution | Passed |
| `verify-remediation-gate.py --crate xtask --files <changed paths>` | Passed, including architecture dependency guard and Clippy |
| Isolated `scaffold_checked_none/admin/storefront/dual` generation | All four passed default architecture lint and publish-metadata validation |
| `cargo test --offline --lib` selecting all eight `rustok-scaffold-checked-*` backend/UI packages | 32 passed: two tests per backend and six per requested UI package, including catalog and actual facade failure behavior |
| `cargo test --offline --lib -p rustok-scaffold-checked-dual-admin -p rustok-scaffold-checked-dual-storefront --features <csr/hydrate/ssr>` | All three profiles passed, twelve tests per profile on the native target |
| Unsupported `--with-db` and duplicate generation | Both rejected; manifest/document diff and existing source hashes unchanged |

The initial isolated validation failure also proved the default rollback path:
the padded table header was rejected, and generation restored its source and
registration files before the parser correction and successful rerun.

The native linker emitted informational messages and dependencies emitted an
existing `proc-macro-error2` future-incompatibility warning. No generated private
transport dead-code warnings remain after exposing and testing the owner facade.
This does not certify the dependency tree for future Rust releases.

Windows recovery increment on 2026-11-03:

| Check | Result |
| --- | --- |
| `cargo test -p xtask --offline --quiet` | 202 passed; the subprocess entrypoint is ignored by the top-level test runner and invoked by the process tests |
| Separate-process recovery tests | Passed for contention, interruption across the write/validation/recovery/commit boundaries, conflicts, invalid state, junctions and a Windows sharing violation |
| Retained production CLI with no Cargo workspace | Recovered an interrupted write; a second invocation left the same restored state |
| Live `--ui dual` in a disposable worktree | Preview created no control state; apply passed lint and metadata validation, retired its journal, and survived an invocation of the retained CLI |
| Staged remediation, ADR governance and Blog source-layout checks | Passed; the source-layout fixture runner skipped two symlink cases requiring Windows privileges |
| `npm run verify:docs` | Failed on the unchanged topology table and 20 existing missing targets in the continuous audit ledger |

Linux recovery was verified on 2026-11-03 using x86-64 Linux 6.18 and Rust
1.96.0 in a disposable Docker checkout with networking disabled.

| Check | Result |
| --- | --- |
| `cargo test -p xtask --locked --offline --quiet` | 201 passed and zero failed. The top-level runner ignored the subprocess entrypoint and the dedicated storage test. |
| Dedicated 4 MiB `tmpfs` storage test above, with `--offline` | Passed. The kernel returned `ENOSPC` during staging and restoration. Staging left the original manifest intact. Failed restoration retained the journal and exact backup. After freeing space, a fresh process restored the original bytes and cleaned the interrupted transaction. |
| Windows regression run, Rust 1.97.1 | `cargo test -p xtask --offline --quiet` passed with 202 tests and the subprocess entrypoint ignored by the top-level runner |

Selected distribution registry execution, docs.rs builds, actual browser/WASM
mounts, power-loss recovery, noncooperating-writer isolation, live business/client
parity and production deployment remain unverified. The full Bash CI script,
workspace suites and long E2E were not executed in this review. Temporary
generated fixtures are kept outside the source checkout and removed after use.
`origin/main` was refreshed again before local integration and remains at the
recorded base SHA. The user requested handoff to local `main` on 2026-11-03;
subsequent scaffolding work continues there.
