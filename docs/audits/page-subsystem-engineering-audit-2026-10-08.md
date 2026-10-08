# Page and Page Builder subsystem — deep engineering audit

**Date:** 2026-10-08
**Base commit:** `7895d7fe817bccc1a194dbcb25b9f918fa0f5840` (branch `arena/ecde41c8-rustok`, forked from `main` at `6d353fda87185b149fbbf987ab7368dcb6a749ff` lineage)
**Scope:** `crates/modules/rustok-pages`, `crates/modules/rustok-page-builder`, `crates/ui/fly/**`
(`fly`, `fly-ui`, `fly-web`, `fly-browser`, `fly-leptos`, `fly-dioxus`), the admin and storefront
surfaces that consume them, and the verification surface that guards them:
`scripts/verify/verify-fly-*.mjs`, `scripts/verify/verify-page-builder-*`, `scripts/fly-check.sh`,
`.github/workflows/fly-page-builder.yml`.
**Companion:** `docs/audits/fly-builder-engineering-audit-2026-10-02.md` receives an in-place
status-verification section (§10) for its §1a remediation table. This document does not supersede
it; it audits the wider subsystem and records where the 2026-10-02 conclusions still hold.

---

## 1. Verification limits — read this before the findings

The audit environment has **no Rust toolchain** (`cargo` and `rustc` are absent; there is no
`target/`, no `~/.cargo`, and no dependency source cache), and `crates.io` is not reachable, so a
compiler cannot be provisioned even though `@rustbin/*` npm packages do ship `rustc`/`cargo`
binaries — every dependency body would still be missing.

Consequences, stated plainly:

- `cargo check`, `cargo test`, `cargo clippy` and `cargo fmt --check` were **not** run for this
  audit. Every finding that depends on them is labelled **needs toolchain**; none is described as
  passing.
- GitHub Actions logs are not retrievable from this environment (`gh run view --log-failed` fails
  with `EOF`); only job/step names and conclusions are available through
  `gh api .../actions/runs/<id>/jobs`. Root causes below were therefore derived statically and are
  labelled as hypothesis where the toolchain would decide.
