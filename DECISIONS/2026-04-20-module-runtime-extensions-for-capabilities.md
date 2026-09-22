# ModuleRuntimeExtensions for runtime capabilities

- Date: 2026-04-20
- Status: Accepted

## Context

The platform already uses support/capability crates alongside tenant-aware modules, but until now there was no
single canonical runtime pattern through which owner modules could register backend
capabilities without modifying the central core module for each new target/provider.

SEO demonstrated this gap especially clearly:

- `rustok-seo` remained the only tenant-aware SEO module;
- persisted storage already used string `target_kind`, but the Rust/runtime contract was tied to a closed enum;
- adding a new SEO-capable backend module required hardcoded dispatch inside `rustok-seo`;
- the host runtime already had a common `ModuleRegistry`, but did not have a common typed extension-registry that
  modules could populate during bootstrap.

A common platform pattern was needed, not an ad-hoc exception only for SEO.

## Decision

The platform introduces module-owned runtime capability registration via `rustok-core::ModuleRuntimeExtensions`.

The following rules are adopted:

- `RusToKModule` gets a fallible hook
  `register_runtime_extensions(&mut ModuleRuntimeExtensions) -> rustok_core::Result<()>`;
- duplicate providers, invalid deployment configuration, and other registration failures must be returned by
  the owner module rather than converted to `expect`/`panic`;
- `ModuleRegistry::build_runtime_extensions()` invokes the hook for every registered module and adds the
  module slug to any returned error;
- the host (`apps/server`) builds a single shared `ModuleRuntimeExtensions` after `build_registry()` and
  propagates registration/materialization failures through the application startup result;
- the resulting `ModuleRuntimeExtensions` is placed in the shared runtime store and into the GraphQL schema data;
- support/capability crates publish typed registries on top of this mechanism, but do not become
  tenant-aware modules themselves as a result.

For SEO this is fixed as follows:

- `rustok-seo-targets` becomes a support/capability crate;
- canonical public contract target kind = validated string `SeoTargetSlug`;
- owner backend modules (`pages`, `product`, `blog`, `forum`) themselves register their
  `SeoTargetProvider` in the runtime registry;
- `rustok-seo` receives a single shared `Arc<SeoTargetRegistry>` from the runtime context and uses it in all
  entrypoints: GraphQL, HTTP, Leptos `#[server]`, storefront SSR helpers, and background workers.

The manifest schema is not extended with a separate runtime-capabilities section. The source of truth for
such registration seams remains the Rust-side hook.

## Consequences

- Adding a new SEO-capable backend module no longer requires modifying `rustok-seo` core.
- A reusable platform pattern emerges for other runtime capabilities, not only for SEO.
- Invalid provider composition fails deployment startup with an actionable module-owned error instead of
  terminating through a panic.
- The host runtime must initialize `ModuleRuntimeExtensions` once and propagate it to all
  shared entrypoints.
- A support/capability crate is still not considered a platform module solely due to participation in runtime wiring.
- Module authors must now document not only manifest wiring, but also runtime capability
  registration if the module publishes a provider seam through `ModuleRuntimeExtensions`.
