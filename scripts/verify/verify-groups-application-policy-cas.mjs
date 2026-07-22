import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const failures = [];
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");
const exists = (relative) => fs.existsSync(path.join(root, relative));
const requireFile = (relative) => {
  if (!exists(relative)) {
    failures.push(`missing Groups application policy CAS artifact: ${relative}`);
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
  "crates/rustok-groups/src/applications_legacy.rs",
  "crates/rustok-groups/src/applications_cas.rs",
  "crates/rustok-groups/src/applications_policy_management.rs",
  "crates/rustok-groups/src/graphql_application_policy_management.rs",
  "crates/rustok-groups/src/graphql_application_cas.rs",
  "crates/rustok-groups/rustok-module.toml",
  "crates/rustok-groups/admin/src/application_model.rs",
  "crates/rustok-groups/admin/src/application_core.rs",
  "crates/rustok-groups/admin/src/transport.rs",
  "crates/rustok-groups/admin/src/transport/native_policy_locale_adapter.rs",
  "crates/rustok-groups/admin/src/transport/graphql_policy_locale_adapter.rs",
  "crates/rustok-groups/admin/src/ui/policy_editor.rs",
  "crates/rustok-groups/storefront/src/application_model.rs",
  "crates/rustok-groups/storefront/src/application_core.rs",
  "crates/rustok-groups/storefront/src/transport.rs",
  "crates/rustok-groups/storefront/src/transport/native_applications_adapter.rs",
  "crates/rustok-groups/storefront/src/transport/graphql_applications_adapter.rs",
  "crates/rustok-groups/storefront/src/ui/application.rs",
  "crates/rustok-groups/contracts/groups-fba-registry.json",
  "crates/rustok-groups/docs/implementation-plan.md",
]) requireFile(relative);

requireMarkers("crates/rustok-groups/src/applications.rs", [
  'include!("applications_legacy.rs")',
  'include!("applications_cas.rs")',
  'include!("applications_policy_management.rs")',
]);
requireMarkers("crates/rustok-groups/src/applications_cas.rs", [
  "GroupApplicationPolicyPrecondition",
  "GroupApplicationCasCommandPort",
  "upsert_group_application_policy_if_current",
  "submit_group_membership_application_if_current",
  'GROUP_APPLICATION_POLICY_CHANGED_CODE: &str =\n    "groups.application_policy_changed"',
  "find_group_for_update",
  "ensure_policy_update_precondition",
  "ensure_loaded_policy_precondition",
  "replay_receipt::<UpsertGroupApplicationPolicyResult>",
  "replay_receipt::<SubmitGroupMembershipApplicationResult>",
  '"expected_revision_enforced": true',
]);
const cas = read("crates/rustok-groups/src/applications_cas.rs");
for (const method of ["upsert_policy_if_current_owned", "submit_application_if_current_owned"]) {
  const start = cas.indexOf(`async fn ${method}`);
  const lock = cas.indexOf("find_group_for_update", start);
  const check = cas.indexOf(method.startsWith("upsert") ? "ensure_policy_update_precondition" : "ensure_loaded_policy_precondition", start);
  const stateWrite = cas.indexOf(method.startsWith("upsert") ? "membership_policy::ActiveModel" : "membership::ActiveModel", start);
  const replay = cas.indexOf("replay_receipt::<", start);
  if (!(start >= 0 && replay > start && lock > replay && check > lock && stateWrite > check)) failures.push(`${method}: receipt replay, group lock, CAS check, and state writes are out of order`);
}

requireMarkers("crates/rustok-groups/src/applications_policy_management.rs", [
  "GroupApplicationPolicyManagementReadPort",
  "read_group_application_policy_for_management",
  "translation_exists: false",
  "policy_id: Some(policy.id)",
]);
requireMarkers("crates/rustok-groups/src/graphql_application_policy_management.rs", [
  "GroupsApplicationPolicyManagementQuery",
  "group_application_policy_locale_catalog",
  "group_application_policy_for_management",
]);
requireMarkers("crates/rustok-groups/src/graphql_application_cas.rs", [
  "MergedObject",
  "GroupsPreApplicationMutationRoot",
  "GroupsApplicationCasMutation",
  "GroupsApplicationPolicyManagementQuery",
  "GroupApplicationPolicyPreconditionInputGql",
  "upsert_group_application_policy_if_current",
  "submit_group_membership_application_if_current",
  "review_group_membership_application",
  "GROUP_APPLICATION_POLICY_CHANGED_CODE",
  "ErrorExtensions",
  'extensions.set("code", GROUP_APPLICATION_POLICY_CHANGED_CODE)',
]);
forbidMarkers("crates/rustok-groups/src/graphql_application_cas.rs", [
  "GroupsMutationRoot as GroupsBaseMutationRoot",
  "async fn upsert_group_application_policy(\n",
  "async fn submit_group_membership_application(\n",
]);
requireMarkers("crates/rustok-groups/rustok-module.toml", [
  'query = "graphql_application_cas::GroupsQueryRoot"',
  'mutation = "graphql_application_cas::GroupsMutationRoot"',
]);
requireMarkers("crates/rustok-groups/src/ports.rs", [
  '"GroupApplicationPolicyManagementReadPort"',
  '"GroupApplicationCasCommandPort"',
]);