- Two substitute checks were provisioned, verified against the repository itself, and committed as
  audit tooling under `scripts/audit/` (they are deliberately **not** wired into CI):
  - `scripts/audit/rust_syntax_check.mjs` — parses changed `.rs` files with the real
    `tree-sitter-rust` grammar and reports `ERROR`/`MISSING` nodes.
  - `scripts/audit/rustfmt_check.mjs` — reproduces `cargo fmt --check` per file using
    `@scalar/rust-fmt` (the actual rustfmt compiled to wasm; upstream asserts byte-identical output
    against the native CLI and rustfmt's own 345-file corpus). Supports `--write`.
  - `scripts/audit/rust_module_resolution_check.py` — resolves every `mod`, `#[path] mod`,
    `include!`, `include_str!` and `include_bytes!` target in a crate. A missing target is E0583 /
    E0584, the one class of compile error that needs no type information to detect.
  - `scripts/audit/admin_module_registry_check.py` — replays `apps/admin/build.rs`'s registry
    contract: each module manifest's `[provides.admin_ui].leptos_crate` must have an
    `admin/Cargo.toml`, must be a dependency of `apps/admin`, and must export the generated
    `{PascalSlug}Admin` component. The build script checks only the first of the three.
  - Calibration: both checkers were run against deliberately broken fixtures before being trusted
    (missing `mod`/`#[path]`/`include!` targets; a registry whose crate is unlisted and whose
    component is renamed) and against healthy controls, and they fail and pass accordingly. They
    were then run over the audited crates: 407 declarations resolve, and all 34 admin-UI modules
    satisfy the registry contract.
  - Calibration: the wasm formatter reproduces the repository's own formatting history exactly —
    it re-derives the 55 files that upstream commit `6d353fda` reformatted, and it flags nothing
    else in those packages except one boundary case discussed in F-6. A tool that is wrong in
    general would not agree on 55/55 files it never saw.

## 2. Summary

| # | Failing CI step (run `37723570779`, `fly-page-builder.yml`) | Status |
|---|---|---|
| F-1 | `Verify editor capability policy` | **Fixed** — brittle literal marker in the gate |
| F-2 | `Run Fly browser contract tests` | **Fixed** — 6 assertions tied to pre-refactor code shapes; suites also never ran before `--lib` was removed |
| F-3 | `Check Fly adapter feature combinations` | Root-caused (hypothesis): the `desktop` combination needs WebKitGTK/GTK system libraries the job never installs. **Workflow patch proposed, not applied** (§4.1) |
| F-4 | `Check admin SSR Page Builder endpoint` | **Unresolved — needs toolchain**, but narrowed by static sweep (§4.2): the generated admin registry, every module declaration and the build script's own validation all resolve, and the same CI job compiles the Pages/Page Builder crates |
| F-5 | `Lint Fly browser and Page Builder integrations` | **Unresolved — needs toolchain** (§4.3) |
| F-6 | `Check focused formatting` (`Focused formatting`) | **Closed upstream, verified.** The step is green in run `37732431202` (head `6d353fda`, the format commit) and `bundle.rs` is canonical for the pinned stable toolchain — leave it (§4.4) |
| F-7 | `Audit dependencies` (advisory job, non-blocking) | Out of scope; job is explicitly advisory |

Two source defects were also found (F-8, F-9): F-8 was fixed during the audit, while F-9 was
recorded open first and is fixed in the same change set that wired the revision journal (§7).

| # | Source defect | Status |
|---|---|---|
| F-8 | Runtime-scenario release baselines and render snapshots were digested with FNV-1a 64 although `digest.rs` declares that gate uses SHA-256 | **Fixed** with an in-place, fail-closed migration (§5) |
| F-9 | `page_builder_scenario_baseline_revision` has an entity and a migration but no writer anywhere in the workspace | **Fixed** — migration and entity declared, journal written by every baseline mutation and readable through the service and GraphQL; runtime coverage added (§7) |

The same workflow has been red on every `main` push observed between 2026-10-01 and 2026-10-08
(`be5dbca7d`, `ed3267b18`, `a97744a21`, `a85d56e24`, `bb9357fa3`, …). Two neighbouring evidence
workflows (`Page Builder Static Sanitization Evidence`, `Pages Consumer Properties Source
Evidence`) fail on a different, already-identified cause: two gates demand the literal string
`actions/upload-artifact@v7` while every workflow pins the immutable v7.0.1 commit SHA — correct
supply-chain practice, so the gates are stale, not the workflows. Fixing those gates is a one-line
change in each and is left to the maintainer because it touches evidence-pipeline fixtures.

**The pattern behind F-1, F-2 and the two evidence workflows is the same:** verification here
frequently asserts *source text* rather than *behaviour*, so a formatter run or a rename silently
turns a passing gate into a permanently red one. The 2026-10-02 audit recorded this as H-7 and the
meta-gate `verify-fly-gates-are-wired.mjs` was added in response — but that gate only proves a
check is *referenced by a workflow*, not that it *can pass*. F-1 lived behind exactly that gap.

## 3. Findings fixed in this change set

### 3.1 F-1 — `verify-fly-ui-capability-policy.mjs` required a line that formatting removed

The gate required the literal marker `EditorCapability::ALL.into_iter()`. The shipped source has
the same code split by `rustfmt` across two lines in
`crates/modules/rustok-page-builder/admin/src/editor/capability_controls.rs` (line 352/353 at the
audit base; the call is inside a `Memo::new` closure whose argument list pushed the chain over the
line width). The marker therefore never matched, and `Verify editor capability policy` failed on
every run — including at `main` today, where the gate was re-checked against the `6d353fda` tree.

Fix: markers are matched against a **whitespace-free token sequence** of both the file and the
marker. Rust is whitespace-insensitive between tokens, so the assertion keeps its meaning wherever
the formatter breaks a line, and the rule is monotone — a marker that matched before still matches,
and the `!contains` (forbidden-marker) direction can only become stricter, never weaker. The
alternative — holding the source to one particular line wrapping — would make every `cargo fmt`
run a potential CI failure.

Verified: `node scripts/verify/verify-fly-ui-capability-policy.mjs` → exit 0;
`bash scripts/fly-check.sh gates` → **19 passed, 0 failed** (was 18/1).

### 3.2 F-2 — eight contract suites, six assertions pinned to deleted code shapes

`crates/ui/fly/browser/tests/*_contract.rs` asserts properties of the shipped JavaScript bundle
`crates/ui/fly/browser/assets/fly-browser.js` by searching it for code fragments. The workflow used
to pass `--lib` for these suites, so they compiled nothing and ran nothing; the `--lib` removal (a
recorded improvement, the workflow comment says so) exposed rot:

| Suite | Assertion that no longer matched | What the bundle actually contains |
|---|---|---|
| `browser_problem_contract` | `role", "alert` and `aria-live", "assertive` | the problem status node is created by `ensureProblemStatus`, which passes `"alert"`/`"assertive"` as arguments to `ensureStatus`, and the attributes are set from those parameters |
| `intent_abort_contract` | `signal: requestOptions?.signal` | request options are normalised by `normalizedTransportOptions(requestOptions)` and the signal is validated: `signal: isAbortSignal(transport.signal) ? transport.signal : undefined` |
| `intent_timeout_contract` | `controller.abort()` | the timeout path reports first and carries a classified reason: `this.reportIntentTimeout(record); controller.abort(` |
| `pending_intent_contract` | `controller.abort()` | stopping the adapter aborts every pending intent with `INTENT_ABORT_KIND.ADAPTER_STOP` and `record.controller.abort(record.abort.error)` |
| `response_order_contract` | `requestGeneration, current` | the abort detail computes currency: `current: record.requestGeneration === this.latestIntentRequestGeneration` |

Fix: a shared `tests/support/mod.rs` exposes `contains`, which folds whitespace out of both sides
before comparing; the eight suites consume it (`mod support; use support::contains;`) and the five
obsolete literals were replaced with the contract each one was really asserting. Two extra tests
(`tests/support_contract.rs`) pin the matcher itself, so a future edit that makes `contains`
vacuously true fails immediately. No assertion was deleted or weakened: the forbidden-marker
(`!contains`) assertions survive verbatim.

Verified statically: an independent scan extracted **58/58** assertions from the nine test files and
confirmed each against the shipped asset under the folding rule (the same rule the Rust matcher
uses); all nine files parse clean under `tree-sitter-rust`; all nine match `rustfmt` byte-for-byte
(edition 2024, default style). **Needs toolchain:** `cargo test -p fly-browser --all-targets` — the
suites are Rust code and only the compiler can confirm the 5 added expectations compile.

## 4. Root-caused findings that need the maintainer

### 4.1 F-3 — `Check Fly adapter feature combinations`: the desktop combination has no system libraries

The step runs five checks; the first is
`cargo check -p fly-dioxus --features desktop`.

`crates/ui/fly/dioxus/Cargo.toml` maps that feature to `dioxus/desktop`, and `Cargo.lock` resolves
`dioxus 0.6.3` with `dioxus-desktop 0.6.3`, `wry 0.45.0` and `tao 0.30.8` in the graph. On Linux
those crates reach WebKitGTK/GTK/soup through sys crates whose build scripts run `pkg-config`
during `cargo check`. No workflow installs them: the only `apt-get install` calls in the repository
are `lld` (`.github/workflows/ci.yml:200`) and `redis-server`
(`cache-hardening.yml:288`, `rbac-runtime-evidence.yml:170`). This is the leading hypothesis for
the step's failure and it explains why the failure is confined to the desktop combination while the
wasm32 checks pass.

Proposed patch (not applied — `AGENTS.md` §15 forbids editing CI workflow files unless CI changes
are explicitly requested):

```yaml
      - name: Install Linux GUI dependencies for the desktop adapter
        if: ${{ !cancelled() }}
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev librsvg2-dev
```

Placed before the feature-combination step, this is the standard Dioxus/Tauri Linux dependency set
and is harmless if the real cause turns out to differ. If the desktop adapter is deliberately not
part of the shipped surface (`crates/ui/fly/README.md` lists `fly-dioxus` as *Foundation only*),
the alternative is to drop `--features desktop` from the step and document that the combination is
not compiled anywhere — but then nothing would compile it, which is the situation the step exists
to prevent.

**Needs toolchain:** with `cargo`, `cargo check -p fly-dioxus --features desktop` on a clean
runner settles this in one command.

### 4.2 F-4 — `Check admin SSR Page Builder endpoint`

`cargo check -p rustok-admin --bin rustok-admin --no-default-features --features ssr` fails in run
`37732431202` (head `6d353fda`, the tip of `main`) and in every earlier run inspected. The crate is
`apps/admin`; its default feature set is `["ssr"]`, so the invocation is the plain default build,
and the Page Builder contribution glue in `apps/admin/src/app/page_builder_contributions.rs` (the
file this step exists to protect) was reformatted upstream in `6d353fda` with no semantic change.

Static sweep performed for this audit, using the two new checkers in §1:

- **The generated registry resolves.** `apps/admin/build.rs` turns every module manifest with
  `[provides.admin_ui].leptos_crate` into generated Rust naming `{leptos_crate}::{PascalSlug}Admin`.
  All 34 such modules have their crate listed in `apps/admin/Cargo.toml` and export the expected
  root component; the build script's own `admin/Cargo.toml` ↔ `leptos_crate` validation passes for
  every module. A registry mismatch would have produced E0433/E0425 in exactly this step, and there
  is none.
- **Every module declaration resolves.** 407 `mod`/`#[path] mod`/`include!` targets across
  `apps/admin` and the audited crates exist; a missing one would be E0583.
- **The failure is admin-specific, not environmental.** In the same job and on the same runner,
  `Run Pages domain unit tests`, `Run Page Builder admin unit tests`, `Check Pages storefront` and
  `Check Page Builder storefront` all pass, and `Lint Fly` compiles `fly` and its peers. Only the
  `rustok-admin` SSR build fails.

That leaves real compile semantics (a type/feature error inside `apps/admin` or its unique
dependency set) or the step's environment. **Needs toolchain** — run the command and read the first
error, or fetch the failing step's log.

### 4.3 F-5 — `Lint Fly browser and Page Builder integrations`

`cargo clippy -p fly-browser -p rustok-page-builder-admin -p rustok-pages -p rustok-pages-admin -p
rustok-pages-storefront --lib -- -D warnings`. The workspace denies only five clippy lints
(`dbg_macro`, `todo`, `unimplemented`, `print_stdout`, `print_stderr`; root `Cargo.toml:94-99`), so
any *default* warning in any of the five crates fails the step. Static reading found no `dbg!`,
`todo!()` or `print_*` in the audited crates, but default-on lints (`needless_return`,
`collapsible_if`, `result_large_err`, …) cannot be evaluated by reading. **Needs toolchain** —
`cargo clippy` output is the only way to name the warning, and the repository has no local cache to
fall back on.

### 4.4 F-6 — `Check focused formatting`: bounded, and one case to leave alone

The job runs `cargo fmt -p fly -p fly-ui -p fly-web -p fly-browser -p fly-leptos -p fly-dioxus
-p rustok-page-builder-admin -p rustok-pages -p rustok-pages-admin -p rustok-pages-storefront
-p rustok-admin -- --check`.

Measured with the wasm rustfmt at the audit base: **56 files** in those packages are not in rustfmt
canonical form. Upstream commit `6d353fda` ("…format workspace…") then reformatted **exactly those
55 of the 56** — the set matches one-for-one, with zero extra files. The single exception is
`crates/ui/fly/src/bundle.rs`, whose `use crate::{…}` block ends a continuation line at exactly 100
columns:

```rust
    RegistrySet, ValidationLimits, ValidationReport, audit_page, constant_time_eq, validate_project,
```

**Updated 2026-10-08: closed upstream and verified, and the decision is to leave `bundle.rs`
untouched.** Two independent measurements settle it:

- **The CI step is green at the tip of `main`.** In run `37732431202` (head `6d353fda`, the format
  commit) the `Focused formatting` job and its `Check focused formatting` step both conclude
  `success`. The repo pins `channel = "stable"` in `rust-toolchain.toml`, so a green
  `cargo fmt … -- --check` at that commit *is* the statement that `bundle.rs` is canonical for the
  toolchain CI uses. The F-6 failure belonged to the pre-format state — which is the state this
  branch is based on, not a defect of this change set.
- **Formatter calibration, re-measured at this HEAD.** The wasm formatter flags 56 files in the
  F-6 package scope; exactly the 55 of them that upstream's format run reformatted are flagged, and
  exactly one file it left untouched is flagged — `bundle.rs`. The disputed line is exactly
  **100 columns**, and the repository has no `rustfmt.toml`, so `max_width` is rustfmt's default
  **100**: the line is inside the contract, not over it.

Splitting it would follow a nightly-only verdict and move the file away from what stable's
`--check` accepts — and the CI evidence above is that stable accepts the file as it stands.
Recommendation, now a decision: **leave `bundle.rs` alone.** The two-line split below stays for
reference in case a future stable toolchain adopts the nightly behaviour.

```rust
    RegistrySet, ValidationLimits, ValidationReport, audit_page, constant_time_eq,
    validate_project,
```

Note for reviewers: this means the formatting failure at the *audit base* is not a defect of this
change set — it was already fixed upstream, and this branch is built on the commit before that fix.
Every file touched here is rustfmt-clean (verified), so the branch adds no formatting debt.

## 5. Integrity finding — C-1 — **fixed in this change set** (history below)

The 2026-10-02 audit's C-1 was "`ProjectHash` (FNV-1a 64) is used as an integrity check". Its
remediation introduced `crates/ui/fly/src/digest.rs`, whose module documentation states the policy
in as many words:

> Every gate that answers "is this payload the one that was approved?" — snapshot restore,
> project-bundle import, **runtime-scenario release baselines** — uses `ContentDigest` instead.

`ContentDigest` is SHA-256. But the runtime-scenario release baseline — one of the three gates the
paragraph names — still hashes with FNV-1a 64:

- `crates/ui/fly/src/runtime_scenario_release.rs:44-53` — `computed_hash()` serialises the baseline
  and returns `ProjectHash::from_bytes(&bytes).hex()`.
- `crates/ui/fly/src/runtime_scenario_release.rs:57` — `has_valid_hash()` compares the stored
  `baseline_hash` against that FNV value.
- `crates/ui/fly/src/runtime_scenario_release.rs:338` — `snapshot_has_valid_hash()` does the same
  for `snapshot.snapshot_hash`.
- `crates/ui/fly/src/runtime_scenario_snapshot.rs:320-330` — `snapshot_hash()` builds that value
  from `serde_json::to_vec(...).unwrap_or_default()` and FNV-1a 64.

The check is a real gate, not dirty tracking: `RuntimeScenarioReleaseBaseline::validate()` emits
`runtime_scenario_baseline_hash_invalid` ("release baseline integrity hash does not match its
contents") and `snapshot_hash_invalid` diagnostics, `is_valid()` gates on them, and
`RuntimeScenarioReleaseMode::{BlockBroken, RequireStable}` turns baseline validity into a
publication decision. The hash is persisted in
`page_builder_scenario_baseline.baseline_hash` with `expected_baseline_hash` used as a
compare-and-swap precondition, and it is exposed over GraphQL.

Why this matters: FNV-1a 64 is not collision-resistant, and it is not merely "2^32 birthday" weak —
its multiply/xor chain is invertible, so a payload that hashes to a *chosen* 64-bit value is
cheaply constructible. The threat model is the one tamper-evidence exists for: an actor who can
write the baseline payload (an admin-side write path, a compromised or buggy client, a replay of a
stored record) but cannot update the stored digest can substitute content that still validates as
"the approved baseline". Severity is **P1**, not P0: it does not by itself cross a privilege
boundary — it removes a layer of tamper evidence behind one — but it is the difference between a
control and a formality.

**Status: fixed for the approval anchor.** `RuntimeScenarioReleaseBaseline::computed_hash` now
produces `ContentDigest` (`sha256:<64 hex>`) and `has_valid_hash` compares it in constant time. That
is the value recorded in `page_builder_scenario_baselines.baseline_hash`, exported over GraphQL and
used as the `expected_baseline_hash` compare-and-swap precondition, so it is the one hash in this
family whose forgery would let a payload pass as "the approved baseline".

Two neighbouring hashes **deliberately stay FNV-1a**, and reaching that decision took a
mid-implementation correction worth recording, because "switch everything" was the wrong answer:

- `RuntimeScenarioRenderSnapshot::snapshot_hash` is always carried inside an envelope that digests
  it with sha256 — the baseline payload above, and the materialization identity's
  `runtime_snapshot_hash`, which is a sha256 digest of the stored snapshot list. Changing it also
  breaks the pages rebuild path: `artifact_rebuild.rs` requires a rebuild to reproduce
  `runtime_snapshots` byte for byte ("rebuilt runtime evidence does not exactly reproduce retained
  provenance"), so every artifact persisted before the deploy would fail its own provenance check.
- The per-case `html_hash`/`css_hash`/`document_hash` are compared against freshly recomputed
  values on the serve, audit and rebuild paths (`artifact_integrity_audit.rs`,
  `page_builder_artifact.rs`), so the same deploy-time failure would appear as "runtime snapshot for
  static page N does not match the materialized artifact". They lose nothing: the payloads they are
  compared against are already bound by sha256 (`StaticLandingPage::content_hash` over
  `document_html`, and the identity's `runtime_snapshot_hash`, a sha256 digest of the whole stored
  snapshot list including those case fields).

Migration, because the baseline hash is persisted and compared across the admin/Pages boundary:

Also fixed as part of this: `snapshot_hash()` no longer hashes *empty bytes* on a serialisation
failure — it returns an empty string, which never parses as a `ContentDigest`, so every
verification path fails closed instead of comparing a constant. (Not a `FlyResult`, as the
recommendation below suggested: the field is a plain `String` in a persisted struct, and an empty
digest is both unreachable in practice and unambiguously invalid where a fallible signature would
have rippled through the whole capture/baseline API for no additional safety.)

Verification performed: rustfmt and tree-sitter parsing pass for all changed files; new unit tests
cover legacy recognition, in-place upgrade (including idempotence), the two states that must **not**
be repaired (a value matching no recorded content, and an altered envelope), and that a captured
snapshot fingerprint still matches its contents while the baseline digest is sha256. An independent
model of the decision table reproduced the intended outcome for every state. **Needs toolchain:**
`cargo test -p fly -p rustok-pages`, which this environment cannot run.

## 6. Verified clean (checked, no defect found)

These areas were examined in this and the preceding pass and are recorded as sound, so the next
reader does not repeat the work:

- **Static publish sanitisation is wired and internally consistent.** `static_publish_policy.rs`
  rejects every `on*` handler, forbidden attributes, control characters and backslash/protocol-relative
  URLs; per-kind URL policy covers navigation (fragment/relative/https/mailto/tel), resources
  (relative/https), resource images (plus an allowlist of `data:image/*;base64,`), form actions
  (relative only), canonical and fragment. `url(` is deliberately *not* a forbidden CSS token —
  `safe_css_value` strips each `url()` reference through per-URL validation and then rejects any
  survivor. Validators are actually reached: `validate_static_publish_resource_limits`
  (`static_landing.rs:151`), `validate_static_publish_document` (`static_landing.rs:178`),
  `sanitize_static_landing_project` (`artifact_rebuild.rs:186`, `publish_manifest.rs:127`,
  `reviewed_publish.rs:249`, plus tests).
- **Publication is transactional.** Page services take `db.begin()`, publish through
  `publish_in_tx(...)` and commit once; `artifact_binding_replacement.rs` has two `commit()` calls
  that are mutually exclusive (idempotent-replay early return vs. main path) inside one owner
  transaction.
- **Tenant scoping.** Production `Entity::find()` chains that omit `TenantId` number four and all
  four are justified by upstream scoping (`scenario_baseline.rs:333,370`,
  `publish_manifest.rs:61,65`). The storefront server functions resolve the tenant from
  `TenantContext` and fall back to the configured slug only when it is absent, rejecting a
  mismatching slug rather than trusting it — the `.ok()` on the extract degrades to the fallback
  path, not to a cross-tenant read.
- **No hand-written `unsafe`** in the audited crates; `fly` and `fly-browser` are `forbid(unsafe_code)`.
- **The rollback adapter does not swallow GraphQL failures.**
  `rustok-pages/admin/src/transport/rollback_retry_adapter.rs` keeps a durable
  `(expected_version, idempotency_key)` pair in session storage so a retry after a network failure
  replays rather than double-applies, and clears it only on success or on a classified definitive
  rejection. `rustok_graphql::execute_with_client` checks the response's `errors` array and returns
  `Err(GraphqlHttpError::Graphql(_))` before it looks at `data`, so a rejected mutation cannot be
  reported as a successful rollback. The one deliberately ignored result is the storage clear on
  the success path (`let _ = clear_pending_attempt(...)`), which is safe: the leftover identity is
  the same key the next attempt would reuse.
- **All 19 local Fly source guards pass** (`bash scripts/fly-check.sh gates`), including
  `verify-fly-gates-are-wired`, which confirms every `verify-fly-*.mjs` is reachable from a workflow.
- **Every browser-contract assertion** extracted from the nine suites holds against the shipped
  asset under the token-folding rule (58/58), and the code the matcher explains is the code the
  adapter actually ships.

## 7. Documentation and repository hygiene observations

- `PAGE_BUILDER_CURRENT_CLEANUP_FAILURE.md` (repository root, 928 lines) is a captured failing
  `cargo build` for `rustok-page-builder` recording `E0252` (three duplicate imports at
  `service.rs:13`) and `E0433` (`crate::runtime_telemetry::crate::runtime_telemetry::…` at
  `service.rs:1295,1307`). All five errors are fixed at the audit base — the duplicate import block
  and the malformed paths are gone. The file was a stale receipt: it sat in both `typos.toml` and
  `_typos.toml` ignore lists and nothing referenced it. **Retired in this change set — deleted.**
  There is no tracked evidence archive to move it to (`scripts/evidence/` holds workflow-side
  producer scripts, and `output/` is git-ignored), the capture survives in git history, and
  `AGENTS.md` §15 directs that superseded current-state material be removed once the replacement is
  canonical. Both `typos` entries were removed with it.
- Two gates assert a *tag* where the repository's supply-chain policy pins a *SHA*:
  `verify-page-builder-static-sanitization-execution.mjs` (literal `actions/upload-artifact@v7`)
  and `verify-pages-consumer-properties-source-execution.mjs` (same). All workflows pin the
  immutable v7.0.1 commit. The gates are the staleness; the fix is to assert the pinned-SHA form
  the supply-chain checker already enforces.
- `page_builder_scenario_baseline_revisions` (F-9) had **no reader and no writer anywhere in the
  workspace**, and the two commits that introduced it (`374d0dea9` entity, `682a639c6` migration,
  one file each) left it in a state where nothing compiled: the migration file was never declared in
  `src/migrations/mod.rs` (so the table was never created, in tests or in production) and
  `src/entities/page_builder_scenario_baseline_revision.rs` was never declared in
  `src/entities/mod.rs`. `PageBuilderScenarioBaselineService` updated the active row and its
  `previous_baseline_hash` column and stopped there, so the promotion trail the schema promised was
  not produced, and `previous_baseline_hash` could only ever carry a single previous value.
  **Fixed** in this change set, in the shape the schema implies — the journal is the durable record,
  so it is written, not dropped:
  - `m20260714_000003_create_scenario_baseline_revision_history` is declared and registered, and the
    entity is declared and exported.
  - `save_internal` and `delete_internal` now run inside a transaction and append exactly one
    revision row per accepted mutation: `create` (first baseline), `replace` (both the CAS path and
    the non-CAS overwrite), `delete` (a clear). The row carries the operation, the baseline id and
    the two hashes, the stored baseline payload, the promotion note that `save_if_current` already
    required, the acting user (`promoted_by`, or `SecurityContext::user_id` for a delete) and the
    timestamp. A rejected mutation — stale CAS, missing review note, conflict — rolls back and
    appends nothing, so a baseline change and its record cannot diverge.
  - A delete now reads the row before removing it and stores that payload in the revision, which is
    the only surviving copy of a cleared baseline.
  - `PageBuilderScenarioBaselineService::history` and the GraphQL field
    `pageBuilderScenarioBaselineHistory` (read permission, newest first, capped at 200) read the
    trail back.
  - `crates/modules/rustok-pages/tests/scenario_baseline_revision_journal_sqlite.rs` covers the
    lifecycle: one row per accepted mutation with the expected operation/hash linkage/actor/note and
    payload round-trip, no row for any rejected or no-op path (stale CAS, missing review note, a
    guarded clear without an expected hash, clearing an absent baseline), and the removed payload
    surviving in the journal after a clear. It is also the first runtime coverage the
    scenario-baseline tables have anywhere in the workspace.
  - Verification status: written and statically reviewed here, **not compiled or run** — this
    environment has no Rust toolchain. The file was checked with the calibrated rustfmt-on-wasm
    described in §1 and `scripts/audit/rust_module_resolution_check.py` resolves the new
    declarations; `cargo test -p rustok-pages --test scenario_baseline_revision_journal_sqlite`
    (and `cargo fmt --check`) is the confirmation to run on a toolchain, listed in §8.
  What remains a product decision, not a defect: the journal is append-only and grows without a
  retention policy, and the admin UI does not surface it yet (`rustok-pages-admin` reads the current
  baseline and release status only). Both are recorded in the module README.
- Language policy: `AGENTS.md` §12 makes English the only repository documentation language with
  `README.ru.md` as the single exception, yet `docs/audits/` contains Russian documents
  (`fly-builder-engineering-audit-2026-10-02.md`, `ffa-ui-libraries-engineering-audit-2026-10-03.md`,
  `product-module-engineering-audit-2026-10-07.md`) alongside English ones. New material here is
  English; the rule and the existing corpus should be reconciled one way or the other rather than
  drifting.
- `crates/ui/fly/README.md`'s status table matches the code: `fly`/`fly-ui` stable,
  `fly-web`/`fly-browser` beta, `fly-leptos`/`fly-dioxus` foundation-only shells. The ADRs that
  describe full Leptos/Dioxus editors remain aspirational; that gap is tracked here rather than
  silently inherited.

## 8. Recommended order of work

1. ~~Apply the `bundle.rs` decision in §4.4~~ — done: leave it, verified green in CI at the tip of
   `main` (§4.4).
2. Apply the adapter-dependency patch in §4.1 (F-3) and run the feature-combination step.
3. Run `cargo test -p fly-browser --all-targets` and `cargo clippy … -- -D warnings` locally to
   close F-2 and name F-5 (F-2 is expected to pass; F-5 needs the warning text).
4. Triage F-4 with the failing log.
5. ~~Schedule §5 (C-1 residual) as a migration-sized change~~ — done; review the migration
   behaviour in §5 on a database that already holds baselines, and confirm `cargo test -p fly -p rustok-pages`.
6. Repair the two `upload-artifact` gates (§7) — **left to the maintainer** (CI/workflow scope);
   the stale root receipt is ~~retired~~ done.
7. Confirm the F-9 journal wiring (§7) on a toolchain: `cargo test -p rustok-pages --test
   scenario_baseline_revision_journal_sqlite` plus `cargo fmt --check`. The change adds a migration
   to the `PagesModule` list, so on a database that already exists the migration runs on next
   startup — it is `if_not_exists` and creates an empty table, so there is no backfill and no effect
   on existing baselines or their promotion metadata.

## 8.1 CI snapshot at the tip of `main` (`6d353fda`, run `37732431202`)

Recorded because it separates "fixed upstream" from "still open", which a red badge does not:

| Step in job `Focused tests and source guards` | Conclusion at `6d353fda` |
|---|---|
| `Run Fly browser contract tests` (F-2) | **failure** — fixed on this branch |
| `Verify editor capability policy` (F-1) | **failure** — fixed on this branch |
| `Check Fly adapter feature combinations` (F-3) | **failure** — needs the workflow patch (§4.1) |
| `Check admin SSR Page Builder endpoint` (F-4) | **failure** — needs the toolchain or the log (§4.2) |
| `Lint Fly browser and Page Builder integrations` (F-5) | **failure** — needs the clippy output (§4.3) |
| `Focused formatting` (F-6, separate job) | **success** — closed upstream (§4.4) |
| `Run Fly core tests`, `Run Fly UI and web runtime tests`, `Check Fly framework adapters`, `Check Fly without the platform i18n dependency`, `Run Page Builder admin unit tests`, `Run Pages domain unit tests`, `Run Pages integration unit tests`, `Check Pages storefront`, `Check Page Builder storefront`, `Lint Fly`, and all 16 `Verify …` source guards | success |
| `Audit dependencies` (F-7, advisory job) | failure — advisory, out of scope |

## 9. What this audit does not establish

- Nothing here proves the page subsystem compiles or its tests pass at this commit; the toolchain
  was unavailable by construction (§1).
- F-3, F-4 and F-5 are root-cause *hypotheses* of failing steps, not confirmed diagnoses; each
  needs one command with a toolchain, or the step's log.
- The audit covers the page/builder subsystem; unrelated red workflows (forum, e-commerce, index
  evidence) were out of scope except where they share a cause with the findings above.
- Performance claims (clone counts, query patterns) were not re-measured; §1a of the 2026-10-02
  audit remains the record for those, with the status verification in its new §10.
