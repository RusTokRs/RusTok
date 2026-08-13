# Pages consumer-properties source execution actualization — 2026-08-13

Status: `source-ready / exact-main-rust-execution-pending / browser-evidence-pending`.

## Cursor

After static sanitization evidence admission and terminal-inventory recomputation, the first remaining Page Builder FBA provider blocker is `/provider/consumer_properties_contract/executed_evidence`.

The consumer-properties cutover source is connected. The metadata revision/isolation source packet and published metadata surface source packet are still execution-pending, and the retained published metadata browser harness is source-ready but has not been executed against a reviewed deployment.

This slice defines the exact-main Rust/source execution packet only. It does not execute Chromium and it does not change the consumer contract or FBA registry.

## Exact-main source execution

The workflow is:

`.github/workflows/pages-consumer-properties-source-evidence.yml`

It is push-main only and read-only. A branch or local run cannot mint the retained receipt. The recorder requires:

- `GITHUB_ACTIONS=true`;
- `GITHUB_SHA` equal to checkout `HEAD`;
- canonical repository `RusTokRs/RusTok`;
- workflow name `Pages Consumer Properties Source Evidence`;
- event `push`;
- branch `main`;
- both the consumer contract and FBA consumer-properties nodes still `pending`.

## Source gates

Before Cargo execution, the workflow runs the current source guards for:

1. registered consumer properties;
2. metadata revision/isolation;
3. selected published metadata surface;
4. the retained published metadata browser harness;
5. this exact-main execution source contract.

The browser-harness verifier proves only source readiness. It does not execute a browser.

## Focused Rust execution

The workflow inventories the test list and requires four focused Rust regressions:

1. `standalone_metadata::tests::published_page_admits_registered_metadata_surface`;
2. `standalone_metadata::tests::non_published_or_missing_page_hides_registered_metadata_surface`;
3. `metadata_properties::tests::stale_metadata_revision_short_circuits_before_patch_transport`;
4. `metadata_properties::tests::metadata_save_is_document_free_and_preserves_dirty_fly_state`.

It then executes all four focused tests and `cargo check --locked -p rustok-pages-admin --all-targets`.

## Retained receipt

Only after all source verifiers, test inventory, four focused Rust regressions and Cargo check pass may the recorder write:

- format `pages_consumer_properties_source_execution_v1`;
- status `rust_source_execution_passed_browser_evidence_pending`.

The receipt retains exact source commit, GitHub run identity, hashes of the consumer contract and FBA registry pre-state, the exact pending JSON pointers, and SHA-256 hashes for every required source file.

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
