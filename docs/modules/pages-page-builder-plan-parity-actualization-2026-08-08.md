# Pages / Page Builder plan parity actualization — 2026-08-08

Status: `canonical-plan-parity-source-ready / forum-runtime-composition-source-ready / pages-reference-consumer-rollout-source-ready / provider-runtime-observation-source-ready / deployment-metrics-source-ready / freshness-signal-source-ready / deployment-identity-contract-source-ready / expected-target-inventory-contract-source-ready / deployment-health-evaluator-source-ready / provider-health-transport-source-ready / provider-health-owner-acceptance-source-ready / provider-health-server-binding-source-ready / execution-acceptance-pending`.

## Current authority

This parity packet now has nine source actualizations:

- the earlier Forum composition reconciliation through PR #3320;
- `docs/modules/pages-page-builder-rollout-plan-actualization-2026-08-08.md`, which supersedes older rollout-specific wording after PRs #3333, #3337, #3345 and #3353;
- `docs/modules/page-builder-provider-health-runtime-observation-actualization-2026-08-09.md`, which introduced bounded process-local runtime observation source;
- `docs/modules/page-builder-provider-health-deployment-metrics-actualization-2026-08-09.md`, which exports the same terminal observations through platform-owned deployment-aggregatable Prometheus metrics and a per-operation freshness signal;
- `docs/modules/page-builder-provider-health-deployment-identity-actualization-2026-08-09.md`, which defines the exact source/deployment identity and expected-target inventory capture contract;
- `docs/modules/page-builder-provider-health-deployment-evaluator-actualization-2026-08-09.md`, which defines the fail-closed backend evaluator over identity-admitted targets;
- `docs/modules/pages-page-builder-provider-health-transport-actualization-2026-08-09.md`, which defines a typed observed-health GraphQL/admin transport with canonical client revalidation;
- `docs/modules/pages-page-builder-provider-health-owner-acceptance-actualization-2026-08-09.md`, which defines the fail-closed owner acceptance packet over retained deployment evaluation;
- `docs/modules/pages-page-builder-provider-health-server-binding-actualization-2026-08-09.md`, which defines host/module authority, exact live identity revalidation, hot accepted/rejected packet replacement and a remaining-freshness lease while leaving live activation and capability-consumer binding pending.

