import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const failures = [];
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");
const requireFile = (relative) => {
  if (!fs.existsSync(path.join(root, relative))) {
    failures.push(`missing Groups membership policy revision artifact: ${relative}`);
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
  "crates/rustok-groups/src/migrations/m20260722_000007_create_group_membership_policy_revisions.rs",
  "crates/rustok-groups/src/migrations/mod.rs",
  "crates/rustok-groups/src/application_entities.rs",
  "crates/rustok-groups/src/policy_history.rs",
  "crates/rustok-groups/src/applications_cas.rs",
  "crates/rustok-groups/src/graphql_policy_history.rs",
  "crates/rustok-groups/src/graphql_application_cas.rs",
  "crates/rustok-groups/src/lib.rs",
  "crates/rustok-groups/rustok-module.toml",
  "crates/rustok-groups/admin/src/application_core.rs",
  "crates/rustok-groups/admin/src/application_model.rs",
  "crates/rustok-groups/admin/src/transport.rs",
  "crates/rustok-groups/admin/src/transport/native_policy_history_adapter.rs",
  "crates/rustok-groups/admin/src/transport/graphql_policy_history_adapter.rs",
  "crates/rustok-groups/admin/src/transport/native_policy_locale_adapter.rs",
  "crates/rustok-groups/admin/src/transport/graphql_policy_locale_adapter.rs",
  "crates/rustok-groups/admin/src/ui/policy_editor.rs",
  "crates/rustok-groups/admin/src/ui/root.rs",
  "crates/rustok-groups/admin/locales/en.json",
  "crates/rustok-groups/admin/locales/ru.json",
  "crates/rustok-groups/contracts/groups-fba-registry.json",
  "crates/rustok-groups/docs/implementation-plan.md",
];
for (const relative of required) requireFile(relative);

requireMarkers(
  "crates/rustok-groups/src/migrations/m20260722_000007_create_group_membership_policy_revisions.rs",
  [
    "group_membership_policy_revisions",
    "PRIMARY KEY (tenant_id, policy_id, revision, locale)",
    "groups_capture_membership_policy_revision",
    "groups_membership_policy_revision_capture_insert",
    "groups_membership_policy_revision_capture_update",
    "group membership policy revisions are append-only",
    "policy revision must advance before changing localized policy",
    "INSERT OR IGNORE INTO group_membership_policy_revisions",
    "ON CONFLICT DO NOTHING",
    "DbBackend::Postgres",
    "DbBackend::Sqlite",
  ],
);
requireMarkers("crates/rustok-groups/src/migrations/mod.rs", [
  "mod m20260722_000007_create_group_membership_policy_revisions;",
  "Box::new(m20260722_000007_create_group_membership_policy_revisions::Migration)",
]);
requireMarkers("crates/rustok-groups/src/application_entities.rs", [
  'table_name = "group_membership_policy_revisions"',
  "pub revision: i64",
  "pub locale: String",
  "pub created_by_user_id: Uuid",
]);
requireMarkers("crates/rustok-groups/src/policy_history.rs", [
  "GroupApplicationPolicyHistoryReadPort",
  "GroupApplicationPolicyHistoryService",
  "list_group_application_policy_revisions",
  "GroupApplicationReadPort::list_group_membership_applications",
  "PortCallPolicy::read()",
  "order_by_desc(membership_policy_revision::Column::Revision)",
]);
requireMarkers("crates/rustok-groups/src/applications_cas.rs", [
  "GroupApplicationCasCommandPort",
  "GROUP_APPLICATION_POLICY_CHANGED_CODE",
  "ensure_policy_update_precondition",
]);
requireMarkers("crates/rustok-groups/src/graphql_policy_history.rs", [
  "MergedObject",
  "GroupsQueryRoot",
  "group_application_policy_revisions",
  "GroupApplicationPolicyHistoryReadPort",
  "GroupApplicationPolicyRevisionConnectionGql",
]);
requireMarkers("crates/rustok-groups/src/graphql_application_cas.rs", [
  "GroupsBaseQueryRoot",
  "GroupsPreApplicationMutationRoot",
  "GroupsApplicationCasMutation",
]);
forbidMarkers("crates/rustok-groups/src/graphql_application_cas.rs", [
  "GroupsMutationRoot as GroupsBaseMutationRoot",
]);
requireMarkers("crates/rustok-groups/rustok-module.toml", [
  'query = "graphql_application_cas::GroupsQueryRoot"',
  'mutation = "graphql_application_cas::GroupsMutationRoot"',
]);
requireMarkers("crates/rustok-groups/src/lib.rs", [
  "pub mod graphql_policy_history;",
  "pub mod graphql_application_cas;",
  "pub mod policy_history;",
  "pub use policy_history::*;",
  "assert_eq!(module.migrations().len(), 7)",
]);

requireMarkers("crates/rustok-groups/admin/src/application_core.rs", [
  "prepare_group_application_policy_query",
  "normalize_locale_tag(locale)",
  "InvalidExpectedPolicy",
]);
requireMarkers("crates/rustok-groups/admin/src/application_model.rs", [
  "pub struct GroupsAdminApplicationPolicyQuery",
  "pub struct GroupsAdminApplicationPolicyPrecondition",
  "pub locale: String",
]);
requireMarkers("crates/rustok-groups/admin/src/transport.rs", [
  "graphql_policy_history_adapter",
  "native_policy_history_adapter",
  "graphql_policy_locale_adapter",
  "native_policy_locale_adapter",
  "load_group_admin_application_policy_revisions",
  '"groups.admin.applications.policy.history"',
  '"groups.admin.applications.policy.upsert_if_current"',
  'GROUPS_ADMIN_TRANSPORT_FALLBACK_POLICY: &str = "never falls back"',
]);
requireMarkers(
  "crates/rustok-groups/admin/src/transport/native_policy_history_adapter.rs",
  [
    "groups/admin/applications/policy-revisions",
    "GroupApplicationPolicyHistoryReadPort",
    "GroupApplicationPolicyHistoryService",
    "with_deadline",
  ],
);
requireMarkers(
  "crates/rustok-groups/admin/src/transport/graphql_policy_history_adapter.rs",
  [
    "GroupsAdminApplicationPolicyHistory",
    "groupApplicationPolicyRevisions",
    "POLICY_HISTORY_QUERY",
  ],
);
requireMarkers(
  "crates/rustok-groups/admin/src/transport/native_policy_locale_adapter.rs",
  [
    "groups/admin/applications/policy-locale",
    "query.locale",
    "GroupApplicationReadPort",
    "GroupApplicationCasCommandPort",
  ],
);
requireMarkers(
  "crates/rustok-groups/admin/src/transport/graphql_policy_locale_adapter.rs",
  [
    "GroupsAdminApplicationPolicyLocale",
    "Some(query.locale.clone())",
    "Some(command.locale.clone())",
    "upsertGroupApplicationPolicyIfCurrent",
    "execute_graphql",
  ],
);
requireMarkers("crates/rustok-groups/admin/src/ui/policy_editor.rs", [
  "GroupsPolicyEditorAdmin",
  "prepare_group_application_policy_query",
  "prepare_upsert_group_application_policy",
  "load_group_admin_application_policy_revisions",
  "move_item",
  "loaded_policy",
  "GroupsAdminApplicationPolicyPrecondition::from",
  "GROUP_APPLICATION_POLICY_CHANGED_CODE",
  "copy.stale",
  "readonly",
  "unwrap_or_default",
  "GroupsAdminApplicationQuestion",
  "GroupsAdminApplicationRule",
]);
forbidMarkers("crates/rustok-groups/admin/src/ui/policy_editor.rs", [
  "graphql_policy_history_adapter",
  "native_policy_history_adapter",
  "graphql_policy_locale_adapter",
  "native_policy_locale_adapter",
  "membership_policy_revision::Entity",
  'unwrap_or_else(|| "en"',
  "preflight_context",
  "current.revision != expected",
]);
requireMarkers("crates/rustok-groups/admin/src/ui/root.rs", [
  "GroupsPolicyEditorAdmin",
]);

const localeKeys = [
  "groups.admin.policyEditor.title",
  "groups.admin.policyEditor.body",
  "groups.admin.policyEditor.load",
  "groups.admin.policyEditor.save",
  "groups.admin.policyEditor.addQuestion",
  "groups.admin.policyEditor.addRule",
  "groups.admin.policyEditor.history",
  "groups.admin.policyEditor.stale",
  "groups.admin.policyEditor.invalid",
];
for (const relative of [
  "crates/rustok-groups/admin/locales/en.json",
  "crates/rustok-groups/admin/locales/ru.json",
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
      failures.push(`${relative}: missing policy editor key ${key}`);
    }
  }
}

