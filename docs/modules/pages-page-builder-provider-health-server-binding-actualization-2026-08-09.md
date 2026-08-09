# Pages / Page Builder provider-health server binding actualization — 2026-08-09

Status: `server-provider-health-binding-source-ready / hot-accept-reject-source-ready / freshness-lease-source-ready / maintainer-activation-pending / ui-ssr-browser-binding-open`.

## Scope

This slice continues the provider-health cursor after the deployment evaluator, typed transport and owner-acceptance packet became source-ready. It closes the missing host/server binding architecture without fabricating runtime evidence or owner acceptance.

Pages now has a deployment-bound `PagesProviderHealthAuthority` that can expose a health snapshot only when a retained owner-acceptance packet is present, accepted, exact-identity bound and still fresh. The authority is published through the existing `ModuleRuntimeExtensions -> HostRuntimeContext -> GraphqlRuntimeInputs` composition path; Pages declares the standard manifest `runtime_data_factory`, so no parallel GraphQL schema wiring is introduced.

## Explicit host configuration

Binding is opt-in and requires all of the following environment values:

- `RUSTOK_PAGES_PROVIDER_HEALTH_ACCEPTANCE_PATH` — an absolute path to the retained owner-acceptance packet;
- `RUSTOK_PAGES_PROVIDER_HEALTH_DEPLOYMENT_ID` — the live deployment id expected by the accepted packet;
- `RUSTOK_PAGES_PROVIDER_HEALTH_DEPLOYMENT_IMAGE_DIGEST` — the maintainer-reviewed immutable `REPOSITORY@sha256:<64hex>` RepoDigest for the live deployment;
- `RUSTOK_SOURCE_COMMIT` — the existing release/build-info source identity already used by Page Builder metrics.

When the Pages-specific binding variables are all absent, no authority is published and GraphQL remains `providerHealthObserved=false` with no payload. Partial or invalid binding configuration is rejected to the same unobserved state with a bounded warning; it does not turn a provider-health configuration mistake into an application-startup outage.

The configured absolute packet path may be absent when the module starts, so a maintainer can atomically install the accepted packet later. If a path already exists at startup it must resolve to a regular non-symlink file; permission/metadata errors are rejected rather than treated as absence. Actual packet bytes remain strictly bounded and validated on every GraphQL status read.

## Accepted packet admission

The binding accepts only `pages_builder_provider_health_owner_acceptance_v1` with status `owner_accepted_server_binding_pending` and decision `accept_for_pages_binding`. The accepted rollback action must be `restore_unobserved_provider_health`.

Every read is fail-closed. The packet must be a bounded regular non-symlink file, parse against a strict `deny_unknown_fields` v1 shape and preserve the owner-acceptance nonclaims. The binding rejects packets that claim a cryptographic owner signature, retain a free-text reason/raw evaluator path, omit prior source-hash verification, or already claim Pages/Forum/FFA/FBA promotion.

The packet is re-bound to live host identity on every read:

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

Source commit, deployment id and RepoDigest shapes are validated again rather than treated as arbitrary strings.

## Evidence and policy revalidation

Server binding does not blindly trust the owner packet's derived snapshot. It rechecks the retained evaluator envelope:

- expected target count is 1..64 and equals verified backend target count;
- query window remains 300..86400 seconds;
- freshness is at least 60 seconds and no larger than the query window;
- retained identity age covers the query window and is no older than 86400 seconds;
- Preview and Publish each retain at least 20 samples;
- evaluator, decision and `health_valid_until` timestamps are canonical UTC millisecond timestamps;
- owner decision does not predate evaluation and cannot exceed the retained health deadline beyond the explicit five-second clock-skew tolerance;
- the maximum retained Preview/Publish freshness age must be finite, non-negative and no greater than the configured freshness bound;
- the server independently recomputes `health_valid_until` from evaluator time plus **remaining** freshness budget;
- `ProviderHealthSnapshot::evaluate` is recomputed from retained observations and must equal the retained state, reasons and thresholds;
- `ProviderSloEvaluation::evaluate` is recomputed and must equal the retained SLO result.

This keeps Rust policy authoritative at the final server binding boundary.

## Remaining-freshness deadline

Owner acceptance no longer restarts the evaluator freshness clock. The evaluator already records an age for each expected target's Preview and Publish last observation. The acceptance runner takes the maximum of those retained ages and derives:

