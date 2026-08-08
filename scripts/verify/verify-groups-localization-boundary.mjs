import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const failures = [];
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");
const requireFile = (relative) => {
  if (!fs.existsSync(path.join(root, relative))) {
    failures.push(`missing Groups localization artifact: ${relative}`);
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

requireMarkers("crates/rustok-groups/src/localization.rs", [
  "GroupLocalizationReadPort",
  "GroupLocalizationCommandPort",
  "normalize_locale_tag",
  "translation::Column::Locale.eq(locale.clone())",
  "the last group translation cannot be deleted",
  "version.saturating_add(1)",
  "PortCallPolicy::read()",
  "PortCallPolicy::write()",
  "reserve_group_write_for_update",
  "require_effective_manager_direct_owned",
  "require_effective_manager_owned",
  "GroupManagerCapability::ManageSettings",
]);
forbidMarkers("crates/rustok-groups/src/localization.rs", [
  "PLATFORM_FALLBACK_LOCALE",
  "build_locale_candidates",
  "rows.first()",
  "GroupMembershipStatus::Active.as_str()",
  "fn require_local_manager(",
]);

requireMarkers("crates/rustok-groups/src/effective_membership_guard.rs", [
  "require_effective_manager_direct_owned",
  "require_effective_manager_owned",
  "resolve_group_membership_enforcement_now_for_update",
  "GroupMembershipEffectiveStatus::Suspended",
  "GroupMembershipEffectiveStatus::LegacyBanned",
  "GroupManagerCapability::ManageSettings",
]);

requireMarkers("crates/rustok-groups/src/membership_enforcement_transaction.rs", [
  "reserve_group_write_for_update",
  "UPDATE groups SET version = version WHERE tenant_id = ? AND id = ?",
  "lock_exclusive()",
  "resolve_group_membership_enforcement_for_update",
]);

requireMarkers("crates/rustok-groups/src/graphql_localization.rs", [
  "group_translations",
  "upsert_group_translation",
  "delete_group_translation",
  "GroupLocalizationReadPort",
  "GroupLocalizationCommandPort",
]);
requireMarkers("crates/rustok-groups/rustok-module.toml", [
  'query = "graphql_application_cas::GroupsQueryRoot"',
  'mutation = "graphql_application_cas::GroupsMutationRoot"',
]);
requireMarkers("crates/rustok-groups/src/graphql_applications.rs", [
  "GroupsBaseQueryRoot",
  "GroupsBaseMutationRoot",
  "pub struct GroupsQueryRoot",
  "pub struct GroupsMutationRoot",
]);
requireMarkers("crates/rustok-groups/src/graphql_policy_history.rs", [
  "GroupsBaseQueryRoot",
  "pub struct GroupsQueryRoot",
  "pub type GroupsMutationRoot",
]);
requireMarkers("crates/rustok-groups/src/graphql_application_cas.rs", [
  "GroupsBaseQueryRoot",
  "GroupsPreApplicationMutationRoot",
  "GroupsApplicationCasMutation",
]);

requireMarkers("apps/server/tests/groups_localization_enforcement_expiry_sqlite.rs", [
  '#![cfg(feature = "mod-groups")]',
  "tempfile::tempdir()",
  "mode=rwc",
  "rustok_groups::migrations::migrations()",
  "GroupLocalizationService::new",
  "GroupLocalizationReadPort::list_group_translations",
  "GroupLocalizationCommandPort::upsert_group_translation",
  "GroupMembershipEnforcementCommandPort::suspend_membership",
  'assert_eq!(stored_status_during_suspension, "active")',
  'assert_eq!(read_error.code, "groups.membership_suspended")',
  'assert_eq!(write_error.code, "groups.membership_suspended")',
  "failed suspended write must not create French translation",
  "tokio::time::sleep",
  "expired suspension should restore administrator management reads without cleanup",
  "expired suspension should restore administrator management writes without cleanup",
  "stored_revision_after_expiry, suspended.membership_revision",
  "group_member_count",
]);
forbidMarkers("apps/server/tests/groups_localization_enforcement_expiry_sqlite.rs", [
  "UPDATE group_membership_enforcements",
  "DELETE FROM group_membership_enforcements",
  "UPDATE group_memberships SET status",
  "groups:manage",
]);

requireMarkers("crates/rustok-groups/docs/localization-enforcement-expiry-sqlite-contract.md", [
  "executable source added / maintainer execution pending",
  "stored membership lifecycle status remains `active`",
  "groups.membership_suspended",
  "no cleanup mutation",
  "membership revision has not changed since the original suspension",
  "localization_transport_parity",
  "localization_concurrency",
]);

requireMarkers("crates/rustok-groups/admin/src/core.rs", [
  "prepare_group_translation_query",
  "prepare_upsert_group_translation",
  "prepare_delete_group_translation",
  "normalize_locale_tag",
  "title.chars().count() > 240",
  "value.chars().count() > 500",
]);
forbidMarkers("crates/rustok-groups/admin/src/core.rs", ["use leptos", "leptos::"]);
requireMarkers("crates/rustok-groups/admin/src/transport.rs", [
  "load_group_admin_translations",
  "upsert_group_admin_translation",
  "delete_group_admin_translation",
  "execute_selected_transport",
  "never falls back",
]);
requireMarkers("crates/rustok-groups/admin/src/ui/localization.rs", [
  "prepare_group_translation_query",
  "prepare_upsert_group_translation",
  "prepare_delete_group_translation",
  "load_group_admin_translations",
  "upsert_group_admin_translation",
  "delete_group_admin_translation",
  "groups.admin.localization.lastTranslationWarning",
]);
forbidMarkers("crates/rustok-groups/admin/src/ui/localization.rs", [
  "graphql_adapter",
  "native_localization_adapter",
  "native_server_adapter",
]);

for (const relative of [
  "crates/rustok-groups/admin/locales/en.json",
  "crates/rustok-groups/admin/locales/ru.json",
]) {
  if (!requireFile(relative)) continue;
  const messages = JSON.parse(read(relative));
  for (const key of [
    "groups.admin.localization.title",
    "groups.admin.localization.locale",
    "groups.admin.localization.save",
    "groups.admin.localization.lastTranslationWarning",
  ]) {
    if (typeof messages[key] !== "string" || messages[key].trim() === "") {
      failures.push(`${relative}: missing localization key ${key}`);
    }
  }
}

if (requireFile("crates/rustok-groups/contracts/groups-fba-registry.json")) {
  const registry = JSON.parse(read("crates/rustok-groups/contracts/groups-fba-registry.json"));
  const enforcementPort = registry?.provider?.ports?.find((port) => port?.name === "GroupMembershipEnforcementCommandPort");
  const readPort = registry?.provider?.ports?.find((port) => port?.name === "GroupLocalizationReadPort");
  const commandPort = registry?.provider?.ports?.find((port) => port?.name === "GroupLocalizationCommandPort");
  if (enforcementPort?.graphql_root !== "graphql_application_cas::GroupsMutationRoot") {
    failures.push("Groups enforcement registry must point at the stable final GraphQL root");
  }
  if (!readPort?.operations?.includes("list_group_translations")) {
    failures.push("Groups registry is missing localization read operation");
  }
  if (!commandPort?.operations?.includes("upsert_group_translation") || !commandPort?.operations?.includes("delete_group_translation")) {
    failures.push("Groups registry is missing localization command operations");
  }
  if (readPort?.authorization !== "effective_active_owner_or_admin_or_platform_manage") {
    failures.push("Groups localization read authorization must use effective manager state");
  }
  if (commandPort?.authorization !== "effective_active_owner_or_admin_or_platform_manage") {
    failures.push("Groups localization command authorization must use effective manager state");
  }
  if (commandPort?.authorization_lock_order !== "group_then_membership_then_enforcement") {
    failures.push("Groups localization command must retain canonical enforcement lock order");
  }
  if (commandPort?.exact_locale_only !== true || commandPort?.last_translation_delete !== "deny") {
    failures.push("Groups localization command invariants are not locked");
  }
  if (registry?.localization?.module_local_fallback !== false) {
    failures.push("Groups localization must reject module-local fallback");
  }
  if (registry?.localization?.effective_authorization !== "implemented_source") {
    failures.push("Groups localization effective authorization must be source-complete");
  }
  if (registry?.localization?.write_lock_order !== "group_then_membership_then_enforcement") {
    failures.push("Groups localization registry must publish the shared write lock order");
  }
  if (registry?.evidence?.localization_transport_parity !== null || registry?.evidence?.localization_concurrency !== null) {
    failures.push("unexecuted localization runtime evidence must remain null");
  }
}

if (failures.length > 0) {
  console.error("Groups localization boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Groups exact-locale localization, effective owner-clock authorization, SQLite suspension/expiry source, shared writer reservation, stable GraphQL root, FBA, FFA, last-row, and no-fallback boundary checks passed.");
