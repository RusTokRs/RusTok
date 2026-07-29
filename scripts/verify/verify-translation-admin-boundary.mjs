#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

const files = {
  lib: "crates/rustok-translation/admin/src/lib.rs",
  core: "crates/rustok-translation/admin/src/core.rs",
  model: "crates/rustok-translation/admin/src/model.rs",
  transport: "crates/rustok-translation/admin/src/transport/mod.rs",
  native:
    "crates/rustok-translation/admin/src/transport/native_server_adapter.rs",
  graphql: "crates/rustok-translation/admin/src/transport/graphql_adapter.rs",
  leptos: "crates/rustok-translation/admin/src/ui/leptos.rs",
  manifest: "crates/rustok-translation/rustok-module.toml",
  adminCargo: "apps/admin/Cargo.toml",
  nextPackage: "apps/next-admin/packages/translation/src/index.tsx",
  nextApi: "apps/next-admin/packages/translation/src/api.ts",
  nextTypes: "apps/next-admin/packages/translation/src/types.ts",
  nextRegistry: "apps/next-admin/src/modules/index.ts",
  nextWrapper: "apps/next-admin/src/modules/translation-admin-client.tsx",
  nextRoute: "apps/next-admin/src/app/dashboard/translation/page.tsx",
  leptosEn: "crates/rustok-translation/admin/locales/en.json",
  leptosRu: "crates/rustok-translation/admin/locales/ru.json",
  nextEn: "apps/next-admin/messages/en.json",
  nextRu: "apps/next-admin/messages/ru.json",
};

function absolute(relativePath) {
  return path.join(repoRoot, relativePath);
}

function fail(message) {
  failures.push(message);
}

function read(relativePath) {
  if (!existsSync(absolute(relativePath))) {
    fail(`${relativePath}: expected file`);
    return "";
  }
  return readFileSync(absolute(relativePath), "utf8");
}

function contains(text, marker, message) {
  const found =
    typeof marker === "string" ? text.includes(marker) : marker.test(text);
  if (!found) fail(message);
}

function excludes(text, marker, message) {
  const found =
    typeof marker === "string" ? text.includes(marker) : marker.test(text);
  if (found) fail(message);
}

function flatten(value, prefix = "") {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return Object.entries(value).flatMap(([key, child]) =>
      flatten(child, prefix ? `${prefix}.${key}` : key),
    );
  }
  return [prefix];
}

const source = Object.fromEntries(
  Object.entries(files).map(([name, relativePath]) => [
    name,
    read(relativePath),
  ]),
);

for (const marker of [
  "leptos::",
  "leptos_",
  "#[component]",
  "#[server]",
  "RwSignal",
  "LocalResource",
  "web_sys::",
]) {
  excludes(
    source.core,
    marker,
    `${files.core}: framework-neutral core contains ${marker}`,
  );
}

contains(
  source.lib,
  "pub use ui::leptos::TranslationAdmin;",
  `${files.lib}: crate root must export TranslationAdmin`,
);
contains(
  source.leptos,
  "use crate::transport;",
  `${files.leptos}: Leptos UI must call the transport facade`,
);
excludes(
  source.leptos,
  "graphql_adapter::",
  `${files.leptos}: Leptos UI must not select the raw GraphQL adapter`,
);
excludes(
  source.leptos,
  "native_server_adapter::",
  `${files.leptos}: Leptos UI must not select the raw native adapter`,
);
contains(
  source.leptos,
  "UiRouteContext",
  `${files.leptos}: Leptos UI must consume host route locale context`,
);
contains(
  source.leptos,
  "core::TAB_QUERY_KEY",
  `${files.leptos}: Leptos UI must use the core-owned tab query key`,
);

