---
id: doc://docs/security/csp-report-only-inventory.md
kind: security_control_inventory
language: markdown
source_language: markdown
status: active
---
# CSP Report-Only Migration Inventory

## Purpose

This inventory defines the target browser policy, the report collection boundary and the evidence required before the UI CSP can remove `unsafe-inline` and `unsafe-eval` from enforcement.

No violation in this document is an automatic allowlist request. The preferred resolution is to remove the dependency, move code into a same-origin static asset, or attach a per-response nonce/hash.

## Collection Contract

- Endpoint: `POST /api/security/csp-report`.
- Accepted formats: legacy `application/csp-report`, Reporting API `application/reports+json`, and JSON-compatible test traffic.
- Maximum request body: 64 KiB.
- Maximum processed Reporting API entries: 20 per request.
- The outer security middleware handles the endpoint before tenant and authentication routing.
- Responses contain no report body and return `204` for accepted reports.
- Invalid content types, JSON and report shapes are rejected with bounded status codes.

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
| `script-src` | `'self'` | No inline scripts and no eval; SSR/bootstrap must use external assets or nonces/hashes |
| `style-src` | `'self'` | No inline styles; migrate generated style blocks to hashes, nonces or static CSS |
| `img-src` | `'self' data: blob: https:` | Remote images remain HTTPS-only |
| `font-src` | `'self' data:` | No remote font origin is currently approved |
| `connect-src` | `'self' https: wss:` | No plaintext HTTP or WebSocket connection |
| `worker-src` | `'self' blob:` | Blob workers are retained for current browser runtime support |
| `object-src` | `'none'` | Plugins and embedded object content forbidden |
| `frame-ancestors` | `'none'` | Embedding forbidden |
| `base-uri` | `'self'` | Base URL rewriting restricted |
| `form-action` | `'self'` | Form submission restricted to same origin |

## Current Migration Debt

The enforced UI policy still contains:

- `script-src 'unsafe-inline' 'unsafe-eval'`;
- `style-src 'unsafe-inline'`;
- plaintext `ws:` for local/development compatibility.

These entries are migration debt, not approved production exceptions. The strict report-only policy intentionally excludes them.

## Triage Rules

1. Group reports by normalized directive and origin.
2. Reproduce each unique violation in admin and storefront browser smoke tests.
3. Classify it as application code, framework bootstrap, third-party dependency or malicious/noise traffic.
4. Remove or replace the source before considering an allowlist.
5. Any new external origin requires a security review, named owner, exact resource purpose and expiry/review date.
6. Never allowlist `unsafe-eval`; replace the dependency or execution path.
7. Never copy a full reported URL, query, fragment or script sample into issues or logs.

## Enforcement Exit Criteria

The enforced policy may be promoted to the strict target only when:

- browser smoke runs for admin and storefront produce no unexplained `script-src`, `style-src` or `connect-src` violations;
- every required inline bootstrap block has a nonce/hash implementation and regression test;
- no production code path requires `eval` or equivalent string compilation;
- the observed external-origin set matches this inventory;
- the CSP reporting endpoint remains bounded and unauthenticated without inheriting tenant context;
- rollback instructions retain the last known safe policy without restoring plaintext network sources.

## Verification

```bash
cargo test -p rustok-server middleware::csp_reports
cargo test -p rustok-server middleware::security_headers
node scripts/verify/verify-csp-reporting-contract.mjs
```
