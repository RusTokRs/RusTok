# rustok-page-builder

## Purpose
`rustok-page-builder` is the FBA-first visual authoring capability and compatibility module for RusTok. It owns the canonical `grapesjs` capability contract and publishes an optional Fly-based Leptos admin surface without taking ownership of consumer documents.

## Responsibilities
- keep vendor-neutral builder contract baseline (`grapesjs` write/read semantics);
- expose module runtime identity, permissions, rollout, health, validation, persistence and rendering seams;
- integrate the framework-neutral `fly` engine with Page Builder backend adapters;
- publish the optional `rustok-page-builder-admin` full-authoring composition surface;
- preserve consumer ownership of Pages, Blog, Forum and other domain document lifecycles.

## Entry points
- `src/lib.rs` — module runtime metadata (`PageBuilderModule`) and permission surface;
- `src/service.rs` — transport-neutral capability service, rollout guard, and authorized handler seam;
- `src/adapters.rs` — canonical transport adapters and Fly project inspection;
- `admin/src/lib.rs` — no-prop generated host entrypoint plus explicit consumer-owned controller entrypoint;
- `rustok-module.toml` — FBA provider and manifest-backed admin UI contract;
- [Public Contract Reference (CRATE_API.md)](./CRATE_API.md) — live API and platform capability contracts;
- [`docs/README.md`](./docs/README.md) — live runtime and integration contract;
- [`docs/implementation-plan.md`](./docs/implementation-plan.md) — module-local delivery plan.

## Interactions
- consumed by `rustok-pages` and other layout/content modules through canonical capability envelopes;
- mounted by `apps/admin` through generated manifest composition;
- delegates canonical project semantics to `fly`, presentation state to `fly-ui`, and browser/Leptos lifecycle to `fly-leptos`;
- remaining work is tracked in the one owned backlog, `docs/modules/page-builder-backlog.md`.

## Static runtime publication

`compile_materialized_static_landing` materializes the reviewed runtime context and host form
endpoints once, rejects blocking diagnostics, then validates and renders that exact lowered tree.
The source identity retains the authoring runtime definitions, even when conditions or repeaters
remove their original targets. Artifact snapshots are checked against the published HTML. An exact
rebuild still checks retained hashes and fails closed if a corrected materialization changes them.

## Known Limitations
- The browser editor now has a Site symbols panel: convert a selected component
  into a shared definition and insert references through `EditorCommand`.
  `rustok-pages` synchronizes the catalog per tenant/locale and expands current
  definitions on reviewed publish. A page must be reloaded, saved and re-reviewed
  after shared definitions change; previously published artifacts never change
  until that page is explicitly re-published. The usage query is available via
  `siteSymbolUsage` GraphQL, but usage is not yet displayed in this panel;
  per-instance overrides and bulk re-publish are not implemented (see
  `DECISIONS/2026-10-09-site-symbols-shared-definitions.md`).
- The admin asset section browses and uploads media through a host-bound
  `AssetProviderPort`. The `rustok-pages` builder host binds the port to
  `rustok-media` admin dispatchers (auth context is captured by the host
  adapter); hosts that never bind the port keep the manual asset form only.
- The media panel renders on `wasm32` builds only (it uses the browser File
  APIs); SSR renders the manual asset form without the panel.
- `srcset`/`sizes` pass static validation in `static_publish_policy` (bounded
  grammar, cap-covered by the policy hash). The policy permits validated
  `srcset`. The static publish policy is a fixed compile-time contract, not
  operator configuration: publication always runs
  `PageBuilderStaticPublishPolicy::default()`, and changing a rule means changing
  the module (and its policy hash), not a deployment setting.
  The renderer emits a validated `srcset` into the artifact: every candidate
  URL is re-checked against the same image-resource policy as `src`, and one
  malformed or disallowed candidate drops the whole attribute (a partial
  `srcset` would silently change which image the browser picks). CSS image(), src(),
  image-set() and -webkit-image-set() quoted candidates are also checked against the same
  image-resource policy.
  Policy v6 shares Fly URL shape, CSS property-name, srcset and CSS resource-string admission.
  It validates project theme tokens, rejects embed extensions even on generic wrapper components,
  and checks Rich Text links against the stricter static navigation URL policy before publication,
  in addition to canonical, Open Graph and Twitter metadata URLs in every locale.
  After the policy hash changes, exact
  rebuild of earlier retained sanitized sources reports hash drift until the
  page is re-published (fail-closed by design, see
  `DECISIONS/2026-10-09-page-builder-media-asset-provider.md`).
- Provider form runtime is host-owned and currently covers the `forms`
  provider only. `src/interaction_providers.rs` registers the
  `forms`/`submit` capability, every host gate uses it (unregistered provider
  forms block the save/publish gate, unregistered provider actions warn), and
  `HostFormProviderEndpoints` binds `POST /api/forms/{form_id}/submit/form`
  into the storefront, admin preview, static-artifact and scenario-matrix
  renders. Release-baseline captures
  (`RuntimeScenarioReleaseBaseline::capture`) stay marker-only, so their case
  HTML hashes are not comparable with artifact snapshot hashes for pages that
  contain provider forms. Registering another provider means extending the
  catalog and the resolver here; the provider id, operation and route template
  are mirrored from `rustok-forms` and drift-checked by
  `scripts/verify/verify-fly-interaction-capabilities.mjs` instead of a Cargo
  edge, so this module stays buildable standalone.

## SSR browser operation recovery

Editable Pages SSR hosts create an owner-bound draft session before rendering
the editor and bind its token into the route. Mutating browser intents carry a
validated `operation_id`; the server fingerprints the envelope and atomically
stores a compact acknowledgement with the draft generation. Replaying the same
operation returns its receipt before the generation check; reusing its ID for
a different payload is a conflict.

The browser persists the exact pending request in tab-scoped `sessionStorage`.
After a lost response or reload it queries the read-only `resolve_operation`
intent. A missing receipt is **unknown**, not a failure: new mutations remain
blocked until the user checks again or explicitly retries the original request.
The browser never automatically resends an ambiguous mutation. Custom
`SsrDraftSessionStore` implementations must opt in to atomic operation receipts
or editable Pages SSR rendering fails closed. Receipts are bounded to 2,048
entries per session and 1 KiB each.

This is the Pages strict SSR path only; direct Next writes and restore/lifecycle
operation IDs remain separate work. The Dioxus UI host now has source-level iframe and
drag/drop/resize integration, but durable SSR operation-receipt parity and mounted browser
verification remain open. Callback-backed Leptos hosts must complete their request through
`PageBuilderCapabilityExecution::complete`; the runtime applies the same save-attempt
validation used by facade-backed requests. See [the save receipt decision](../../../DECISIONS/2026-11-23-page-document-save-receipts.md)
and [the Fly save-attempt decision](../../../DECISIONS/2026-11-24-fly-save-attempt-correlation-and-peer-write-fencing.md).