The larger shared/local/central plans remain useful for the full Pages/Page Builder programme. Where an older paragraph still refers to hardcoded Pages rollout flags, rollout binding as pending, a matrix that is only executable but not source-defined, or a reference candidate that consumes only artifact/browser evidence, the rollout actualization is the current source cursor. Where older text says there is no live provider-health observation, transport, owner-acceptance or server-binding source, the 2026-08-09 provider-health overlays are the current refinement: local Preview/Publish observation, deployment-aggregatable metrics/freshness, exact source/deployment identity + expected-target inventory, deployment-health evaluator, typed observed-health transport, owner acceptance packet, and fail-closed server binding are source-ready. Pages remains `unobserved` in retained execution evidence because exact identity/evaluator/owner acceptance have not been executed and installed for the live deployment, and current UI/SSR/browser-intent capability consumers still use rollout-only status.

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
- the reference candidate requires artifact/HTTP, browser, rollout runtime matrix and canonical feature-preflight packets bound to one exact source/deployment chain;
- default Fly composition retains bounded process-local Preview/Publish terminal observations through the existing runtime-telemetry seam, with a 256-sample cap per operation and no health snapshot below 20 Preview plus 20 Publish samples;
- the same matched terminal calls export platform-owned Prometheus duration histograms, terminal outcome counters and per-operation last-observation timestamps with fixed `preview|publish` / terminal-outcome labels only;
- deployment metrics deliberately carry no tenant/page/revision/correlation/deployment application labels; scrape/discovery infrastructure owns target identity and reset-aware aggregation;
- provider metrics expose `rustok_page_builder_provider_build_info{source_commit="<sha>"} 1` only when `RUSTOK_SOURCE_COMMIT` is canonical, reusing the release `github.sha -> OCI_REVISION -> RUSTOK_SOURCE_COMMIT` identity chain;
- the deployment identity capture contract requires a maintainer-supplied immutable image RepoDigest plus a complete 1..64 expected-target inventory, rejects partial target verification, and requires every target to report the exact checkout source commit;
- the deployment-health evaluator requires a complete 1:1 mapping from those exact target ids to bounded Prometheus exact-match topology labels instead of assuming a particular `instance`/`job` convention;
- evaluator source admission requires current exact build-info, admitted-source build-info samples across the full query window, no unexpected source build-info inside that window, unique current backend series per expected target and complete target success;
- Prometheus `time()` is the evaluator clock; the query window is explicitly bounded to 300..86400 seconds and Preview/Publish freshness is required for every expected target with an explicit freshness bound no larger than that window;
- evaluator aggregation is reset-aware through `increase(...)`, requires at least 20 Preview and 20 Publish terminal completions, sums cumulative histogram buckets across admitted targets before p95 evaluation, and applies the same 1500ms / 3000ms / 1% / 1% provider-health policy as Rust `ProviderHealthSnapshot::evaluate`;
- a retained deployment health evaluator packet can therefore contain a deployment-bound provider snapshot and SLO evaluation without raw Prometheus URL, raw PromQL, raw backend responses, raw target matcher values or credential values;
- Pages GraphQL has a typed optional provider-health payload alongside `providerHealthObserved`; its default rollout snapshot remains literal `provider_health_observed: false` plus `provider_health: None`;
- the stateless admin transport enforces boolean/payload consistency, rejects invalid failure rates, recomputes canonical `ProviderHealthSnapshot::evaluate`, and rejects transported state/reasons that do not match that canonical result;
- `PagesBuilderRolloutSnapshot` can retain validated optional health and derive `PageBuilderAdminProviderStatus`, while current workspace, authoritative SSR facade and standalone browser-intent paths still deliberately consume flags only;
- the owner acceptance packet runner admits only retained `page_builder_provider_health_deployment_evaluation_v1` evidence under repository `target/`, requires evaluation source commit to equal checkout `HEAD`, rehashes the evaluator source set against the checkout, rejects incomplete target/sample/histogram populations and recomputes health state/reasons/SLO evaluation before recording any decision;
- owner acceptance also takes the maximum retained Preview/Publish freshness age across every admitted target, derives exact `health_valid_until = evaluated_at + freshness_seconds - max_freshness_age`, retains that deadline, and rejects `accept_for_pages_binding` once that remaining freshness budget is exhausted beyond the bounded clock-skew tolerance;
- accepted owner evidence requires explicit `accept_for_pages_binding` plus rollback action `restore_unobserved_provider_health`; rejection is also retained, while the owner identifier is explicitly an operator assertion rather than a cryptographic signature;
- Pages server binding is opt-in through `RUSTOK_PAGES_PROVIDER_HEALTH_ACCEPTANCE_PATH`, `RUSTOK_PAGES_PROVIDER_HEALTH_DEPLOYMENT_ID`, `RUSTOK_PAGES_PROVIDER_HEALTH_DEPLOYMENT_IMAGE_DIGEST` and the existing `RUSTOK_SOURCE_COMMIT`; absent or partial/invalid binding configuration yields unobserved health rather than a fabricated snapshot;
- `PagesModule::register_runtime_extensions` publishes a `SharedPagesProviderHealthAuthority`, the Pages manifest uses the standard `graphql::attach_schema_data` runtime-data factory, and `pageBuilderRolloutSnapshot` consumes only that typed runtime authority rather than reading evidence/configuration directly;
- the authority revalidates accepted packet contract, exact live source/deployment identity, target/sample/query/freshness bounds, exact remaining-freshness deadline and canonical Rust health/SLO policy on every rollout-status read;
- accepted packets are observed only through the retained `evaluation.health_valid_until` deadline plus the explicit five-second clock-skew tolerance; missing, rejected, malformed, identity-mismatched or expired packets return the default unobserved GraphQL shape;
- the retained packet path is reread per rollout-status request, so accepted→rejected replacement, packet removal or later accepted replacement can revoke/restore server observed transport without a process restart;
- raw metrics target URLs, metric bodies and credential values are not retained by identity capture; target URL/body digests and credential environment names are retained instead;
- the image RepoDigest association remains a maintainer-reviewed external fact because the running process cannot cryptographically derive its post-push RepoDigest from source SHA alone;
- source inspection does not execute target identity capture, Prometheus queries, deployment evaluator, owner acceptance, accepted packet installation or an observed GraphQL request and therefore does not produce runtime health evidence;
- the process-local window remains restartable and is not deployment-wide health authority; pre-telemetry validation/inspection is outside its current measurement boundary;
- Pages remains `unobserved` in current retained execution evidence: a live accepted packet matching the configured deployment has not been produced/installed, and current capability consumers have not been promoted to health-driven narrowing;
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
-> deployment health backend evaluator [source-ready]
-> typed observed-health transport [source-ready]
-> owner acceptance packet + exact health_valid_until [source-ready]
-> server provider-health binding + hot revoke + remaining-freshness lease [source-ready / maintainer activation pending]
-> live exact-target identity capture + retained deployment health evaluator packet + accepted owner packet [maintainer execution pending]
-> UI / SSR / browser-intent provider-health binding [source-open / runtime activation blocked]
-> observed-health acceptance decision [pending]
```

Source inspection alone must not mark any execution or acceptance step complete. Raw/process-local observations, unbound Prometheus data, a source-ready inventory without live complete target capture, a source-ready evaluator without retained runtime output, typed transport without owner authority, an unexecuted owner-acceptance source, or server-binding code without a live accepted packet must not be substituted for exact deployment provider-health evidence.

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
crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-evaluator.mjs
crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-transport.mjs
crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-owner-acceptance.mjs
crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-server-binding.mjs
```

The first guard locks the bounded process-local window. The second locks platform metric names, bounded label vocabulary, registry wiring and reset-aware aggregation/freshness source. The third locks release source identity, build-info, complete expected-target inventory and fail-closed direct capture. The fourth locks complete backend target mapping, exact-source window admission, backend-clock freshness, sample floors, histogram aggregation and Rust health-policy parity. The fifth locks the typed GraphQL/admin health shape and canonical client re-evaluation. The sixth locks owner acceptance, exact evaluator/source/deployment admission, maximum retained operation freshness age, the remaining-freshness deadline and explicit rollback. The seventh locks Pages host authority composition, live source/deployment identity binding, hot packet revoke/reaccept, exact deadline enforcement and the continued absence of UI/SSR/browser-intent health-driven capability binding.

The Pages reference-consumer gate continues to list the plan-parity verifier as a required source guard.

## Execution boundary

No tests, Node verifiers, Cargo commands, formatting, builds, Prometheus scrapes, backend queries, deployment identity captures, evaluator executions, owner acceptance executions, accepted packet installations, observed GraphQL/HTTP requests, Playwright/browser runs, workflows, CI, migrations or runtime evidence were executed by this slice.

Suggested maintainer commands, intentionally not run:

```bash
node crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-runtime-observation.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-metrics.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-identity.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-evaluator.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-transport.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-owner-acceptance.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-server-binding.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-server-snapshot.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-binding.mjs
```

All execution and acceptance evidence remains maintainer-owned.
