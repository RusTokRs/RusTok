import fs from "node:fs";

const testPath = "apps/server/tests/groups_localization_graphql_postgres_parity.rs";
const docsPath = "crates/modules/rustok-groups/docs/localization-graphql-postgres-parity-contract.md";
const registryPath = "crates/modules/rustok-groups/contracts/groups-fba-registry.json";
const test = fs.readFileSync(testPath, "utf8");
const docs = fs.readFileSync(docsPath, "utf8");
const registry = JSON.parse(fs.readFileSync(registryPath, "utf8"));

function requireText(source, marker, message) {
  if (!source.includes(marker)) throw new Error(message);
}

for (const marker of [
  '#![cfg(feature = "mod-groups")]',
  '#[ignore = "requires RUSTOK_GROUPS_TEST_POSTGRES_URL"]',
  "RUSTOK_GROUPS_TEST_POSTGRES_URL",
  "options=-csearch_path%3D",
  "CREATE SCHEMA",
  "DROP SCHEMA",
  "rustok_groups::migrations::migrations()",
  "GroupsQueryRoot::default()",
  "GroupsMutationRoot::default()",
  "HostRuntimeContext::new(db)",
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
  "Some(native_delete.group_version)",
  "graphql_final.len(), 1",
]) {
  requireText(test, marker, `Groups localization GraphQL PostgreSQL parity source is missing ${marker}`);
}

// Require the equality assertion, not a one-line rustfmt rendering of its operands.
const compactTest = test.replace(/\s+/g, "");
for (const locale of ["en", "fr"]) {
  const operands = `graphql_${locale}["groupVersion"].as_u64(),Some(native_${locale}.group_version)`;
  if (!compactTest.includes(`assert_eq!(${operands});`)
      && !compactTest.includes(`assert_eq!(${operands},);`)) {
    throw new Error(`Groups localization parity must assert ${locale} aggregate version equality`);
  }
}

for (const forbidden of [
  "SET search_path",
  "translation::ActiveModel",
  "GroupsLocalizationMutation::default()",
  "rustok_moderation::",
]) {
  if (test.includes(forbidden)) {
    throw new Error(`Groups localization GraphQL PostgreSQL parity source contains shortcut ${forbidden}`);
  }
}

for (const marker of [
  "executable source added / maintainer execution pending",
  "PostgreSQL isolation",
  "Equivalent owner fixtures",
  "Parity contract",
  "groups.conflict",
  "BAD_USER_INPUT",
  "Final-root composition",
  "localization_transport_parity",
  "empty effective permission list",
]) {
  requireText(docs, marker, `Groups localization GraphQL PostgreSQL parity handoff is missing ${marker}`);
}

if (registry?.evidence?.localization_transport_parity !== null) {
  throw new Error("unexecuted localization transport parity evidence must remain null");
}

console.log("Groups localization native/GraphQL PostgreSQL parity source guard passed");
