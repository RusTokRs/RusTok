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

and machine contract:

```text
crates/rustok-forum/contracts/evidence/forum-category-taxonomy-browser-execution-contract.json
```

The source guard is:

```text
scripts/verify/verify-forum-category-taxonomy-browser-evidence.mjs
```

The runner reuses the repository's existing `@playwright/test` dependency. It performs browser navigation only; it does not seed fixtures, call GraphQL directly, read owner tables, or bypass Forum authorization.

## Maintainer fixture boundary

Prepare one tenant whose Category owner data is already Taxonomy-backed and visible through the normal mounted applications.

The fixture must include:

- one RTL locale (for example `ar`) with a root Category and one child Category;
- root and child canonical Taxonomy slugs;
- root canonical icon/color presentation;
- deterministic root position `0` and child depth/position `1/0` for the prepared browser fixture;
- one requested locale that has no Category translation and therefore resolves to a different Taxonomy `effective_locale`;
- one historical Category alias whose storefront route redirects to the current Taxonomy canonical route.

The admin browser state must be a normal authenticated Playwright storage-state file for an operator allowed to read/manage Forum Categories. Do not put tokens, passwords or cookies in fixture URLs.

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

The supplied mounted URLs must be credential-free HTTP(S) URLs without fragments.

## Browser cases

### Admin RTL hierarchy/order/presentation

The mounted `/modules/forum/categories` surface is opened with the normal authenticated browser state. The runner proves:

- the requested RTL locale is present in the mounted route;
- localized Category headings render with `lang=<Taxonomy effective_locale>` and `dir="auto"`;
- slug badges remain `dir="ltr"`;
- the prepared root/child hierarchy renders as depth/position `0/0` and `1/0`;
- root precedes child in the flattened canonical tree order;
- the prepared Taxonomy icon and accent presentation are visible in the mounted card.

### Admin locale fallback

A requested locale with no exact Category translation must render the prepared Taxonomy fallback copy. The heading's `lang` must be the different `effective_locale`, while the route identifier remains LTR and uses the fallback canonical slug.

### Storefront RTL and canonical route

The mounted Forum category rail must render the same RTL Category copy, preserve owner ordering, keep localized content `dir="auto"`, keep route identifiers LTR, and link to the canonical Taxonomy localized category paths.

### Storefront locale fallback

A storefront request in the untranslated requested locale must render fallback copy tagged with the Taxonomy `effective_locale`. The Category link must point at the effective-locale canonical route rather than synthesizing a route from the requested locale.

### Storefront alias redirect

The prepared historical alias URL must navigate to the exact current canonical Category URL. The browser runner checks the final committed page URL after redirect and verifies the canonical Category card is mounted.

## Source verification

The source guard also pins the ownership boundary beneath the browser surface:

- Forum admin localized cards retain `effective_locale`, `lang`, `dir="auto"`, LTR route identifiers, hierarchy/order and presentation rendering;
- Forum storefront Category cards retain effective-locale copy and canonical hrefs;
- the Forum Category tree reader still consumes `TaxonomyOwnerCategoryReader` projections for copy/hierarchy/presentation;
- the mounted storefront route retains canonical/redirect handling;
- none of those retained owner/mount sources can regress to `forum_category_translations`, `forum_category_route_aliases` or `ForumCategoryTranslationTargetProvider`.

## Maintainer execution

From the repository root, after the environment above is populated:

```bash
node scripts/verify/verify-forum-category-taxonomy-browser-evidence.mjs
cd apps/next-admin
npx playwright test --config playwright.forum-category-taxonomy.config.ts
```

A successful browser run is the missing mounted multilingual/RTL parity evidence required before TAXONOMY-CAT-5 can be marked complete. This source packet itself does **not** claim browser execution, deployment provenance, rollout completion or CAT-5 completion.

No browser launch, source verifier, Playwright command, CI workflow, database command or other test was executed while preparing this slice.
