# Pages / Page Builder rollout binding actualization — 2026-08-08

Status: `ui-provider-binding-source-ready / authoritative-ssr-binding-source-ready / runtime-matrix-evidence-pending`.

Base: `main@488803a831e725ef0fbeaa8540f09458ee461b85`.

This slice closes the hardcoded rollout binding identified after PR #3330.

The selected Pages workspace now obtains server-owned `BuilderCapabilityFlags` from `pages_builder_rollout_flags()` before mounting Page Builder and supplies them to `PagesBuilderFacade::with_provider_flags(...)`. The facade exposes that snapshot only as unobserved provider status; it does not fabricate live SLO health.

Preview and publish do not trust the workspace copy. Authoritative SSR dispatch independently calls `load_trusted_pages_builder_rollout_snapshot()` for every capability request, verifies the request snapshot tenant slug against the routed trusted tenant slug, and composes Page Builder handlers from the freshly normalized persisted flags.

The old hardcoded all-on consumer helper is removed. Browser intent does not carry rollout flags and persistence still reaches the same trusted SSR dispatch.

This makes `all_on`, `publish_off`, `preview_off`, and `builder_off` source-exercisable through the real Pages reference consumer, but it does not create or accept runtime evidence. `pages_reference_consumer_gate` remains `accepted=false`, provider health remains `unobserved`, Forum Wave remains blocked, and FFA/FBA promotion is not claimed.

The next cursor is a bounded four-profile runtime matrix source harness plus maintainer execution on one exact source revision and immutable deployment, including Pages-owned read guarantees.

No tests, Node verifiers, Cargo commands, formatting, database scenarios, HTTP requests, browser runs, workflows, or CI were executed by this implementation slice.
