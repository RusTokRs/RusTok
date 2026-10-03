# Changelog

All notable changes to the Fly page builder crates (`fly`, `fly-ui`, `fly-web`, `fly-browser`,
`fly-leptos`, `fly-dioxus`).

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Fly is pre-1.0 and
versioned with the host workspace, so until it is extracted (see the `rustok-ui-i18n` blocker in
`standalone-Cargo.toml`) entries are grouped by date rather than by released version.

## [Unreleased]

Engineering audit remediation — see
`docs/audits/fly-builder-engineering-audit-2026-10-02.md` for the full findings register.

### Security

- **Inline edit contract hardened.** The authorization proof was compared with `!=` inside an
  `||` chain, leaking timing both per byte and by which field mismatched; it is now compared with
  `constant_time_eq`, separately from the public fields. `validate_request` normalized the value
  into a discarded copy, so a request deserialized straight from JSON could pass validation and
  carry a carriage return into stored content; a value that is not already in normal form is now
  rejected. Plain text rejected only NUL, letting every other C0/C1 control and the
  bidirectional override characters (`U+202E` and friends) through — these survive HTML escaping
  and can make rendered text read differently from what was stored and reviewed.

- **Opaque components no longer bypass validation.** `ComponentNode::visit` stopped at every
  `Opaque` node, so a subtree the typed model could not parse counted toward neither
  `maximum_nodes` nor `maximum_depth`, and the ids inside it were checked neither for duplication
  nor for a valid character set. Validation now descends into opaque subtrees — and into opaque
  *child lists*, a second blind spot where `children()` reported them as empty. `Opaque` itself is
  retained: lossless round-tripping of unknown providers is the point of the codec.
- **Stylesheet and attribute escaping rewritten.** Component ids reached `<style>` selectors
  unescaped; `escape_css_attribute` now works from an allow-list and `escape_style_element_text`
  prevents element breakout.
- **URL policy consolidated.** Five diverging implementations (two of them byte-for-byte copies
  differing only in an infix) replaced by `safe_url`, with `UrlAttributeKind` and `UrlPolicy`.
- **Same-origin enforcement in the browser bridge.** `fly-browser.js` validates intent endpoints
  and response-driven navigation against the document origin.
- **Decode depth is bounded** (`MAXIMUM_DECODE_DEPTH`), checked iteratively so that a hostile
  document cannot overflow the stack during the check itself.
- **Identifier charset enforced**: non-empty, ≤128 bytes, `[A-Za-z0-9_.:-]`.
- **Snapshot integrity** via `ContentDigest` (SHA-256, constant-time comparison) and
  `restore_verified`.
- `#![forbid(unsafe_code)]` in `fly`, `fly-ui` and `fly-browser`; `#![deny(unsafe_code)]` in the
  crates whose framework macros emit their own `allow` attributes.
- Added `SECURITY.md` describing the trust boundaries and the threat model.

### Fixed

- **`fly-web` geometry rejects non-finite input.** Browser geometry crosses the iframe bridge as
  JSON, where an out-of-range literal deserializes to an infinity rather than an error. An
  infinite rectangle contained every point, passed the size guard, and then yielded a NaN ratio
  that made `resolve_drop_position` answer `After` for a rectangle that does not exist; a single
  non-finite transform offset turned every converted coordinate into NaN; and candidate ranking
  used `partial_cmp(..).unwrap_or(Equal)`, a comparator that is not a total order and that
  `slice::sort_by` may panic on. Ranking now uses `total_cmp`, unusable scores sink to
  `f32::MIN`, and unreal rectangles are dropped at the boundary.

- **Nested repeaters work.** They previously failed in *both* processing orders: expanding the
  outer repeater first renamed the inner repeater's target out of existence, and expanding the
  inner one first evaluated it against the global context so every outer copy received identical
  data. Replaced by a single depth-first expansion in which each clone expands its own nested
  repeaters against its own local context.
