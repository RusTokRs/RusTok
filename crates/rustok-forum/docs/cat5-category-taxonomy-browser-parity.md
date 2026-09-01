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
- `workflow_dispatch` selects a maintainer-configured GitHub environment and is the only path that executes the mounted browser cases, and the mounted run fails closed unless it was dispatched from `refs/heads/main`.

Before authenticated admin state is materialized, the mounted workflow also validates every non-secret fixture value as bounded, non-empty and free of NUL/CR/LF control characters; validates all mounted targets as credential-free HTTP(S) URLs without fragments; and proves the static locale/URL, fallback-locale and alias/canonical relationships already required by the runner. The focused workflow path filter covers every runtime/source file read by the retained verifier plus the next-admin package manifests, so dependency or guarded-source drift re-runs the source contract.

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

The raw secret is exposed only to the materialization step. Source verification, dependency installation and Chromium setup complete before the credential file exists. The workflow then creates the file under `RUNNER_TEMP` with `umask 077`, validates that it contains JSON, exposes its path only as a materialization-step output to the Playwright execution step, and removes it in an `always()` cleanup step. The file path is not exported job-wide through `$GITHUB_ENV`.

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

For the manual GitHub Actions workflow, configure every value above except `RUSTOK_FORUM_CATEGORY_ADMIN_STORAGE_STATE` as a variable on the selected GitHub environment. Every configured value must be non-empty after trimming, at most 4096 bytes in the retained workflow/runner contract, and contain no NUL/CR/LF characters. The supplied mounted URLs must be credential-free HTTP(S) URLs without fragments. The RTL and fallback requested locales must occur as exact path segments in their respective admin/storefront URLs, the fallback requested/effective locales must differ, and the normalized storefront alias/canonical URLs must differ.

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
- the focused pull-request path set covers every guarded runtime/source seam and the next-admin package manifests;
- mounted execution remains `workflow_dispatch`-only, requires `refs/heads/main` and uses a maintainer-selected GitHub environment;
- every non-secret mounted fixture value is bounded and structurally preflighted before authenticated storage-state materialization, including URL safety and static locale/fallback/alias relationships;
- the raw authenticated storage-state secret is step-scoped, the credential file is materialized late, only the Playwright execution step receives its path, and cleanup is unconditional;
- ordinary pull requests can verify/compile-list the retained runner without receiving mounted credentials.

## Retained source-ready provenance

The execution packet has been hardened incrementally without changing its `maintainer execution pending` status:

- PR #3708 (`560ac9108fde99349e4f7ed8028600eddb761cf4`, merged as `0df613755400682054535d2dc80c131d10fee456`) introduced the retained mounted multilingual/RTL browser runner, dedicated Playwright config, machine contract and source verifier. It deliberately made no browser-execution claim.
- PR #3750 (`6cf7325a4f32a1cff6859792978523e21913e873`, merged as `287db4a8857663e0355712d9cb5893f118f65608`) added the credential-safe manual execution workflow. Focused run `33306801105` passed the exact-head source contract and Playwright compile-list while the mounted job was correctly skipped on the pull-request event.
- PR #3752 (`448e50a2b499aedeea45c5d448b0f832a51f9da0`, merged as `2f5e03e8cb5c0577686e0cf527f93d8897b46fbe`) made the mounted Forum Category admin route locale-addressable through the exact normalized `categories/<locale>` subpath. Focused `Forum Category Taxonomy Browser Evidence` run `33365642047` passed on that exact head.
- PR #3753 (`cfe009e9cfa10d0c78c7768489467669577003e0`, merged as `4471ef63d0f49f683b819b527baaf13d45ec8297`) strengthened RTL evidence so authenticated admin and storefront root/child localized headings must resolve to browser-computed `direction: rtl`, not merely retain `dir="auto"`. Focused run `33387441260` passed exact checkout, the source verifier and Playwright compile-list; the mounted job remained correctly skipped on the pull-request event.
- PR #3755 (`c33b632710401a34caedab96fb384aff88d99c78`, merged as `f4de7c3ccbc1c0b32c9c6bcd6f0394e07981a063`) aligned the machine claims with the computed-RTL runner. Focused run `33389998207` passed exact checkout, source verification, dependency install and Playwright compile-list while mounted execution remained skipped.
- PR #3757 (`4dd055b54e5d15f7329acff802688a74e52d3a7e`, merged as `246aadc49857bfd8442bb526d0da7713061d0b60`) made mounted evidence fail closed off `refs/heads/main`. Focused run `33393245584` passed the exact-head source contract; mounted execution remained skipped on the pull-request event.
- PR #3758 (`362fd08972875cd20f0ff45487d56cd82daa77f5`, merged as `b04434a1172db0282eaf4afe45e603ba6f75edb2`) narrowed the raw authenticated storage-state secret to the materialization step. Focused run `33395511932` passed the retained source/compile-list contract.
- PR #3759 (`5644a2ffd6c4836bb6d7a36178f65af87a8f0099`, merged as `befdc7f6ddcd506a24f5d069dbb4ad17a14556fb`) delayed credential-file creation until after source/dependency/browser setup, exposed its path only through the materialization step output to Playwright and retained unconditional cleanup. Focused run `33395884361` passed.
- PR #3760 (`8f29938750c71cc41711eb5a8b39022fc64faa68`, merged as `14ffe3729870a929c2621ed68483ddaf10a3dfee`) moved credential-free HTTP(S)/no-fragment URL validation ahead of authenticated-state materialization while retaining runner-side validation. Focused run `33398827860` passed.
- PR #3761 (`aff03fdf06df6529cc9176dff02a524d07b035ff`, merged as `2e2774e67bae5c7c8aef3db56f3d402771137d48`) closed the focused pull-request path set over every verifier-owned runtime/source seam plus the next-admin package manifests. Focused run `33403955051` passed.
- PR #3762 (`067f6ec3c2c138a29265a172ae17632c9269827b`, merged as `a64f2b1c36112f6e8e4cd9166040bfcfaf11e877`) moved bounded non-empty/control-character validation for every non-secret fixture value before authenticated-state materialization. Focused run `33404692954` passed.
- PR #3763 (`5bd030a05e5ca59dcd6b6c317cf22400fc20b1f9`, merged as `261ce7e00629b0759042d3ebca62e6c9e2f39216`) preflighted the four locale→URL relationships, fallback requested/effective inequality and storefront alias/canonical inequality before authenticated-state materialization. Focused run `33405293672` passed.

