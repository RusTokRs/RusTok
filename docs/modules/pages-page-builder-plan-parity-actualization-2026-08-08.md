# Pages / Page Builder plan parity actualization — 2026-08-08

Status: `canonical-plan-parity-source-ready / forum-runtime-composition-source-ready / pages-reference-consumer-rollout-source-ready / provider-runtime-observation-source-ready / deployment-metrics-source-ready / freshness-signal-source-ready / deployment-identity-contract-source-ready / expected-target-inventory-contract-source-ready / deployment-health-evaluator-open / execution-acceptance-pending`.

## Current authority

This parity packet now has five source actualizations:

- the earlier Forum composition reconciliation through PR #3320;
- `docs/modules/pages-page-builder-rollout-plan-actualization-2026-08-08.md`, which supersedes older rollout-specific wording after PRs #3333, #3337, #3345 and #3353;
- `docs/modules/page-builder-provider-health-runtime-observation-actualization-2026-08-09.md`, which introduced bounded process-local runtime observation source;
- `docs/modules/page-builder-provider-health-deployment-metrics-actualization-2026-08-09.md`, which exports the same terminal observations through platform-owned deployment-aggregatable Prometheus metrics and a per-operation freshness signal;
- `docs/modules/page-builder-provider-health-deployment-identity-actualization-2026-08-09.md`, which defines the exact source/deployment identity and expected-target inventory capture contract while leaving live capture and the deployment health evaluator pending.

The larger shared/local/central plans remain useful for the full Pages/Page Builder programme. Where an older paragraph still refers to hardcoded Pages rollout flags, rollout binding as pending, a matrix that is only executable but not source-defined, or a reference candidate that consumes only artifact/browser evidence, the rollout actualization is the current source cursor. Where older text says there is no live provider-health observation source at all, the 2026-08-09 provider-health overlays are the current refinement: local Preview/Publish observation, deployment-aggregatable metrics/freshness, and the exact source/deployment identity + expected-target inventory capture contract are source-ready. Pages remains `unobserved` because live target identity capture, the deployment health backend evaluator and retained runtime evidence do not yet exist.

## Current source truth

The synchronized source boundary is now:

- Pages source architecture remains complete with execution evidence open;
- Forum is the second production Page Builder consumer;
- Forum canonical metadata, Fly adapter/component registry, owner preview and owner-backed property editing are source-ready;
- Forum persistence, visibility, widget schemas, validation and authorization remain Forum-owned;
- Pages rollout settings are server-owned and persisted per tenant;
- Pages UI provider status, authoritative Preview/Publish SSR composition and standalone browser-intent preflight consume server-owned rollout state;
- the four canonical rollout profiles have a bounded real-consumer runtime-matrix harness with production settings writes, Pages reads, UI/SSR/bypass checks and verified settings restoration;
- standalone browser-intent denial remains the distinct `FLY_CAPABILITY_DENIED` security contract;
- the canonical provider degraded error catalog is separately proved through a non-mutating server-owned `feature-disabled / FEATURE_DISABLED` capability preflight;
- the reference candidate requires artifact/HTTP, browser, runtime-matrix and canonical feature-preflight packets bound to one exact source/deployment chain;
- default Fly composition retains bounded process-local Preview/Publish terminal observations through the existing runtime-telemetry seam, with a 256-sample cap per operation and no health snapshot below 20 Preview plus 20 Publish samples;
- the same matched terminal calls export platform-owned Prometheus duration histograms, terminal outcome counters and per-operation last-observation timestamps with fixed `preview|publish` / terminal-outcome labels only;
- deployment metrics deliberately carry no tenant/page/revision/correlation/deployment application labels; scrape/discovery infrastructure owns target identity and reset-aware aggregation;
- provider metrics now expose `rustok_page_builder_provider_build_info{source_commit="<sha>"} 1` only when `RUSTOK_SOURCE_COMMIT` is canonical, reusing the release `github.sha -> OCI_REVISION -> RUSTOK_SOURCE_COMMIT` identity chain;
- the deployment identity capture contract requires a maintainer-supplied immutable image RepoDigest plus a complete 1..64 expected-target inventory, rejects partial target verification, and requires every target to report the exact checkout source commit;
- raw metrics target URLs, metric bodies and credential values are not retained by identity capture; target URL/body digests and credential environment names are retained instead;
- the image RepoDigest association is intentionally recorded as a maintainer-reviewed external fact because the running process cannot cryptographically derive its post-push RepoDigest from source SHA alone;
- source inspection does not supply a production target inventory, execute target capture, choose the backend health window, or evaluate deployment SLO health;
- the process-local window remains restartable and is not deployment-wide health authority; pre-telemetry validation/inspection is outside its current measurement boundary;
- Pages remains `unobserved`: `provider_health_observed = false` and admin provider health are intentionally not promoted until exact live deployment identity is captured and an admitted deployment evaluator produces retained evidence;
- `pages_reference_consumer_gate` remains `accepted = false` and `execution_gate = pending`;
- Forum browser/runtime/deployment evidence and observed Wave remain blocked by the Pages gate;
- FFA/FBA promotion remains unclaimed.

