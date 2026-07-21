import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const failures = [];
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");
const requireFile = (relative) => {
  if (!fs.existsSync(path.join(root, relative))) {
    failures.push(`missing Groups targeted invitation artifact: ${relative}`);
    return false;
  }
  return true;
};
const requireMarkers = (relative, markers) => {
  if (!requireFile(relative)) return;
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) {
      failures.push(`${relative}: missing marker ${JSON.stringify(marker)}`);
    }
  }
};
const forbidMarkers = (relative, markers) => {
  if (!requireFile(relative)) return;
  const source = read(relative);
  for (const marker of markers) {
    if (source.includes(marker)) {
      failures.push(`${relative}: forbidden marker ${JSON.stringify(marker)}`);
    }
  }
};

const required = [
  "crates/rustok-groups/Cargo.toml",
  "crates/rustok-groups/src/migrations/m20260721_000005_create_group_domain_events.rs",
  "crates/rustok-groups/src/migrations/mod.rs",
  "crates/rustok-groups/src/group_event_entities.rs",
  "crates/rustok-groups/src/targeted_invitations.rs",
  "crates/rustok-groups/src/notification_source.rs",
  "crates/rustok-groups/src/graphql_invitations.rs",
  "crates/rustok-groups/src/lib.rs",
  "crates/rustok-groups/src/ports.rs",
  "crates/rustok-groups/rustok-module.toml",
  "crates/rustok-groups/storefront/src/core.rs",
  "crates/rustok-groups/storefront/src/model.rs",
  "crates/rustok-groups/storefront/src/transport.rs",
  "crates/rustok-groups/storefront/src/transport/native_server_adapter.rs",
  "crates/rustok-groups/storefront/src/transport/graphql_adapter.rs",
  "crates/rustok-groups/storefront/src/ui/invitation_acceptance.rs",
  "crates/rustok-groups/storefront/locales/en.json",
  "crates/rustok-groups/storefront/locales/ru.json",
  "crates/rustok-groups/contracts/groups-fba-registry.json",
  "crates/rustok-groups/docs/implementation-plan.md",
];
for (const relative of required) requireFile(relative);

const migration =
  "crates/rustok-groups/src/migrations/m20260721_000005_create_group_domain_events.rs";
requireMarkers(migration, [
  "group_domain_events",
  "groups.invitation.targeted_created",
  "group domain events are append-only",
  "groups_targeted_invitation_created_event",
  "AFTER INSERT ON group_invitations",
  "WHEN NEW.target_user_id IS NOT NULL",
  "NEW.invited_by_user_id",
  "'invitation_id'",
  "'group_id'",
  "'target_user_id'",
  "idx_group_domain_events_tenant_sequence",
  "idx_group_domain_events_tenant_aggregate",
  "idx_group_domain_events_tenant_type",
]);
forbidMarkers(migration, [
  "'token'",
  "'token_hash'",
  "NEW.token_hash",
  "plaintext_token",
]);
requireMarkers("crates/rustok-groups/src/migrations/mod.rs", [
  "mod m20260721_000005_create_group_domain_events;",
  "Box::new(m20260721_000005_create_group_domain_events::Migration)",
]);
requireMarkers("crates/rustok-groups/src/group_event_entities.rs", [
  'table_name = "group_domain_events"',
  "pub sequence_no: i64",
  "pub event_id: Uuid",
  "pub aggregate_id: Uuid",
  "pub payload: Json",
]);

requireMarkers("crates/rustok-groups/src/targeted_invitations.rs", [
  "AcceptTargetedGroupInvitationRequest",
  "GroupTargetedInvitationCommandPort",
  "GroupTargetedInvitationService",
  'groups.accept_targeted_invitation.v1',
  "model.target_user_id != Some(actor_user_id)",
  "model.max_uses != 1",
  "lock_exclusive()",
  "redemption::ActiveModel",
  "GroupMembershipStatus::Active",
  "member_count.saturating_add(1)",
  "group.targeted_invitation_accepted",
  "store_receipt",
  "replay_receipt",
  "GroupsError::NotFound",
  "PortCallPolicy::write()",
]);
forbidMarkers("crates/rustok-groups/src/targeted_invitations.rs", [
  "token_hash",
  "invitation_token_hash",
  "AcceptGroupInvitationRequest { token",
]);