requireMarkers("crates/rustok-groups/admin/src/application_model.rs", [
  "GroupsAdminApplicationPolicyManagementView",
  "GroupsAdminApplicationPolicyPrecondition",
  "expected_policy: Option<GroupsAdminApplicationPolicyPrecondition>",
  "pub fn precondition",
]);
requireMarkers("crates/rustok-groups/admin/src/application_core.rs", [
  "prepare_group_application_policy_locale_catalog_query",
  "InvalidExpectedPolicy",
  "expected.revision == 0",
  "expected.locale != locale",
]);
requireMarkers("crates/rustok-groups/admin/src/transport.rs", [
  '"groups.admin.applications.policy.locales"',
  '"groups.admin.applications.policy.management_read"',
  '"groups.admin.applications.policy.upsert_if_current"',
  "native_policy_locale_adapter::upsert_group_application_policy",
  "graphql_policy_locale_adapter::upsert_group_application_policy",
  "execute_selected_transport",
  "never falls back",
]);
requireMarkers("crates/rustok-groups/admin/src/transport/native_policy_locale_adapter.rs", [
  "GroupApplicationPolicyManagementReadPort",
  "GroupApplicationCasCommandPort",
  "request::RequestContext",
  "request.locale",
  "upsert_group_application_policy_if_current",
  'format!("{}: {}", error.code, error.message)',
]);
forbidMarkers("crates/rustok-groups/admin/src/transport/native_policy_locale_adapter.rs", [
  "GroupApplicationReadPort",
  "PortActor::user(auth.user_id.to_string()),\n            query.locale",
  "PortActor::user(auth.user_id.to_string()),\n            command.locale.clone()",
]);
requireMarkers("crates/rustok-groups/admin/src/transport/graphql_policy_locale_adapter.rs", [
  "groupApplicationPolicyLocaleCatalog",
  "groupApplicationPolicyForManagement",
  "upsertGroupApplicationPolicyIfCurrent",
  "GroupApplicationPolicyPreconditionInputGql",
  "expectedPolicy",
]);
forbidMarkers("crates/rustok-groups/admin/src/transport/graphql_policy_locale_adapter.rs", [
  "Some(query.locale.clone())",
  "Some(command.locale.clone())",
  "groupApplicationPolicy(groupId",
]);
requireMarkers("crates/rustok-groups/admin/src/ui/policy_editor.rs", [
  "loaded_policy",
  "management_loaded",
  "policy.precondition()",
  'list="groups-policy-locales"',
  "GROUP_APPLICATION_POLICY_CHANGED_CODE",
  "copy.stale",
]);
forbidMarkers("crates/rustok-groups/admin/src/ui/policy_editor.rs", [
  "preflight_context",
  "current.revision != expected",
  "prop:value=move || locale.get() readonly",
]);

