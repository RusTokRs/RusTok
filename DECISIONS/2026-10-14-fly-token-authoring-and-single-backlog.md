# Fly token authoring and the single owned backlog (Phase 6)

Date: 2026-10-14
Status: accepted
Scope: `fly` (theme commands), `fly-ui` (capability classification),
`rustok-page-builder-admin` (Design tokens panel), repository docs.

## Context

Phase 3 landed design tokens as validated project data with rendering, but no
authoring path: adapters displayed them read-only and hosts had to poke the
`flyTheme` extension directly, bypassing validation and history. Separately, the
audit's documentation finding (P3-19) recorded three parallel plans whose open
checkboxes could not distinguish release blockers from "someday".

## Decisions

1. **Token mutation is an editor command.** `EditorCommand::Theme` carries a
   `ThemeCommand` (replace/upsert/remove). Applying it goes through the editor's
   speculative-clone transaction, full validation (which already gates on
   `theme_token_invalid`), and undo history, like every other edit. Removing the
   last token removes the `flyTheme` extension entirely; removing a missing token
   is an error. The capability classifier requires `Styles` for theme commands.
   Renaming a token deliberately does *not* rewrite `var(--name)` references in
   declarations (recorded in the Fly README): usage-aware rename is a separate,
   future authoring feature, not a silent side effect.
2. **One owned backlog.** `docs/modules/page-builder-backlog.md` is the single
   forward-looking Fly/Page Builder plan, ordered by priority with owners. The
   implementation plan and the parity actualization notes are marked historical
   (they keep the record; they no longer collect open items), and `docs/index.md`
   points at the backlog. The backlog's first item is the verification debt:
   `cargo test`/`clippy`/benchmarks for the whole 2026-10-14 remediation still
   need a maintainer with a toolchain.

## Verification

As before: no Rust toolchain in this sandbox. `rustfmt_check.mjs` conforms on all
changed files (35/35), `rust_syntax_check.mjs` is clean apart from the known
tree-sitter false positives, and `scripts/fly-check.sh gates` passes 20/20 — the
capability-policy gate initially caught the reformat of the classifier line,
which is exactly what it is for. Behavioural runs remain maintainer/CI work.
