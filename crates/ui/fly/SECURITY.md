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
| Inbound canvas messages | Accepted only when the sender is the canvas iframe *and* its origin equals the configured origin; an unset origin, the empty string and `*` refuse the mount | `fly-browser.js::normalizeExpectedOrigin`, `web/src/origin.rs::normalize_expected_origin` |
| Intent credentials | The same-origin session cookie only; no token is read from `localStorage` and no tenant claim is sent | `fly-browser.js::postIntent` |

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

* **`expected_origin` has no default and the adapter refuses to start without one.** A sandboxed
  `srcdoc` iframe genuinely reports the opaque origin `null`, so hosts that render one pass `"null"`
  explicitly — the Page Builder admin does it through `PAGE_BUILDER_CANVAS_EXPECTED_ORIGIN`, next to
  the `sandbox="allow-scripts"` attribute it belongs to. Silent fallback to `"null"` would have made
  "never configured" indistinguishable from "intentionally sandboxed", so the missing value is
  treated as the configuration error it is.
* **`fly` and `fly-ui` are declared but unused by `fly-leptos` / `fly-dioxus`.** This fixes the
  permitted direction of dependencies ahead of the adapters being implemented, and is enforced by
  `scripts/verify/verify-fly-dependency-boundaries.mjs`.
* **`data:` URLs are rejected for stored actions but allowed for image resources.** Action targets
  are stored and later acted upon; an inline image is only ever rendered. See the module docs in
  `safe_url.rs`.

## Known gaps

These are tracked in `docs/audits/fly-builder-engineering-audit-2026-10-02.md` and are **not**
currently mitigated:

* The admin session cookie is issued by the browser, not the server: the admin shell mirrors the
  `localStorage` session into `rustok-admin-session-v1` so SSR can revalidate it, so the token stays
  readable by any script that achieves execution on the admin origin. The Fly adapter no longer
  reads or forwards it, but the HttpOnly cookie itself is a server-side change owned by the admin
  shell (`CLEAN-060` in `docs/verification/PLATFORM_HARDENING_IMPLEMENTATION_PLAN.md`).
* The intent route authenticates cookie-borne requests without an explicit server-side CSRF
  check. Removing the bearer header also removed the property that made a cross-site forgery
  impossible, so the remaining protection is incidental: the session cookie is issued
  `SameSite=Lax`, a cross-site `<form>` cannot produce the `application/json` content type the
  route requires, and the standalone admin serves no CORS layer. These hold today but are
  properties of the surrounding host, not of the route; an explicit check belongs to the admin
  shell's session work (`CLEAN-060`).
* `ComponentNode` uses `#[serde(untagged)]` with an `Opaque(Value)` fallback, so a malformed
  component degrades silently into opaque JSON instead of reporting a diagnostic.
* `cargo deny` / `cargo audit` are not yet part of the Fly CI pipeline.

## Reporting

Report suspected vulnerabilities through the RusTok repository's private security advisory
process rather than a public issue.
