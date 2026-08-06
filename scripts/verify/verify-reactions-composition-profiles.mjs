import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));

function fail(message) {
  throw new Error(`Reactions composition profile verification failed: ${message}`);
}

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) fail(`missing required file ${relativePath}`);
  return readFileSync(absolute, "utf8");
}

function compact(value) {
  return value.replace(/\s+/gu, " ").trim();
}

const contract = JSON.parse(
  read("crates/rustok-reactions/contracts/reactions-host-composition.json"),
);
const tests = read("apps/server/tests/reactions_composition_profiles.rs");
const serverCargo = read("apps/server/Cargo.toml");
const modules = read("modules.toml");
const reactionsPlan = compact(read("crates/rustok-reactions/docs/implementation-plan.md"));
const forumPlan = compact(read("crates/rustok-forum/docs/implementation-plan.md"));

if (contract.schema_version !== 1) fail("unexpected host contract schema");
if (contract.contract !== "reactions_optional_host_composition_v1") {
  fail("unexpected host contract identity");
}
if (contract.status !== "source_ready_maintainer_execution_pending") {
  fail("source must not claim retained execution");
}
if (contract.executable_evidence?.test_target !== "reactions_composition_profiles") {
  fail("unexpected test target");
}
if (contract.executable_evidence?.database !== "sqlite::memory:") {
  fail("profile tests must use isolated SQLite in-memory storage");
}
if (contract.executable_evidence?.retained_execution_required !== true) {
  fail("retained maintainer execution must remain required");
}

const profiles = Object.values(contract.profiles);
for (const profile of profiles) {
  if (typeof profile.test !== "string" || !tests.includes(`async fn ${profile.test}()`)) {
    fail(`missing executable test ${profile.test}`);
  }
}

for (const fragment of [
  '#[cfg(all(feature = "mod-forum", not(feature = "mod-reactions")))]',
  '#[cfg(all(feature = "mod-reactions", not(feature = "mod-forum")))]',
  '#[cfg(all(feature = "mod-forum", feature = "mod-reactions"))]',
  '#[cfg(feature = "mod-reactions")]',
  'Database::connect("sqlite::memory:")',
  "SharedForumAudienceFactsPort",
  "SharedForumNotificationRecipientContextPort",
  "assert!(subjects.is_empty());",
  'get_by_str("forum")',
  'assert_eq!(kinds, vec!["reply".to_string(), "topic".to_string()]);',
  contract.profiles.selected_feature_without_owner.error,
]) {
  if (!tests.includes(fragment)) fail(`test target is missing ${fragment}`);
}

const commands = contract.executable_evidence.commands;
if (!Array.isArray(commands) || commands.length !== 4) {
  fail("exactly four maintainer commands are required");
}
for (const profile of profiles) {
  if (!commands.some((command) => command.includes(profile.test))) {
    fail(`maintainer command is missing ${profile.test}`);
  }
}

const defaultFeatures = serverCargo.match(/^default\s*=\s*\[[\s\S]*?^\]/mu)?.[0] ?? "";
if (/"mod-reactions"/u.test(defaultFeatures)) {
  fail("server defaults must not enable Reactions");
}
const defaultEnabled = modules.match(/default_enabled\s*=\s*\[[\s\S]*?\]/mu)?.[0] ?? "";
if (/"reactions"/u.test(defaultEnabled)) {
  fail("tenant defaults must not enable Reactions");
}

for (const fragment of [
  "executable composition profile tests",
  "retained execution evidence remains pending",
  "verify-reactions-composition-profiles.mjs",
]) {
  if (!reactionsPlan.includes(fragment)) fail(`Reactions plan is missing ${fragment}`);
}
for (const fragment of [
  "executable source evidence for all three optional profiles",
  "retained execution evidence remains pending",
  "verify-reactions-composition-profiles.mjs",
]) {
  if (!forumPlan.includes(fragment)) fail(`Forum plan is missing ${fragment}`);
}

console.log("Reactions executable composition profiles verified.");