requireMarkers("crates/rustok-groups/src/notification_source.rs", [
  "GroupsNotificationSourceProviderFactory",
  "NotificationSourceProviderFactory",
  "NotificationSourceProvider",
  'const GROUPS_SOURCE: &str = "groups"',
  'const TARGETED_INVITATION_CREATED_TYPE: &str = "groups.invitation.targeted_created"',
  'const GROUP_INVITATION_TARGET: &str = "groups.invitation"',
  "NotificationTemplateData::try_new",
  '"invitation_id".to_string()',
  '"group_id".to_string()',
  "request.bounded_limit() == 0",
  "request.cursor.is_some()",
  "vec![NotificationAudienceCandidate { recipient_id }]",
  "invitation.target_user_id != Some(request.recipient_id)",
  '"/modules/groups?invitation={}"',
  "NotificationOpenAuthorization::Unavailable",
]);
forbidMarkers("crates/rustok-groups/src/notification_source.rs", [
  "token_hash",
  "plaintext_token",
  '"token".to_string()',
  "NotificationTargetRoute::new(format!(\"http",
]);

requireMarkers("crates/rustok-groups/Cargo.toml", [
  "rustok-notifications-api.workspace = true",
]);
requireMarkers("crates/rustok-groups/src/lib.rs", [
  "register_notification_source_provider_factory",
  "notification_source::GroupsNotificationSourceProviderFactory",
  "pub mod group_event_entities;",
  "pub mod targeted_invitations;",
  "assert_eq!(module.migrations().len(), 5)",
]);
requireMarkers("crates/rustok-groups/src/ports.rs", [
  '"GroupTargetedInvitationCommandPort"',
]);
requireMarkers("crates/rustok-groups/src/graphql_invitations.rs", [
  "accept_targeted_group_invitation",
  "AcceptTargetedGroupInvitationRequest",
  "GroupTargetedInvitationCommandPort",
  "GroupTargetedInvitationService",
  "with_idempotency_key",
]);
requireMarkers("crates/rustok-groups/rustok-module.toml", [
  'query = "graphql_invitations::GroupsQueryRoot"',
  'mutation = "graphql_invitations::GroupsMutationRoot"',
]);

requireMarkers("crates/rustok-groups/storefront/src/core.rs", [
  'GROUP_TARGETED_INVITATION_QUERY_KEY: &str = "invitation"',
  "prepare_accept_targeted_group_invitation",
  "groups-storefront-accept-targeted-invitation-",
  "InvalidInvitationId",
]);
forbidMarkers("crates/rustok-groups/storefront/src/core.rs", [
  "use leptos",
  "leptos::",
  "acceptTargetedGroupInvitation",
]);
requireMarkers("crates/rustok-groups/storefront/src/model.rs", [
  "AcceptTargetedGroupInvitationCommand",
]);
requireMarkers("crates/rustok-groups/storefront/src/transport.rs", [
  "accept_groups_storefront_targeted_invitation",
  '"groups.storefront.targeted_invitation.accept"',
  "native_server_adapter::accept_targeted_invitation",
  "graphql_adapter::accept_targeted_invitation",
  'GROUPS_STOREFRONT_TRANSPORT_FALLBACK_POLICY: &str = "never falls back"',
]);
requireMarkers(
  "crates/rustok-groups/storefront/src/transport/native_server_adapter.rs",
  [
    "groups/storefront/targeted-invitations/accept",
    "AcceptTargetedGroupInvitationCommand",
    "AcceptTargetedGroupInvitationRequest",
    "GroupTargetedInvitationCommandPort",
    "GroupTargetedInvitationService",
    "AuthContext",
    "with_idempotency_key",
  ],
);
requireMarkers(
  "crates/rustok-groups/storefront/src/transport/graphql_adapter.rs",
  [
    "GroupsStorefrontAcceptTargetedInvitation",
    "acceptTargetedGroupInvitation",
    "AcceptTargetedInvitationVariables",
    "access_token",
  ],
);
requireMarkers(
  "crates/rustok-groups/storefront/src/ui/invitation_acceptance.rs",
  [
    "prepare_accept_targeted_group_invitation",
    "accept_groups_storefront_targeted_invitation",
    "GROUP_TARGETED_INVITATION_QUERY_KEY",
    "query_writer.clear_key(GROUP_TARGETED_INVITATION_QUERY_KEY)",
    "PendingInvitationAcceptance::Targeted",
    "groups.storefront.invitation.targetedBody",
    "groups.storefront.invitation.targetedHint",
    "groups.storefront.invitation.invalidInvitationId",
  ],
);
forbidMarkers(
  "crates/rustok-groups/storefront/src/ui/invitation_acceptance.rs",
  [
    "notification_source",
    "GroupTargetedInvitationService",
    "token_hash",
    "<code",
  ],
);

