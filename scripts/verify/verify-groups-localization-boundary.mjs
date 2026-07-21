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
  "lock_exclusive()",
  "PortCallPolicy::read()",
  "PortCallPolicy::write()",
]);
forbidMarkers("crates/rustok-groups/src/localization.rs", [
  "PLATFORM_FALLBACK_LOCALE",
  "build_locale_candidates",
  "rows.first()",
]);

requireMarkers("crates/rustok-groups/src/graphql_localization.rs", [
  "group_translations",
  "upsert_group_translation",
  "delete_group_translation",
  "GroupLocalizationReadPort",
  "GroupLocalizationCommandPort",
]);
requireMarkers("crates/rustok-groups/rustok-module.toml", [
  'query = "graphql_applications::GroupsQueryRoot"',
  'mutation = "graphql_applications::GroupsMutationRoot"',
]);
requireMarkers("crates/rustok-groups/src/graphql_applications.rs", [
  "GroupsBaseQueryRoot",
  "GroupsBaseMutationRoot",
  "pub struct GroupsQueryRoot",
  "pub struct GroupsMutationRoot",
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
  const readPort = registry?.provider?.ports?.find((port) => port?.name === "GroupLocalizationReadPort");
  const commandPort = registry?.provider?.ports?.find((port) => port?.name === "GroupLocalizationCommandPort");
  if (!readPort?.operations?.includes("list_group_translations")) {
    failures.push("Groups registry is missing localization read operation");
  }
  if (!commandPort?.operations?.includes("upsert_group_translation") || !commandPort?.operations?.includes("delete_group_translation")) {
    failures.push("Groups registry is missing localization command operations");
  }
  if (commandPort?.exact_locale_only !== true || commandPort?.last_translation_delete !== "deny") {
    failures.push("Groups localization command invariants are not locked");
  }
  if (registry?.localization?.module_local_fallback !== false) {
    failures.push("Groups localization must reject module-local fallback");
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

console.log("Groups exact-locale localization, FBA, FFA, last-row, and no-fallback boundary checks passed.");
