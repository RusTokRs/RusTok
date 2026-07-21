# rustok-groups-admin

Module-owned Leptos admin FFA package for Groups.

## Structure

- `core.rs`: framework-neutral UUID/locale/text/invitation validation, command
  preparation, and transport profile;
- `application_core.rs`: framework-neutral policy/review validation and fresh
  idempotency-key preparation;
- `model.rs`: serializable directory, governance, localization, and invitation
  command/result models;
- `application_model.rs`: application policy, snapshot, review, and membership
  command/result models;
- `transport.rs`: selected directory, governance, localization, invitation, and
  application facade;
- `transport/native_server_adapter.rs`: SSR/hydrate directory, role-delegation,
  and ownership-transfer paths;
- `transport/native_localization_adapter.rs`: SSR/hydrate exact-locale list,
  upsert, and delete paths;
- `transport/native_invitations_adapter.rs`: SSR/hydrate invitation list, create,
  and revoke paths;
- `transport/native_applications_adapter.rs`: SSR/hydrate application-policy read/
  upsert, pending-list, and review paths;
- `transport/graphql_adapter.rs`: CSR/headless GraphQL directory, governance, and
  localization paths;
- `transport/graphql_invitations_adapter.rs`: CSR/headless invitation list, create,
  and revoke paths;
- `transport/graphql_applications_adapter.rs`: CSR/headless application policy,
  listing, and review paths;
- `ui/leptos.rs`: thin Leptos directory and governance form binding;
- `ui/localization.rs`: exact-locale translation workspace using only the core and
  transport facade;
- `ui/applications.rs`: pending application snapshot/review workspace;
- `ui/invitations.rs`: targeted/shareable invitation management with one-time token
  display;
- `ui/root.rs`: module-owned composition root;
- `locales/`: English and Russian copy.

The application facade exposes policy read/upsert, pending application listing, and
approve/reject commands. Policy copy is stored per exact normalized locale; fallback
remains a host responsibility. Every submitted application carries the policy
revision, locale, question/rule snapshot, answers, and acknowledgements used by the
candidate. Review calls the same owner service from native and GraphQL paths and
never falls back implicitly. Approval/rejection, membership state, group version,
audit, and idempotency receipt commit together.

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

The invitation facade exposes `load_group_admin_invitations`,
`create_group_admin_invitation`, and `revoke_group_admin_invitation`. The core
validates UUIDs, 300-second-to-30-day expiry, 1-to-100 use limits, and the targeted
single-use rule. Native and GraphQL paths call the same invitation owner ports and
never retry through the other transport. The UI displays plaintext only when the
first create response supplies a token; reload and idempotent replay cannot recover
that token because Groups stores only its SHA-256 digest.

The current UI provides localized role-delegation, ownership-transfer,
translation-management, pending-application review, and invitation-management
forms. Manual group/member/application/invitation UUID entry remains an intermediate
operator surface. It intentionally does not reimplement local-role, ownership,
locale-fallback, question/rule, review, token, expiry, revocation, or redemption
policy.

The package receives tenant, auth, locale, and route context from the host. It
never reads another module's tables or embeds another module's UI. Policy visual
editing, member/group pickers, explicit confirmation, audit/receipt history, bulk
review, accessibility evidence, and executed native/GraphQL parity remain later
slices; source presence alone does not promote FFA readiness.
