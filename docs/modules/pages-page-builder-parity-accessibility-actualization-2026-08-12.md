# Pages / Page Builder parity and generic editor accessibility actualization — 2026-08-12

Status: `source-parity-rechecked / generic-editor-accessibility-source-ready / focused-ci-gate-ready / rendered-dom-accessibility-evidence-partial / generic-accessibility-browser-harness-source-ready / browser-accessibility-evidence-pending / rollout-execution-pending`.

Base rechecked: `main@39dd5feb1659cdeffff7b949435b199f7a39ca8f`.

## Recheck result

The canonical Pages / Page Builder source architecture remains complete through the provider-health observation/evaluator/binding chain, Pages reference-consumer gate acceptance source, Forum owner-preserving contribution runtime and Forum Wave admission source. No later merged Pages/Page Builder change after PR #3453 introduces a new persistence, provider-health, contribution-registry, owner-port or rollout-architecture gap.

PR #3444 closed the source-level generic Page Builder editor control accessibility gap. PR #3453 then retained bounded executable native SSR DOM evidence for selected-page state, the add-page programmatic name and disabled capability fieldsets.

The merged editor source provides programmatic names for generic asset, property, style, responsive-style and trait controls, object-scoped names for repeated actions, and `aria-pressed` selected-state semantics for page/layer controls. `scripts/verify/verify-page-builder-admin-accessibility.mjs` source-locks those guarantees.

Source semantics and native rendered-DOM evidence do not establish WCAG conformance or replace executable keyboard, browser accessibility-tree or screen-reader evidence.

## Parity correction

The shared, local and central plans expose the same boundary:

- generic typed editor controls and programmatic accessibility semantics: **source-ready**;
- static accessibility anti-drift verification: **source-ready**;
- native SSR selected-state/name/disabled semantics: **partial executable evidence retained**;
- keyboard navigation and focus behavior: **browser harness source-ready / maintainer browser execution pending**;
- built-surface accessible-name/state inspection: **native SSR evidence retained / browser accessibility-tree harness source-ready / execution pending**;
- screen-reader evidence: **execution pending**;
- provider-health, Pages gate, Forum Wave and FFA/FBA acceptance: **unchanged and execution/owner-decision pending**.

The stale central-plan instruction to complete generic asset/accessibility controls as if the source were missing remains superseded. The implementation cursor is executable evidence retention, not another generic-control architecture slice.

## Focused CI continuation

`.github/workflows/pages-page-builder-parity.yml` is the read-only focused source gate for this boundary. It runs:

```text
node scripts/verify/verify-page-builder-admin-accessibility.mjs
node scripts/verify/verify-page-builder-accessibility-browser-evidence-harness.mjs
node crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs
node scripts/verify/verify-pages-page-builder-accessibility-plan-sync.mjs
```

The workflow has `contents: read` only. It validates retained source and contracts; it does not execute the deployed accessibility browser packet, build, publish, deploy, mutate tenant state, accept provider health, accept the Pages reference-consumer gate, execute Forum Wave admission or promote FFA/FBA.

`verify-pages-page-builder-accessibility-plan-sync.mjs` prevents the three active plans from drifting back to the pre-#3444 source cursor. `verify-page-builder-accessibility-browser-evidence-harness.mjs` prevents the new browser evidence source from silently weakening its exact-source/deployment identity, privacy or no-conformance-claim boundaries.

## Rendered accessibility evidence continuation

PR #3453 adds `crates/rustok-page-builder/admin/src/ssr_accessibility_evidence_tests.rs` to the ordinary `rustok-page-builder-admin` unit-test target. Unlike the static source guard, these tests render the real Leptos `PageBuilderAdmin` with a concrete `AdminCanvasController` and assert facts in the generated HTML. The focused `cargo test -p rustok-page-builder-admin --lib` execution is retained green for that slice.

The retained SSR evidence covers only a bounded subset of the open execution cursor:

