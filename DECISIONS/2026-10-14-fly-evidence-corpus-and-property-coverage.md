# Fly evidence corpus and property coverage (Phase 5)

- Date: 2026-10-14
- Decision status: Accepted
- Implementation status: Implemented
- Owners: `fly`, `rustok-page-builder`
- Extends: [Fly command-path performance and save-conflict state](./2026-10-14-fly-performance-selection-cache-and-save-conflicts.md)
- Supersedes: None
- Superseded by: None

## Context

The audit's evidence findings (P1-8/P1-9) were that GrapesJS compatibility rested on
one small structural baseline, the single arbitrary-document property generator only
produced text/section trees with two style properties, and several safety invariants
were covered by hand-picked examples rather than properties. This sandbox has no
browser and no way to run a real GrapesJS editor, so genuine browser captures cannot
be produced here — and must not be fabricated.

## Decision

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

## Sources of truth and ownership

- GrapesJS compatibility fixtures and manifests: `fixtures/grapesjs`.
- Property-based testing harnesses and invariants: `crates/ui/fly/tests`.

## Invariants

- Every registered fixture in `manifest.json` decodes and encodes deterministically.
- Property test generator outputs strictly enforce render safety (no script tags, no `javascript:` URIs).
- Embed iframes produced by generators adhere to the provider allow-list.

## Non-goals

- Claiming simulated browser captures as physical headless browser artifacts.
- Emulating arbitrary third-party GrapesJS custom plugins outside the core schema.

## Data, transaction, and concurrency boundary

- Fixture tests operate as read-only invariants during test verification.
- Document codecs preserve lossless round-trips across versioned serialization formats.

## Context dimensions

- Fixtures validate cross-viewport responsive styles and internationalized text payloads.

## Events and projections

- Property generators project document ASTs to HTML and CSS validation targets.

## Failure semantics

- Corrupt or non-conforming fixtures fail immediately during test harness execution.

## Migration and cutover

- Fixture schema updates require corresponding updates in `fixtures/grapesjs/manifest.json`.

## Alternatives considered

- **Mocking browser runtime in pure Node**: Rejected in favor of clear provenance tagging (`real_browser_capture: false`).
- **Single monolithic fixture file**: Rejected in favor of isolated, scenario-focused test documents.

## Verification

- `fixtures/grapesjs/manifest.json` integrity validation.
- Property-based tests covering codec round-trips and HTML injection safety.

## Consequences

- The platform maintains strict regression defenses against malformed imports and unsafe document outputs.
