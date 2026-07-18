---
id: doc://docs/security/csp-report-only-inventory.md
kind: security_control_inventory
language: markdown
source_language: markdown
status: active
---
# CSP Report-Only Migration Inventory

## Purpose

This inventory defines the target browser policy, the report collection boundary and the evidence required before the UI CSP removes its remaining enforced inline-style-attribute allowance. Inline scripts and trusted style elements already require a per-response nonce, inline event handlers are blocked, `unsafe-eval` is prohibited and production connections are HTTPS/WSS-only.

No violation in this document is an automatic allowlist request. The preferred resolution is to remove the dependency, move code into a same-origin static asset, attach a per-response nonce/hash to a trusted element, or replace a style attribute with a reviewed CSS class, native element attribute, SVG geometry contract or bounded DOM adapter.

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

The Prometheus family `rustok_module_errors_total` records the same bounded directive or rejection reason with `module="security"` and `severity="warning"`.

## Target Policy Inventory

| Directive | Target sources | Decision |
|---|---|---|
| `default-src` | `'self'` | Baseline deny for unspecified resource classes |
| `script-src` | `'self' 'nonce-<per-response>'` | Only same-origin external scripts and explicitly trusted nonce-bearing inline scripts; inline event handlers and eval are forbidden |
| `script-src-attr` | `'none'` | Inline event-handler attributes are forbidden |
| `style-src` | `'self' 'nonce-<per-response>'` | Only same-origin stylesheets and explicitly trusted nonce-bearing style elements |
| `style-src-attr` | `'none'` | Inline style attributes are forbidden |
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
- Next source contains no runtime `<style>` element under the reviewed source roots; the runtime-style-element ratchet is zero.

## Connection Profile Boundary

- Production environments (`RUSTOK_ENV`, `RUST_ENV` or `APP_ENV` set to `prod`/`production`) use `'self' https: wss:` on both server-hosted and standalone admin surfaces.
- Non-production profiles may additionally use `ws:` for local development.
- Plaintext `http:` is absent from every UI policy.
- Both production hosts reject startup without an explicit `RUSTOK_HTTPS` declaration and emit HSTS when it is present.
- The static CSP gate verifies that the secure source sets cannot regain `ws:`.

## Current Migration State

The default enforced UI policies still retain:

- `style-src-attr 'unsafe-inline'`.

This is now a rollout guard rather than source-level migration debt. The strict main-server report-only policy already uses `style-src-attr 'none'`. Source promotion prerequisites are complete, but default enforced promotion remains blocked until cross-stack browser smoke runs show no unexplained `style-src-attr` or `style-src` violations.

### Rust-hosted boundary

`docs/security/csp-inline-style-attribute-exceptions.json` is empty. Its gate observes exactly **0 Rust-hosted style attributes across 0 files** with a non-increasing `0/0` ratchet.

### Next/React boundary

`docs/security/csp-next-style-exceptions.json` is empty. Its gate observes exactly:

- **0 JSX style props across 0 registered files**;
- **0 runtime style elements**;
- **0 direct DOM style writes**.

The gate scans every `.tsx` and `.jsx` file under `apps/next-admin` and `apps/next-frontend`, rejects unregistered source sites and protects all completed migrations with explicit required/forbidden markers.

### Classic admin boundary

The bundled classic admin bootstrap no longer writes `document.documentElement.style`. It toggles only the `dark` class, while `apps/admin/input.css` owns `color-scheme: light` and `color-scheme: dark`; the document declares `<meta name="color-scheme" content="light dark">`.

## Strict Enforcement Smoke Profile

Both the main server host and the standalone admin host support the same opt-in environment flag:

```bash
export RUSTOK_CSP_STRICT_STYLE_ATTRIBUTES=true
```

Truthy values use the existing normalized flag grammar: `1`, `true`, `yes` or `on`. With the flag enabled, UI responses use the strict static template containing `style-src-attr 'none'`. Without the flag, the default enforced template retains `'unsafe-inline'`; API paths remain on the scriptless deny policy in either mode.

The flag is intended for browser smoke and staged rollout evidence. It does not change the report-only policy, nonce handling, production connection profile or tenant/auth routing. Global default promotion should occur only after the strict profile succeeds across embedded admin, standalone admin, Next admin and storefront journeys.

### Required smoke evidence

For every reviewed surface, retain the response CSP header, the browser console output and the bounded collector output. A successful run has no unexplained `style-src`, `style-src-elem` or `style-src-attr` violation and no visual or interaction regression.

