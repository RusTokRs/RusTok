import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath = path.join(
  repoRoot,
  "crates/rustok-forum/contracts/forum-shared-capability-ownership.json",
);
const contract = JSON.parse(readFileSync(contractPath, "utf8"));

function fail(message) {
  throw new Error(`Forum shared capability ownership verification failed: ${message}`);
}

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) fail(`missing required file ${relativePath}`);
  return readFileSync(absolute, "utf8");
}

if (contract.schema_version !== 1) fail("unsupported contract schema version");
if (contract.module !== "forum") fail("contract module must be forum");
if (contract.contract !== "forum_shared_capability_ownership_v1") {
  fail("unexpected contract identity");
}

const plan = read(contract.canonical_plan);
for (const fragment of contract.required_plan_fragments) {
  if (!plan.includes(fragment)) fail(`canonical plan is missing: ${fragment}`);
}
for (const fragment of contract.forbidden_plan_fragments) {
  if (plan.includes(fragment)) fail(`canonical plan retains stale ownership: ${fragment}`);
}

if (!existsSync(path.join(repoRoot, contract.archive_snapshot))) {
  fail("the exact pre-correction plan snapshot is missing");
}

const modules = read("modules.toml");
for (const slug of [
  "profiles",
  "media",
  "social_graph",
  "reactions",
  "moderation",
  "notifications",
  "translation",
  "search",
  "index",
  "seo",
  "outbox",
  "events",
]) {
  const pattern = new RegExp(`^${slug}\\s*=`, "mu");
  if (!pattern.test(modules)) fail(`modules.toml does not register ${slug}`);
}

const profiles = read("crates/rustok-profiles/README.md");
if (!profiles.includes("owns the universal public profile domain")) {
  fail("Profiles does not declare universal public profile ownership");
}
if (!profiles.includes("Serves `rustok-blog` and `rustok-forum` through `ProfilesReader`")) {
  fail("Profiles does not declare the Forum batch-reader boundary");
}

const media = read("crates/rustok-media/README.md");
for (const fragment of [
  "owns media asset uploads",
  "Own storage-backed media lifecycle state",
  "MediaAssetReadPort",
]) {
  if (!media.includes(fragment)) fail(`Media owner contract is missing: ${fragment}`);
}

const reactions = read("crates/rustok-reactions/README.md");
for (const fragment of [
  "optional shared owner for reusable reactions",
  "never reads producer-private tables",
  "Existing Forum votes remain unchanged",
]) {
  if (!reactions.includes(fragment)) {
    fail(`Reactions owner contract is missing: ${fragment}`);
  }
}

const moderation = read("crates/rustok-moderation/README.md");
for (const fragment of [
  "cross-domain owner for moderation reports",
  "ModerationSubjectCommandPort",
  "never updates another module's tables",
]) {
  if (!moderation.includes(fragment)) {
    fail(`Moderation owner contract is missing: ${fragment}`);
  }
}

console.log("Forum shared capability ownership contract verified.");
