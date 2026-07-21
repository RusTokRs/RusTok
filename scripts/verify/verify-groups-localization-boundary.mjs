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
  "crates/rustok-groups/src/localization.rs",
  "crates/rustok-groups/src/graphql_localization.rs",
  "crates/rustok-groups/src/graphql_governance.rs",
  "crates/rustok-groups/src/graphql_invitations.rs",
  "crates/rustok-groups/src/ports.rs",
  "crates/rustok-groups/rustok-module.toml",
  "crates/rustok-groups/contracts/groups-fba-registry.json",
  "crates/rustok-groups/admin/src/core.rs",
  "crates/rustok-groups/admin/src/model.rs",
  "crates/rustok-groups/admin/src/transport.rs",
  "crates/rustok-groups/admin/src/transport/graphql_adapter.rs",
  "crates/rustok-groups/admin/src/transport/native_localization_adapter.rs",
  "crates/rustok-groups/admin/src/ui/localization.rs",
  "crates/rustok-groups/admin/src/ui/root.rs",
  "crates/rustok-groups/admin/locales/en.json",
  "crates/rustok-groups/admin/locales/ru.json",
  "crates/rustok-groups/docs/README.md",
  "crates/rustok-groups/docs/implementation-plan.md",
];
for (const relative of required) requireFile(relative);

requireMarkers("crates/rustok-groups/src/localization.rs", [
  "GroupLocalizationService",
  "GroupLocalizationReadPort",
  "GroupLocalizationCommandPort",
  "normalize_locale_tag",
  "translation::Column::Locale.eq(locale.clone())",
  "the last group translation cannot be deleted",
  "version.saturating_add(1)",
  "lock_exclusive()",
  "DbBackend::Postgres | DbBackend::MySql",
  "PortCallPolicy::read()",
  "PortCallPolicy::write()",
]);
forbidMarkers("crates/rustok-groups/src/localization.rs", [
  "PLATFORM_FALLBACK_LOCALE",
  "build_locale_candidates",
  "rows.first()",
]);

requireMarkers("crates/rustok-groups/src/ports.rs", [
  "pub trait GroupLocalizationReadPort",
  "list_group_translations",
  "pub trait GroupLocalizationCommandPort",
  "upsert_group_translation",
  "delete_group_translation",
]);

requireMarkers("crates/rustok-groups/src/graphql_localization.rs", [
  "GroupsQueryRoot",
  "GroupsLocalizationQuery",
  "GroupsLocalizationMutation",
  "group_translations",
  "upsert_group_translation",
  "delete_group_translation",
  "GroupLocalizationReadPort",
  "GroupLocalizationCommandPort",
]);
requireMarkers("crates/rustok-groups/src/graphql_governance.rs", [
  "GroupsLocalizationMutation",
  "GroupsMutationRoot",
]);
requireMarkers("crates/rustok-groups/src/graphql_invitations.rs", [
  "GroupsBaseQueryRoot",
  "GroupsBaseMutationRoot",
  "pub struct GroupsQueryRoot",
  "pub struct GroupsMutationRoot",
]);
requireMarkers("crates/rustok-groups/rustok-module.toml", [
  "query = \"graphql_invitations::GroupsQueryRoot\"",
  "mutation = \"graphql_invitations::GroupsMutationRoot\"",
]);

requireMarkers("crates/rustok-groups/admin/src/core.rs", [
  "prepare_group_translation_query",
  "prepare_upsert_group_translation",
  "prepare_delete_group_translation",
  "normalize_locale_tag",
  "title.chars().count() > 240",
  "value.chars().count() > 500",
  "groups-admin-upsert-translation",
  "groups-admin-delete-translation",
]);
forbidMarkers("crates/rustok-groups/admin/src/core.rs", [
  "use leptos",
  "leptos::",
]);

requireMarkers("crates/rustok-groups/admin/src/transport.rs", [
  "native_localization_adapter",
  "load_group_admin_translations",
  "upsert_group_admin_translation",
  "delete_group_admin_translation",
  "execute_selected_transport",
  "never falls back",
]);
requireMarkers("crates/rustok-groups/admin/src/transport/native_localization_adapter.rs", [
  "groups/admin/localization/translations",
  "groups/admin/localization/upsert-translation",
  "groups/admin/localization/delete-translation",
  "GroupLocalizationReadPort",
  "GroupLocalizationCommandPort",
  "with_idempotency_key",
]);
requireMarkers("crates/rustok-groups/admin/src/transport/graphql_adapter.rs", [
  "GroupsAdminTranslations",
  "GroupsAdminUpsertTranslation",
  "GroupsAdminDeleteTranslation",
  "groupTranslations",
  "upsertGroupTranslation",
  "deleteGroupTranslation",
]);