requireMarkers("crates/rustok-groups/storefront/src/application_model.rs", [
  "GroupsStorefrontApplicationPolicyPrecondition",
  "expected_policy: GroupsStorefrontApplicationPolicyPrecondition",
  "pub locale: String",
]);
requireMarkers("crates/rustok-groups/storefront/src/application_core.rs", [
  "GROUP_APPLICATION_POLICY_CHANGED_CODE",
  "GroupsStorefrontApplicationPolicyPrecondition::from(policy)",
  "is_application_policy_changed",
  "normalize_locale_tag",
]);
requireMarkers("crates/rustok-groups/storefront/src/transport.rs", [
  '"groups.storefront.applications.submit_if_current"',
  "execute_selected_transport",
  "never falls back",
]);
requireMarkers("crates/rustok-groups/storefront/src/transport/native_applications_adapter.rs", [
  "GroupApplicationCasCommandPort",
  "submit_group_membership_application_if_current",
  "command.expected_policy.locale.clone()",
  'format!("{}: {}", error.code, error.message)',
]);
requireMarkers("crates/rustok-groups/storefront/src/transport/graphql_applications_adapter.rs", [
  "submitGroupMembershipApplicationIfCurrent",
  "GroupApplicationPolicyPreconditionInputGql",
  "expectedPolicy",
  "Some(command.expected_policy.locale.clone())",
]);
requireMarkers("crates/rustok-groups/storefront/src/ui/application.rs", [
  "policy_changed",
  "is_application_policy_changed",
  "reload_policy",
  "page_state.refetch()",
  "set_answers.set(BTreeMap::new())",
  "set_acknowledged_rules.set(BTreeSet::new())",
  "query_writer.clear_key(GROUP_APPLICATION_QUERY_KEY)",
]);
for (const relative of [
  "crates/rustok-groups/admin/src/ui/policy_editor.rs",
  "crates/rustok-groups/storefront/src/ui/application.rs",
]) forbidMarkers(relative, ["graphql_application_cas", "native_applications_adapter", "graphql_applications_adapter"]);

for (const relative of [
  "crates/rustok-groups/admin/locales/en.json",
  "crates/rustok-groups/admin/locales/ru.json",
  "crates/rustok-groups/storefront/locales/en.json",
  "crates/rustok-groups/storefront/locales/ru.json",
]) {
  if (!requireFile(relative)) continue;
  try { JSON.parse(read(relative)); }
  catch (error) { failures.push(`${relative}: invalid JSON: ${error.message}`); }
}

if (requireFile("crates/rustok-groups/contracts/groups-fba-registry.json")) {
  const registry = JSON.parse(read("crates/rustok-groups/contracts/groups-fba-registry.json"));
  const casPort = registry?.provider?.ports?.find((port) => port?.name === "GroupApplicationCasCommandPort");
  if (!casPort?.operations?.includes("upsert_group_application_policy_if_current") || !casPort?.operations?.includes("submit_group_membership_application_if_current")) failures.push("Groups registry is missing application CAS operations");
  if (casPort?.conflict_code !== "groups.application_policy_changed") failures.push("Groups application CAS conflict code is not stable");
  const managementPort = registry?.provider?.ports?.find((port) => port?.name === "GroupApplicationPolicyManagementReadPort");
  if (managementPort?.selected_locale_source !== "typed_request" || managementPort?.port_context_locale_owner !== "host_runtime") failures.push("Groups management locale contract is not explicit");
  const applications = registry?.membership_applications;
  if (applications?.atomic_expected_revision_guard !== "implemented_source") failures.push("Groups atomic expected-revision guard must be source-complete");
  if (applications?.admin_policy_locale_picker !== "implemented_source") failures.push("Groups admin policy locale picker must be source-complete");
  if (applications?.storefront_stale_submit_ux !== "implemented_source") failures.push("Groups storefront stale-submit UX must be source-complete");
  if (applications?.final_graphql_legacy_application_mutations !== "not_exposed") failures.push("Groups final GraphQL root must not expose legacy application mutations");
  if (applications?.legacy_unconditional_application_command_port !== "rust_compatibility_only_not_exposed_by_final_graphql_or_module_ffa") failures.push("Groups registry must disclose the legacy Rust-only compatibility port");
  if (registry?.evidence?.membership_application_policy_cas !== null) failures.push("unexecuted application CAS runtime evidence must remain null");
  if (registry?.evidence?.membership_application_policy_locale_management !== null) failures.push("unexecuted locale-management runtime evidence must remain null");
}

requireMarkers("crates/rustok-groups/docs/implementation-plan.md", [
  "GroupApplicationPolicyManagementReadPort",
  "GroupApplicationCasCommandPort",
  "groups.application_policy_changed",
  "receipt replay is checked before the precondition",
  "final GraphQL root does not expose",
  "legacy unconditional",
  "verify-groups-application-policy-cas.mjs",
  "verify-groups-application-policy-locales.mjs",
  "membership_application_policy_cas` remains `null",
]);

if (failures.length > 0) {
  console.error("Groups application policy CAS boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Groups application policy CAS, explicit management locale, stable conflict, GraphQL no-bypass, FFA recovery, and no-fallback boundary checks passed.");
