import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contract = JSON.parse(
  readFileSync(
    path.join(
      repoRoot,
      "crates/rustok-reactions/contracts/reactions-host-composition.json",
    ),
    "utf8",
  ),
);

function fail(message) {
  throw new Error(`Reactions host composition verification failed: ${message}`);
}

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) fail(`missing required file ${relativePath}`);
  return readFileSync(absolute, "utf8");
}

function featureLine(manifest, feature) {
  return manifest
    .split(/\r?\n/u)
    .find((line) => line.trimStart().startsWith(`${feature} =`));
}

function defaultFeatureBlock(manifest) {
  return manifest.match(/^default\s*=\s*\[[\s\S]*?^\]/mu)?.[0] ?? "";
}

function compact(value) {
  return value.replace(/\s+/gu, " ").trim();
}

if (contract.schema_version !== 1) fail("unsupported contract schema version");
if (contract.contract !== "reactions_optional_host_composition_v1") {
  fail("unexpected contract identity");
}
if (contract.owner_module !== "reactions") fail("owner module must be reactions");
if (contract.owner_feature !== "mod-reactions") fail("owner feature must be mod-reactions");
if (contract.default_enabled !== false) fail("Reactions must remain disabled by default");
if (contract.forum_implies_owner !== false) fail("Forum must not imply the Reactions owner");
if (contract.status !== "source_ready_maintainer_execution_pending") {
  fail("runtime execution must not be claimed by the source contract");
}
for (const relativePath of contract.required_files) read(relativePath);

const modules = read("modules.toml");
const forumCargo = read("crates/rustok-forum/Cargo.toml");
const distributionCargo = read("crates/rustok-distribution/Cargo.toml");
const distributionLib = read("crates/rustok-distribution/src/lib.rs");
const serverCargo = read("apps/server/Cargo.toml");
const dispatcher = read("apps/server/src/services/module_event_dispatcher.rs");
const forumPlan = compact(read("crates/rustok-forum/docs/implementation-plan.md"));
const reactionsPlan = compact(read("crates/rustok-reactions/docs/implementation-plan.md"));

if (!modules.includes('reactions = { crate = "rustok-reactions"')) {
  fail("modules.toml must retain the optional Reactions module descriptor");
}
const defaultEnabled = modules.match(/default_enabled\s*=\s*\[[\s\S]*?\]/mu)?.[0] ?? "";
if (/"reactions"/u.test(defaultEnabled)) {
  fail("modules.toml default_enabled must not include reactions");
}
if (!forumCargo.includes('rustok-reactions-api = { path = "../rustok-reactions-api" }')) {
  fail("Forum must resolve the neutral Reactions API through an explicit path dependency");
}
if (/^rustok-reactions\s*=/mu.test(forumCargo)) {
  fail("Forum must not depend on the Reactions owner crate");
}

if (featureLine(distributionCargo, "mod-reactions") !== 'mod-reactions = ["dep:rustok-reactions"]') {
  fail("distribution mod-reactions feature must select only the owner crate");
}
if (!distributionCargo.includes('rustok-reactions = { path = "../rustok-reactions", optional = true }')) {
  fail("distribution must declare rustok-reactions as optional");
}
for (const fragment of [
  '#[cfg(feature = "mod-reactions")]',
  'registry = registry.register(rustok_reactions::ReactionsModule);',
  'module.slug == "reactions"',
]) {
  if (!distributionLib.includes(fragment)) fail(`distribution registration is missing ${fragment}`);
}

if (
  featureLine(serverCargo, "mod-reactions") !==
  'mod-reactions = ["dep:rustok-reactions", "rustok-reactions/graphql", "rustok-distribution/mod-reactions"]'
) {
  fail("server mod-reactions feature must select the owner and distribution feature");
}
if (!serverCargo.includes('rustok-reactions = { path = "../../crates/rustok-reactions", optional = true }')) {
  fail("server must declare rustok-reactions as optional");
}
if (/"mod-reactions"/u.test(defaultFeatureBlock(serverCargo))) {
  fail("server default features must not include mod-reactions");
}
if ((featureLine(serverCargo, "mod-forum") ?? "").includes("mod-reactions")) {
  fail("server mod-forum must not imply mod-reactions");
}
if ((featureLine(distributionCargo, "mod-forum") ?? "").includes("mod-reactions")) {
  fail("distribution mod-forum must not imply mod-reactions");
}

for (const fragment of [
  "reaction_subject_registry_from_extensions(&extensions).is_none()",
  "Reactions feature is selected but ReactionsModule is missing from ModuleRegistry",
  "materialize_reaction_subject_registry(&mut extensions, &host)",
  "reaction subject provider materialization failed",
]) {
  if (!dispatcher.includes(fragment)) fail(`host composition is missing ${fragment}`);
}

const audienceIndex = dispatcher.indexOf("extensions.insert(audience_facts);");
const recipientIndex = dispatcher.indexOf("extensions.insert(recipient_context);");
const reactionIndex = dispatcher.indexOf("materialize_reaction_subject_registry");
const notificationIndex = dispatcher.indexOf("materialize_notification_source_registry");
if ([audienceIndex, recipientIndex, reactionIndex, notificationIndex].some((index) => index < 0)) {
  fail("host materialization ordering markers are incomplete");
}
if (!(audienceIndex < recipientIndex && recipientIndex < reactionIndex)) {
  fail("Reactions providers must materialize after Forum audience and recipient facts");
}
if (!(reactionIndex < notificationIndex)) {
  fail("Reactions provider materialization must remain before Notifications source materialization");
}

for (const fragment of [
  "optional distribution/server host materialization",
  "enabled/disabled runtime evidence",
  "node scripts/verify/verify-reactions-host-composition.mjs",
]) {
  if (!reactionsPlan.includes(fragment)) fail(`Reactions plan is missing ${fragment}`);
}
for (const fragment of [
  "Optional owner selection and host materialization",
  "Reactions-disabled Forum composition",
  "node scripts/verify/verify-reactions-host-composition.mjs",
]) {
  if (!forumPlan.includes(fragment)) fail(`Forum plan is missing ${fragment}`);
}

console.log("Reactions optional host composition source contract verified.");
