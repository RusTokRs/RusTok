---
id: doc://docs/security/csp-report-only-inventory.md
kind: security_control_inventory
language: markdown
source_language: markdown
status: active
---
# CSP Report-Only Migration Inventory

## Purpose

This inventory defines the target browser policy, the report collection boundary and the evidence required before the UI CSP can remove the remaining inline-style-attribute allowance from enforcement. Inline scripts and trusted style elements already require a per-response nonce, inline event handlers are blocked, `unsafe-eval` is prohibited and production connections are HTTPS/WSS-only.

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

The existing Prometheus family `rustok_module_errors_total` records the same bounded directive or rejection reason with `module="security"` and `severity="warning"`.

## Target Policy Inventory

| Directive | Target sources | Decision |
|---|---|---|
| `default-src` | `'self'` | Baseline deny for unspecified resource classes |
| `script-src` | `'self' 'nonce-<per-response>'` | Only same-origin external scripts and explicitly trusted nonce-bearing inline scripts; inline event handlers and eval are forbidden |
| `script-src-attr` | `'none'` | Inline event-handler attributes are forbidden |
| `style-src` | `'self' 'nonce-<per-response>'` | Only same-origin stylesheets and explicitly trusted nonce-bearing style elements |
| `style-src-attr` | `'none'` | Target state forbids inline style attributes; migrate them to reviewed classes, native attributes, SVG geometry or bounded adapters |
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
- The Next chart adapter still creates one runtime `<style>` element without a demonstrated host nonce path. It remains registered migration debt and blocks strict promotion.

## Connection Profile Boundary

- Production environments (`RUSTOK_ENV`, `RUST_ENV` or `APP_ENV` set to `prod`/`production`) use `'self' https: wss:` on both server-hosted and standalone admin surfaces.
- Non-production profiles may additionally use `ws:` for local development.
- Plaintext `http:` is absent from every UI policy.
- Both production hosts reject startup without an explicit `RUSTOK_HTTPS` declaration and emit HSTS when it is present.
- The static CSP gate verifies that the secure source sets cannot regain `ws:`.

## Current Migration Debt

The enforced UI policies now isolate their remaining exception to:

- `style-src-attr 'unsafe-inline'`.

The broader `style-src 'unsafe-inline'` source, `script-src 'unsafe-inline'`, `unsafe-eval`, plaintext HTTP and production plaintext WebSocket have been removed from enforcement and are protected by CSP verification gates. The remaining attribute-level entry is migration debt, not an approved permanent production exception. The strict main-server report-only policy already uses `style-src-attr 'none'` to expose affected components.

### Rust-hosted and classic admin boundary

`docs/security/csp-inline-style-attribute-exceptions.json` is empty. Its gate observes exactly **0 Rust-hosted style attributes across 0 files** and has a non-increasing `0/0` ratchet.

The last Rust sites were removed by:

- mapping the forum admin category accent to a finite class before attaching it to the DOM;
- replacing the admin module build bar with native `<progress max="100">` and clamping the numeric value to `0..=100`.

The bundled classic admin bootstrap no longer writes `document.documentElement.style`. It toggles only the `dark` class, while `apps/admin/input.css` owns `color-scheme: light` and `color-scheme: dark`; the document declares `<meta name="color-scheme" content="light dark">`.

### Next/React boundary

`docs/security/csp-next-style-exceptions.json` now records exactly **54 JSX style props across 5 Next files** and **1 runtime style element**, down from the initial 60 props across 10 files. Review is due no later than **2026-08-15**.

| Surface | Files | JSX style props | Runtime style elements | Primary exit path |
|---|---:|---:|---:|---|
| Next storefront search package | 1 | 43 | 0 | Shared storefront classes and explicit state variants |
| Next admin table/chart/shell boundary | 4 | 11 | 1 | Bounded table geometry, finite chart palette or nonce-aware adapter, static shell classes and deterministic skeletons |
| **Total** | **5** | **54** | **1** | Empty the register before strict promotion |

