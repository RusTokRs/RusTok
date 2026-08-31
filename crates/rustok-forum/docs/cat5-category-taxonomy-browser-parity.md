# TAXONOMY-CAT-5 Forum Category mounted multilingual/RTL browser parity

Status: **executable browser source / maintainer execution pending**

## Purpose

This is the final mounted-browser evidence source for the Forum Category → Taxonomy cutover. The backend ownership/storage cutover is already complete; this packet does not change Forum or Taxonomy production behavior.

The retained runner is:

```text
apps/next-admin/tests/forum-category-taxonomy/browser-evidence.spec.ts
```

with config:

```text
apps/next-admin/playwright.forum-category-taxonomy.config.ts
```

machine contract:

```text
crates/rustok-forum/contracts/evidence/forum-category-taxonomy-browser-execution-contract.json
```

manual execution workflow:

```text
.github/workflows/forum-category-taxonomy-browser-evidence.yml
```

and source guard:

```text
scripts/verify/verify-forum-category-taxonomy-browser-evidence.mjs
```

The runner reuses the repository's existing `@playwright/test` dependency. It performs browser navigation only; it does not seed fixtures, call GraphQL directly, read owner tables, or bypass Forum authorization.

The manual workflow is deliberately split into two boundaries:

- pull-request runs execute the source verifier and `playwright --list` only; they do not receive mounted credentials or run the mounted evidence;
- `workflow_dispatch` selects a maintainer-configured GitHub environment and is the only path that executes the mounted browser cases.

## Maintainer fixture boundary

Prepare one tenant whose Category owner data is already Taxonomy-backed and visible through the normal mounted applications.

The fixture must include:

- one RTL locale (for example `ar`) with a root Category and one child Category;
- root and child canonical Taxonomy slugs;
- root canonical icon/color presentation;
- deterministic root position `0` and child depth/position `1/0` for the prepared browser fixture;
- one requested locale that has no Category translation and therefore resolves to a different Taxonomy `effective_locale`;
- one historical Category alias whose storefront route redirects to the current Taxonomy canonical route.

The admin browser state must be a normal authenticated Playwright storage-state document for an operator allowed to read/manage Forum Categories. Do not put tokens, passwords or cookies in fixture URLs.

For GitHub Actions execution, store that JSON document only in the selected GitHub environment secret:

```text
RUSTOK_FORUM_CATEGORY_ADMIN_STORAGE_STATE_JSON
```

The workflow materializes it under `RUNNER_TEMP`, exports only the temporary file path to Playwright as `RUSTOK_FORUM_CATEGORY_ADMIN_STORAGE_STATE`, validates that the file contains JSON, and removes it in an `always()` cleanup step.

## Required environment

Authenticated admin/browser targets:

```text
RUSTOK_FORUM_CATEGORY_ADMIN_STORAGE_STATE
RUSTOK_FORUM_CATEGORY_ADMIN_RTL_E2E_URL
RUSTOK_FORUM_CATEGORY_ADMIN_FALLBACK_E2E_URL
RUSTOK_FORUM_CATEGORY_STOREFRONT_RTL_E2E_URL
RUSTOK_FORUM_CATEGORY_STOREFRONT_FALLBACK_E2E_URL
RUSTOK_FORUM_CATEGORY_STOREFRONT_ALIAS_E2E_URL
RUSTOK_FORUM_CATEGORY_STOREFRONT_CANONICAL_E2E_URL
```

The two admin targets are locale-addressable Forum Category routes. After the deployment's admin mount prefix, use the exact module subpath `modules/forum/categories/<requested_locale>`; for the embedded `/admin/` mount this is `/admin/modules/forum/categories/<requested_locale>`. The locale segment is normalized by the Forum admin route before Category reads, so an RTL request such as `ar` and a fallback requested locale survive a fresh browser navigation instead of depending on transient in-memory locale-switch state. Legacy `/modules/forum/categories` remains supported and continues to inherit the existing admin locale.

Expected Taxonomy-owned fixture values:

```text
RUSTOK_FORUM_CATEGORY_E2E_RTL_REQUESTED_LOCALE
RUSTOK_FORUM_CATEGORY_E2E_RTL_EFFECTIVE_LOCALE
RUSTOK_FORUM_CATEGORY_E2E_FALLBACK_REQUESTED_LOCALE
RUSTOK_FORUM_CATEGORY_E2E_FALLBACK_EFFECTIVE_LOCALE
RUSTOK_FORUM_CATEGORY_E2E_ROOT_NAME
RUSTOK_FORUM_CATEGORY_E2E_ROOT_SLUG
RUSTOK_FORUM_CATEGORY_E2E_CHILD_NAME
RUSTOK_FORUM_CATEGORY_E2E_CHILD_SLUG
RUSTOK_FORUM_CATEGORY_E2E_ROOT_ICON
RUSTOK_FORUM_CATEGORY_E2E_ACCENT_CLASS
RUSTOK_FORUM_CATEGORY_E2E_ROOT_CANONICAL_PATH
RUSTOK_FORUM_CATEGORY_E2E_CHILD_CANONICAL_PATH
RUSTOK_FORUM_CATEGORY_E2E_FALLBACK_NAME
RUSTOK_FORUM_CATEGORY_E2E_FALLBACK_SLUG
RUSTOK_FORUM_CATEGORY_E2E_FALLBACK_CANONICAL_PATH
```

For the manual GitHub Actions workflow, configure every value above except `RUSTOK_FORUM_CATEGORY_ADMIN_STORAGE_STATE` as a variable on the selected GitHub environment. The supplied mounted URLs must be credential-free HTTP(S) URLs without fragments.

