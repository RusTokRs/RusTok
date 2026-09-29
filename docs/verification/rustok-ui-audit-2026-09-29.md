---
id: doc://docs/verification/rustok-ui-audit-2026-09-29.md
kind: verification_status
language: markdown
source_language: markdown
status: active
as_of: 2026-09-29
---
# rustok-ui component audit — 2026-09-29

**Scope:** `crates/ui/rustok-ui` (core), `crates/ui/rustok-ui/leptos` (`rustok-ui-leptos`), `crates/ui/rustok-ui/dioxus` (`rustok-ui-dioxus`), and the `crates/ui/leptos-ui` consumer boundary.

## Method and evidence limits

Every module of the three crates was read end-to-end, together with all in-tree
call sites (`grep` over `apps/` + `crates/`), both Tailwind pipelines, the crate
registry/ledger, and the guardrail scripts that pin this area.

The audit environment has **no Rust toolchain** (`cargo`, `rustc`, `rustup` are
absent and `static.rust-lang.org` / crates.io mirrors are unreachable), so
`cargo fmt`, `clippy`, `check`, and `test` were **not** executed. Evidence in
this document is therefore:

- executed Node guardrails from `scripts/verify/` (before/after comparison);
- an executed Tailwind CSS rebuild of the storefront artifact;
- source-level review plus static `grep`/diff checks.

Compilation and test evidence remains a maintainer action (see the end of this
document).

## Findings and remediation

| ID | Severity | Finding | Remediation | Status |
|---|---|---|---|---|
| UI-CORE-01 | High | Every class resolver accepted `disabled`/`loading`/`required` parameters it silently ignored (`_disabled`, `_loading`, `_required`), so callers believed state was reflected in classes. | Removed the dead parameters; state is expressed through native attributes (`disabled`, `aria-invalid`, …) and the `disabled:` utilities that are part of every resolver. All 14 adapter call sites updated. | Fixed |
| UI-CORE-02 | Medium | `merge_classes` was private, allocated a `Vec<&str>` per call, and joined untrimmed fragments. | Public, allocation-free implementation that trims, skips empties and never emits doubled separators; doctest + unit tests added. | Fixed |
| UI-CORE-03 | Medium | `extract_initials` could return more than two characters (`ß` → `SS`) and accepted punctuation/digit "initials". | Rewritten: first alphanumeric character per word, one uppercase character per initial, at most two characters, `?` fallback; documented with doctests. | Fixed |
| UI-CORE-04 | Low | `Cargo.toml` declared `serde_json` as a runtime dependency although only tests use it; the crate description promised non-existent "theme resolution". | `serde_json` moved to `dev-dependencies`; description aligned with the actual (class resolver + contract) responsibility. | Fixed |
| UI-CORE-05 | Low | Public contracts (`SelectOption`, `TabItem`, `DialogState`, `TabsState`) and most token constants/functions had no documentation, and the crate had no lint floor. | Full rustdoc on public items, `#![forbid(unsafe_code)]` and `#![warn(missing_docs)]` added (same floor as `rustok-forms` / `rustok-grid`). | Fixed |
| UI-LEPTOS-01 | High | After the core signature change, all seven arity call sites in the Leptos adapter still passed the removed parameters — the adapter could not compile. | Call sites updated (`button`, `input`, `textarea`, `select`, `checkbox`, `switch`, `label`). | Fixed |
| UI-LEPTOS-02 | High | `Tabs` built its wrapper class with `format!("w-full {}", class)`, emitting a trailing space (and a dangling separator with no custom class). | Wrapper now uses `merge_classes`. | Fixed |
| UI-LEPTOS-03 | High | `Dialog` marked its **clickable** backdrop `aria-hidden="true"`, never moved focus into the modal, and had no `Escape` handling. | Backdrop no longer hidden from assistive tech; dialog receives focus on open (`NodeRef` + `Effect`) and closes on `Escape`. Focus trap/`aria-labelledby` remain a follow-up. | Fixed (partial, documented) |
| UI-LEPTOS-04 | Medium | `Spinner` hardcoded `aria-label="Loading"`, so localized hosts announced English. | `aria_label` prop with `Loading` fallback, documented for localized callers. | Fixed |
| UI-LEPTOS-05 | Medium | `Separator` and `TabsList` never announced their orientation; screen readers assumed horizontal. | `aria-orientation` rendered from `Orientation::as_str()`. | Fixed |
| UI-LEPTOS-06 | Medium | `Select` rendered initial `selected` attributes only; programmatic value changes did not update the DOM selection. | `prop:value` binding added (mirrors the raw `<select prop:value=…>` idiom already used by consumers). | Fixed |
| UI-LEPTOS-07 | Low | `Checkbox` set `prop:indeterminate` from a plain `bool`, so it reacted only to the first render. | Reactive closure form (`prop:indeterminate=move || indeterminate`). | Fixed |
| UI-DIOXUS-01 | High | Same arity drift as the Leptos adapter, across all seven call sites. | Call sites updated. | Fixed |
| UI-DIOXUS-02 | Medium | `Checkbox` exposed an `indeterminate` prop that can never render: HTML defines no such attribute (DOM property only) and `dioxus-html` 0.6.3 declares no `indeterminate` constant. | Prop removed and the limitation documented in rustdoc + adapter README. | Fixed |
| UI-DIOXUS-03 | Medium | `Select` had no element-level value binding. | `value` attribute added (declared by `dioxus-html` 0.6.3), alongside per-option selection state. | Fixed |
| UI-DIOXUS-04 | Medium | `Spinner` hardcoded `aria-label="Loading"`. | Optional `aria_label` prop with `Loading` fallback. | Fixed |
| UI-DIOXUS-05 | Medium | `Tabs` emitted a trailing space, `TabsList`/`Separator` lacked orientation, `Dialog` backdrop was `aria-hidden` while clickable. | Fixed; `Escape`/focus handling is documented as host-provided for this adapter. | Fixed |
| UI-DIOXUS-06 | Low | Adapter pinned `dioxus = "0.6"` instead of inheriting the workspace dependency. | `dioxus = { workspace = true }`. | Fixed |
| UI-API-01 | Medium | The "canonical" `ui_*` alias surface in both adapters omitted every `Card*` and `Tabs*` alias. | Aliases completed in both adapters. | Fixed |
| UI-TW-01 | High | Tailwind sources for `apps/admin` and `apps/storefront` never scanned the crates that own the class strings, so resolver utilities (`shadow-xs`, `h-9`, `translate-x-5`, `bg-amber-100`, `text-[10px]`, …) were missing from compiled CSS. | `@source`/`content` globs extended to `crates/ui/rustok-ui/{src,leptos}/src` and `crates/ui/leptos-ui/src`; the storefront CSS artifact was rebuilt and verified. | Fixed |
| UI-FACADE-01 | Medium | `crates/ui/leptos-ui/src/{card,label,separator}.rs` were unreachable duplicates of the adapter components with hard-coded class strings; one of them was even pinned by a guardrail, giving a false sense of coverage. | Files deleted; `verify-translation-admin-boundary.mjs` now pins the canonical `crates/ui/rustok-ui/leptos/src/label.rs` (script passes). | Fixed |
| UI-DOC-01 | Low | `docs/modules/crates-registry.md` had no rows for `rustok-ui`, `rustok-ui-leptos`, `rustok-ui-dioxus`; the ledger pointed at the removed `crates/ui/rustok-ui-forms` path; the crate README showed the old resolver signature. | Registry rows added, ledger link repaired, READMEs (core + both adapters) updated, audit section appended to the ledger. | Fixed |