- the active and inactive page controls render explicit `aria-pressed="true"` / `aria-pressed="false"` state;
- the new-page control renders a programmatic `Add page: Page name` name;
- visible `Page name` and `Page id` labels survive the actual SSR render path;
- denied `edit` and `properties` capability fieldsets render both native `disabled` semantics and `aria-disabled="true"`.

This is executable rendered-DOM evidence, not a browser accessibility-tree, keyboard/focus or screen-reader result. WASM-only asset/property/style controls are also outside this native SSR subset.

## Generic accessibility browser evidence harness continuation

The next retained source slice adds an exact-source/deployment Playwright packet rather than a synthetic HTML fixture:

- contract: `crates/rustok-page-builder/contracts/evidence/page-builder-generic-accessibility-browser-execution-contract.json`;
- runner: `apps/next-admin/tests/page-builder-accessibility/browser-evidence.spec.ts`;
- config: `apps/next-admin/playwright.page-builder-accessibility.config.ts`;
- stale-output guard: `apps/next-admin/tests/page-builder-accessibility/global-setup.ts`;
- source verifier: `scripts/verify/verify-page-builder-accessibility-browser-evidence-harness.mjs`.

The packet format is `page_builder_generic_accessibility_browser_execution_v1`. Its source status is `generic-accessibility-browser-harness-source-ready`; **maintainer browser execution pending** remains the execution state until an exact deployed source commit, reviewed immutable deployment RepoDigest, authenticated editor storage state and the required full/read-only profile URLs are supplied and a passing packet is retained.

The full-profile browser scenario requires at least two pages and proves a bounded set of generic editor facts:

- native sequential focus moves between adjacent page buttons with `Tab` / `Shift+Tab`;
- keyboard `Enter` activation moves the browser-observed pressed state to the newly active page;
- Playwright accessibility-tree matching observes the pressed button state and the `Add page: Page name` textbox accessible name;
- the add-page textbox and Add page button participate in sequential focus order;
- role/name lookup resolves the visible `Page name` and `Page id` labels.

The read-only profile independently proves that `edit` and `properties` fieldsets are browser-disabled and expose `aria-disabled="true"`, mutation controls are disabled through native fieldset semantics, while page selection remains keyboard-operable for non-mutating navigation.

The retained output contains exact source/deployment identity, source-file hashes, storage-state hash/size, profile URL hashes and bounded boolean/count observations only. It does not retain raw URLs, cookies, authorization headers, storage-state contents, page names/ids, raw DOM/HTML or ARIA snapshot text. Trace, screenshots and video are disabled in the dedicated config.

Even after a passing browser packet exists, **screen-reader execution remains pending** and **WCAG conformance remains unclaimed**. Browser accessibility-tree evidence is not substituted for assistive-technology execution or a conformance audit.

## Current execution cursor

Generic editor accessibility now has source semantics, native SSR evidence and a retained browser execution harness. The next accessibility action is maintainer execution of the exact-source/deployment full/read-only browser packet, followed separately by screen-reader evidence if the project elects to retain it.

The broader parity cursor remains:

1. execute exact provider-health deployment identity/metrics/evaluator/binding evidence and owner decisions;
2. execute the rollout-only Pages reference candidate and take the explicit Pages gate owner + rollback decision;
3. execute Forum browser/runtime/server-function evidence and Forum Wave admission on the admitted exact-source boundary;
4. retain the separate observed control-plane Wave, rollback/approval/waiver evidence and owner review;
5. promote FFA/FBA only after accepted observed evidence.

Generic editor accessibility evidence can be retained in parallel, but its source semantics are no longer an implementation blocker.

## Boundaries

This actualization does not change Fly commands, Page Builder capability policy, Pages persistence/publication/cache authority, Forum owner contracts, provider-health policy, rollout flags or tenant state. It makes no current deployment-health claim, no current browser-execution claim, no screen-reader-execution claim and no accessibility-conformance claim.
