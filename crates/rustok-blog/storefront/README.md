# rustok-blog-storefront

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../docs/UI/module-package-verification.md)

Leptos storefront UI package for the `rustok-blog` module.

## Responsibilities

- Exposes the blog storefront root view used by `apps/storefront`.
- Keeps blog-specific storefront UI inside the module package.
- Participates in the manifest-driven UI composition path through `rustok-module.toml`.
- Owns dual-path read access for published posts and selected `?slug=` rendering.
- Renders the selected post's approved public comments from the Comments-owned projection; pending, spam, trash, and deleted comments never enter the storefront DTO.
- Keeps storefront shell copy, selected-post route/query state, fetch request state, and presentation view-model helpers in framework-agnostic `core` so Leptos remains a thin render/host-context adapter.
- Keeps Leptos render/bind code in `storefront/src/ui/leptos.rs`; `storefront/src/lib.rs` only wires modules and re-exports `BlogView`.
- Native Leptos `#[server]` calls are isolated in `transport/native_server_adapter.rs`, with GraphQL kept as the required parallel selected path in `transport/graphql_adapter.rs` behind the build-profile-selected facade.
- Native SSR transport receives DB and `TransactionalEventBus` through `HostRuntimeContext`; this package does not depend on host-framework runtime context.

## Entry Points

- `BlogView` — root storefront view rendered from the host storefront slot registry.

## Interactions

- Consumed by `apps/storefront` via manifest-driven `build.rs` code generation.
- Uses native `#[server] -> HostRuntimeContext -> PostService/CommentService -> owner ports` on the SSR/hydrate path and the `rustok-blog` GraphQL post adapter on the headless/CSR path.
- GraphQL exposes approved public comments as the nested `GqlPost.publicComments` field, while the native adapter maps the same owner projection into `BlogPostDetail.publicComments`.
- Consumes the host-provided effective locale from `UiRouteContext` for shell copy, reads the stable selected-post query key `slug` through core-owned route state, and passes `BlogStorefrontFetchRequest` into transport adapters.
- Should remain compatible with the host storefront slot and generic module page contract, including locale-prefixed routes via `UiRouteContext::module_route_base()`.

## Documentation

- See [platform docs](../../../docs/index.md).