contains(
  source.transport,
  "execute_selected_transport",
  `${files.transport}: transport facade must use selected-path execution`,
);
contains(
  source.transport,
  "without protocol fallback",
  `${files.transport}: transport facade must document no protocol fallback`,
);
contains(
  source.native,
  "#[server",
  `${files.native}: native adapter must expose a server function`,
);
contains(
  source.native,
  "HostRuntimeContext",
  `${files.native}: native adapter must use host runtime context`,
);
contains(
  source.graphql,
  "rustok_graphql",
  `${files.graphql}: GraphQL adapter must use rustok-graphql`,
);

const rustOperations = [
  "ReadPolicy",
  "ReadMachineOperationStatus",
  "ListTargets",
  "ListGlossaries",
  "ReadGlossary",
  "ListMemoryEntries",
  "ReadMemoryEntry",
  "LookupMemory",
  "ReadJobProgress",
  "ExportJob",
  "ReadProviderProgress",
  "ReadRequiredProviderProgress",
  "ReplacePolicy",
  "CreateGlossary",
  "UpdateGlossary",
  "ReplaceGlossaryTerms",
  "SetGlossaryActive",
  "SetMemoryRetention",
  "TombstoneMemoryEntry",
  "PurgeMemoryEntry",
  "CreateJob",
  "AddItem",
  "SaveProposal",
  "ImportItem",
  "GenerateMachineProposal",
  "CancelMachineOperation",
  "RecoverMachineOperation",
  "SubmitProposal",
  "ApproveProposal",
  "ApplyProposal",
  "AssignItem",
  "UnassignItem",
  "CancelJob",
  "RetryItem",
  "RecoverApply",
  "RebuildJobProgress",
  "SyncProviderInventory",
  "RebuildProviderInventory",
];
for (const operation of rustOperations) {
  contains(
    source.model,
    operation,
    `${files.model}: missing typed operation ${operation}`,
  );
}

const sharedWorkbenchOperations = [
  "read_policy",
  "read_machine_operation_status",
  "list_targets",
  "list_glossaries",
  "read_glossary",
  "list_memory_entries",
  "read_memory_entry",
  "lookup_memory",
  "replace_policy",
  "create_glossary",
  "update_glossary",
  "replace_glossary_terms",
  "set_glossary_active",
  "set_memory_retention",
  "tombstone_memory_entry",
  "purge_memory_entry",
  "create_job",
  "export_job",
  "import_item",
  "read_job_progress",
  "rebuild_job_progress",
  "sync_inventory",
  "rebuild_inventory",
  "read_provider_progress",
  "read_required_progress",
  "add_item",
  "save_proposal",
  "generate_machine_proposal",
  "cancel_machine_operation",
  "recover_machine_operation",
  "submit_proposal",
  "approve_proposal",
  "apply_proposal",
];
for (const operation of sharedWorkbenchOperations) {
  contains(
    source.nextTypes,
    `'${operation}'`,
    `${files.nextTypes}: Next workbench is missing ${operation}`,
  );
  contains(
    source.nextApi,
    `case '${operation}'`,
    `${files.nextApi}: Next GraphQL adapter is missing ${operation}`,
  );
}

for (const tab of [
  "overview",
  "jobs",
  "glossaries",
  "memory",
  "inventory",
  "workflow",
]) {
  contains(
    source.core,
    `"${tab}"`,
    `${files.core}: Rust tab contract is missing ${tab}`,
  );
  contains(
    source.nextPackage,
    `'${tab}'`,
    `${files.nextPackage}: Next tab contract is missing ${tab}`,
  );
}

for (const marker of [
  'ui_classification = "admin_only"',
  'recommended_admin_surfaces = ["leptos-admin", "next-admin"]',
  "[provides.admin_ui]",
  'leptos_crate = "rustok-translation-admin"',
  'route_segment = "translation"',
  'next_package = "@rustok/translation-admin"',
  'leptos_locales_path = "admin/locales"',
]) {
  contains(
    source.manifest,
    marker,
    `${files.manifest}: missing manifest marker ${marker}`,
  );
}

