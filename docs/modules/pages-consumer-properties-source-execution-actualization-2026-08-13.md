# Pages consumer-properties source execution actualization — 2026-08-13

Status: `source-ready / exact-main-rust-execution-pending / browser-evidence-pending`.

## Recheck — 2026-08-18

Rechecked from fresh `main@eee796fb81dbd893994e4bfa0f3364dd0bbf7d1d` after terminal inventory PR #3628 reduced the recursive Page Builder FBA blocker set to exactly one node:

`/provider/consumer_properties_contract/executed_evidence`.

The historical exact-main Rust/source receipt from workflow run `31702550557` on `a8bf89c642baa7d1e70bab8c3439fd5d19ed6d8f` remains useful lineage for the already-admitted nested Pages metadata-properties evidence, but it is not admissible for the current provider-level consumer-properties admission. Current `main` is an exact descendant and required receipt-bound source files have changed since that execution, including:

- `.github/workflows/pages-consumer-properties-source-evidence.yml`;
- `.github/workflows/pages-published-metadata-browser-evidence.yml`;
- `apps/next-admin/playwright.pages-published-metadata.config.ts`;
- `apps/next-admin/tests/pages-published-metadata/browser-evidence.spec.ts`;
- `apps/next-admin/tests/pages-published-metadata/global-setup.ts`.

Therefore the provider target remains `pending` and requires a fresh exact-main Rust/source receipt. The 2026-08-18 revalidation merge intentionally changed this tracked actualization without runtime or registry mutation so the existing push-to-main `Pages Consumer Properties Source Evidence` workflow could execute against the exact merged source and retain a fresh bounded receipt.

That fresh Rust/source receipt is still only one prerequisite. Provider admission additionally requires a successful retained published-metadata browser packet against the reviewed deployment plus reviewed deployment provenance and admissible source lineage. Until those browser/deployment requirements are present, neither the consumer-properties contract nor `/provider/consumer_properties_contract/executed_evidence` may be changed to `verified`, and terminal FBA inventory must remain `1` rather than being falsely promoted to `0`.

## Lifecycle hardening — 2026-08-18

The exact-main source workflow is now made directly re-runnable and observable instead of requiring another receipt-bound touch commit whenever maintainers need a fresh source receipt.

- `pull_request` against `main` executes the source guards, test inventory, four focused Rust regressions and Cargo check, but deliberately skips receipt generation and artifact upload;
- `push` to `main` keeps the existing receipt path;
- `workflow_dispatch` provides manual exact-main revalidation without changing repository source;
- the recorder still refuses any receipt unless the checkout equals `GITHUB_SHA`, the repository/workflow identities are canonical, the event is `push` or `workflow_dispatch`, the ref is `main`, and both consumer-properties evidence targets remain `pending`;
- manual dispatch from a non-main ref fails before Cargo execution;
- after a push/main or dispatch/main run, the gate publishes the commit-status context `pages-consumer-properties-source-evidence-index` pointing at the exact GitHub Actions run;
- that status is an evidence discovery index only: it mutates commit status metadata, not repository content, the consumer contract or the FBA registry.

This fixes the previous observability loop: the exact-main `run_id` can be recovered from the commit status and then used to inspect jobs and the retained artifact without manufacturing another documentation change. It does not weaken browser/deployment admission and does not promote any evidence state.

## Cursor

After static sanitization evidence admission and terminal-inventory recomputation, the first remaining Page Builder FBA provider blocker is `/provider/consumer_properties_contract/executed_evidence`.

The consumer-properties cutover source is connected. The metadata revision/isolation source packet and published metadata surface source packet are still execution-pending, and the retained published metadata browser harness is source-ready but has not been executed against a reviewed deployment.

This slice defines the exact-main Rust/source execution packet only. It does not execute Chromium and it does not change the consumer contract or FBA registry.

## Exact-main source execution

The workflow is:

`.github/workflows/pages-consumer-properties-source-evidence.yml`

The workflow is read-only with respect to repository contents. PR runs validate the exact PR source but cannot mint the retained receipt. Receipt creation is limited to exact `main` runs from either a normal push or manual exact-main revalidation through `workflow_dispatch`. The recorder requires:

- `GITHUB_ACTIONS=true`;
- `GITHUB_SHA` equal to checkout `HEAD`;
- canonical repository `RusTokRs/RusTok`;
- workflow name `Pages Consumer Properties Source Evidence`;
- event `push` or `workflow_dispatch`;
- branch `main`;
- both the consumer contract and FBA consumer-properties nodes still `pending`.

The gate may write only the bounded GitHub commit-status index `pages-consumer-properties-source-evidence-index`; it has no repository-content write permission.

## Source gates

Before Cargo execution, the workflow runs the current source guards for:

1. registered consumer properties;
2. metadata revision/isolation;
3. selected published metadata surface;
4. the retained published metadata browser harness;
5. the published metadata browser execution workflow;
6. this exact-main execution source contract.

The browser-harness and browser-workflow verifiers prove only source readiness. They do not execute a browser.

## Focused Rust execution

The workflow inventories the test list and requires four focused Rust regressions:

1. `standalone_metadata::tests::published_page_admits_registered_metadata_surface`;
2. `standalone_metadata::tests::non_published_or_missing_page_hides_registered_metadata_surface`;
3. `metadata_properties::tests::stale_metadata_revision_short_circuits_before_patch_transport`;
4. `metadata_properties::tests::metadata_save_is_document_free_and_preserves_dirty_fly_state`.

It then executes all four focused tests and `cargo check --locked -p rustok-pages-admin --all-targets`.

## Retained receipt

Only after all source verifiers, test inventory, four focused Rust regressions and Cargo check pass may an exact-main push/dispatch recorder write:

- format `pages_consumer_properties_source_execution_v1`;
- status `rust_source_execution_passed_browser_evidence_pending`.

The receipt retains exact source commit, GitHub run identity including the real event name, hashes of the consumer contract and FBA registry pre-state, the exact pending JSON pointers, and SHA-256 hashes for every required source file.

The receipt does not embed raw test logs, credentials, cookies, tenant identity, GraphQL/browser payloads or metadata values.

## Governance boundary

A successful Rust/source receipt is **not** consumer-properties admission. In particular:

- consumer properties executed evidence remains `pending`;
- `/provider/consumer_properties_contract/executed_evidence` remains `pending`;
- browser evidence remains pending;
- deployment provenance remains externally reviewable and unverified by this packet;
- terminal inventory remains incomplete;
- owner/platform approval is not claimed;
- Pages FFA and Page Builder FBA are not promoted.

A later evidence-containing admission must bind the exact Rust receipt, a successful retained browser packet, reviewed deployment provenance and source lineage before either pending consumer-properties evidence node may change.
