import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath = path.join(
  repoRoot,
  "crates/rustok-forum/contracts/forum-reaction-subject-provider.json",
);
const contract = JSON.parse(readFileSync(contractPath, "utf8"));

function fail(message) {
  throw new Error(`Forum reaction subject provider verification failed: ${message}`);
}

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) fail(`missing required file ${relativePath}`);
  return readFileSync(absolute, "utf8");
}

if (contract.schema_version !== 1) fail("unsupported contract schema version");
if (contract.module !== "forum") fail("contract module must be forum");
if (contract.contract !== "forum_reaction_subject_provider_v1") {
  fail("unexpected contract identity");
}
if (contract.source !== "forum") fail("provider source must be forum");
if (JSON.stringify(contract.supported_kinds) !== JSON.stringify(["topic", "reply"])) {
  fail("provider kinds must remain topic and reply");
}
if (contract.catalog.selection !== "single") fail("v1 catalog must remain single-select");
if (JSON.stringify(contract.catalog.keys) !== JSON.stringify(["like"])) {
  fail("v1 catalog must contain only like");
}

for (const relativePath of contract.required_files) read(relativePath);

const provider = read("crates/rustok-forum/src/reaction_subject.rs");
const forumLib = read("crates/rustok-forum/src/lib.rs");
const forumCargo = read("crates/rustok-forum/Cargo.toml");
const modules = read("modules.toml");
const forumPlan = read("crates/rustok-forum/docs/implementation-plan.md");
const reactionsPlan = read("crates/rustok-reactions/docs/implementation-plan.md");

for (const fragment of contract.required_source_fragments) {
  if (!provider.includes(fragment)) fail(`provider is missing ${fragment}`);
}
for (const fragment of contract.forbidden_source_fragments) {
  if (provider.includes(fragment)) fail(`provider contains forbidden fragment ${fragment}`);
}

for (const fragment of [
  'rustok-reactions-api = { path = "../rustok-reactions-api" }',
  "rustok-notifications-api.workspace = true",
]) {
  if (!forumCargo.includes(fragment)) fail(`Forum Cargo boundary is missing ${fragment}`);
}
if (/^rustok-reactions\s*=/mu.test(forumCargo)) {
  fail("Forum must depend only on rustok-reactions-api, not the Reactions owner");
}

for (const fragment of [
  "mod reaction_subject;",
  "register_reaction_subject_provider_factory",
  "ForumReactionSubjectProviderFactory",
  '&["content", "taxonomy"]',
]) {
  if (!forumLib.includes(fragment)) fail(`Forum module registration is missing ${fragment}`);
}

const defaultEnabled = modules.match(/default_enabled\s*=\s*\[[\s\S]*?\]/mu)?.[0] ?? "";
if (/"reactions"/u.test(defaultEnabled)) {
  fail("Reactions must remain outside default_enabled in the provider-only slice");
}

for (const fragment of [
  "topic`/`reply` provider factory",
  "latest captured Forum revision id + 1",
  "optional owner selection and host materialization",
  "node scripts/verify/verify-forum-reaction-subject-provider.mjs",
]) {
  if (!forumPlan.includes(fragment)) fail(`Forum plan is missing ${fragment}`);
}
for (const fragment of [
  "| `REACTIONS-03` | `in_progress` |",
  "Forum producer boundary",
  "outside default profiles",
]) {
  if (!reactionsPlan.includes(fragment)) fail(`Reactions plan is missing ${fragment}`);
}

for (const fragment of [
  "forum_topic::Entity::find()",
  "forum_reply::Entity::find()",
  "forum_topic_revision::Entity::find()",
  "forum_reply_revision::Entity::find()",
  "ReplyStatus::Approved",
  "TopicStatus::Open",
  "context.channel.as_deref()",
]) {
  if (!provider.includes(fragment)) fail(`provider owner authorization is missing ${fragment}`);
}

console.log("Forum reaction subject provider source contract verified.");
