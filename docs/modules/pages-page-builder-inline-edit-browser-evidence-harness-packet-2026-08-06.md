# Pages / Page Builder Inline Edit Browser Evidence Harness

Date: 2026-08-06  
Status: `source-ready / maintainer-execution-pending`

## Scope

This packet defines the maintainer-run Chromium evidence boundary for the source-ready Pages authenticated inline-authoring path. It starts only after the artifact/HTTP packet for the exact same source commit has passed.

The harness does not seed tenants, pages, roles or sessions. Maintainers provide reviewed fixture routes and external Playwright storage-state files. Source inspection does not prove browser execution.

## Owners

- Pages admin owns the optional draft-only same-origin launch control.
- The storefront host owns authenticated route admission, CSP and authoring shell composition.
- Pages owns bootstrap issuance, grant verification and `save_document(expected_revision)`.
- Page Builder/Fly owns the instrumented renderer, real-DOM eligibility and canonical patch session.
- The evidence harness observes those owners; it does not add another persistence, authorization or rendering path.

## Source files

```text
crates/rustok-pages/contracts/evidence/pages-inline-edit-browser-execution-contract.json
crates/rustok-pages/contracts/evidence/pages-inline-edit-browser-evidence-harness-source.json
apps/next-admin/playwright.pages-inline-edit.config.ts
apps/next-admin/tests/pages-inline-edit/browser-evidence.spec.ts
crates/rustok-pages/scripts/verify/verify-pages-inline-edit-browser-evidence-harness.mjs
```

The harness reuses the repository's existing pinned `@playwright/test` dependency. Chromium is explicit, workers are fixed to one, retries are disabled and trace, screenshots and video are disabled so retained evidence cannot accidentally include credentials, grants, authoring HTML or edited text.

## Required predecessor

The environment input

```text
RUSTOK_PAGES_INLINE_EDIT_BROWSER_ARTIFACT_HTTP_EVIDENCE
```

must reference a passing packet with:

```text
format: pages_inline_edit_artifact_http_execution_v1
status: artifact_http_execution_passed_browser_rollout_pending
```

The browser harness requires:

- the same exact source commit;
- the same deployed origin;
- the same immutable deployment RepoDigest;
- that RepoDigest to be present in the predecessor Docker evidence;
- predecessor browser and rollout boundaries to remain false.

A stale or unrelated artifact/HTTP packet fails closed.

## Fixture contract

Maintainers provide three external storage states:

- a direct authenticated user with effective `pages:update`;
- an authenticated user without effective `pages:update`;
- a session for a standalone admin build that lacks the same-origin acknowledgement.

Maintainers also provide reviewed admin routes for:

- one unpublished exact-locale GrapesJS page;
- one published page;
- one locale-less page;
- one missing page identity;
- the equivalent standalone-admin page route.

The draft fixture must contain distinct stable component IDs for:

- one eligible static leaf text component;
- one provider-owned component;
- one composite component;
- one templated component;
- one interactive component;
- one runtime-owned component.

The server grant TTL must be shorter than the configured browser expiry delay. The harness does not mutate configuration or fixture metadata.

## Launch evidence

The harness records bounded pass/fail facts for:

- the launch being visible to the allowed editor on the unpublished page;
- the launch being hidden for published, locale-less, missing, unauthorized and standalone-admin states;
- a relative same-origin href;
- exact canonical Pages UUID and locale query binding;
- absence of token, session, grant, proof, authorization or signing keys in the URL;
- the launch opening the exact expected authoring route.

No admin path or page UUID is retained in the final packet; only hashes and booleans are retained.

## SSR and hydration privacy

The authenticated authoring SSR response must return `200` and bind only:

```text
pages-inline-edit-client-root
data-pages-page-id
data-pages-locale
```

Both SSR HTML and the hydrated DOM are scanned for forbidden session, grant, proof, access-token, refresh-token and signing-secret markers.

The Page Builder hydrated root must:

- omit `data-inline-session` and `data-inline-proof`;
- use a deterministic id derived from Fly page identity plus expected project hash;
- retain revision and project-hash facts needed for stale/reload observation.

The trusted grant session remains inside Rust/WASM objects and request bodies only. Raw SSR HTML and hydrated HTML are never written to retained evidence.

## Dedicated client observation

The browser observes successful `200` responses for:

```text
/assets/pages-inline-edit-bootstrap.js
/assets/pages-inline-edit/rustok_storefront.js
/assets/pages-inline-edit/rustok_storefront_bg.wasm
```

The scenario requires zero page errors, zero console errors and zero critical document/script/stylesheet/WASM request failures before editing.

## Eligibility evidence

The one eligible static leaf must receive:

```text
data-fly-inline-editable="content"
contenteditable="plaintext-only"
```

Provider-owned, composite, templated, interactive and runtime-owned fixtures must not receive either attribute. This checks the rendered browser boundary while the canonical Page Builder server/session checks remain authoritative.

## Save and reload evidence

A changed static leaf dispatches one `focusout`. The harness requires exactly one commit request and a successful response.

It then requires:

- a replacement revision;
- a replacement project hash;
- the saved text and replacement identity to survive reload;
- no critical browser failures.

Request and response bodies remain memory-only. The retained packet stores only byte lengths and SHA-256 hashes.

## Stale evidence

Two browser tabs load the same revision and project hash before the first save. After the first tab succeeds, the second tab submits a different value using its old bootstrap.

The second request must fail, and reload must show the first saved value and current revision. This proves no partial stale write through the browser path.

## Replay evidence

The successful commit request is retained only in memory and replayed once through the same browser context. The exact request must be rejected.

The final packet retains only response status, byte length and SHA-256. It does not retain request headers, cookies, authorization values, request body or response body.

## Expiry evidence

A fresh tab loads the current revision, waits longer than the reviewed short-lived grant TTL, then attempts a changed `focusout`.

The request must fail, and reload must still show the previously saved value and revision. The configured delay is retained; the grant and its expiry claim are not retained.

## Output

Default output:

```text
target/pages-inline-edit-browser-evidence.json
```

Required identity:

```text
format: pages_inline_edit_browser_execution_v1
status: browser_execution_passed_rollout_pending
```

The output is atomically replaced and does not edit canonical source files.

It retains:

- source commit and source-file hashes;
- Node and Playwright versions;
- hashes and sizes of predecessor/storage-state inputs;
- deployment RepoDigest;
- hashes of origin, page, locale, component IDs and edited values;
- statuses, counts, sizes, hashes and bounded boolean observations.

It does not retain:

- Authorization or Cookie values;
- storage-state contents;
- bearer/session tokens or session IDs;
- grants, authorization proofs or signing keys;
- raw request or response bodies;
- raw SSR or hydrated HTML;
- console message text;
- admin paths, page IDs, component IDs or edited text;
- traces, screenshots or video.

## Maintainer command

After the predecessor packet and reviewed fixture inputs exist:

```bash
cd apps/next-admin
npx --no-install playwright test \
  --config playwright.pages-inline-edit.config.ts
```

The source guard is:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-inline-edit-browser-evidence-harness.mjs
```

## Promotion boundary

A passing browser packet closes only launch, SSR/hydration privacy, client mount, eligibility, save/reload, stale, replay and expiry browser evidence for one reviewed environment and image digest.

Tenant rollout, observation windows, rollback rehearsal, FFA and FBA remain separate. No browser execution is claimed by this source packet.
