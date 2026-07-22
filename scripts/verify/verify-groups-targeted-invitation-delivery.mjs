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
    if (!source.includes(marker)) failures.push(`${relative}: missing marker ${JSON.stringify(marker)}`);
  }
};
const forbidMarkers = (relative, markers) => {
  if (!requireFile(relative)) return;
  const source = read(relative);
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${relative}: forbidden marker ${JSON.stringify(marker)}`);
  }
};

requireMarkers("crates/rustok-groups/src/migrations/m20260721_000005_create_group_domain_events.rs", [
  "group_domain_events",
  "groups.invitation.targeted_created",
  "group domain events are append-only",
  "AFTER INSERT ON group_invitations",
  "WHEN NEW.target_user_id IS NOT NULL",
  "'invitation_id'",
  "'group_id'",
  "'target_user_id'",
]);
forbidMarkers("crates/rustok-groups/src/migrations/m20260721_000005_create_group_domain_events.rs", [
  "NEW.token_hash",
  "'token_hash'",
  "plaintext_token",
]);

requireMarkers("crates/rustok-groups/src/targeted_invitations.rs", [
  "GroupTargetedInvitationCommandPort",
  "GroupTargetedInvitationService",
  "model.target_user_id != Some(actor_user_id)",
  "model.max_uses != 1",
  "lock_exclusive()",
  "redemption::ActiveModel",
  "member_count.saturating_add(1)",
  "group.targeted_invitation_accepted",
  "store_receipt",
  "replay_receipt",
]);
forbidMarkers("crates/rustok-groups/src/targeted_invitations.rs", [
  "invitation_token_hash",
  "AcceptGroupInvitationRequest { token",
]);

requireMarkers("crates/rustok-groups/src/notification_source.rs", [
  "GroupsNotificationSourceProviderFactory",
  "NotificationSourceProviderFactory",
  "groups.invitation.targeted_created",
  "NotificationTemplateData::try_new",
  "vec![NotificationAudienceCandidate { recipient_id }]",
  "invitation.target_user_id != Some(request.recipient_id)",
  '"/modules/groups?invitation={}"',
]);
forbidMarkers("crates/rustok-groups/src/notification_source.rs", [
  "token_hash",
  "plaintext_token",
  "NotificationTargetRoute::new(format!(\"http",
]);

requireMarkers("crates/rustok-groups/src/graphql_invitations.rs", [
  "accept_targeted_group_invitation",
  "AcceptTargetedGroupInvitationRequest",
  "GroupTargetedInvitationCommandPort",
  "with_idempotency_key",
]);
requireMarkers("crates/rustok-groups/rustok-module.toml", [
  'query = "graphql_policy_history::GroupsQueryRoot"',
  'mutation = "graphql_policy_history::GroupsMutationRoot"',
]);
requireMarkers("crates/rustok-groups/src/lib.rs", [
  "register_notification_source_provider_factory",
  "notification_source::GroupsNotificationSourceProviderFactory",
  "pub mod targeted_invitations;",
  "assert_eq!(module.migrations().len(), 7)",
]);

requireMarkers("crates/rustok-groups/storefront/src/core.rs", [
  'GROUP_TARGETED_INVITATION_QUERY_KEY: &str = "invitation"',
  "prepare_accept_targeted_group_invitation",
]);
forbidMarkers("crates/rustok-groups/storefront/src/core.rs", ["use leptos", "leptos::"]);
requireMarkers("crates/rustok-groups/storefront/src/transport.rs", [
  "accept_groups_storefront_targeted_invitation",
  '"groups.storefront.targeted_invitation.accept"',
  'GROUPS_STOREFRONT_TRANSPORT_FALLBACK_POLICY: &str = "never falls back"',
]);
requireMarkers("crates/rustok-groups/storefront/src/ui/invitation_acceptance.rs", [
  "prepare_accept_targeted_group_invitation",
  "accept_groups_storefront_targeted_invitation",
  "GROUP_TARGETED_INVITATION_QUERY_KEY",
  "query_writer.clear_key(GROUP_TARGETED_INVITATION_QUERY_KEY)",
]);

if (requireFile("crates/rustok-groups/contracts/groups-fba-registry.json")) {
  const registry = JSON.parse(read("crates/rustok-groups/contracts/groups-fba-registry.json"));
  const targetedPort = registry?.provider?.ports?.find(
    (port) => port?.name === "GroupTargetedInvitationCommandPort",
  );
  if (!targetedPort?.operations?.includes("accept_targeted_group_invitation")) {
    failures.push("Groups registry is missing targeted invitation acceptance");
  }
  if (registry?.invitations?.targeted_notification_audience_max !== 1) {
    failures.push("Groups targeted notification audience must remain bounded to one recipient");
  }
  if (registry?.invitations?.targeted_domain_event_token_fields !== "forbidden") {
    failures.push("Groups targeted event must forbid token fields");
  }
  if (registry?.evidence?.targeted_invitation_notification_runtime !== null) {
    failures.push("unexecuted targeted invitation runtime evidence must remain null");
  }
}

for (const relative of [
  "crates/rustok-groups/storefront/locales/en.json",
  "crates/rustok-groups/storefront/locales/ru.json",
]) {
  if (!requireFile(relative)) continue;
  const messages = JSON.parse(read(relative));
  for (const key of [
    "groups.storefront.invitation.targetedBody",
    "groups.storefront.invitation.targetedHint",
    "groups.storefront.invitation.invalidInvitationId",
  ]) {
    if (typeof messages[key] !== "string" || messages[key].trim() === "") {
      failures.push(`${relative}: missing targeted invitation key ${key}`);
    }
  }
}

if (failures.length > 0) {
  console.error("Groups targeted invitation delivery verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Groups targeted invitation owner-event, exact-recipient, acceptance, FFA, and no-token boundary checks passed.");
