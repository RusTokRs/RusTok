# Pages / Page Builder rollout binding actualization — 2026-08-08

Status: `server-owner-snapshot-source-ready / ui-provider-binding-source-ready / authoritative-ssr-binding-source-ready / browser-intent-preflight-binding-source-ready / runtime-matrix-evidence-pending`.

Current correction base: `main@9221664d677f3f8775ef4ade66ffe14a4b316c54`.

Pages rollout authority is now server-owned by GraphQL `pageBuilderRolloutSnapshot`. The resolver binds auth tenant to routed tenant, requires `Pages:Read`, reads the exact enabled Pages settings row, and normalizes flags with `BuilderCapabilityFlags::from_module_settings`. Standalone admin remains a stateless token/tenant transport consumer.

The selected Pages workspace fetches the typed snapshot and supplies its flags to `PagesBuilderFacade::with_provider_flags(...)`; provider health remains explicitly unobserved.

Preview and publish independently fetch the same server-owned snapshot on authoritative SSR dispatch and compose handlers from those freshly returned flags.

The standalone `/api/admin/pages/{page_id}/builder/intents` route now also fetches the server-owned snapshot and intersects rollout with role capabilities before browser-intent preflight. This closes the handcrafted-intent bypass for `builder_off`, properties-disabled, and publish-disabled states before draft mutation.

The hardcoded all-on consumer helper remains removed. Browser-supplied rollout flags are not authoritative.

The four declared profiles are source-exercisable through the real Pages consumer, but no runtime matrix has been executed or accepted. `pages_reference_consumer_gate` remains `accepted=false`; provider health remains `unobserved`; Forum Wave and FFA/FBA remain blocked.

Next: retain a bounded exact-source four-profile runtime matrix covering UI, authoritative SSR, browser-intent denial, and Pages-owned reads with mandatory settings restoration; maintainer executes it separately.

No tests, Node verifiers, Cargo commands, formatting, database scenarios, HTTP requests, browser runs, workflows, CI, or `git diff --check` were executed.
