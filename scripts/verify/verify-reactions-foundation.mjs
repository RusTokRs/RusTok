import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath = path.join(
  repoRoot,
  "crates/rustok-reactions/contracts/reactions-foundation.json",
);
const contract = JSON.parse(readFileSync(contractPath, "utf8"));

function fail(message) {
  throw new Error(`Reactions foundation verification failed: ${message}`);
}

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) fail(`missing required file ${relativePath}`);
  return readFileSync(absolute, "utf8");
}

if (contract.schema_version !== 1) fail("unsupported contract schema version");
if (contract.module !== "reactions") fail("contract module must be reactions");
if (contract.contract !== "reactions_foundation_v1") {
  fail("unexpected contract identity");
}

for (const relativePath of contract.required_files) read(relativePath);

const apiCargo = read("crates/rustok-reactions-api/Cargo.toml");
const apiSource = [
  read("crates/rustok-reactions-api/src/lib.rs"),
  read("crates/rustok-reactions-api/src/model.rs"),
  read("crates/rustok-reactions-api/src/provider.rs"),
].join("\n");
const ownerCargo = read("crates/rustok-reactions/Cargo.toml");
const ownerSource = read("crates/rustok-reactions/src/lib.rs");

for (const symbol of contract.required_api_symbols) {
  if (!apiSource.includes(symbol)) fail(`neutral API is missing ${symbol}`);
}
for (const limit of contract.required_limits) {
  if (!apiSource.includes(limit)) fail(`neutral API is missing bound ${limit}`);
}
for (const dependency of contract.forbidden_api_dependencies) {
  if (apiCargo.includes(dependency)) {
    fail(`neutral API imports forbidden dependency ${dependency}`);
  }
}
for (const dependency of contract.forbidden_owner_dependencies) {
  if (ownerCargo.includes(dependency)) {
    fail(`foundation owner imports forbidden dependency ${dependency}`);
  }
}

for (const fragment of [
  "tenant_id",
  "subject_revision",
  "command_id",
  "actor_id",
  "ReactionSelectionPolicy",
  "register_reaction_subject_provider",
  "materialize_reaction_subject_registry",
]) {
  if (!apiSource.includes(fragment)) fail(`neutral API is missing ${fragment}`);
}

for (const fragment of [
  "ReactionsModule",
  "ensure_reaction_subject_registry",
  "ensure_reaction_subject_factory_registry",
  "&[\"outbox\"]",
]) {
  if (!ownerSource.includes(fragment)) fail(`owner foundation is missing ${fragment}`);
}

for (const forbiddenPath of [
  "crates/rustok-reactions/src/entities",
  "crates/rustok-reactions/src/migrations",
  "crates/rustok-reactions/migrations",
  "crates/rustok-reactions/admin",
  "crates/rustok-reactions/storefront",
]) {
  if (existsSync(path.join(repoRoot, forbiddenPath))) {
    fail(`foundation unexpectedly contains ${forbiddenPath}`);
  }
}

const modules = read("modules.toml");
if (!/^reactions\s*=\s*\{[^\n]*crate\s*=\s*"rustok-reactions"/mu.test(modules)) {
  fail("modules.toml does not register the reactions owner");
}
const defaultEnabled = modules.match(/default_enabled\s*=\s*\[[\s\S]*?\]/mu)?.[0] ?? "";
if (/"reactions"/u.test(defaultEnabled)) {
  fail("reactions must remain outside default_enabled in this foundation slice");
}

const manifest = read("crates/rustok-reactions/rustok-module.toml");
for (const fragment of [
  'slug = "reactions"',
  'entry_type = "ReactionsModule"',
  'outbox = { version_req = ">=0.1.0" }',
]) {
  if (!manifest.includes(fragment)) fail(`module manifest is missing ${fragment}`);
}

const forumPlan = read("crates/rustok-forum/docs/implementation-plan.md");
for (const fragment of [
  "| Reaction catalog, actor reactions and aggregate reaction counts | `rustok-reactions` |",
  "| `FORUM-18` | `in_progress` |",
  "Add the Forum topic/reply `ReactionSubjectProvider` adapter",
  "Existing Forum votes remain Forum semantics",
]) {
  if (!forumPlan.includes(fragment)) fail(`Forum plan is missing ${fragment}`);
}

const forumOwnership = JSON.parse(
  read("crates/rustok-forum/contracts/forum-shared-capability-ownership.json"),
);
if (forumOwnership.required_shared_owners.reactions !== "rustok-reactions") {
  fail("Forum ownership contract does not name the active Reactions owner");
}

console.log("Reactions neutral capability foundation verified.");