const localeKeys = [
  "groups.storefront.invitation.targetedBody",
  "groups.storefront.invitation.targetedHint",
  "groups.storefront.invitation.invalidInvitationId",
];
for (const relative of [
  "crates/rustok-groups/storefront/locales/en.json",
  "crates/rustok-groups/storefront/locales/ru.json",
]) {
  if (!requireFile(relative)) continue;
  let messages;
  try {
    messages = JSON.parse(read(relative));
  } catch (error) {
    failures.push(`${relative}: invalid JSON: ${error.message}`);
    continue;
  }
  for (const key of localeKeys) {
    if (typeof messages[key] !== "string" || messages[key].trim() === "") {
      failures.push(`${relative}: missing targeted invitation key ${key}`);
    }
  }
}

if (requireFile("crates/rustok-groups/contracts/groups-fba-registry.json")) {
  let registry;
  try {
    registry = JSON.parse(
      read("crates/rustok-groups/contracts/groups-fba-registry.json"),
    );
  } catch (error) {
    failures.push(`Groups FBA registry is invalid JSON: ${error.message}`);
  }
  if (registry) {
    const targetedPort = registry?.provider?.ports?.find(
      (port) => port?.name === "GroupTargetedInvitationCommandPort",
    );
    if (
      !targetedPort?.operations?.includes("accept_targeted_group_invitation") ||
      targetedPort?.authorization !== "authenticated_exact_target_only"
    ) {
      failures.push("Groups registry is missing the targeted invitation command contract");
    }
    const invitations = registry?.invitations;
    const expected = {
      storefront_targeted_query_key: "invitation",
      targeted_acceptance: "authenticated_invitation_id",
      targeted_acceptance_wrong_recipient: "not_found",
      targeted_domain_event_table: "group_domain_events",
      targeted_domain_event_type: "groups.invitation.targeted_created",
      targeted_domain_event_atomicity: "same_owner_transaction_via_trigger",
      targeted_domain_event_token_fields: "forbidden",
      targeted_notification_source: "groups",
      targeted_notification_audience_max: 1,
      targeted_notification_open_authorization: "active_exact_recipient_only",
      shareable_notification_event: false,
      notifications_consumer_required_for_create: false,
    };
    for (const [key, value] of Object.entries(expected)) {
      if (invitations?.[key] !== value) {
        failures.push(`Groups targeted invitation registry mismatch: ${key}`);
      }
    }
    if (
      registry?.evidence?.targeted_invitation_delivery_static_boundary !==
      "scripts/verify/verify-groups-targeted-invitation-delivery.mjs"
    ) {
      failures.push("Groups targeted invitation static evidence path is not registered");
    }
    if (
      registry?.evidence?.invitation_delivery !== null ||
      registry?.evidence?.targeted_invitation_notification_runtime !== null
    ) {
      failures.push("Groups targeted invitation runtime evidence must remain null before execution");
    }
  }
}

requireMarkers("crates/rustok-groups/docs/implementation-plan.md", [
  "groups.invitation.targeted_created",
  "GroupTargetedInvitationCommandPort",
  "verify-groups-targeted-invitation-delivery.mjs",
  "targeted invitation notification runtime",
]);

if (failures.length > 0) {
  console.error("Groups targeted invitation delivery verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Groups targeted invitation delivery verification passed.");