| Surface | Minimum journeys |
|---|---|
| Embedded admin | authentication bootstrap, dashboard shell, sidebar/infobar, tables, charts, forms, module build progress |
| Standalone admin | authentication bootstrap, dashboard shell, dark/light theme, tables, charts, forms |
| Next admin | dashboard, overview charts, data tables, progress/toasts, forms, responsive sidebar |
| Storefront | home/module route, JSON-LD page, search suggestions, presets, filters and results |
| Page Builder | canvas, viewport size/zoom, overlays, selection, resize handles and nested layers |

Rollback for smoke environments is limited to unsetting `RUSTOK_CSP_STRICT_STYLE_ATTRIBUTES`; no source exception or broader CSP source should be restored.

## Completed Attribute and Runtime-Style Migrations

- the unreferenced legacy Page Builder `editor/admin_canvas.rs` duplicate was removed;
- modular Page Builder layer indentation uses a bounded class scale;
- Page Builder overlays and resize controls use SVG geometry attributes;
- Page Builder custom viewport dimensions and continuous zoom use SVG `viewBox`/`foreignObject` geometry with native iframe dimensions;
- storefront and forum-admin category accents map validated colors to a finite utility-class palette;
- the admin module build indicator uses native `<progress max="100">` with a clamped numeric value;
- the classic admin theme bootstrap uses classes and stylesheet-owned color-scheme declarations;
- Next textarea resize modes map a closed union to finite classes;
- Next overview skeleton heights are deterministic classes;
- the Sonner adapter exposes no `style` prop and uses static token classes;
- shared Next progress uses SVG width geometry;
- the data-table skeleton removed its unused free-form width API;
- Next infobar and sidebar fixed widths, transition state and skeleton widths use classes and data attributes;
- the Next chart adapter uses a finite indicator palette, contains no runtime `<style>` element and no JSX style props;
- the unused data-table pinning style adapter and helper were removed after confirming there was no pinning consumer;
- the Next storefront search surface moved 43 presentation declarations to utility classes with explicit active/error variants.

## Triage Rules

1. Group reports by normalized directive and origin.
2. Reproduce each unique violation in embedded admin, standalone admin, Next admin and storefront browser smoke tests.
3. Classify it as application code, framework bootstrap, third-party dependency or malicious/noise traffic.
4. Remove or replace a source before considering an allowlist.
5. Any new external origin requires a security review, named owner, exact resource purpose and expiry/review date.
6. Never allowlist `unsafe-eval`; replace the dependency or execution path.
7. Never copy a full reported URL, query, fragment or script sample into issues or logs.
8. Never add a nonce through blanket post-processing of tenant or user-authored HTML.
9. Do not advertise a report endpoint from a deployment process that does not own the bounded collector.
10. Do not add a new Rust-hosted `style=` source site; its ratchet is zero.
11. Do not add a Next JSX style prop, runtime style element or direct DOM style write; all three ratchets are zero.

## Enforcement Exit Criteria

The default enforced policy may be promoted to `style-src-attr 'none'` only when:

- browser smoke runs with `RUSTOK_CSP_STRICT_STYLE_ATTRIBUTES=true` for embedded admin, standalone admin, Next admin and storefront produce no unexplained `style-src-attr` or `style-src` violations;
- both exception registers remain empty;
- both source gates observe zero style attributes/props and the Next gate observes zero runtime style elements and zero DOM style writes;
- no production code path requires `eval` or equivalent string compilation;
- the observed external-origin set matches this inventory;
- the CSP reporting endpoint remains bounded and unauthenticated without inheriting tenant context;
- rollback instructions retain the last known safe policy without restoring inline scripts, blanket inline style elements or plaintext connection sources.

## Verification

```bash
node scripts/verify/verify-csp-reporting-contract.mjs
node scripts/verify/verify-csp-inline-style-exceptions.mjs
node scripts/verify/verify-csp-next-style-boundary.mjs

export RUSTOK_CSP_STRICT_STYLE_ATTRIBUTES=true
# Start the normal main-server and standalone-admin smoke environments,
# then exercise embedded admin, standalone admin, Next admin and storefront journeys.

cargo test -p rustok-ui-core
cargo test -p rustok-forum-admin
cargo test -p rustok-forum-storefront
cargo test -p rustok-page-builder-admin
cargo test -p rustok-admin --features ssr app::security
cargo test -p rustok-admin --features ssr app::auth_ssr
cargo test -p rustok-storefront --features ssr
cargo test -p rustok-server services::app_router
cargo test -p rustok-server middleware::csp_reports
cargo test -p rustok-server middleware::security_headers
```
