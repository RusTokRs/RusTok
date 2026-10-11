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

- **The runtime-scenario release baseline hash is now a SHA-256 digest.** `digest.rs` states that
  every gate answering "is this payload the one that was approved?" uses `ContentDigest`, and names
  the runtime-scenario release baseline as one of them — but
  `RuntimeScenarioReleaseBaseline::computed_hash` still returned a `ProjectHash` (FNV-1a 64)
  fingerprint. FNV-1a's `xor`/`multiply` chain is invertible, so a payload can be constructed to
  match a chosen 64-bit value, and this hash is what `runtime_scenario_baseline_hash_invalid`,
  `RuntimeScenarioReleaseMode::BlockBroken` and the `expected_baseline_hash` compare-and-swap rest
  on. It now produces `sha256:<64 hex>`, verified with a constant-time comparison.

  `RuntimeScenarioRenderSnapshot::snapshot_hash` deliberately stays on `ProjectHash`. It is not an
  approval anchor: a snapshot is always carried inside an envelope that digests it with sha256 (the
  baseline payload above, and the materialization identity's `runtime_snapshot_hash`), and
  snapshots are persisted inside `page_static_landing_artifact` and
  `page_publish_rebuild_source`, where the rebuild path requires a rebuild to reproduce them *byte
  for byte* — changing the algorithm would fail every retained artifact with "rebuilt runtime
  evidence does not exactly reproduce retained provenance". The same reasoning kept the per-case
  `html_hash`/`css_hash`/`document_hash` on FNV-1a.

- **Baselines persisted in the old form keep working and are upgraded on first read.** The legacy
  FNV-1a fingerprint is still recognised, so no row is retroactively rejected and no publishing
  path breaks on upgrade; `rustok-pages` rewrites `baseline_hash` the first time such a row is read
  (contents untouched, compare-and-swap on the old hash, a concurrent writer's row wins and is
  returned instead). A value that matches no recorded fingerprint — an altered envelope, exactly
  what this hash exists to catch — is never repaired, and new captures never produce the legacy
  form.
- `EditorCommand::RestoreSnapshot` now restores through `restore_verified`. `restore` verifies
  the content digest only when one is present, falling back to `project_hash` — a cheap FNV-1a
  change-detection token that is trivial to forge. Nothing untrusted reaches that path today, but
  `EditorCommand` is `Deserialize`, so the first API accepting a command would have made the
  downgrade reachable. Everything `capture` produces carries a digest, so requiring one costs
  nothing.

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

- **The browser adapter's expected origin is fail-closed and has no default.** `expectedOrigin` used
  to fall back to the literal `"null"` — the opaque origin a sandboxed `srcdoc` iframe reports — so
  a host that never configured an origin silently accepted messages from the sandboxed canvas
  instead of refusing to mount. `BrowserAdapterConfig::expected_origin` is now `Option<String>`
  with no default, the shipped adapter throws `EXPECTED_ORIGIN_REQUIRED` from `start()` when
  neither `expectedOrigin` nor `data-fly-expected-origin` is set, and the empty value and the `*`
  wildcard resolve to "not configured" rather than to "accept anything" — the same rule
  `fly_web::normalize_expected_origin` applies on the Rust side. The Page Builder admin host, whose
  canvas is rendered `sandbox="allow-scripts"` with `srcdoc`, states the matching origin explicitly
  through `PAGE_BUILDER_CANVAS_EXPECTED_ORIGIN`.

- **The browser adapter no longer reads the admin token or the tenant slug from `localStorage`.**
  It used to scrape `rustok-admin-token` and `rustok-admin-tenant` and send them as
  `authorization: Bearer` and `x-tenant-slug` on every intent request, although the request already
  runs with `credentials: "same-origin"` and the admin session cookie is revalidated server-side
  before it becomes a request-scoped auth snapshot. Copying a credential into script-readable
  storage widened the XSS surface, and a client-supplied tenant was a claim the adapter had no
  business making. Requests now authenticate solely through the session cookie; `x-csrf-token` is
  still forwarded when the host configures one.

### Fixed

- **The `raw_html` block no longer advertises a capability the engine does not have.** The palette
  offered a block called "Raw html" whose content the renderer escapes and prints as text: the
  engine has no raw HTML sink at all, so a pasted `<iframe>` published as visible markup with
  nothing anywhere explaining why (functional audit `G-28`). The type stays — the codec must
  round-trip documents authored elsewhere — but its palette label now reads "Raw HTML (escaped as
  text)", its template text says the same, and `validation` reports `raw_html_not_rendered` as a
  warning for any document carrying visible content in one. A warning rather than an error, so
  documents that already use the block keep publishing.

- **`fly-web` can deliver outbound iframe messages to the sandboxed canvas.** `IframeMessagePort`
  validated its destination with the inbound rule, which accepts `"null"` — the origin a sandboxed
  `srcdoc` iframe reports — and then handed that same string to `postMessage`. The platform accepts
  only `*`, `/`, or an absolute URL as `targetOrigin`, so the call raised `SyntaxError: Invalid
  target origin`, and substituting a real origin instead makes the browser drop the message because
  it can never equal the recipient's reported origin. An opaque destination is now sent to `*`
  through `outbound_target_origin`, a host-tested decision in `origin.rs` next to the inbound rule
  it is deliberately asymmetric with; `OPAQUE_ORIGIN` names the value both rules turn on.

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
- The renderer classified only `href`, `src`, `poster`, `action` and `formaction` as URL-bearing,
  so `cite` and `usemap` reached the output with no URL check at all while static publishing
  rejected the same document. Both are now classified, with a new `Fragment` kind for `usemap`.
- Static publishing judged CSS references by a blanket `url(` ban while already permitting
  `https://` and relative URLs for image attributes, so the same image was accepted through `src`
  and refused through `background-image`. References are now validated per URL with that same
  rule. Layers still differ from each other — publishing refuses plain `http://` — but each layer
  now applies one rule to attributes and references alike.
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

- `fly-leptos` has a conventional crate root. `Cargo.toml` declares `[lib] path = "src/root.rs"`, so
  `root.rs` is the crate root; the layout-shell module it used to pull in through
  `#[path = "lib.rs"] mod foundation;` is now the ordinary `src/foundation.rs` module. Nothing
  leaves the public API: the shim `real_dom_inline.rs` duplicated names that `pub use fly_web::*`
  already exported (and needed two `#[allow(unused_imports)]` suppressions to stay quiet), and
  `#![deny(unsafe_code)]` moved to the crate root so it covers the feature-gated browser modules
  rather than only the included file.
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
- The admin session cookie is still issued by the browser rather than by the server: the admin
  shell mirrors the `localStorage` session into `rustok-admin-session-v1` so SSR can revalidate it,
  which keeps the token reachable by any script on the admin origin. Fly's adapter no longer reads
  or forwards it, but server-issued HttpOnly sessions — and the explicit server-side CSRF check
  that the cookie-only transport now leans on — are the admin shell's change, tracked as
  `CLEAN-060` in `docs/verification/PLATFORM_HARDENING_IMPLEMENTATION_PLAN.md`.
- 48 glob re-exports (`pub use module::*`) remain.
- The provider capability catalog is a host contract nobody fills yet: `InteractionCapabilityRegistry`
  and `evaluate_runtime_publish_gate_with_capabilities` are implemented and tested, the default
  `InteractionCapabilityPolicy` is `Allow` for actions and forms, and no crate in this repository
  registers a capability or calls the capability-aware gate. See the *Known Limitations* section of
  `README.md`.
