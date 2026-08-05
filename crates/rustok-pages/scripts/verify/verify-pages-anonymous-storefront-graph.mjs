#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "..", "..", "..", "..");
const failures = [];

const evidencePath =
  "crates/rustok-pages/contracts/evidence/pages-anonymous-storefront-graph-source.json";
const packetPath =
  "docs/modules/pages-page-builder-anonymous-storefront-graph-packet-2026-08-05.md";
const planPath = "docs/modules/pages-page-builder-parity-continuation-plan.md";
const localPlanPath = "crates/rustok-pages/docs/implementation-plan.md";

const read = (relativePath) =>
  readFileSync(path.join(repoRoot, relativePath), "utf8");
const need = (text, marker, label) => {
  if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (text, marker, label) => {
  if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};

const evidence = JSON.parse(read(evidencePath));
const packet = read(packetPath);
const plan = read(planPath);
const localPlan = read(localPlanPath);
const pagesManifestPath = "crates/rustok-pages/storefront/Cargo.toml";
const builderManifestPath = "crates/rustok-page-builder-storefront/Cargo.toml";
const hostManifestPath = "apps/storefront/Cargo.toml";
const pagesManifest = read(pagesManifestPath);
const builderManifest = read(builderManifestPath);
const hostManifest = read(hostManifestPath);

if (evidence.format !== "pages_anonymous_storefront_graph_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_anonymous_storefront_graph_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const key of [
  "pages_storefront_manifest_checked",
  "page_builder_storefront_manifest_checked",
  "host_storefront_manifest_checked",
  "pages_storefront_default_profile_defined",
  "pages_storefront_hydrate_profile_defined",
  "pages_storefront_ssr_profile_defined",
  "host_storefront_csr_profile_defined",
  "host_storefront_hydrate_profile_defined",
  "host_storefront_ssr_profile_defined",
  "feature_resolved_cargo_metadata_required",
  "dev_dependencies_excluded_from_reachability",
  "storefront_source_trees_scanned",
  "pages_admin_forbidden",
  "page_builder_admin_forbidden",
  "admin_host_forbidden",
  "fly_browser_forbidden",
  "fly_ui_forbidden",
  "fly_leptos_forbidden",
  "page_builder_storefront_required_for_pages_profiles",
  "pages_storefront_required_for_host_ssr",
  "host_client_profiles_keep_optional_pages_module_disabled"
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`source_contract.${key} must be true`);
  }
}
for (const key of [
  "actual_bundle_artifact_proof_complete",
  "production_pages_behavior_changed",
  "production_page_builder_behavior_changed",
  "production_storefront_behavior_changed",
  "dependencies_changed",
  "features_changed",
  "database_schema_changed",
  "public_route_changed",
  "ffa_promoted",
  "fba_promoted"
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`source_contract.${key} must be false`);
  }
}

const expectedProfiles = [
  ["pages-storefront-default", pagesManifestPath, [], null],
  ["pages-storefront-hydrate", pagesManifestPath, ["hydrate"], "wasm32-unknown-unknown"],
  ["pages-storefront-ssr", pagesManifestPath, ["ssr"], null],
  ["host-storefront-csr", hostManifestPath, ["csr"], "wasm32-unknown-unknown"],
  ["host-storefront-hydrate", hostManifestPath, ["hydrate"], "wasm32-unknown-unknown"],
  ["host-storefront-ssr", hostManifestPath, ["ssr"], null]
];
const actualProfiles = (evidence.profiles ?? []).map((profile) => [
  profile.id,
  profile.manifest,
  profile.features,
  profile.target
]);
if (JSON.stringify(actualProfiles) !== JSON.stringify(expectedProfiles)) {
  failures.push("evidence profile matrix does not match the retained six-profile contract");
}

const forbiddenPackages = new Set([
  "rustok-pages-admin",
  "rustok-page-builder-admin",
  "rustok-admin",
  "fly-browser",
  "fly-ui",
  "fly-leptos"
]);
if (
  JSON.stringify([...forbiddenPackages]) !==
  JSON.stringify(evidence.forbidden_packages)
) {
  failures.push("forbidden package set does not match evidence");
}

