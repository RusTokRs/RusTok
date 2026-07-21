# rustok-groups-admin

Module-owned Leptos admin FFA package for Groups.

## Structure

- `core.rs`: framework-neutral view policy and transport profile;
- `model.rs`: serializable admin read models;
- `transport.rs`: selected transport facade;
- `transport/native_server_adapter.rs`: SSR/hydrate `#[server]` path;
- `transport/graphql_adapter.rs`: CSR/headless GraphQL path;
- `ui/leptos.rs`: thin Leptos binding;
- `locales/`: English and Russian copy.

The package receives tenant, auth, locale, and route context from the host. It
never reads another module's tables or embeds another module's UI.