requireMarkers("crates/rustok-groups/admin/src/ui/localization.rs", [
  "prepare_group_translation_query",
  "prepare_upsert_group_translation",
  "prepare_delete_group_translation",
  "load_group_admin_translations",
  "upsert_group_admin_translation",
  "delete_group_admin_translation",
  "GroupsAdminLocalizationInputError",
  "groups.admin.localization.lastTranslationWarning",
]);
forbidMarkers("crates/rustok-groups/admin/src/ui/localization.rs", [
  "graphql_adapter",
  "native_localization_adapter",
  "native_server_adapter",
]);
requireMarkers("crates/rustok-groups/admin/src/ui/root.rs", [
  "GroupsAdminCore",
  "GroupsLocalizationAdmin",
]);

const localizationLocaleKeys = [
  "groups.admin.localization.title",
  "groups.admin.localization.body",
  "groups.admin.localization.groupId",
  "groups.admin.localization.locale",
  "groups.admin.localization.translationTitle",
  "groups.admin.localization.summary",
  "groups.admin.localization.translationBody",
  "groups.admin.localization.load",
  "groups.admin.localization.save",
  "groups.admin.localization.delete",
  "groups.admin.localization.empty",
  "groups.admin.localization.busy",
  "groups.admin.localization.error",
  "groups.admin.localization.loaded",
  "groups.admin.localization.saved",
  "groups.admin.localization.deleted",
  "groups.admin.localization.version",
  "groups.admin.localization.lastTranslationWarning",
  "groups.admin.localization.invalidGroupId",
  "groups.admin.localization.invalidLocale",
  "groups.admin.localization.missingTitle",
  "groups.admin.localization.titleTooLong",
  "groups.admin.localization.summaryTooLong",
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
  for (const key of localizationLocaleKeys) {
    if (typeof messages[key] !== "string" || messages[key].trim() === "") {
      failures.push(`${relative}: missing localization key ${key}`);
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
    const readPort = registry?.provider?.ports?.find(
      (port) => port?.name === "GroupLocalizationReadPort",
    );
    const commandPort = registry?.provider?.ports?.find(
      (port) => port?.name === "GroupLocalizationCommandPort",
    );
    if (!readPort?.operations?.includes("list_group_translations")) {
      failures.push("Groups FBA registry is missing the localization read operation");
    }
    if (
      !commandPort?.operations?.includes("upsert_group_translation") ||
      !commandPort?.operations?.includes("delete_group_translation")
    ) {
      failures.push("Groups FBA registry is missing localization command operations");
    }
    if (commandPort?.exact_locale_only !== true) {
      failures.push("Groups localization commands must declare exact-locale semantics");
    }
    if (commandPort?.last_translation_delete !== "deny") {
      failures.push("Groups localization commands must deny last-translation deletion");
    }
    if (commandPort?.serialized_group_row !== "exclusive_lock_where_supported") {
      failures.push("Groups localization commands must declare base-row serialization");
    }
    if (registry?.localization?.module_local_fallback !== false) {
      failures.push("Groups localization registry must reject module-local fallback");
    }
    if (registry?.localization?.mutation_serialization !== "base_group_row") {
      failures.push("Groups localization registry must bind mutation serialization to the group row");
    }
    if (
      registry?.evidence?.localization_static_boundary !==
      "scripts/verify/verify-groups-localization-boundary.mjs"
    ) {
      failures.push("Groups localization registry must reference the focused static guard");
    }
    const profile = registry?.transport_profiles?.find(
      (entry) => entry?.name === "embedded_localization_native",
    );
    for (const surface of ["rust_port", "graphql", "leptos_server_function"]) {
      if (!profile?.surfaces?.includes(surface)) {
        failures.push(`Groups localization profile is missing surface: ${surface}`);
      }
    }
    if (profile?.implicit_fallback !== false) {
      failures.push("Groups localization profile must reject implicit transport fallback");
    }
  }
}

requireMarkers("crates/rustok-groups/docs/README.md", [
  "GroupLocalizationReadPort",
  "GroupLocalizationCommandPort",
  "delete rejects removal of the final translation row",
  "translation mutation and group-version increment commit in one transaction",
  "do not yet persist",
  "replay receipts",
]);
requireMarkers("crates/rustok-groups/docs/implementation-plan.md", [
  "localization idempotent receipts/replay",
  "last-translation delete rejection",
  "localization idempotency replay",
]);

if (failures.length > 0) {
  console.error("Groups localization boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Groups exact-locale localization boundary checks passed.");