## Current next cursor

No additional Pages/Page Builder rollout architecture slice is identified by the source reconciliation.

The rollout acceptance cursor remains maintainer execution in this order:

```text
artifact/HTTP
-> browser
-> rollout runtime matrix
-> canonical FEATURE_DISABLED preflight
-> reference-consumer candidate
-> owner sign-off + explicit rollback decision
-> Pages gate acceptance
-> Forum browser/runtime/deployment evidence and observed Wave
```

In parallel, the provider-health source cursor is now:

```text
bounded process-local Preview/Publish observation [source-ready]
-> deployment-aggregatable metrics + freshness signal [source-ready]
-> exact source/deployment identity + expected-target inventory contract [source-ready]
-> live exact-target identity capture [maintainer execution pending]
-> deployment health backend evaluator [open]
-> Pages provider-status transport/binding [blocked]
-> retained deployment/runtime evidence [maintainer execution]
-> observed-health acceptance decision [pending]
```

Source inspection alone must not mark any execution or acceptance step complete. Raw/process-local, unbound Prometheus observations, or a source-ready inventory contract without a live complete target capture must not be substituted for exact deployment provider-health evidence.

## Anti-drift guard

`crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs` continues to source-lock the synchronized rollout cursor across:

- the shared Pages/Page Builder parity plan;
- the local Page Builder implementation plan;
- the central Page Builder plan;
- the rollout-specific current actualization;
- the Pages reference-consumer gate source packet;
- the matrix/feature-preflight candidate registration;
- the current Forum contribution manifest;
- the Forum Wave source packet.

The guard rejects the former `forum-fly-adapter-open`/discovery-only cursor, an accepted Pages gate without execution evidence, fabricated provider health, Forum Wave promotion while the Pages gate remains pending, and any plan wording that treats `FLY_CAPABILITY_DENIED` as equivalent to the canonical provider `FEATURE_DISABLED` contract.

Provider-health source is independently fail-closed by:

```text
crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-runtime-observation.mjs
crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-metrics.mjs
crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-identity.mjs
```

The first guard locks the bounded process-local window. The second locks the platform metric names, bounded label vocabulary, registry wiring, reset-aware aggregation/freshness contract and the continued absence of Pages observed-health binding. The third locks the release source identity, build-info metric, complete expected-target inventory contract, fail-closed capture harness and continued non-promotion of Pages health.

The Pages reference-consumer gate continues to list the plan-parity verifier as a required source guard.

## Execution boundary

No tests, Node verifiers, Cargo commands, formatting, builds, Prometheus scrapes, backend queries, deployment identity captures, GraphQL/HTTP requests, Playwright/browser runs, workflows, CI, migrations or runtime evidence were executed by this slice.

Suggested maintainer commands, intentionally not run:

```bash
node crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-runtime-observation.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-metrics.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-identity.mjs
```

All execution and acceptance evidence remains maintainer-owned.
