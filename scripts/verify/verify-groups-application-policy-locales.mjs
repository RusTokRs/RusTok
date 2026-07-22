import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const failures = [];
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");
const exists = (relative) => fs.existsSync(path.join(root, relative));

const requireFile = (relative) => {
  if (!exists(relative)) {
    failures.push(`missing Groups policy locale management artifact: ${relative}`);
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
  "crates/rustok-groups/src/applications_policy_management.rs",
  "crates/rustok-groups/src/graphql_application_policy_management.rs",
  "crates/rustok-groups/src/graphql_application_cas.rs",
  "crates/rustok-groups/src/ports.rs",
  "crates/rustok-groups/admin/src/application_model.rs",
  "crates/rustok-groups/admin/src/application_core.rs",
  "crates/rustok-groups/admin/src/transport.rs",
  "crates/rustok-groups/admin/src/transport/native_policy_locale_adapter.rs",
  "crates/rustok-groups/admin/src/transport/graphql_policy_locale_adapter.rs",
  "crates/rustok-groups/admin/src/ui/policy_editor.rs",
  "crates/rustok-groups/contracts/groups-fba-registry.json",
  "crates/rustok-groups/docs/implementation-plan.md",
]) requireFile(relative);

requireMarkers("crates/rustok-groups/src/applications.rs", [
  'include!("applications_policy_management.rs")',
]);
requireMarkers("crates/rustok-groups/src/applications_policy_management.rs", [
  "GroupApplicationPolicyManagementReadPort",
  "list_group_application_policy_locales",
  "read_group_application_policy_for_management",
  "normalize_locale_tag(&request.locale)",
  "authorize_policy_management",
  "translation_exists: false",
  "policy_id: Some(policy.id)",
  "revision: Some(policy.revision.max(1) as u64)",
  "questions: Vec::new()",
  "rules: Vec::new()",
]);
forbidMarkers("crates/rustok-groups/src/applications_policy_management.rs", [
  "load_policy_for_locale",
  "context.locale",
  "unwrap_or_else(|| locales",
  "first()",
]);

requireMarkers("crates/rustok-groups/src/graphql_application_policy_management.rs", [
  "GroupsApplicationPolicyManagementQuery",
  "group_application_policy_locale_catalog",
  "group_application_policy_for_management",
  "ListGroupApplicationPolicyLocalesRequest",
  "ReadGroupApplicationPolicyForManagementRequest",
]);
requireMarkers("crates/rustok-groups/src/graphql_application_cas.rs", [
  "GroupsApplicationPolicyManagementQuery",
  "GroupsApplicationLifecycleQuery,",
  "GroupsApplicationPolicyManagementQuery,",
]);
requireMarkers("crates/rustok-groups/src/ports.rs", [
  '"GroupApplicationPolicyManagementReadPort"',
]);

requireMarkers("crates/rustok-groups/admin/src/application_model.rs", [
  "GroupsAdminApplicationPolicyLocaleCatalog",
  "GroupsAdminApplicationPolicyManagementView",
  "translation_exists: bool",
  "pub fn precondition",
]);
requireMarkers("crates/rustok-groups/admin/src/application_core.rs", [
  "prepare_group_application_policy_locale_catalog_query",
  "prepare_group_application_policy_query",
  "normalize_locale_tag(locale)",
]);
requireMarkers("crates/rustok-groups/admin/src/transport.rs", [
  "load_group_admin_application_policy_locale_catalog",
  "load_group_admin_application_policy_for_management",
  '"groups.admin.applications.policy.locales"',
  '"groups.admin.applications.policy.management_read"',
  "execute_selected_transport",
  "never falls back",
]);
requireMarkers("crates/rustok-groups/admin/src/transport/native_policy_locale_adapter.rs", [
  "request::RequestContext",
  "request.locale",
  "ListGroupApplicationPolicyLocalesRequest",
  "ReadGroupApplicationPolicyForManagementRequest",
  "locale: query.locale",
]);
forbidMarkers("crates/rustok-groups/admin/src/transport/native_policy_locale_adapter.rs", [
  "PortActor::user(auth.user_id.to_string()),\n            query.locale",
  "PortActor::user(auth.user_id.to_string()),\n            command.locale.clone()",
  "GroupApplicationReadPort",
]);
requireMarkers("crates/rustok-groups/admin/src/transport/graphql_policy_locale_adapter.rs", [
  "groupApplicationPolicyLocaleCatalog",
  "groupApplicationPolicyForManagement",
  "ManagementVariables",
  "translation_exists",
  "tenant_slug,\n        None,",
]);
forbidMarkers("crates/rustok-groups/admin/src/transport/graphql_policy_locale_adapter.rs", [
  "Some(query.locale.clone())",
  "Some(command.locale.clone())",
  "groupApplicationPolicy(groupId",
]);
requireMarkers("crates/rustok-groups/admin/src/ui/policy_editor.rs", [
  "locale_options",
  "management_loaded",
  "translation_exists",
  'list="groups-policy-locales"',
  "load_group_admin_application_policy_locale_catalog",
  "load_group_admin_application_policy_for_management",
  "policy.precondition()",
  "new_translation",
  "disabled=move || !management_loaded.get()",
]);
forbidMarkers("crates/rustok-groups/admin/src/ui/policy_editor.rs", [
  "prop:value=move || locale.get() readonly",
  "native_policy_locale_adapter",
  "graphql_policy_locale_adapter",
]);

for (const relative of [
  "crates/rustok-groups/admin/locales/en.json",
  "crates/rustok-groups/admin/locales/ru.json",
]) {
  if (!requireFile(relative)) continue;
  try { JSON.parse(read(relative)); } catch (error) {
    failures.push(`${relative}: invalid JSON: ${error.message}`);
  }
  requireMarkers(relative, [
    "groups.admin.policyEditor.availableLocales",
    "groups.admin.policyEditor.existingTranslation",
    "groups.admin.policyEditor.newTranslation",
  ]);
}

if (requireFile("crates/rustok-groups/contracts/groups-fba-registry.json")) {
  const registry = JSON.parse(read("crates/rustok-groups/contracts/groups-fba-registry.json"));
  const port = registry?.provider?.ports?.find((entry) => entry?.name === "GroupApplicationPolicyManagementReadPort");
  if (!port?.operations?.includes("list_group_application_policy_locales") || !port?.operations?.includes("read_group_application_policy_for_management")) failures.push("registry is missing policy locale management operations");
  if (port?.selected_locale_source !== "typed_request") failures.push("selected management locale must come from the typed request");
  if (port?.port_context_locale_owner !== "host_runtime") failures.push("PortContext locale must remain host-owned");
  if (registry?.membership_applications?.management_policy_locale_context_substitution !== "forbidden") failures.push("registry must forbid selected-locale context substitution");
  if (registry?.membership_applications?.admin_policy_locale_picker !== "implemented_source") failures.push("admin policy locale picker must be source-complete");
  if (registry?.evidence?.membership_application_policy_locale_management !== null) failures.push("unexecuted policy locale management evidence must remain null");
}

requireMarkers("crates/rustok-groups/docs/implementation-plan.md", [
  "GroupApplicationPolicyManagementReadPort",
  "selected exact locale",
  "typed request",
  "empty management view",
  "verify-groups-application-policy-locales.mjs",
  "membership_application_policy_locale_management` remains `null",
]);

if (failures.length > 0) {
  console.error("Groups application policy locale management verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Groups application policy locale catalog, explicit selected-locale management, host-locale separation, CAS, FFA/FBA, and no-fallback checks passed.");