## Browser cases

### Admin RTL hierarchy/order/presentation

The mounted `/modules/forum/categories/<requested_locale>` surface is opened with the normal authenticated browser state. The runner proves:

- the requested RTL locale is present in the mounted route and initializes the Forum Category content locale;
- localized root and child Category headings render with `lang=<Taxonomy effective_locale>` and `dir="auto"`, and the mounted browser resolves their computed CSS `direction` to `rtl`;
- slug badges remain `dir="ltr"`;
- the prepared root/child hierarchy renders as depth/position `0/0` and `1/0`;
- root precedes child in the flattened canonical tree order;
- the prepared Taxonomy icon and accent presentation are visible in the mounted card.

### Admin locale fallback

A requested locale with no exact Category translation must render the prepared Taxonomy fallback copy. The heading's `lang` must be the different `effective_locale`, while the route identifier remains LTR and uses the fallback canonical slug.

### Storefront RTL and canonical route

The mounted Forum category rail must render the same root and child RTL Category copy, preserve owner ordering, keep localized content `dir="auto"` while the mounted browser resolves its computed CSS `direction` to `rtl`, keep route identifiers LTR, and link to the canonical Taxonomy localized category paths.

### Storefront locale fallback

A storefront request in the untranslated requested locale must render fallback copy tagged with the Taxonomy `effective_locale`. The Category link must point at the effective-locale canonical route rather than synthesizing a route from the requested locale.

### Storefront alias redirect

The prepared historical alias URL must navigate to the exact current canonical Category URL. The browser runner checks the final committed page URL after redirect and verifies the canonical Category card is mounted.

## Source verification

The source guard pins both the mounted source and the manual execution boundary:

- Forum Category admin accepts only the exact `categories/<locale>` mounted subpath as a locale override, normalizes that locale through the shared locale contract, and keeps legacy `/categories` behavior unchanged;
- Forum admin localized cards retain `effective_locale`, `lang`, `dir="auto"`, browser-resolved RTL direction, LTR route identifiers, hierarchy/order and presentation rendering;
- Forum storefront Category cards retain effective-locale copy, browser-resolved RTL direction and canonical hrefs;
- the Forum Category tree reader still consumes `TaxonomyOwnerCategoryReader` projections for copy/hierarchy/presentation;
- the mounted storefront route retains canonical/redirect handling;
- none of those retained owner/mount sources can regress to `forum_category_translations`, `forum_category_route_aliases` or `ForumCategoryTranslationTargetProvider`;
- mounted execution remains `workflow_dispatch`-only and requires a maintainer-selected GitHub environment;
- authenticated storage state comes from an environment secret and the credential-free fixture values come from environment variables;
- ordinary pull requests can verify/compile-list the retained runner without receiving mounted credentials.

## Retained source-ready provenance

The execution packet has been hardened incrementally without changing its `maintainer execution pending` status:

- PR #3708 (`560ac9108fde99349e4f7ed8028600eddb761cf4`, merged as `0df613755400682054535d2dc80c131d10fee456`) introduced the retained mounted multilingual/RTL browser runner, dedicated Playwright config, machine contract and source verifier. It deliberately made no browser-execution claim.
- PR #3750 (`6cf7325a4f32a1cff6859792978523e21913e873`, merged as `287db4a8857663e0355712d9cb5893f118f65608`) added the credential-safe manual execution workflow. Focused run `33306801105` passed the exact-head source contract and Playwright compile-list while the mounted job was correctly skipped on the pull-request event.
- PR #3752 (`448e50a2b499aedeea45c5d448b0f832a51f9da0`, merged as `2f5e03e8cb5c0577686e0cf527f93d8897b46fbe`) made the mounted Forum Category admin route locale-addressable through the exact normalized `categories/<locale>` subpath. Focused `Forum Category Taxonomy Browser Evidence` run `33365642047` passed on that exact head.
- PR #3753 (`cfe009e9cfa10d0c78c7768489467669577003e0`, merged as `4471ef63d0f49f683b819b527baaf13d45ec8297`) strengthened RTL evidence so authenticated admin and storefront root/child localized headings must resolve to browser-computed `direction: rtl`, not merely retain `dir="auto"`. Focused run `33387441260` passed exact checkout, the source verifier and Playwright compile-list; the mounted job remained correctly skipped on the pull-request event.

No successful `Forum Category Taxonomy Browser Evidence` `workflow_dispatch` run is retained yet. The source-ready runs above prove the executable packet and its security/route/RTL contracts only; they do not prove deployment provenance, mounted browser execution, production rollout completion or TAXONOMY-CAT-5 completion.

## Maintainer execution

Preferred repository execution after the workflow is present on `main`:

1. configure a GitHub environment with the secret and variables documented above;
2. open **Actions → Forum Category Taxonomy Browser Evidence → Run workflow**;
3. select that environment and run against the intended `main` revision;
4. retain the successful workflow run URL/ID and exact head SHA as CAT-5 mounted evidence before changing the CAT-5 status.

The equivalent local execution remains available from the repository root after the environment above is populated:

```bash
node scripts/verify/verify-forum-category-taxonomy-browser-evidence.mjs
cd apps/next-admin
npx playwright test --config playwright.forum-category-taxonomy.config.ts
```

A successful mounted browser run is the missing multilingual/RTL parity evidence required before TAXONOMY-CAT-5 can be marked complete. Adding or merging the workflow itself still does **not** claim browser execution, deployment provenance, rollout completion or CAT-5 completion.