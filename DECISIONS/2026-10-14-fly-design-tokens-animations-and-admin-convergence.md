# Fly design tokens, animation actions, and admin convergence (Phase 3)

- Date: 2026-10-14
- Decision status: Accepted
- Implementation status: Implemented
- Owners: `fly`, `fly-browser`, `fly-ui`, `fly-leptos`, `fly-dioxus`, `rustok-page-builder-admin`
- Extends: [Fly rich text, embeds, action runtime, and framework adapters](./2026-10-14-fly-rich-text-embed-action-runtime-and-adapters.md)
- Supersedes: None
- Superseded by: None

## Context

After Phase 2, the remaining authoring gaps from the 2026-10-14 audit were: design
tokens/CSS variables and a global theme (rest of P0-3), an animation/transition model
(rest of P0-5), and the page-builder admin still deriving its palette/layer models
locally instead of from the shared `fly-ui` contracts (part of P0-2). Style
declarations already admitted `var(--name)` references through the sanitizer; only
the definitions, validation, and surfaces were missing.

## Decision

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

## Sources of truth and ownership

- Theme schema and design token validation: `crates/ui/fly/src/theme.rs`.
- Animation action definitions: `crates/ui/fly/src/action.rs`.
- Client-side animation executor: `crates/ui/fly/browser/fly-actions.js`.
- Shared palette and layer view-model derivations: `crates/ui/fly/ui`.
- Page builder admin integration: `crates/modules/rustok-page-builder/admin`.

## Invariants

- Token identifiers and values adhere strictly to CSS variable name and safe-value patterns.
- `:root` variable blocks emit ahead of component and page style rules.
- Animation triggers respect `prefers-reduced-motion` user preferences by instantly transitioning to final values.

## Non-goals

- Authoring custom keyframe animations or arbitrary CSS cubic-bezier timing curves in this phase.
- Replacing the entire host-specific page builder admin controller with generic UI workspaces.

## Data, transaction, and concurrency boundary

- Themes and token dictionaries are committed atomically with project metadata.
- Layer trees and palette models are immutable view projections derived from document state.

## Context dimensions

- Theme variables provide the design system foundation for responsive and dark-mode styling contexts.

## Events and projections

- Token modifications trigger reactive updates to canvas root styles.
- Flattened layer trees are projected on-demand for hierarchy inspection.

## Failure semantics

- Invalid token syntax fails document validation with `theme_token_invalid`.
- Out-of-range animation durations are clamped or rejected at command validation time.

## Migration and cutover

- Existing projects without theme definitions default to empty token maps.
- Transition from legacy local layer flattening to `fly-ui` view-models is backward-compatible.

## Alternatives considered

- **Embedding CSS `@keyframes` in component rules**: Rejected to avoid unbounded CSS generation and CSS injection vectors.
- **Immediate total replacement of page-builder admin UI**: Rejected to avoid breaking localized forms and SSR intent pathways.

## Verification

- Crate tests in `fly` for token parsing and serialization.
- `fly-actions.js` contract checks for animation dispatch and reduced motion handling.
- Verification that page builder admin consumes shared `fly-ui` layer flattening.

## Consequences

- Consistent design token management across pages and reusable components.
- Reduced duplication between framework adapters and module admin interfaces.