for (const marker of [
  'default = []',
  '"rustok-page-builder-storefront/hydrate"',
  '"rustok-page-builder-storefront/ssr"',
  'rustok-page-builder-storefront = { path = "../../rustok-page-builder-storefront" }'
]) {
  need(pagesManifest, marker, "Pages storefront manifest");
}
for (const forbidden of [
  "rustok-pages-admin",
  "rustok-page-builder-admin",
  "rustok-admin",
  "fly-browser",
  "fly-ui",
  "fly-leptos"
]) {
  forbid(pagesManifest, forbidden, "Pages storefront manifest");
}

for (const marker of [
  'default = []',
  'hydrate = ["leptos/hydrate"]',
  'ssr = ["leptos/ssr"]',
  'fly = { path = "../fly" }',
  'rustok-page-builder = { path = "../rustok-page-builder", default-features = false }'
]) {
  need(builderManifest, marker, "Page Builder storefront manifest");
}
for (const forbidden of forbiddenPackages) {
  forbid(builderManifest, forbidden, "Page Builder storefront manifest");
}

for (const marker of [
  'csr = ["leptos/csr", "leptos_i18n/csr"]',
  'hydrate = ["leptos/hydrate", "leptos_i18n/hydrate"]',
  '"rustok-pages-storefront/ssr"',
  'rustok-pages-storefront = { path = "../../crates/rustok-pages/storefront", default-features = false, optional = true }'
]) {
  need(hostManifest, marker, "host storefront manifest");
}
for (const forbidden of forbiddenPackages) {
  forbid(hostManifest, forbidden, "host storefront manifest");
}

function walkRustFiles(relativeDirectory) {
  const absoluteDirectory = path.join(repoRoot, relativeDirectory);
  if (!existsSync(absoluteDirectory)) {
    failures.push(`source tree missing: ${relativeDirectory}`);
    return [];
  }
  const result = [];
  const stack = [absoluteDirectory];
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of readdirSync(current)) {
      const absolute = path.join(current, entry);
      const stat = statSync(absolute);
      if (stat.isDirectory()) stack.push(absolute);
      else if (entry.endsWith(".rs")) result.push(absolute);
    }
  }
  return result;
}

const forbiddenSourceMarkers = [
  "rustok_pages_admin",
  "rustok_page_builder_admin",
  "fly_browser",
  "fly_ui",
  "fly_leptos",
  "PagesAdmin",
  "PagesFlyBuilder",
  "PageBuilderAdminHostContext",
  "PageBuilderAdmin",
  "ConsumerPropertiesPanel"
];
for (const sourceRoot of [
  "crates/rustok-pages/storefront/src",
  "crates/rustok-page-builder-storefront/src",
  "apps/storefront/src"
]) {
  for (const file of walkRustFiles(sourceRoot)) {
    const source = readFileSync(file, "utf8");
    const label = path.relative(repoRoot, file);
    for (const marker of forbiddenSourceMarkers) {
      forbid(source, marker, label);
    }
  }
}

function cargoMetadata(profile) {
  const args = [
    "metadata",
    "--format-version",
    "1",
    "--manifest-path",
    path.join(repoRoot, profile.manifest),
    "--no-default-features"
  ];
  if (profile.features.length > 0) {
    args.push("--features", profile.features.join(","));
  }
  if (profile.target) {
    args.push("--filter-platform", profile.target);
  }
  const result = spawnSync("cargo", args, {
    cwd: repoRoot,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    env: { ...process.env, CARGO_TERM_COLOR: "never" }
  });
  if (result.error) {
    failures.push(`${profile.id}: cargo metadata failed to start: ${result.error.message}`);
    return null;
  }
  if (result.status !== 0) {
    const diagnostic = `${result.stderr ?? ""}`.trim().slice(0, 4000);
    failures.push(`${profile.id}: cargo metadata exited ${result.status}: ${diagnostic}`);
    return null;
  }
  try {
    return JSON.parse(result.stdout);
  } catch (error) {
    failures.push(`${profile.id}: invalid cargo metadata JSON: ${error.message}`);
    return null;
  }
}

