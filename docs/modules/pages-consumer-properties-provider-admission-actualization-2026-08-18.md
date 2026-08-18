# Pages consumer-properties provider admission actualization — 2026-08-18

Status: `source-ready / live-input-execution-pending / registry-update-blocked`.

## Fresh source boundary

This slice starts from fresh `main@4127d8015d6a846d5fbda89119f286eae28cd85a`. The canonical Page Builder FBA terminal inventory still contains exactly one recursive pending evidence node:

`/provider/consumer_properties_contract/executed_evidence`.

The source-only consumer contract is also intentionally still `executed_evidence: pending`. This slice does not change `executed_evidence` in either file and does not reduce terminal inventory from `1` to `0`.

## Gap closed by this slice

The repository already has three evidence families needed for provider consumer-properties admission:

1. the exact-main `Pages Consumer Properties Source Evidence` Rust receipt;
2. the retained published-metadata browser packet;
3. the Page Builder provider deployment identity packet that verifies the exact reported source commit on every expected live target while retaining the immutable RepoDigest as a `maintainer_reviewed_external_fact`.

Before this slice there was no dedicated fail-closed runner that bound those inputs together and checked their current source lineage before a registry update. The new source contract and runner make that boundary machine-readable instead of relying on a manual comparison.

## Admission source

The source contract is:

`crates/rustok-pages/contracts/evidence/pages-consumer-properties-provider-admission-source.json`.

The runner is:

`scripts/evidence/admit-pages-consumer-properties-provider.mjs`.

A successful run requires:

- a Rust receipt with format `pages_consumer_properties_source_execution_v1`, successful Rust/source status, canonical `push`/`main` GitHub Actions provenance, all commands passed, and both target values recorded as `pending`;
- a published-metadata browser packet with format `pages_published_metadata_browser_execution_v1`, successful browser status, all four required profiles passing with zero critical failures, no retained secrets or metadata values, and the consumer-properties admission still pending;
- a deployment identity packet with format `page_builder_provider_health_deployment_identity_v1`, complete expected-target inventory, every target returning the exact deployment source commit, and no raw metrics URLs/responses or credential values retained.

The browser packet and deployment identity must use the same exact source commit and immutable RepoDigest. The Rust receipt may come from an earlier ancestor commit only when every source file retained by its execution contract is byte-identical to the current checkout. The same retained-source check is applied independently to browser and deployment packets. Required-source drift therefore fails closed, while unrelated descendant commits do not force evidence re-execution.

The Rust source commit must also be an ancestor of the browser/deployment source commit. The runner requires the relevant commits to be available in checkout history for that ancestry check.

## Deployment provenance boundary

The deployment identity source already verifies the live Page Builder provider build-info source commit on every expected target. Its RepoDigest remains a maintainer-reviewed external identity; the packet explicitly records:

`origin_to_repo_digest_binding = maintainer_reviewed_external_fact`

and:

`cryptographic_origin_to_repo_digest_binding = false`.

This admission source preserves that distinction. Equality of the browser RepoDigest and deployment identity RepoDigest does not upgrade the reviewed external fact into cryptographic origin-to-image proof.

## Output boundary

A successful runner writes only a bounded packet with:

- format `pages_consumer_properties_provider_admission_v1`;
- status `provider_consumer_properties_inputs_admitted_registry_update_pending`;
- checkout/source lineage;
- reviewed deployment identity and exact-target source verification result;
- byte lengths and SHA-256 hashes of the three input packets;
- current pending target identities;
- SHA-256 hashes of the admission source set;
- explicit non-mutation/non-promotion boundaries.

Raw input paths, profile URLs, metrics URLs, cookies, credentials, DOM, metadata values and raw response bodies are not copied into the admission packet.

The runner performs no network requests, browser execution, Cargo execution, repository mutation, registry mutation or consumer-contract mutation.

## Fail-closed synthetic coverage

`scripts/evidence/admit-pages-consumer-properties-provider.test.mjs` covers one accepted synthetic exact-checkout chain and rejects:

- stale Rust retained-source hashes;
- pull-request provenance presented as an exact-main Rust receipt;
- a browser/deployment RepoDigest mismatch;
- a failed browser profile;
- incomplete deployment target verification;
- a target without exact-source verification;
- retained raw deployment URL evidence;
- an FBA-promotion claim inside a predecessor packet.

The static guard is:

`node scripts/verify/verify-pages-consumer-properties-provider-admission.mjs`.

The runner coverage is:

`node --test scripts/evidence/admit-pages-consumer-properties-provider.test.mjs`.

Both commands are wired into the existing `.github/workflows/fly-page-builder.yml` **Focused tests and source guards** job. Its pull-request/push path filter also includes the provider admission runner/test/verifier paths, so future changes to this source layer cannot bypass focused CI.

## Governance and next cursor

This is source readiness only. No live Rust receipt, browser execution or deployment identity execution is claimed by this PR.

After merge, the next valid sequence remains:

1. recover or execute an admissible Rust receipt;
2. execute the published-metadata browser packet against the reviewed deployment;
3. capture the exact-source live deployment identity for the same browser source and RepoDigest;
4. execute the provider admission runner and retain its successful packet;
5. in a separate evidence-containing PR, change only the admitted consumer-properties evidence target(s);
6. recompute the terminal inventory only after the canonical FBA registry update.

Pages still contains `execution-rollout-pending`, so Pages FFA and owner/platform terminal readiness remain independently blocked even after the Page Builder FBA evidence inventory eventually reaches zero.

No owner approval, platform approval, Pages FFA promotion or Page Builder FBA promotion is claimed.
