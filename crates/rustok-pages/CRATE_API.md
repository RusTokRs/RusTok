# rustok-pages / CRATE_API

## Public modules

`controllers`, `dto`, `entities`, `error`, `graphql`, `migrations`, `openapi`,
`services`.

## Primary public types

- `PagesModule`
- `PageService`
- `PageBuilderArtifactService`
- `PageBuilderScenarioBaselineService`
- `MenuService`
- `Page`, `Menu`, published artifact entities
- `PagesError`, `PagesResult<T>`

## Current document contract

- Page visual content is stored in `PageBodyInput` with format `grapesjs`.
- `pages[].component` is the component-tree authority.
- Page metadata, channel visibility and the Fly document are Pages-owned data.
- Published storefront output is selected through immutable landing artifacts.
- There is no public block entity, block service, block DTO or block mutation.

## Events

Page/menu lifecycle events are published through `TransactionalEventBus` in the
same transaction as the domain mutation.

## Domain invariants

- Tenant/resource isolation and effective permission checks are mandatory.
- Slugs are unique per tenant and locale.
- Writes use optimistic page versions.
- Builder feature gates fail with typed errors.
- Metadata-only changes must not replace an existing page body.
- Publishing a Fly document validates readiness, compiles a deterministic
  artifact and binds it before the published state becomes visible.
- Missing providers or invalid projects fail visibly; no fallback model is
  synthesized.

## Adapter obligations

Changes to public DTO fields require synchronized GraphQL, HTTP, admin and
storefront updates. Error classes for validation, authorization, conflict,
feature-disabled, integrity and not-found outcomes must retain their semantics.