- **Conditions inside repeater templates.** A condition on `item.*` was evaluated once in a global
  pre-pass where the path does not resolve, so the component was judged falsy and deleted from the
  template for every item. Such conditions are now deferred to each item's local context.
- `safe_style` rejected every `url(`, which made `background-image` unusable. Safe references are
  allowed, validated against the same resource policy as `src`; `behavior`, `-moz-binding`,
  `-ms-behavior` and `expression` are rejected by property name.
- An empty `EditorCommand::Batch` computed an empty capability requirement, and the state
  machine skips its entire guarded branch when nothing is required — so the batch bypassed the
  read-only check and still reached `FlyEditor::apply`, which records history and marks the
  revision changed. An empty batch now requires `Edit`, failing closed.
- `validate_origin` tested `origin.trim()` but returned the untrimmed string, so an expected
  origin configured with a stray space passed validation and then never matched the canonical
  `MessageEvent::origin()` — the iframe bridge silently discarded every inbound message. The rule
  now lives in `web/src/origin.rs`, outside the `cfg(wasm32)` gate that had made this pure string
  logic impossible to test.
- Context schema validation rejected only exact duplicate field paths, so declaring both `user`
  and `user.name` validated clean and then produced a JSON Schema that depended on declaration
  order: one order yields a string carrying `properties` (which validators ignore), the other
  silently overwrites the nested field. Ancestor/descendant conflicts are now reported as
  `runtime_context_field_path_conflict`, compared by path segment so `user` and `username` stay
  unrelated.
- Four separate copies of id-reference substitution consolidated into `id_reference`, which
  rewrites only declared reference positions instead of any string that happens to match an id.

### Added

- `MAX_REPEATER_DEPTH` (8) and `MAX_TOTAL_REPEATED_NODES` (50 000) with
  `runtime_repeater_depth_exceeded` / `runtime_repeater_budget_exceeded` diagnostics.
- `ComponentIndex`: one traversal answering id lookups that previously walked every page per
  query.
- `StyleRuleIdentity`: reads a rule's identity without copying its declarations or raw JSON, for
  the predicate call sites that discarded the full descriptor immediately.
- History memory budget (64 MiB, `with_history_memory_budget`).
- Property-based tests covering escaping, codec round-trips, identifier/CSS-escape agreement and
  node-count completeness.
- `LocaleResolver` trait with a CLDR-backed `PlatformLocaleResolver` (feature `platform-i18n`,
  on by default) and a self-contained `BasicLocaleResolver`. This inverts the last dependency
  blocking extraction into a standalone repository.
- Shared `[workspace.lints]` policy, mirrored into `standalone-Cargo.toml` and checked for drift.
- CI: wasm32 jobs, adapter feature-combination checks, a dependency advisory job, and
  `verify-fly-gates-are-wired.mjs`, which failed loudly on the 7 verification gates that no
  workflow ran — all 4 rotted gates were among those 7.

### Changed

- `FlyEditor::apply` performs one deep document copy per command instead of three, and `undo`/
  `redo` perform none, by sharing documents between the editor and adjacent history entries via
  `Arc`. History memory roughly halves. The public API is unchanged.
- `maximum_nodes` / `maximum_depth` defaults named and justified (`DEFAULT_MAXIMUM_NODES`,
  `DEFAULT_MAXIMUM_DEPTH`).
- Command history uses `VecDeque`.

### Known gaps

- `HistoryEntry` still stores whole documents rather than inverse commands, so
  `approximate_bytes` must serialize to measure, and it over-estimates by ~2x now that adjacent
  entries share allocations (which keeps the budget conservative).
- A standalone build (`--no-default-features`) loses CLDR likely-subtag inference: `zh-TW` falls
  back to `zh` rather than `zh-Hant`, and `iw` is not canonicalized to `he`.
- The admin access token is still read from `localStorage`; moving it to an httpOnly cookie
  requires a coordinated server change.
- 48 glob re-exports (`pub use module::*`) remain.
