import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath = path.join(
  repoRoot,
  "crates/modules/rustok-forum/contracts/forum-reaction-subject-provider.json",
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

function compact(value) {
  return value.replace(/\s+/gu, " ").trim();
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

const provider = read("crates/modules/rustok-forum/src/reaction_subject.rs");
const forumLib = read("crates/modules/rustok-forum/src/lib.rs");
const forumCargo = read("crates/modules/rustok-forum/Cargo.toml");
const modules = read("modules.toml");
const forumPlan = compact(read("crates/modules/rustok-forum/docs/implementation-plan.md"));
const reactionsPlan = compact(read("crates/modules/rustok-reactions/docs/implementation-plan.md"));

for (const fragment of contract.required_source_fragments) {
  if (!provider.includes(fragment)) fail(`provider is missing ${fragment}`);
}
for (const fragment of contract.forbidden_source_fragments) {
  if (provider.includes(fragment)) fail(`provider contains forbidden fragment ${fragment}`);
}

for (const fragment of [
  'rustok-reactions = { workspace = true, default-features = false }',
  "rustok-notifications.workspace = true",
]) {
  if (!forumCargo.includes(fragment)) fail(`Forum Cargo boundary is missing ${fragment}`);
}

for (const fragment of [
  "mod reaction_subject;",
  "register_reaction_subject_provider_factory",
  "ForumReactionSubjectProviderFactory",
]) {
  if (!forumLib.includes(fragment)) fail(`Forum module registration is missing ${fragment}`);
}

// The module's own `dependencies()` and the registry entry must agree; a literal dependency list
// here would rot the moment an unrelated packet adds a dependency (it did: `media` arrived with
// the attachment work while the registry stayed authoritative).
const declaredBody = compact(
  forumLib.match(/fn dependencies\(&self\)[^{]*\{[\s\S]*?\n\s*\}/u)?.[0] ?? "",
);
const registryEntry = compact(
  modules.match(/^forum\s*=\s*\{[^\n]*\}/mu)?.[0] ?? "",
);
// Only quoted-name arrays count, so the `&[&'static str]` return type is not mistaken for it.
const declaredList = declaredBody.match(/&\[\s*(?:"[^"]+"\s*,?\s*)+\]/u)?.[0]?.replace(/^&\[|\]$/gu, "") ?? "";
const registryList = registryEntry.match(/depends_on\s*=\s*\[([^\]]*)\]/u)?.[1] ?? "";
if (declaredList.length === 0 || registryList.length === 0) {
  fail("forum module dependency lists could not be located in lib.rs or modules.toml");
}
const sortNames = (value) =>
  value
    .split(",")
    .map((name) => name.trim().replace(/^"|"$/gu, ""))
    .filter((name) => name.length > 0)
    .sort()
    .join(",");
if (sortNames(declaredList) !== sortNames(registryList)) {
  fail(
    `forum dependencies() [${sortNames(declaredList)}] must match modules.toml depends_on [${sortNames(
      registryList,
    )}]`,
  );
}

const defaultEnabled = modules.match(/default_enabled\s*=\s*\[[\s\S]*?\]/mu)?.[0] ?? "";
if (/"reactions"/u.test(defaultEnabled)) {
  fail("Reactions must remain outside default_enabled in the provider-only slice");
}

for (const fragment of [
  "topic`/`reply` provider factory",
  "latest captured Forum revision id + 1",
  "Optional owner selection and host materialization",
  "node scripts/verify/verify-forum-reaction-subject-provider.mjs",
]) {
  if (!forumPlan.includes(fragment)) fail(`Forum plan is missing ${fragment}`);
}
for (const fragment of [
  "| `REACTIONS-03` | `in_progress` |",
  "Forum producer boundary",
  "outside defaults",
]) {
  if (!reactionsPlan.includes(fragment)) fail(`Reactions plan is missing ${fragment}`);
}

// Authorization must resolve the subject revision through the owner's revision service and reject
// anything that is not exactly the current one. The earlier pins demanded the provider query
// `forum_*_revision::Entity::find()` itself, which was superseded when the revision lookup moved
// behind `RevisionService`; pinning the service call plus `allow_exact_revision` keeps the
// invariant (stale or unavailable revision => the command is refused) without freezing the SQL.
for (const fragment of [
  "forum_topic::Entity::find()",
  "forum_reply::Entity::find()",
  "RevisionService::new(",
  ".current_topic_revision(",
  ".current_reply_revision(",
  "allow_exact_revision",
  "ReplyStatus::Approved",
  "TopicStatus::Open",
  "context.channel.as_deref()",
]) {
  if (!provider.includes(fragment)) fail(`provider owner authorization is missing ${fragment}`);
}

// A substring check passes as long as one call site survives, so the guardrails that must apply to
// both subject kinds are counted: dropping channel scoping or the exact-revision refusal from
// either kind fails here even though the pattern is still present somewhere in the file.
const countOccurrences = (haystack, needle) => haystack.split(needle).length - 1;
for (const [fragment, expected] of [
  ["context.channel.as_deref()", 2],
  ["self.allow_exact_revision(request, current_revision)", 2],
  ["RevisionService::new(", 2],
]) {
  const found = countOccurrences(provider, fragment);
  if (found !== expected) {
    fail(
      `provider must apply ${fragment} to both subject kinds (found ${found}, expected ${expected})`,
    );
  }
}

console.log("Forum reaction subject provider source contract verified.");