The Next gate:

- scans every `.tsx` and `.jsx` file under `apps/next-admin` and `apps/next-frontend`;
- rejects unregistered files, count changes, stale entries, duplicate paths and expired review dates;
- caps the baseline at `54` style props, `5` files and `1` runtime style element;
- globally forbids direct DOM `.style` writes in those source roots;
- protects the class-only classic admin color-scheme bootstrap;
- protects completed class/SVG migrations for textarea resize, bar skeletons, Sonner, shared progress and table skeletons.

The runtime chart `<style dangerouslySetInnerHTML>` is tracked separately from JSX style props because it is governed by nonce-bearing `style-src`, not only `style-src-attr`.

## Completed Attribute Migrations

- the unreferenced legacy Page Builder `editor/admin_canvas.rs` duplicate was removed after confirming it had no `mod`, `#[path]` or source reference;
- modular layer indentation uses a bounded nine-step Tailwind class scale and caps deeper trees at the final class;
- hover, selection and insertion overlays use SVG `x`, `y`, `width` and `height` attributes;
- resize preview geometry uses an SVG `<rect>`;
- the eight resize handles use SVG `<circle>` positions and a closed cursor-class mapping while retaining pointer capture;
- Page Builder custom viewport width, height and continuous zoom use SVG `viewBox`/`foreignObject` geometry with native iframe dimensions rather than CSS sizing or `transform:scale`;
- storefront and forum-admin category accents map validated colors to a finite eight-color utility-class palette or reviewed gradient fallback and no longer attach a CSS declaration to the DOM;
- the admin build progress indicator uses native progress semantics;
- the classic admin theme bootstrap uses classes and stylesheet-owned color-scheme declarations rather than DOM style writes;
- Next textarea resize modes map the closed configuration union to four finite classes;
- Next overview bar skeletons use a deterministic, build-time-visible height-class sequence;
- the Sonner adapter exposes no `style` prop and owns its fixed token bridge through static arbitrary-property classes;
- the shared Next progress adapter preserves the Radix root contract while rendering clamped progress through an SVG width attribute;
- the data-table skeleton removed its unused free-form width API and relies on normal table layout.

The static modular Page Builder three-column layout had already moved from an inline attribute to a Tailwind arbitrary grid class and remains outside the exception registers.

## Triage Rules

1. Group reports by normalized directive and origin.
2. Reproduce each unique violation in embedded admin, standalone admin, Next admin and storefront browser smoke tests.
3. Classify it as application code, framework bootstrap, third-party dependency or malicious/noise traffic.
4. Replace each required style attribute with a reviewed class, native attribute, SVG geometry contract, bounded DOM adapter or another non-inline representation.
5. Remove or replace a source before considering an allowlist.
6. Any new external origin requires a security review, named owner, exact resource purpose and expiry/review date.
7. Never allowlist `unsafe-eval`; replace the dependency or execution path.
8. Never copy a full reported URL, query, fragment or script sample into issues or logs.
9. Never add a nonce through blanket post-processing of tenant or user-authored HTML.
10. Do not advertise a report endpoint from a deployment process that does not own the bounded collector.
11. Do not add a new Rust-hosted `style=` source site; its ratchet is zero.
12. Do not add a new Next style-prop file, runtime style element or direct DOM style write without failing and deliberately revising the migration boundary.

## Enforcement Exit Criteria

The enforced policy may be promoted to the strict target only when:

- browser smoke runs for embedded admin, standalone admin, Next admin and storefront produce no unexplained `style-src-attr` or `style-src` violations;
- the Rust exception register remains empty and its gate observes zero sites;
- the Next exception register is empty and the gate observes zero JSX style props;
- every runtime style element is removed or receives the exact request nonce through a reviewed host boundary;
- direct DOM style writes remain at zero across reviewed UI source roots;
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
node scripts/verify/verify-csp-next-style-boundary.mjs
```
