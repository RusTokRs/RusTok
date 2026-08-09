# Pages / Page Builder provider-health server binding actualization — 2026-08-09

Status: `server-provider-health-binding-source-ready / hot-accept-reject-source-ready / freshness-lease-source-ready / consumer-binding-source-ready / maintainer-activation-pending`.

## Scope

This overlay defines the fail-closed Pages host/server authority for deployment provider health. The later consumer continuation in `docs/modules/pages-page-builder-provider-health-consumer-binding-actualization-2026-08-09.md` supersedes the original consumer-open cursor: workspace, authoritative SSR and standalone browser-intent health binding are now source-ready, while live activation remains maintainer-owned.

Pages has a deployment-bound `PagesProviderHealthAuthority` that can expose a health snapshot only when a retained owner-acceptance packet is present, accepted, exact-identity bound and still fresh. The authority is published through the existing `ModuleRuntimeExtensions -> HostRuntimeContext -> GraphqlRuntimeInputs` path; Pages uses the standard manifest `runtime_data_factory`, so no parallel GraphQL schema wiring exists.

## Explicit host configuration

Binding is opt-in and requires:

- `RUSTOK_PAGES_PROVIDER_HEALTH_ACCEPTANCE_PATH`;
- `RUSTOK_PAGES_PROVIDER_HEALTH_DEPLOYMENT_ID`;
- `RUSTOK_PAGES_PROVIDER_HEALTH_DEPLOYMENT_IMAGE_DIGEST`;
- existing `RUSTOK_SOURCE_COMMIT`.

All Pages-specific binding values absent means no authority is published. Partial/invalid configuration is rejected to the unobserved state with a bounded warning rather than creating an application-startup outage.

The configured packet path must be absolute. It may be absent when the module starts so a maintainer can atomically install an accepted packet later; if it exists, metadata errors, symlinks and non-files are rejected. Actual packet bytes are bounded and revalidated on each rollout-status read.

## Accepted packet and exact identity

Only `pages_builder_provider_health_owner_acceptance_v1` with status `owner_accepted_server_binding_pending`, decision `accept_for_pages_binding` and rollback action `restore_unobserved_provider_health` can authorize observed health.

Every read revalidates:

```text
packet deployment.source_commit
= packet binding.required_live_source_commit
= RUSTOK_SOURCE_COMMIT

packet deployment.deployment_id
= RUSTOK_PAGES_PROVIDER_HEALTH_DEPLOYMENT_ID

packet deployment.deployment_image_digest
= packet binding.required_deployment_image_digest
= RUSTOK_PAGES_PROVIDER_HEALTH_DEPLOYMENT_IMAGE_DIGEST
```

The parser is strict, unknown fields are rejected, source/deployment identity shapes are checked again, prior promotion claims must remain false, and raw evaluator/Prometheus material is not accepted.

## Remaining-freshness lease

Owner acceptance does not restart the evaluator freshness clock. It retains the maximum target-operation freshness age and exact:

```text
health_valid_until
  = evaluation.evaluated_at
  + deployment.freshness_seconds
  - evaluation.max_target_operation_freshness_age_seconds
```

The server independently recomputes this deadline and admits observed health only while:

```text
server_now <= evaluation.health_valid_until + 5s bounded clock-skew tolerance
```

Target counts, query/freshness bounds, identity age, minimum Preview/Publish populations, timestamps, canonical `ProviderHealthSnapshot::evaluate` and canonical `ProviderSloEvaluation::evaluate` are also revalidated.

Missing, rejected, malformed, identity-mismatched or expired evidence returns `None`. The explicit retained path is reread for every `pageBuilderRolloutSnapshot` request, so atomic accepted→rejected replacement, removal or later reaccept can revoke/restore observed transport without process restart.

## GraphQL composition

`PagesModule::register_runtime_extensions` publishes `SharedPagesProviderHealthAuthority`. The Pages manifest declares:

```toml
runtime_data_factory = "graphql::attach_schema_data"
```

`PagesGraphqlRuntimeData` reads the shared authority. `pageBuilderRolloutSnapshot` keeps its literal unobserved constructor (`providerHealthObserved=false`, `providerHealth=null`) and attaches health only after current authority/freshness admission.

The admin transport independently re-evaluates the typed health payload, so host evidence admission and client/admin transport validation remain separate fail-closed boundaries.

## Consumer continuation

The subsequent consumer-binding slice now consumes this validated optional health end-to-end:

- workspace passes `PagesBuilderRolloutSnapshot::provider_status()` into the canonical Page Builder admin facade;
- authoritative Preview/Publish SSR re-fetches the snapshot per request and composes handlers from `PageBuilderAdminProviderStatus::effective_runtime_flags()`;
- standalone browser-intent applies `pages_editor_capabilities_for_snapshot` after role capability evaluation.

The shared policy only narrows configured rollout state: Degraded disables Publish, Unavailable disables the builder, and Ready/Unobserved preserve configured flags. No health state can grant a capability.

Pages remains `unobserved` without a live accepted packet. Source-ready consumer wiring does not imply that a packet was installed or that observed behavior was executed.

## Non-promotion

No deployment identity capture, Prometheus evaluation, owner acceptance, accepted packet installation, observed GraphQL/consumer request, Pages gate acceptance, Forum Wave, FFA or FBA promotion is claimed by this source slice.

## Current cursor

```text
bounded runtime observation [source-ready]
-> deployment metrics/freshness [source-ready]
-> exact deployment identity [source-ready]
-> deployment evaluator [source-ready]
-> typed observed-health transport [source-ready]
-> owner acceptance packet + exact health_valid_until [source-ready]
-> server provider-health binding + hot revoke + remaining-freshness lease [source-ready]
-> UI / SSR / browser-intent provider-health binding [source-ready]
-> retained identity + evaluator + accepted owner packet + observed consumer behavior [maintainer execution pending]
-> observed-health acceptance decision [pending]
```

## Validation boundary

Tests were not run. Node verifiers, Cargo commands, formatting, builds, GraphQL/HTTP requests, deployment identity capture, Prometheus queries, evaluator execution, owner-acceptance execution, accepted-packet installation, browser runs, workflows and CI were intentionally not executed.

Suggested maintainer checks, intentionally not run:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-consumer-binding.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-server-binding.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-transport.mjs
node crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs
```
