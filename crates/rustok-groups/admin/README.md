# rustok-groups-admin

Module-owned Leptos admin FFA package for Groups.

## Structure

- `core.rs`: framework-neutral view policy, UUID/locale normalization, command
  preparation, and transport profile;
- `model.rs`: serializable directory, governance, and localization models;
- `transport.rs`: selected directory, governance, and localization facade;
- `transport/native_server_adapter.rs`: SSR/hydrate `#[server]` directory,
  role-delegation, and ownership-transfer paths;
- `transport/native_localization_adapter.rs`: SSR/hydrate exact-locale list,
  upsert, and delete paths;
- `transport/graphql_adapter.rs`: CSR/headless GraphQL directory, governance, and
  localization paths;
- `ui/leptos.rs`: thin Leptos directory and governance form binding;
- `ui/localization.rs`: exact-locale translation workspace using only the core and
  transport facade;
- `ui/root.rs`: module-owned composition root;
- `locales/`: English and Russian copy.

The governance facade exposes `change_group_admin_role` and
`transfer_group_admin_ownership`. Both choose exactly one configured transport and
call the same `GroupGovernanceCommandPort`; an owner error never triggers an
implicit retry through the other transport. Governance state, idempotency receipt,
and immutable audit entry remain owned by `rustok-groups`.

The localization facade exposes `load_group_admin_translations`,
`upsert_group_admin_translation`, and `delete_group_admin_translation`. Native and
GraphQL paths call `GroupLocalizationReadPort` or
`GroupLocalizationCommandPort`; neither path selects a fallback locale. The core
normalizes the exact locale tag and applies Unicode title/summary limits, while the
owner service re-checks active owner/admin or platform-manage authority inside the
write transaction. Translation mutations increment the group version atomically,
and deletion of the last translation row is rejected.

The current UI provides localized role-delegation, ownership-transfer, and
translation-management forms. Manual group/member UUID entry remains an
intermediate operator surface. It intentionally does not reimplement local-role,
ownership, or locale-fallback policy.

The package receives tenant, auth, locale, and route context from the host. It
never reads another module's tables or embeds another module's UI. Member/group
pickers, confirmation workflow, audit history, accessibility evidence, idempotent
localization receipts, and executed native/GraphQL parity remain later slices;
source presence alone does not promote FFA readiness.
