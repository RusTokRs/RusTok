# Fly style states, class rules, unique ids, and head metadata

- Date: 2026-10-14
- Decision status: Accepted
- Implementation status: Implemented in this change (core + publish policy + admin panel);
  Rust toolchain verification is delegated to CI because the authoring sandbox has no
  `cargo`/`rustc` (see Verification).
- Owners: `fly` (style-rule scope, rendering, id generation, page head), `fly-ui`
  (presentation descriptors), `rustok-page-builder` (static publish policy, admin surface)
- Extends: [Fly page-builder architecture](./2026-07-13-fly-page-builder-architecture.md),
  [Fly SSR-first browser runtime](./2026-07-16-fly-ssr-first-browser-runtime.md)
- Supersedes: None
- Superseded by: None

## Context

The deep engineering audit of the Fly page builder (`docs/audits/`, 2026-10-14) found the
authoring core narrower than every comparable builder (GrapesJS, Webflow, Framer,
WordPress, Builder.io):

- `StyleRuleScope` knew only `Base` and a free-form media query: no pseudo-class states
  (`:hover`, `:focus`, ...), so a button could not get a hover style without raw CSS.
- Style rules without a component binding (reusable GrapesJS class rules) were parsed and
  then silently dropped by the renderer.
- `FlyEditor` minted ids with a per-session sequential generator, so two authors editing
  copies of one project concurrently produced identical ids for different components.
- `PageHead` emitted only title/description/canonical/robots/Open Graph basics: no Twitter
  cards, no `og:type`, no JSON-LD, no `hreflang` alternates.

## Decision

1. **Pseudo-class states are a closed allow-list, owned once by `fly`.**
   `fly::SUPPORTED_PSEUDO_CLASSES` plus `normalize_pseudo_class` are the single gate.
   `StyleRuleScope` gains `State { pseudo }` and `MediaState { query, pseudo }`, mapped to
   and from the GrapesJS rule shape (`state`, plus `atRuleType`/`mediaText` for the media
   combination) so documents round-trip through GrapesJS unchanged. Unsupported states are
   preserved verbatim for round-tripping, reported by validation as
   `unsupported_pseudo_class`, refused by the static publish policy, and never interpolated
   into a rendered selector. Functional pseudo-classes (`:nth-child(2)`, `:not(...)`) and
   pseudo-elements are deliberately not representable.
2. **Class rules render as class selectors, strictly validated.** In the Fly runtime
   renderer an unbound rule renders only when every selector name is a plain class name
   (`[A-Za-z0-9_-]+`, no leading digit); anything else is skipped, never escaped into a
   selector. The static landing artifact contract stays component-bound by design and now
   states that rationale explicitly instead of claiming the renderer would omit such rules.
3. **`fly-ui` only labels what the core allows.** `builtin_style_states()` filters its
   editor descriptors through `fly::is_supported_pseudo_class`, and the Page Builder admin
   gains an "Interaction states" panel next to the responsive panel that issues the same
   `StyleRuleCommand::UpsertComponentRule` under a `State` scope.
4. **Editor-minted ids are collision-resistant across sessions.** `UniqueIdGenerator`
   (nanosecond timestamp + process id + process-wide instance salt + per-instance counter)
   is the `FlyEditor` default and keeps ids inside the documented identifier charset.
   `SequentialIdGenerator` remains for deterministic contexts that ask for it explicitly
   (fixtures, static landing materialization). Ids are not credentials.
5. **Head metadata is completed, still fail-closed.** `PageMetadata`/`PageHead` carry
   `open_graph_type`, Twitter card fields (with OG/title fallback), JSON-LD
   `structured_data` (rendered with `<`, `>`, `&` unicode-escaped so a payload can never
   terminate the script element), and consumer-supplied `hreflang` alternates that are
   URL- and locale-validated at attachment time. Fly renders; the consumer that owns
   localized routing supplies the alternates.

## Alternatives considered

- **Free-form state strings** (what GrapesJS accepts): rejected, the state is interpolated
  into a CSS selector and would become a selector-injection sink.
- **Refactoring `StyleRuleScope` into independent media/state fields**: rejected for this
  change; the four-variant enum keeps every existing exhaustive match site explicit, and
  the GrapesJS mapping stays one function (`scope_from_rule_object`).
- **ULID/UUID dependency for ids**: rejected; the timestamp+salt+counter construction
  needs no new dependency and stays sortable-by-time within a session.

## Consequences

- Existing documents are unaffected: new metadata fields are additive and skip-serialized
  when empty; state rules appear only when authored.
- Any future layer that wants to accept or reject a pseudo-class must call
  `fly::normalize_pseudo_class`; duplicating the list is a contract violation.
- Deferred and still open (tracked in `crates/ui/fly/README.md` Known Limitations and the
  audit document): framework adapters beyond layout shells, rich-text payload wiring,
  embed/custom-code blocks, a runtime for `data-fly-action`, collaboration/presence,
  design tokens and global theme, and the admin shell's server-issued HttpOnly session
  (`CLEAN-060`).

## Verification

- Unit tests added beside each change (style-rule round-trips, renderer pseudo/class
  emission and injection refusal, head emission and JSON-LD breakout, id uniqueness and
  charset, `fly-ui` allow-list drift guard).
- `scripts/fly-check.sh gates` must stay green; the static publish policy gained the
  mirroring arms for the new scopes in the same change.
- Sandbox limitation: no Rust toolchain is available in the authoring environment, so
  `cargo test -p fly -p fly-ui -p rustok-page-builder`, clippy, and `cargo bench` are CI /
  maintainer steps for this change; changed files were parsed with the repository's
  tree-sitter check and formatted with the wasm rustfmt check under `scripts/audit/`.