```text
remaining_freshness_seconds
  = deployment.freshness_seconds
  - evaluation.max_target_operation_freshness_age_seconds

health_valid_until
  = evaluation.evaluated_at
  + remaining_freshness_seconds
```

An `accept_for_pages_binding` decision is rejected when the maintainer clock is already beyond `health_valid_until` plus the five-second skew tolerance. The accepted packet retains both `max_target_operation_freshness_age_seconds` and canonical `health_valid_until` so the server can recompute and verify the same deadline without access to raw Prometheus data.

This avoids silently granting a second full freshness window after evaluator execution or owner review.

## Freshness lease and rollback

The accepted packet is not converted into an indefinitely cached observed-health claim. `PagesProviderHealthAuthority` rereads and revalidates the packet on every `pageBuilderRolloutSnapshot` read. Observed health is admitted only while:

```text
server_now <= evaluation.health_valid_until + 5s clock-skew tolerance
```

The deadline itself is derived from the original evaluator freshness budget as described above; the five seconds compensate only for bounded cross-clock skew and do not restart the SLO freshness window. Missing, rejected, malformed, identity-mismatched or expired evidence returns `None`, which preserves the rollout snapshot's default `false + null` provider-health state.

The authority keeps the explicit packet path rather than caching one accepted snapshot forever. An atomic packet replacement from accepted to rejected, packet removal, or later replacement with a new accepted packet changes the next rollout-status read without a process restart. This is the concrete server-side implementation of the accepted rollback action `restore_unobserved_provider_health`.

## GraphQL composition

`PagesModule::register_runtime_extensions` publishes `SharedPagesProviderHealthAuthority` only when Pages binding environment is complete and structurally valid. The Pages manifest declares:

```toml
runtime_data_factory = "graphql::attach_schema_data"
```

`PagesGraphqlRuntimeData` selects the shared authority from `GraphqlRuntimeInputs`. `pageBuilderRolloutSnapshot` keeps its unobserved constructor as the default and calls `with_provider_health` only when the runtime data returns a fresh canonical snapshot.

The existing admin transport continues to re-evaluate the typed GraphQL payload independently, so host admission and browser/admin transport validation are separate fail-closed boundaries.

## Current consumer boundary

This slice intentionally stops at server/GraphQL binding. Current production capability consumers remain unchanged:

- Pages workspace composition still consumes rollout flags only;
- authoritative Preview/Publish SSR facade still constructs `PageBuilderAdminProviderStatus::unobserved` from rollout flags;
- standalone browser-intent preflight still applies `pages_editor_capabilities_for_rollout` to rollout flags.

Therefore server binding source is ready, but UI/SSR/browser-intent health-driven capability narrowing is the next source slice. A live accepted packet is still maintainer-owned execution and is not fabricated here.

## Non-promotion

Source inspection does not claim that deployment identity capture, Prometheus evaluation, owner acceptance, accepted packet installation, observed GraphQL execution, Pages gate acceptance, Forum Wave, FFA or FBA promotion occurred.

Without a live retained accepted packet matching the configured source/deployment identity and remaining-freshness deadline, Pages remains `unobserved`.

## Next cursor

```text
bounded runtime observation [source-ready]
-> deployment metrics/freshness [source-ready]
-> exact deployment identity [source-ready]
-> deployment evaluator [source-ready]
-> typed observed-health transport [source-ready]
-> owner acceptance packet + exact health_valid_until [source-ready]
-> server provider-health binding + hot revoke + freshness lease [source-ready]
-> retained identity + evaluator + accepted owner packet [maintainer execution pending]
-> UI / SSR / browser-intent provider-health binding [source-open, runtime activation blocked]
-> observed-health acceptance decision [pending]
```

## Validation boundary

Tests were not run. Node verifiers, Cargo commands, formatting, builds, GraphQL/HTTP requests, deployment identity capture, Prometheus queries, evaluator execution, owner-acceptance execution, browser runs, workflows and CI were intentionally not executed by this implementation slice.

Suggested maintainer source checks, intentionally not run:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-server-binding.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-owner-acceptance.mjs
node crates/rustok-pages/scripts/verify/verify-pages-builder-provider-health-transport.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-provider-health-deployment-evaluator.mjs
node crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs
```