for (const profile of ["csr", "hydrate", "ssr"]) {
  contains(
    source.adminCargo,
    `"rustok-translation-admin/${profile}"`,
    `${files.adminCargo}: missing Translation ${profile} feature wiring`,
  );
}
contains(
  source.adminCargo,
  'rustok-translation-admin = { path = "../../crates/rustok-translation/admin"',
  `${files.adminCargo}: missing Translation admin dependency`,
);
contains(
  source.nextRegistry,
  "packages/translation/src",
  `${files.nextRegistry}: Next module registry must import Translation package`,
);
contains(
  source.nextWrapper,
  "graphql={graphqlRequest}",
  `${files.nextWrapper}: host wrapper must inject the shared GraphQL executor`,
);
contains(
  source.nextRoute,
  "<TranslationAdminClient",
  `${files.nextRoute}: Next route must compose the package-owned workbench`,
);
for (const marker of [
  "executeTranslationOperation",
  "TranslationOperation",
  "replaceTranslationPolicy",
  "createTranslationJob",
]) {
  excludes(
    source.nextRoute,
    marker,
    `${files.nextRoute}: host route must not own Translation operation ${marker}`,
  );
}
contains(
  source.nextPackage,
  "useTranslations('translation')",
  `${files.nextPackage}: Next package must consume host next-intl messages`,
);
contains(
  source.nextPackage,
  "searchParams.get('tab')",
  `${files.nextPackage}: Next package must keep tab selection in the URL`,
);
contains(
  source.nextPackage,
  "searchParams.get('glossary_id')",
  `${files.nextPackage}: Next package must keep glossary selection in the URL`,
);
contains(
  source.leptos,
  "core::GLOSSARY_ID_QUERY_KEY",
  `${files.leptos}: Leptos package must keep glossary selection in the URL`,
);
contains(
  source.nextPackage,
  "searchParams.get('memory_entry_id')",
  `${files.nextPackage}: Next package must keep memory selection in the URL`,
);
contains(
  source.leptos,
  "core::MEMORY_ENTRY_ID_QUERY_KEY",
  `${files.leptos}: Leptos package must keep memory selection in the URL`,
);
excludes(
  source.nextApi,
  /\bfetch\s*\(/,
  `${files.nextApi}: Next adapter must use the injected host GraphQL executor`,
);
for (const marker of [
  "document.cookie",
  "localStorage",
  "sessionStorage",
  "rustok-admin-locale",
]) {
  excludes(
    source.nextPackage,
    marker,
    `${files.nextPackage}: package-local locale fallback is forbidden (${marker})`,
  );
  excludes(
    source.leptos,
    marker,
    `${files.leptos}: package-local locale fallback is forbidden (${marker})`,
  );
}

try {
  const leptosEnKeys = flatten(JSON.parse(source.leptosEn)).sort();
  const leptosRuKeys = flatten(JSON.parse(source.leptosRu)).sort();
  const nextEn = JSON.parse(source.nextEn);
  const nextRu = JSON.parse(source.nextRu);
  const nextEnKeys = flatten(nextEn.translation, "translation").sort();
  const nextRuKeys = flatten(nextRu.translation, "translation").sort();
  const expected = JSON.stringify(leptosEnKeys);
  if (JSON.stringify(leptosRuKeys) !== expected) {
    fail("Translation Leptos locale bundles do not expose identical keys");
  }
  if (JSON.stringify(nextEnKeys) !== expected) {
    fail("Next English Translation keys differ from the Leptos contract");
  }
  if (JSON.stringify(nextRuKeys) !== expected) {
    fail("Next Russian Translation keys differ from the Leptos contract");
  }
} catch (error) {
  fail(`Translation locale bundle parsing failed: ${error.message}`);
}

const staleSupportPath = "crates/rustok-translation-admin-support";
if (existsSync(absolute(staleSupportPath))) {
  fail(`${staleSupportPath}: superseded support package must stay removed`);
}

if (failures.length > 0) {
  console.error("Translation admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Translation admin boundary verification passed");
