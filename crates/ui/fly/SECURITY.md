# Security model — Fly

Fly turns untrusted, author-supplied document JSON into HTML and CSS. Every guarantee below
exists because some part of that pipeline is reachable by input the operator does not control.

## Threat model

The document is **untrusted input**. It may arrive from a pasted GrapesJS project, an imported
bundle, a restored snapshot, or a lower-privileged author in a multi-tenant installation. Fly
assumes an attacker can choose any component id, attribute name, attribute value, style
declaration, URL and nesting shape.

Fly does **not** defend against a compromised host application. If the consumer renders Fly output
with its own escaping disabled, serves it on the same origin as the admin, or hands Fly a document
it then trusts for authorisation decisions, Fly cannot help.

## Guarantees

| Surface | Guarantee | Enforced by |
|---|---|---|
| HTML text | Escaped on a single pass; no lossy pre-stripping | `render.rs::push_escaped_html` |
| HTML attributes | Escaped including quotes; attribute names allow-listed | `push_escaped_attribute`, `safe_attribute_name` |
| `<style>` contents | Cannot terminate the element — `<` is CSS-escaped | `render.rs::escape_style_element_text` |
| CSS selectors built from ids | Allow-list escape, not a deny-list | `render.rs::escape_css_attribute` |
| Component and page ids | `[A-Za-z0-9_.:-]`, non-empty, ≤128 bytes | `validation.rs::validate_identifier` |
| URLs | One policy module; per-slot scheme allow-list | `safe_url.rs` |
| Decode depth | Bounded, checked iteratively (no recursion) | `codec.rs::MAXIMUM_DECODE_DEPTH` |
| Document size | Bounded node count and nesting depth | `validation.rs::ValidationLimits` |
| Undo history | Bounded by entry count *and* retained bytes | `command/model.rs::History` |
| Snapshot / bundle integrity | SHA-256, compared in constant time | `digest.rs::ContentDigest` |

## Integrity: `ProjectHash` vs `ContentDigest`

These are **not** interchangeable, and conflating them is the mistake this section exists to
prevent.

* **`ProjectHash`** is FNV-1a 64. It is a *change-detection* token for dirty flags, ETag-style
  comparison and optimistic concurrency, where both sides are trusted and the only question is
  "did this change?". It is non-cryptographic and 64-bit: an attacker who controls the payload can
  produce a collision on demand. **Never** use it to answer "is this the payload that was
  approved?".
* **`ContentDigest`** is SHA-256, serialized as `sha256:<64 hex>` and compared in constant time.
  This is the integrity primitive. `ProjectSnapshot::restore` verifies it before anything else;
  `BundleDecodePolicy::verified()` additionally requires it to be present.

`ProjectHash` is retained because its `u64` is embedded in signed inline-edit claims and in
migration baselines, so its representation cannot change without a data migration.

## Deliberate decisions that look like bugs

* **`expected_origin` defaults to `"null"`** in the browser adapter. A sandboxed `srcdoc` iframe
  genuinely has the opaque origin `null`; this is correct, not an oversight.
* **`fly` and `fly-ui` are declared but unused by `fly-leptos` / `fly-dioxus`.** This fixes the
  permitted direction of dependencies ahead of the adapters being implemented, and is enforced by
  `scripts/verify/verify-fly-dependency-boundaries.mjs`.
* **`data:` URLs are rejected for stored actions but allowed for image resources.** Action targets
  are stored and later acted upon; an inline image is only ever rendered. See the module docs in
  `safe_url.rs`.

## Known gaps

These are tracked in `docs/audits/fly-builder-engineering-audit-2026-10-02.md` and are **not**
currently mitigated:

* The browser access token is held in `localStorage`, so it is readable by any script that
  achieves execution on the admin origin. Moving it to an httpOnly cookie requires a coordinated
  server change.
* `ComponentNode` uses `#[serde(untagged)]` with an `Opaque(Value)` fallback, so a malformed
  component degrades silently into opaque JSON instead of reporting a diagnostic.
* `cargo deny` / `cargo audit` are not yet part of the Fly CI pipeline.

## Reporting

Report suspected vulnerabilities through the RusTok repository's private security advisory
process rather than a public issue.
