---
id: doc://docs/security/csp-report-only-inventory.md
kind: security_control_inventory
language: markdown
source_language: markdown
status: active
---
# CSP Report-Only Migration Inventory

## Purpose

This inventory defines the target browser policy, the report collection boundary and the evidence required before the UI CSP can remove the remaining inline-style-attribute allowance from enforcement. Inline scripts and style elements already require a per-response nonce, inline event handlers are blocked, `unsafe-eval` is prohibited and production connections are HTTPS/WSS-only.

No violation in this document is an automatic allowlist request. The preferred resolution is to remove the dependency, move code into a same-origin static asset, attach a per-response nonce/hash to a trusted element, or replace a style attribute with a reviewed CSS class, native element attribute or SVG geometry contract.

## Collection Contract

- Endpoint: `POST /api/security/csp-report` on the main RusToK server host.
- Accepted formats: legacy `application/csp-report`, Reporting API `application/reports+json`, and JSON-compatible test traffic.
- Maximum request body: 64 KiB.
- Maximum processed Reporting API entries: 20 per request.
- The outer security middleware handles the endpoint before tenant and authentication routing.
- Responses contain no report body and return `204` for accepted reports.
- Invalid content types, JSON and report shapes are rejected with bounded status codes.
- The standalone admin process does not advertise a report endpoint it cannot receive; it emits enforced CSP only.

The collector never records `script-sample`, full document paths, URL queries or URL fragments. Structured logs retain only normalized origins or fixed values such as `inline`, `eval`, `data` and `blob`.

## Telemetry Contract

Accepted reports emit structured events with target `rustok.security.csp` and bounded fields:

| Field | Values |
|---|---|
| `report_format` | `legacy`, `reporting_api` |
| `directive` | `script-src`, `style-src`, `connect-src`, `img-src`, `font-src`, `worker-src`, `frame-src`, `frame-ancestors`, `object-src`, `base-uri`, `form-action`, `default-src`, `other` |
| `disposition` | Browser-provided report/enforce disposition |
| locations | Origin only, fixed keyword, scheme-only value or `opaque` |
| source position | Optional line and column |
| status | Optional document response status |

The existing Prometheus family `rustok_module_errors_total` records the same bounded directive or rejection reason with `module="security"` and `severity="warning"`.

## Target Policy Inventory

| Directive | Target sources | Decision |
|---|---|---|
| `default-src` | `'self'` | Baseline deny for unspecified resource classes |
| `script-src` | `'self' 'nonce-<per-response>'` | Only same-origin external scripts and explicitly trusted nonce-bearing inline scripts; inline event handlers and eval are forbidden |
| `script-src-attr` | `'none'` | Inline event-handler attributes are forbidden |
| `style-src` | `'self' 'nonce-<per-response>'` | Only same-origin stylesheets and explicitly trusted nonce-bearing style elements |
| `style-src-attr` | `'none'` | Target state forbids inline style attributes; migrate them to reviewed classes, native attributes or SVG geometry |
| `img-src` | `'self' data: blob: https:` | Remote images remain HTTPS-only |
| `font-src` | `'self' data:` | No remote font origin is currently approved |
| `connect-src` | `'self' https: wss:` | Production permits only secure HTTP and WebSocket connections |
| `worker-src` | `'self' blob:` | Blob workers are retained for current browser runtime support |
| `object-src` | `'none'` | Plugins and embedded object content forbidden |
| `frame-ancestors` | `'none'` | Embedding forbidden |
| `base-uri` | `'self'` | Base URL rewriting restricted |
| `form-action` | `'self'` | Form submission restricted to same origin |

## Trusted Script and Style Element Nonce Boundary

- `rustok-web::CspNonce` creates one UUIDv4-derived nonce per UI response.
- The outer main-server security middleware inserts the nonce into request extensions and uses the same value in enforced and report-only headers.
- Embedded admin processing applies the nonce only to script and style elements in the immutable bundled `index.html`; it is never applied to tenant or user-authored HTML.
- Storefront processing applies the nonce only to the exact JSON-LD opening tag emitted by the typed SEO renderer.
- The standalone admin middleware inserts the same nonce type into Axum request extensions, copies it into the Leptos render context and applies it to the transitional auth bootstrap script.
- The classic standalone admin shell intentionally contains no `HydrationScripts`, `AutoReload` or inline style element producer.
- Missing nonce state fails closed to the API deny policy rather than restoring a blanket `unsafe-inline` source.

## Connection Profile Boundary

- Production environments (`RUSTOK_ENV`, `RUST_ENV` or `APP_ENV` set to `prod`/`production`) use `'self' https: wss:` on both server-hosted and standalone admin surfaces.
- Non-production profiles may additionally use `ws:` for local development.
- Plaintext `http:` is absent from every UI policy.
- Both production hosts reject startup without an explicit `RUSTOK_HTTPS` declaration and emit HSTS when it is present.
- The static CSP gate verifies that the secure source sets cannot regain `ws:`.

## Current Migration Debt

The enforced UI policies now isolate their remaining exception to:

- `style-src-attr 'unsafe-inline'`.

