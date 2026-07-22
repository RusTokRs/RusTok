import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const failures = [];
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");
const exists = (relative) => fs.existsSync(path.join(root, relative));

const requireFile = (relative) => {
  if (!exists(relative)) {
    failures.push(`missing Groups application lifecycle artifact: ${relative}`);
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

for (const relative of [
  "crates/rustok-groups/src/applications.rs",
  "crates/rustok-groups/src/applications_lifecycle.rs",
  "crates/rustok-groups/src/graphql_application_cas.rs",
  "crates/rustok-groups/src/graphql_application_lifecycle.rs",
  "crates/rustok-groups/rustok-module.toml",
  "crates/rustok-groups/admin/src/application_core.rs",
  "crates/rustok-groups/admin/src/application_model.rs",
  "crates/rustok-groups/admin/src/transport.rs",
  "crates/rustok-groups/admin/src/transport/native_application_lifecycle_adapter.rs",
  "crates/rustok-groups/admin/src/transport/graphql_application_lifecycle_adapter.rs",
  "crates/rustok-groups/admin/src/ui/applications.rs",
  "crates/rustok-groups/storefront/src/application_core.rs",
  "crates/rustok-groups/storefront/src/application_model.rs",
  "crates/rustok-groups/storefront/src/transport.rs",
  "crates/rustok-groups/storefront/src/transport/native_application_lifecycle_adapter.rs",
  "crates/rustok-groups/storefront/src/transport/graphql_application_lifecycle_adapter.rs",
  "crates/rustok-groups/storefront/src/ui/application.rs",
  "crates/rustok-groups/contracts/groups-fba-registry.json",
  "crates/rustok-groups/docs/implementation-plan.md",
]) requireFile(relative);

requireMarkers("crates/rustok-groups/src/applications.rs", [
  'include!("applications_lifecycle.rs")',
]);
requireMarkers("crates/rustok-groups/src/applications_lifecycle.rs", [
  "GroupApplicationLifecycleReadPort",
  "GroupApplicationLifecycleCommandPort",
  "read_my_group_membership_application",
  "cancel_group_membership_application",
  "reopen_group_membership_application",
  "only a pending membership application can be cancelled",
  "GroupMembershipStatus::Left",
  "GroupApplicationStatus::Cancelled",
  "GroupApplicationStatus::Rejected | GroupApplicationStatus::Cancelled",
  "authorize_application_review",
  '"group.membership_application_cancelled"',
  '"group.membership_application_reopened"',
  '"snapshot_preserved": true',
  "replay_receipt::<GroupApplicationLifecycleResult>",
  "store_receipt",
]);
forbidMarkers("crates/rustok-groups/src/applications_lifecycle.rs", [
  "rustok_profiles::",
  "rustok_notifications::",
  "policy_snapshot = Set(",
]);

const lifecycle = read("crates/rustok-groups/src/applications_lifecycle.rs");
for (const method of ["cancel_application_owned", "reopen_application_owned"]) {
  const start = lifecycle.indexOf(`async fn ${method}`);
  const applicationLock = lifecycle.indexOf("find_application_for_update", start);
  const groupLock = lifecycle.indexOf("find_group_for_update", start);
  const stateWrite = lifecycle.indexOf("ActiveModel", groupLock);
  if (!(start >= 0 && applicationLock > start && groupLock > applicationLock && stateWrite > groupLock)) {
    failures.push(`${method}: lifecycle lock order must be application then group before state writes`);
  }
  const replay = lifecycle.indexOf("replay_receipt::<GroupApplicationLifecycleResult>", start);
  if (!(replay > start && replay < applicationLock)) {
    failures.push(`${method}: idempotent replay must precede lifecycle state locks`);
  }
}

requireMarkers("crates/rustok-groups/src/graphql_application_lifecycle.rs", [
  "GroupsApplicationLifecycleQuery",
  "GroupsApplicationLifecycleMutation",
  "my_group_membership_application",
  "cancel_group_membership_application",
  "reopen_group_membership_application",
]);
requireMarkers("crates/rustok-groups/src/graphql_application_cas.rs", [
  "GroupsApplicationLifecycleQuery",
  "GroupsApplicationLifecycleMutation",
  "GroupsBaseQueryRoot, GroupsApplicationLifecycleQuery",
  "GroupsApplicationCasMutation,",
  "GroupsApplicationLifecycleMutation,",
]);
requireMarkers("crates/rustok-groups/rustok-module.toml", [
  'query = "graphql_application_cas::GroupsQueryRoot"',
  'mutation = "graphql_application_cas::GroupsMutationRoot"',
]);

requireMarkers("crates/rustok-groups/storefront/src/application_core.rs", [
  "prepare_my_group_membership_application_query",
  "prepare_cancel_group_membership_application",
  "groups-storefront-cancel-application-",
]);
requireMarkers("crates/rustok-groups/storefront/src/transport.rs", [
  "load_groups_storefront_my_application",
  "cancel_groups_storefront_membership_application",
  '"groups.storefront.applications.my"',
  '"groups.storefront.applications.cancel"',
  "execute_selected_transport",
  "never falls back",
]);
requireMarkers("crates/rustok-groups/storefront/src/ui/application.rs", [
  "pending_existing",
  "approved_existing",
  "rejected_existing",
  "cancelled_existing",
  "prepare_cancel_group_membership_application",
  "cancel_groups_storefront_membership_application",
  "page_state.refetch()",
  "query_writer.clear_key(GROUP_APPLICATION_QUERY_KEY)",
]);
forbidMarkers("crates/rustok-groups/storefront/src/ui/application.rs", [
  "native_application_lifecycle_adapter",
  "graphql_application_lifecycle_adapter",
  "GroupApplicationService",
]);

requireMarkers("crates/rustok-groups/admin/src/application_core.rs", [
  "prepare_reopen_group_membership_application",
  "groups-admin-reopen-application-",
]);
requireMarkers("crates/rustok-groups/admin/src/transport.rs", [
  "reopen_group_admin_membership_application",
  '"groups.admin.applications.reopen"',
  "execute_selected_transport",
  "never falls back",
]);
requireMarkers("crates/rustok-groups/admin/src/ui/applications.rs", [
  "prepare_reopen_group_membership_application",
  "reopen_group_admin_membership_application",
  'matches!(item.status.as_str(), "rejected" | "cancelled")',
  "set_status",
]);
forbidMarkers("crates/rustok-groups/admin/src/ui/applications.rs", [
  "native_application_lifecycle_adapter",
  "graphql_application_lifecycle_adapter",
  "GroupApplicationService",
]);

for (const relative of [
  "crates/rustok-groups/admin/locales/en.json",
  "crates/rustok-groups/admin/locales/ru.json",
  "crates/rustok-groups/storefront/locales/en.json",
  "crates/rustok-groups/storefront/locales/ru.json",
]) {
  if (!requireFile(relative)) continue;
  try { JSON.parse(read(relative)); } catch (error) {
    failures.push(`${relative}: invalid JSON: ${error.message}`);
  }
}

if (requireFile("crates/rustok-groups/contracts/groups-fba-registry.json")) {
  const registry = JSON.parse(read("crates/rustok-groups/contracts/groups-fba-registry.json"));
  const readPort = registry?.provider?.ports?.find((port) => port?.name === "GroupApplicationLifecycleReadPort");
  const commandPort = registry?.provider?.ports?.find((port) => port?.name === "GroupApplicationLifecycleCommandPort");
  if (!readPort?.operations?.includes("read_my_group_membership_application")) failures.push("registry is missing candidate application read operation");
  if (!commandPort?.operations?.includes("cancel_group_membership_application") || !commandPort?.operations?.includes("reopen_group_membership_application")) failures.push("registry is missing application lifecycle commands");
  if (registry?.membership_applications?.manager_reopen_snapshot !== "preserve") failures.push("manager reopen must preserve the submitted policy snapshot");
  if (registry?.membership_applications?.fresh_resubmit_snapshot !== "replace_with_current_cas_policy") failures.push("fresh resubmit must use the current CAS policy");
  if (registry?.membership_applications?.transport_fallback !== "never") failures.push("application lifecycle transport must not fall back");
  if (registry?.evidence?.membership_application_lifecycle !== null) failures.push("unexecuted application lifecycle evidence must remain null");
}

requireMarkers("crates/rustok-groups/docs/implementation-plan.md", [
  "GroupApplicationLifecycleReadPort",
  "GroupApplicationLifecycleCommandPort",
  "candidate cancellation",
  "manager reopen",
  "verify-groups-application-lifecycle.mjs",
  "membership_application_lifecycle` remains `null",
]);

if (failures.length > 0) {
  console.error("Groups membership application lifecycle verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Groups membership application cancel, reopen, resubmit, snapshot, FFA/FBA, and no-fallback boundary checks passed.");