Post-cutover backend cleanup accepted after the browser packet was already source-ready:

- PR #3771 (`13919479c99ec755d1f7cc3f758c14680d821659`, merged as `2f1f97a65c4719ae4f793ff81b925fe0e6121936`) confined the historical `forum_category_translation` SeaORM entity to crate-private migration compatibility and removed the retired runtime Category→translation relation. Focused `Forum Taxonomy Category Backfill Contract` run `33436045515` passed the updated source boundary and full Forum library compile.
- PR #3772 (`4cf6e711d6b56dbd9cf9222bbfd4c3f04d1a0391`, merged as `3b15ab139636d31dca027045d3a53a5769a62b1a`) removed the unreachable private pre-cutover Category read-model donor path while retaining Topic/Reply behavior. Focused `Forum Read Model Category Taxonomy Copy` run `33437621508` passed exact-SHA source verification, Rust formatting, full Forum library compilation and the retained Topic unread SQLite regression.
- PR #3774 (`6815c17cffbac3f10db3f94a7058aba78201bef8`, merged as `7ec72d3d890e3ead9d8618db179fa55bf50477e7`) isolated the still-live Category tree-lock/order/name/locale/slug mutation support from the narrow `CategoryService` persistence seam without changing import or Taxonomy-backed projection-owner behavior. Focused `Forum Taxonomy Category Binding Contract` run `33468821321` passed exact-head source verification, focused Rust formatting, the runtime binding contract and full Forum library compilation.
- PR #3775 (`1d4db5dd40f0183f36c4802261b8193d377a3db8`, merged as `95a25063b613105ce78eac13b3b025aaa3e17380`) made transactional Category metadata updates fail closed on placement changes and removed the dormant direct position write so `move_category` / `reorder_siblings` remain the only placement path with Taxonomy hierarchy synchronization. Focused `Forum Taxonomy Category Binding Contract` run `33470821294` passed exact-head source verification, focused Rust formatting, the runtime binding contract and full Forum library compilation.

These cleanup and hardening merges do not alter the browser packet's execution contract or substitute for mounted evidence. They narrow the remaining historical/runtime bypass surface while preserving the published backfill migration and the still-required maintainer execution boundary.

No successful `Forum Category Taxonomy Browser Evidence` `workflow_dispatch` run is retained yet. The source-ready runs above prove the executable packet and its security/route/RTL contracts only; they do not prove deployment provenance, mounted browser execution, production rollout completion or TAXONOMY-CAT-5 completion.

## Maintainer execution

Preferred repository execution after the workflow is present on `main`:

1. configure a GitHub environment with the secret and variables documented above;
2. open **Actions → Forum Category Taxonomy Browser Evidence → Run workflow**;
3. run the workflow from `main`, select that environment and keep the prepared fixture values within the preflight contract above;
4. retain the successful workflow run URL/ID and exact head SHA as CAT-5 mounted evidence before changing the CAT-5 status.

The equivalent local execution remains available from the repository root after the environment above is populated:

```bash
node scripts/verify/verify-forum-category-taxonomy-browser-evidence.mjs
cd apps/next-admin
npx playwright test --config playwright.forum-category-taxonomy.config.ts
```

A successful mounted browser run is the missing multilingual/RTL parity evidence required before TAXONOMY-CAT-5 can be marked complete. Adding or merging the workflow itself still does **not** claim browser execution, deployment provenance, rollout completion or CAT-5 completion.
