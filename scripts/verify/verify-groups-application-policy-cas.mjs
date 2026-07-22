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

for (const relative of [
  "crates/rustok-groups/src/applications.rs",
  "crates/rustok-groups/src/applications_legacy.rs",
  "crates/rustok-groups/src/applications_cas.rs",
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
  "crates/rustok-groups/storefront/src/transport/native_applications_adapter.rs",
  "crates/rustok-groups/storefront/src/transport/graphql_applications_adapter.rs",
  "crates/rustok-groups/storefront/src/ui/application.rs",
  "crates/rustok-groups/contracts/groups-fba-registry.json",
  "crates/rustok-groups/docs/implementation-plan.md",
]) {
  requireFile(relative);
}

requireMarkers("crates/rustok-groups/src/applications.rs", [
  'include!("applications_legacy.rs")',
  'include!("applications_cas.rs")',
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

if (requireFile("crates/rustok-groups/src/applications_cas.rs")) {
  const source = read("crates/rustok-groups/src/applications_cas.rs");
  for (const method of [
    "upsert_policy_if_current_owned",
    "submit_application_if_current_owned",
  ]) {
    const start = source.indexOf(`async fn ${method}`);
    if (start < 0) continue;
    const lock = source.indexOf("find_group_for_update", start);
    const check = source.indexOf(
      method.startsWith("upsert")
        ? "ensure_policy_update_precondition"
        : "ensure_loaded_policy_precondition",
      start,
    );
    const stateWrite = source.indexOf(
      method.startsWith("upsert")
        ? "membership_policy::ActiveModel"
        : "membership::ActiveModel",
      start,
    );
    if (!(lock >= 0 && check > lock && stateWrite > check)) {
      failures.push(`${method}: CAS check must follow the group lock and precede owner state writes`);
    }
  }
  for (const method of [
    "upsert_policy_if_current_owned",
    "submit_application_if_current_owned",
  ]) {
    const start = source.indexOf(`async fn ${method}`);
    const replay = source.indexOf("replay_receipt::<", start);
    const lock = source.indexOf("find_group_for_update", start);
    if (!(replay >= 0 && lock > replay)) {
      failures.push(`${method}: receipt replay must be evaluated before CAS precondition recheck`);
    }
  }
}

requireMarkers("crates/rustok-groups/src/graphql_application_cas.rs", [
  "MergedObject",
  "GroupsApplicationCasMutation",
  "GroupApplicationPolicyPreconditionInputGql",
  "upsert_group_application_policy_if_current",
  "submit_group_membership_application_if_current",
  "GROUP_APPLICATION_POLICY_CHANGED_CODE",
]);
requireMarkers("crates/rustok-groups/rustok-module.toml", [
  'query = "graphql_application_cas::GroupsQueryRoot"',
  'mutation = "graphql_application_cas::GroupsMutationRoot"',
]);
requireMarkers("crates/rustok-groups/src/ports.rs", [
  '"GroupApplicationCasCommandPort"',
]);

requireMarkers("crates/rustok-groups/admin/src/application_model.rs", [
  "GroupsAdminApplicationPolicyPrecondition",
  "expected_policy: Option<GroupsAdminApplicationPolicyPrecondition>",
]);
requireMarkers("crates/rustok-groups/admin/src/application_core.rs", [
  "InvalidExpectedPolicy",
  "expected.revision == 0",
  "expected.locale != locale",
]);
requireMarkers("crates/rustok-groups/admin/src/transport.rs", [
  '"groups.admin.applications.policy.upsert_if_current"',
  "native_policy_locale_adapter::upsert_group_application_policy",
  "graphql_policy_locale_adapter::upsert_group_application_policy",
  "execute_selected_transport",
  "never falls back",
]);
requireMarkers("crates/rustok-groups/admin/src/transport/native_policy_locale_adapter.rs", [
  "GroupApplicationCasCommandPort",
  "upsert_group_application_policy_if_current",
  'format!("{}: {}", error.code, error.message)',
]);
requireMarkers("crates/rustok-groups/admin/src/transport/graphql_policy_locale_adapter.rs", [
  "upsertGroupApplicationPolicyIfCurrent",
  "GroupApplicationPolicyPreconditionInputGql",
  "expectedPolicy",
]);
requireMarkers("crates/rustok-groups/admin/src/ui/policy_editor.rs", [
  "loaded_policy",
  "GroupsAdminApplicationPolicyPrecondition::from",
  "GROUP_APPLICATION_POLICY_CHANGED_CODE",
  "copy.stale",
]);
forbidMarkers("crates/rustok-groups/admin/src/ui/policy_editor.rs", [
  "preflight_context",
  "current.revision != expected",
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
  "policy.refetch()",
  "set_answers.set(BTreeMap::new())",
  "set_acknowledged_rules.set(BTreeSet::new())",
  "query_writer.clear_key(GROUP_APPLICATION_QUERY_KEY)",
]);

for (const relative of [
  "crates/rustok-groups/admin/src/ui/policy_editor.rs",
  "crates/rustok-groups/storefront/src/ui/application.rs",
]) {
  forbidMarkers(relative, [
    "graphql_application_cas",
    "native_applications_adapter",
    "graphql_applications_adapter",
  ]);
}

for (const relative of [
  "crates/rustok-groups/admin/locales/en.json",
  "crates/rustok-groups/admin/locales/ru.json",
  "crates/rustok-groups/storefront/locales/en.json",
  "crates/rustok-groups/storefront/locales/ru.json",
]) {
  if (!requireFile(relative)) continue;
  try {
    JSON.parse(read(relative));
  } catch (error) {
    failures.push(`${relative}: invalid JSON: ${error.message}`);
  }
}

if (requireFile("crates/rustok-groups/contracts/groups-fba-registry.json")) {
  const registry = JSON.parse(
    read("crates/rustok-groups/contracts/groups-fba-registry.json"),
  );
  const casPort = registry?.provider?.ports?.find(
    (port) => port?.name === "GroupApplicationCasCommandPort",
  );
  if (
    !casPort?.operations?.includes("upsert_group_application_policy_if_current") ||
    !casPort?.operations?.includes("submit_group_membership_application_if_current")
  ) {
    failures.push("Groups registry is missing application CAS operations");
  }
  if (casPort?.conflict_code !== "groups.application_policy_changed") {
    failures.push("Groups application CAS conflict code is not stable");
  }
  const applications = registry?.membership_applications;
  if (applications?.atomic_expected_revision_guard !== "implemented_source") {
    failures.push("Groups atomic expected-revision guard must be source-complete");
  }
  if (applications?.storefront_stale_submit_ux !== "implemented_source") {
    failures.push("Groups storefront stale-submit UX must be source-complete");
  }
  if (
    applications?.legacy_unconditional_application_command_port !==
    "compatibility_only_not_used_by_module_ffa"
  ) {
    failures.push("Groups registry must disclose the legacy unconditional command port");
  }
  if (registry?.evidence?.membership_application_policy_cas !== null) {
    failures.push("unexecuted application CAS runtime evidence must remain null");
  }
}

requireMarkers("crates/rustok-groups/docs/implementation-plan.md", [
  "GroupApplicationCasCommandPort",
  "groups.application_policy_changed",
  "receipt replay is checked before the precondition",
  "legacy unconditional",
  "verify-groups-application-policy-cas.mjs",
  "membership_application_policy_cas` remains `null",
]);

if (failures.length > 0) {
  console.error("Groups application policy CAS boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Groups application policy CAS, stable conflict, exact-locale, FFA recovery, and no-fallback boundary checks passed.");