if (requireFile("crates/rustok-groups/contracts/groups-fba-registry.json")) {
  let registry;
  try {
    registry = JSON.parse(read("crates/rustok-groups/contracts/groups-fba-registry.json"));
  } catch (error) {
    failures.push(`Groups FBA registry is invalid JSON: ${error.message}`);
  }
  if (registry) {
    const historyPort = registry?.provider?.ports?.find(
      (port) => port?.name === "GroupApplicationPolicyHistoryReadPort",
    );
    if (
      !historyPort?.operations?.includes("list_group_application_policy_revisions") ||
      historyPort?.authorization !==
        "active_owner_admin_moderator_or_platform_manage"
    ) {
      failures.push("Groups registry is missing manager-only policy history port");
    }
    const applications = registry?.membership_applications;
    if (applications?.policy_revision_history !== "implemented_source") {
      failures.push("Groups policy revision history must remain source-only before execution");
    }
    if (applications?.admin_policy_stale_preflight !== "replaced_by_owner_cas") {
      failures.push("Groups stale preflight must be replaced by owner CAS");
    }
    if (applications?.atomic_expected_revision_guard !== "implemented_source") {
      failures.push("Groups atomic expected-revision guard must be source-complete");
    }
    if (applications?.final_graphql_legacy_application_mutations !== "not_exposed") {
      failures.push("Groups final GraphQL root must not expose legacy application mutations");
    }
    if (
      registry?.evidence?.membership_policy_revision_static_boundary !==
      "scripts/verify/verify-groups-membership-policy-revisions.mjs"
    ) {
      failures.push("Groups policy revision static evidence path is not registered");
    }
    if (registry?.evidence?.membership_application_policy_revision !== null) {
      failures.push("Groups policy revision runtime evidence must remain null before execution");
    }
    if (registry?.evidence?.membership_application_policy_cas !== null) {
      failures.push("Groups policy CAS runtime evidence must remain null before execution");
    }
  }
}

requireMarkers("crates/rustok-groups/docs/implementation-plan.md", [
  "group_membership_policy_revisions",
  "visual policy editor",
  "GroupApplicationCasCommandPort",
  "groups.application_policy_changed",
  "legacy unconditional",
  "verify-groups-membership-policy-revisions.mjs",
]);

if (failures.length > 0) {
  console.error("Groups membership policy revision verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Groups membership policy revision, exact-locale CAS, GraphQL no-bypass, editor, and append-only boundary checks passed.");
