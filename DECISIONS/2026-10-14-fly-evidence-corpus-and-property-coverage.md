# Fly evidence corpus and property coverage (Phase 5)

Date: 2026-10-14
Status: accepted
Scope: `crates/ui/fly` test suite and `fixtures/grapesjs` corpus.

## Context

The audit's evidence findings (P1-8/P1-9) were that GrapesJS compatibility rested on
one small structural baseline, the single arbitrary-document property generator only
produced text/section trees with two style properties, and several safety invariants
were covered by hand-picked examples rather than properties. This sandbox has no
browser and no way to run a real GrapesJS editor, so genuine browser captures cannot
be produced here — and must not be fabricated.

## Decisions

1. **The fixture corpus grows to twelve registered entries**, nine of them new:
   hero/CTA with class + state + media rules, a table-like structure, a provider
   form, a rich-text article, allow-listed and off-registry embeds, toggle/animate
   interactions with design tokens, a frames-carrying page, nav/footer links, and a
   661-component catalog. Every entry is registered in
   `fixtures/grapesjs/manifest.json` with the existing honesty fields:
   `real_browser_capture: false`, `browser_roundtrip: false` with a skip reason.
   They are structural baselines in GrapesJS shape, and the manifest says so.
2. **A corpus harness test pins the shape contract**: every manifest entry must
   decode, render (when it has pages), validate without panicking, and encode
   idempotently; the corpus must stay at ten entries or more.
3. **The property generator grows teeth.** A diverse generator now produces typed
   leaves (text, headings, buttons with toggle/scroll/animate/emit actions, images
   with hostile `src` values, rich-text payloads with a `javascript:` link, valid
   and invalid embeds) inside section trees, wrapped in projects with class/state/
   media style rules and design tokens. Two properties are stated over it:
   byte-exact codec round-trip, and render safety (no `<script`, no `javascript:`
   URLs, every iframe sandboxed on an allow-listed host, tokens always emitting
   their `:root` rule).

## Still open (honest)

Real GrapesJS browser captures (the manifest's `browser_roundtrip` entries remain
the three originals), Playwright end-to-end editor scenarios, and executable
accessibility evidence all require a browser environment. They stay tracked under
P1-8/P1-9 for a maintainer with that environment; nothing here claims them.

## Verification

No Rust toolchain or browser exists in this sandbox, so neither `cargo test` nor a
browser harness was run. What was run: `rust_syntax_check.mjs` (one tree-sitter
false positive in the new test file, which real rustfmt parses and formats
cleanly), `rustfmt_check.mjs`, JSON validation of every fixture, and
`scripts/fly-check.sh gates` (20 passed, 0 failed) — the gates include the fixture
manifest checks. The new tests must be executed by CI before being called green.
