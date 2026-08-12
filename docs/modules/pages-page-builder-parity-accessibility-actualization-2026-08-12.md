# Pages / Page Builder parity and generic editor accessibility actualization — 2026-08-12

Status: `source-parity-rechecked / generic-editor-accessibility-source-ready / focused-ci-gate-ready / rendered-dom-accessibility-evidence-partial / browser-accessibility-evidence-pending / rollout-execution-pending`.

Base rechecked: `main@389fa1acdb1bbe7f554380ecb5ea178c5f73bda9`.

## Recheck result

The canonical Pages / Page Builder source architecture remains complete through the provider-health observation/evaluator/binding chain, Pages reference-consumer gate acceptance source, Forum owner-preserving contribution runtime and Forum Wave admission source. No later merged Pages/Page Builder change after PR #3444 introduces a new persistence, provider-health, contribution-registry, owner-port or rollout-architecture gap.

PR #3444 is newer than the last shared/local/central plan reconciliation and closes one source-level gap that those plans still described as open: generic Page Builder editor control accessibility semantics.

The merged editor source now provides programmatic names for generic asset, property, style, responsive-style and trait controls, object-scoped names for repeated actions, and `aria-pressed` selected-state semantics for page/layer controls. `scripts/verify/verify-page-builder-admin-accessibility.mjs` source-locks those guarantees.

This is source accessibility semantics only. It does not establish WCAG conformance or replace executable keyboard, focus, browser, accessibility-tree or screen-reader evidence.

## Parity correction

The shared, local and central plans must expose the same boundary:

- generic typed editor controls and programmatic accessibility semantics: **source-ready**;
- static accessibility anti-drift verification: **source-ready**;
- keyboard navigation and focus behavior: **execution pending**;
- built-surface accessible-name/state inspection: **partial native SSR evidence retained; browser/accessibility-tree execution pending**;
- browser and screen-reader evidence: **execution pending**;
- provider-health, Pages gate, Forum Wave and FFA/FBA acceptance: **unchanged and execution/owner-decision pending**.

The stale central-plan instruction to complete generic asset/accessibility controls as if the source were missing is superseded. The implementation cursor is now evidence retention, not another generic-control architecture slice.

## Focused CI continuation

`.github/workflows/pages-page-builder-parity.yml` is the read-only focused gate for this boundary. It runs:

```text
node scripts/verify/verify-page-builder-admin-accessibility.mjs
node crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs
node scripts/verify/verify-pages-page-builder-accessibility-plan-sync.mjs
```

The workflow has `contents: read` only and does not build, publish, deploy, mutate tenant state, accept provider health, accept the Pages reference-consumer gate, execute Forum Wave admission or promote FFA/FBA.

`verify-pages-page-builder-accessibility-plan-sync.mjs` prevents the three active plans from drifting back to the pre-#3444 source cursor and requires the dated accessibility actualization plus source guard to remain wired into the verification programme.

## Rendered accessibility evidence continuation

PR #3453 adds `crates/rustok-page-builder/admin/src/ssr_accessibility_evidence_tests.rs` to the ordinary `rustok-page-builder-admin` unit-test target. Unlike the static source guard, these tests render the real Leptos `PageBuilderAdmin` with a concrete `AdminCanvasController` and assert facts in the generated HTML. The focused `cargo test -p rustok-page-builder-admin --lib` execution is retained green for this slice.

The retained SSR evidence covers only a bounded subset of the open execution cursor:

- the active and inactive page controls render explicit `aria-pressed="true"` / `aria-pressed="false"` state;
- the new-page control renders a programmatic `Add page: Page name` name;
- visible `Page name` and `Page id` labels survive the actual SSR render path;
- denied `edit` and `properties` capability fieldsets render both native `disabled` semantics and `aria-disabled="true"`.

This is executable rendered-DOM evidence, not a browser accessibility-tree, keyboard/focus or screen-reader result. WASM-only asset/property/style controls are also outside this native SSR subset. Therefore the shared Phase 9 checkbox remains open and browser/WCAG claims remain prohibited.

## Current execution cursor

The broader parity cursor remains:

1. execute exact provider-health deployment identity/metrics/evaluator/binding evidence and owner decisions;
2. execute the rollout-only Pages reference candidate and take the explicit Pages gate owner + rollback decision;
3. execute Forum browser/runtime/server-function evidence and Forum Wave admission on the admitted exact-source boundary;
4. retain the separate observed control-plane Wave, rollback/approval/waiver evidence and owner review;
5. promote FFA/FBA only after accepted observed evidence.

Generic editor accessibility evidence can be retained in parallel, but its source semantics are no longer an implementation blocker.

## Boundaries

This actualization does not change Fly commands, Page Builder capability policy, Pages persistence/publication/cache authority, Forum owner contracts, provider-health policy, rollout flags or tenant state. It makes no current deployment-health claim and no accessibility-conformance claim.
