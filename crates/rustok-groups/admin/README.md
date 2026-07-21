# rustok-groups-admin

Module-owned Leptos admin FFA package for Groups.

## Structure

- `core.rs`: framework-neutral view policy, UUID normalization, command
  preparation, and transport profile;
- `model.rs`: serializable directory and governance command/result models;
- `transport.rs`: selected directory and governance transport facade;
- `transport/native_server_adapter.rs`: SSR/hydrate `#[server]` directory,
  role-delegation, and ownership-transfer paths;
- `transport/graphql_adapter.rs`: CSR/headless GraphQL directory and governance
  paths;
- `ui/leptos.rs`: thin Leptos directory and governance form binding;
- `locales/`: English and Russian copy.

The governance facade exposes `change_group_admin_role` and
`transfer_group_admin_ownership`. Both choose exactly one configured transport and
call the same `GroupGovernanceCommandPort`; an owner error never triggers an
implicit retry through the other transport. Governance state, idempotency receipt,
and immutable audit entry remain owned by `rustok-groups`.

The current UI provides localized role-delegation and ownership-transfer forms,
validates and normalizes UUID input through the framework-neutral core, and shows
the resulting group version and idempotent-replay evidence. It intentionally does
not reimplement local-role authorization or ownership rules.

The package receives tenant, auth, locale, and route context from the host. It
never reads another module's tables or embeds another module's UI. Member/group
pickers, confirmation workflow, audit history, accessibility evidence, and executed
native/GraphQL parity remain later slices; source presence alone does not promote
FFA readiness.