function reachableNonDevPackages(metadata, profile) {
  const manifest = path.resolve(repoRoot, profile.manifest);
  const rootPackage = metadata.packages.find(
    (candidate) => path.resolve(candidate.manifest_path) === manifest
  );
  if (!rootPackage) {
    failures.push(`${profile.id}: root package not found for ${profile.manifest}`);
    return new Set();
  }
  const nodeById = new Map((metadata.resolve?.nodes ?? []).map((node) => [node.id, node]));
  const packageById = new Map(metadata.packages.map((pkg) => [pkg.id, pkg]));
  const seen = new Set();
  const stack = [rootPackage.id];
  while (stack.length > 0) {
    const id = stack.pop();
    if (seen.has(id)) continue;
    seen.add(id);
    const node = nodeById.get(id);
    if (!node) continue;
    for (const dep of node.deps ?? []) {
      const kinds = dep.dep_kinds ?? [];
      const reachable =
        kinds.length === 0 || kinds.some((kind) => kind.kind !== "dev");
      if (reachable) stack.push(dep.pkg);
    }
  }
  return new Set(
    [...seen]
      .map((id) => packageById.get(id)?.name)
      .filter((name) => typeof name === "string")
  );
}

for (const raw of evidence.profiles) {
  const profile = {
    id: raw.id,
    manifest: raw.manifest,
    features: raw.features,
    target: raw.target
  };
  const metadata = cargoMetadata(profile);
  if (!metadata) continue;
  const packages = reachableNonDevPackages(metadata, profile);
  for (const forbiddenPackage of forbiddenPackages) {
    if (packages.has(forbiddenPackage)) {
      failures.push(`${profile.id}: reaches forbidden authoring package ${forbiddenPackage}`);
    }
  }
  const required = new Set();
  if (profile.id.startsWith("pages-storefront-")) {
    required.add("rustok-pages-storefront");
    required.add("rustok-page-builder-storefront");
    required.add("fly");
    required.add("rustok-page-builder");
  }
  if (profile.id === "pages-storefront-ssr") required.add("rustok-pages");
  if (profile.id === "host-storefront-ssr") {
    required.add("rustok-storefront");
    required.add("rustok-pages-storefront");
    required.add("rustok-page-builder-storefront");
  }
  if (profile.id === "host-storefront-csr" || profile.id === "host-storefront-hydrate") {
    required.add("rustok-storefront");
    if (packages.has("rustok-pages-storefront")) {
      failures.push(`${profile.id}: optional Pages storefront unexpectedly enabled`);
    }
  }
  for (const packageName of required) {
    if (!packages.has(packageName)) {
      failures.push(`${profile.id}: missing required package ${packageName}`);
    }
  }
}

for (const marker of [
  "source-ready / execution-pending",
  "Six feature-resolved graphs",
  "Dev-dependencies are excluded",
  "compiled bundle artifact evidence remains pending"
]) {
  need(packet, marker, "anonymous storefront graph packet");
}
for (const marker of [
  "anonymous-storefront-graph-source-ready",
  "Anonymous storefront authoring exclusion: source-ready",
  "feature-resolved `cargo metadata`",
  "bundle artifact execution remains pending"
]) {
  need(plan, marker, "canonical Pages/Page Builder plan");
}
for (const marker of [
  "anonymous storefront dependency graph verifier",
  "Compiled SSR/CSR/hydrate bundle artifact evidence remains open"
]) {
  need(localPlan, marker, "Pages local plan");
}

if (failures.length > 0) {
  console.error("[verify-pages-anonymous-storefront-graph] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  "[verify-pages-anonymous-storefront-graph] PASS graph_profiles=6 authoring_packages=excluded bundle_artifact_execution=pending"
);
