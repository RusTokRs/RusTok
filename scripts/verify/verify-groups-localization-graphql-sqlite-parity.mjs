import fs from "node:fs";

const testPath = "apps/server/tests/groups_localization_graphql_sqlite_parity.rs";
const docsPath = "crates/rustok-groups/docs/localization-graphql-sqlite-parity-contract.md";
const registryPath = "crates/rustok-groups/contracts/groups-fba-registry.json";
const test = fs.readFileSync(testPath, "utf8");
const docs = fs.readFileSync(docsPath, "utf8");
const registry = JSON.parse(fs.readFileSync(registryPath, "utf8"));

function requireText(source, marker, message) {
  if (!source.includes(marker)) throw new Error(message);
}

for (const marker of [
  '#![cfg(feature = "mod-groups")]',
  "tempfile::tempdir()",
  "mode=rwc",
  "rustok_groups::migrations::migrations()",
  "GroupsQueryRoot::default()",
  "GroupsMutationRoot::default()",
  "HostRuntimeContext::new(db)",
  "AuthContext",
  "TenantContext",
  "permissions: Vec::new()",
  "GroupLocalizationService::new",
  "GroupLocalizationCommandPort::upsert_group_translation",
  "GroupLocalizationReadPort::list_group_translations",
  "GroupLocalizationCommandPort::delete_group_translation",
  "upsertGroupTranslation",
  "groupTranslations",
  "deleteGroupTranslation",
  'assert_eq!(native_last_error.code, "groups.conflict")',
  'Some("BAD_USER_INPUT".to_string())',
  "graphql_last_error.message, native_last_error.message",
  "graphql_en[\"groupVersion\"].as_u64(), Some(native_en.group_version)",
  "graphql_fr[\"groupVersion\"].as_u64(), Some(native_fr.group_version)",
  "Some(native_delete.group_version)",
  "graphql_final.len(), 1",
]) {
  requireText(test, marker, `Groups localization GraphQL SQLite parity source is missing ${marker}`);
}

for (const forbidden of [
  "translation::ActiveModel",
  "GroupsLocalizationMutation::default()",
  "rustok_moderation::",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups localization GraphQL SQLite parity source contains shortcut ${forbidden}`);
  }
}

for (const marker of [
  "executable source added / maintainer execution pending",
  "Equivalent owner fixtures",
  "Parity contract",
  "groups.conflict",
  "BAD_USER_INPUT",
  "Final-root composition",
  "localization_transport_parity",
  "empty permission list",
]) {
  requireText(docs, marker, `Groups localization GraphQL SQLite parity handoff is missing ${marker}`);
}

if (registry?.evidence?.localization_transport_parity !== null) {
  throw new Error("unexecuted localization transport parity evidence must remain null");
}

console.log("Groups localization native/GraphQL SQLite parity source guard passed");