## Deferred decisions (explicitly not changed)

- **`Button::on_click: Option<Box<dyn Fn() + 'static>>` preserved.** Leptos
  `Callback<()>` requires a `Fn(())` closure, so migrating means rewriting the
  closure shape at 46 `crates/modules/rustok-translation/admin` call sites and
  imposing `Send + Sync` on every handler. The breaking change was judged out of
  proportion for this round; the contract is documented in the adapter README and
  the migration is recommended when those call sites are next touched.
- **Core `InputType` is not wired into the adapters.** The adapters forward the
  raw HTML `type` string (no in-tree caller passes `r#type`), which keeps room
  for `file`, `color`, `datetime-local`, …; `InputType` remains a host-facing
  contract.
- **Dialog focus trap / `aria-labelledby`** require a focus-manager and id
  plumbing; only focus-on-open + `Escape` were implemented.
- **Legacy `UI/leptos` (`iu-leptos`)** is a diverged twin of
  `crates/ui/rustok-ui/leptos` still referenced by the root manifest; it is
  outside this audit's scope and should be handled in a dedicated round.
- **Pre-existing guardrail failures** (blog comments write surface, forum
  notifications, topic-merge, groups invitation, `next-frontend` i18n, CSP
  inline-style ratchet, product-admin inline style) reproduce identically on a
  clean checkout and are unrelated to this crate.

## Verification executed in this round

- `node scripts/verify/verify-translation-admin-boundary.mjs` → **pass**
  (after re-pointing the pinned label to the canonical adapter file).
- Guardrails relevant to the touched area → **pass**:
  `verify-blog-admin-boundary`, `verify-blog-storefront-boundary`,
  `verify-comments-admin-boundary`, `verify-csp-next-style-boundary`,
  `verify-ffa-ui-boundary-sweep`, `verify-ffa-ui-doc-patterns`,
  `verify-fly-multilingual`, `verify-forum-storefront-boundary`,
  `verify-ui-client-dependency-boundary`.
- `node scripts/verify/verify-docs.mjs` → 19 errors, all present in the
  pre-change baseline (20 before the ledger link repair).
- Storefront CSS: `npm ci && npm run build:css` executed successfully; the
  rebuilt `apps/storefront/static/app.css` contains **every** class selector of
  the previous artifact (0 removed, 196 added) and all utilities listed in
  UI-TW-01 (`shadow-xs`, `translate-x-5`, `bg-amber-100`, `h-9`, …).
- Admin CSS: `npm ci && npm run tw:build` executed successfully against the
  extended `@source`/`content` globs (large utility output present; the artifact
  is intentionally not committed).
- Documentation drift: `docs/UI/module-package-implementation.md`,
  `docs/UI/rust-ui-component-catalog.md` and `docs/index.md` now point at the
  canonical `crates/ui/rustok-ui/**` sources and link this audit.

## Maintainer follow-ups

1. `cargo fmt -p rustok-ui -p rustok-ui-leptos -p rustok-ui-dioxus -- --check`
2. `cargo clippy -p rustok-ui -p rustok-ui-leptos -p rustok-ui-dioxus --all-targets -- -D warnings`
3. `cargo test -p rustok-ui -p rustok-ui-leptos -p rustok-ui-dioxus`
4. `cd apps/admin && npm run tw:build` (CSS artifact is not committed)
5. Re-run the storefront CSS build after any further class-string change
6. Browser smoke of the storefront/admin pages that use `leptos_ui` components
   (dialog Escape/focus, select value, spinner label, tabs orientation)
