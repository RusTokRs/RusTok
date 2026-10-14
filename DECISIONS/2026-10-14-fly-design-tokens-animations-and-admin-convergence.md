# Fly design tokens, animation actions, and admin convergence (Phase 3)

Date: 2026-10-14
Status: accepted
Scope: `fly` (theme, action model), `fly-browser` (action runtime), `fly-ui`
(editor view-models), `fly-leptos` / `fly-dioxus` (workspace token display), and
`rustok-page-builder-admin` (controller derivations).

## Context

After Phase 2, the remaining authoring gaps from the 2026-10-14 audit were: design
tokens/CSS variables and a global theme (rest of P0-3), an animation/transition model
(rest of P0-5), and the page-builder admin still deriving its palette/layer models
locally instead of from the shared `fly-ui` contracts (part of P0-2). Style
declarations already admitted `var(--name)` references through the sanitizer; only
the definitions, validation, and surfaces were missing.

## Decisions

1. **Design tokens are project data owned by `fly::theme`.** A project carries a
   `flyTheme` extension (`{ "tokens": { "brand": "#0b5fff" } }`), which round-trips
   through the codec's flattened extension map with no format migration. Token names
   become `--{name}` custom properties; the name charset (lowercase, digits, single
   hyphens) and the value subset (no structural breakout, no `url(`/`expression`/
   `javascript:` constructs; `var(--other)` chaining allowed) are validated in exactly
   one place. The renderer emits a single `:root { ... }` rule before the project
   styles and fails closed on a malformed theme; validation reports
   `theme_token_invalid`. Declaration-level `var(--brand)` references need no new
   syntax and pass the existing declaration sanitizer.
2. **Animations are a closed action kind, not author CSS.** `ComponentAction::Animate`
   carries a target id, an `AnimationKind` from a closed enum (`fade_in`, `fade_out`,
   `slide_up`, `slide_down`, `zoom_in`, `pulse`), and an optional duration bounded to
   50..=10000 ms at validation. Materialization reuses the server-owned
   `data-fly-action` contract; `fly-actions.js` maps kind names to fixed Web
   Animations keyframes — payloads can name an animation but never supply CSS — and
   honours `prefers-reduced-motion` by applying only the end state. The admin action
   form covers the new kind. Tabs/accordion/slider remain unbuilt interaction
   primitives and stay recorded as such.
3. **Admin convergence proceeds by shared derivations first.** `fly-ui` now owns the
   layer flattening (with parent id and sibling index, plus a single-page variant)
   and the palette listing (items for adapters, templates for inserting hosts). The
   page-builder admin controller consumes both and its local `collect_layers` /
   palette mapping copies are deleted. The admin keeps its own controller, intent
   protocol, i18n, and contribution access checks — those are host concerns — while
   every derivation that the adapters also need has exactly one owner. Replacing the
   admin's panels with the adapter `FlyEditorWorkspace` wholesale is *not* done: the
   workspace does not yet cover contribution access, SSR intent dispatch, or the
   localized panels, and a blind swap would delete host behaviour. That migration
   remains open and tracked under P0-2.

## Verification

As in Phases 1–2, this sandbox has no Rust toolchain, so `cargo test` / `clippy`
were not run. What was run: `rust_syntax_check.mjs` and `rustfmt_check.mjs` over the
changed Rust files (all conform), `node --check` for `fly-actions.js`, a token-level
simulation of the contract-test assertions against the shipped JS, and
`scripts/fly-check.sh gates` (20 passed, 0 failed). The new Rust test suites
(theme decode/render, animation validation, `fly-ui` page layers) still need a real
`cargo test` run by CI or a maintainer and must not be reported as green until then.