The broader `style-src 'unsafe-inline'` source, `script-src 'unsafe-inline'`, `unsafe-eval`, plaintext HTTP and production plaintext WebSocket have been removed from enforcement and are protected by the CSP verification gate. The remaining attribute-level entry is migration debt, not an approved permanent production exception. The strict main-server report-only policy uses `style-src-attr 'none'` to expose the affected components.

The machine-readable register is `docs/security/csp-inline-style-attribute-exceptions.json`. It now records exactly **2 source sites across 2 Rust-hosted UI files**, down from 15 sites across 7 files, and must be reviewed no later than **2026-08-14**. The verification gate fails on an unregistered file, a changed occurrence count, missing constraint evidence, stale entries, an expired review date or an increase above the 2-site/2-file ratchet.

| Source | Sites | Reviewed constraint | Exit path |
|---|---:|---|---|
| `apps/admin/src/features/modules/components/modules_list.rs` | 1 | Percent width is formatted from typed `BuildJob.progress: i32`, never from a CSS string | Move to a native progress or finite class contract |
| `crates/rustok-forum/admin/src/ui/leptos.rs` | 1 | The entity hook rejects invalid colors before persistence, and the admin JSON boundary normalizes valid hex tokens again | Reuse the finite storefront palette class |

Completed attribute migrations in this batch:

- the unreferenced legacy `editor/admin_canvas.rs` duplicate was removed after confirming it had no `mod`, `#[path]` or source reference;
- modular layer indentation now uses a bounded nine-step Tailwind class scale and caps deeper trees at the final class;
- hover, selection and insertion overlays now use SVG `x`, `y`, `width` and `height` attributes;
- resize preview geometry now uses an SVG `<rect>`;
- the eight resize handles now use SVG `<circle>` positions and a closed cursor-class mapping while retaining pointer capture;
- storefront forum accents now map validated colors to a finite eight-color utility-class palette or reviewed gradient fallback and no longer emit a runtime CSS declaration;
- Page Builder custom viewport width, height and continuous zoom now use an SVG `viewBox` plus an explicit XHTML `<foreignObject>` integration point; the iframe retains native width/height attributes and no longer requires CSS sizing or `transform:scale`.

The static modular Page Builder three-column layout had already moved from an inline attribute to a Tailwind arbitrary grid class and remains outside the exception register.

Forum category accents previously accepted an arbitrary persisted CSS fragment. The SeaORM `before_save` hook rejects any non-hex category color before insert or update. `rustok-ui-core::normalize_css_hex_color` and both Rust UI transport models independently retain the same strict `#RGB`, `#RGBA`, `#RRGGBB` or `#RRGGBBAA` grammar. `rustok-ui-core::css_hex_accent_class` then converts valid tokens into one of the reviewed `rose`, `amber`, `emerald`, `cyan`, `sky`, `violet`, `fuchsia` or `slate` classes; invalid and absent values use the fixed sky-to-amber gradient. The storefront view model exposes only the selected `'static` class. The remaining admin attribute must migrate to the same policy.

## Triage Rules

1. Group reports by normalized directive and origin.
2. Reproduce each unique violation in embedded admin, standalone admin and storefront browser smoke tests.
3. Classify it as application code, framework bootstrap, third-party dependency or malicious/noise traffic.
4. Replace each required style attribute with a reviewed class, native attribute, SVG geometry contract or another non-inline representation.
5. Remove or replace a source before considering an allowlist.
6. Any new external origin requires a security review, named owner, exact resource purpose and expiry/review date.
7. Never allowlist `unsafe-eval`; replace the dependency or execution path.
8. Never copy a full reported URL, query, fragment or script sample into issues or logs.
9. Never add a nonce through blanket post-processing of tenant or user-authored HTML.
10. Do not advertise a report endpoint from a deployment process that does not own the bounded collector.
11. Do not add a new Rust-hosted `style=` source site without updating the register with a narrow grammar, owner and removal plan.

## Enforcement Exit Criteria

The enforced policy may be promoted to the strict target only when:

- browser smoke runs for embedded admin, standalone admin and storefront produce no unexplained `style-src-attr` violations;
- every registered inline style attribute has moved to a reviewed class or another non-inline contract;
- the exception register is empty and the gate observes zero Rust-hosted source sites;
- no production code path requires `eval` or equivalent string compilation;
- the observed external-origin set matches this inventory;
- the CSP reporting endpoint remains bounded and unauthenticated without inheriting tenant context;
- rollback instructions retain the last known safe policy without restoring inline scripts, blanket inline style elements or plaintext connection sources.

## Verification

```bash
cargo test -p rustok-ui-core
cargo test -p rustok-forum
cargo test -p rustok-forum-admin
cargo test -p rustok-forum-storefront
cargo test -p rustok-page-builder-admin
cargo test -p rustok-admin --features ssr app::security
cargo test -p rustok-admin --features ssr app::auth_ssr
cargo test -p rustok-storefront --features ssr
cargo test -p rustok-server services::app_router
cargo test -p rustok-server middleware::csp_reports
cargo test -p rustok-server middleware::security_headers
node scripts/verify/verify-csp-reporting-contract.mjs
node scripts/verify/verify-csp-inline-style-exceptions.mjs
```
