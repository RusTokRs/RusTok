import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath = path.join(
  repoRoot,
  "crates/rustok-blog/contracts/blog-reaction-subject-provider.json",
);
const contract = JSON.parse(readFileSync(contractPath, "utf8"));

function fail(message) {
  throw new Error(`Blog reaction subject provider verification failed: ${message}`);
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
if (contract.module !== "blog") fail("contract module must be blog");
if (contract.contract !== "blog_reaction_subject_provider_v1") {
  fail("unexpected contract identity");
}
if (contract.source !== "blog") fail("provider source must be blog");
if (JSON.stringify(contract.supported_kinds) !== JSON.stringify(["post"])) {
  fail("provider kind must remain post");
}
if (contract.catalog.selection !== "single") fail("v1 catalog must remain single-select");
if (JSON.stringify(contract.catalog.keys) !== JSON.stringify(["like"])) {
  fail("v1 catalog must contain only like");
}

for (const relativePath of contract.required_files) read(relativePath);

const provider = read("crates/rustok-blog/src/reaction_subject.rs");
const blogLib = read("crates/rustok-blog/src/lib.rs");
const blogCargo = read("crates/rustok-blog/Cargo.toml");
const reactionsPlan = compact(read("crates/rustok-reactions/docs/implementation-plan.md"));

for (const fragment of contract.required_source_fragments) {
  if (!provider.includes(fragment)) fail(`provider is missing ${fragment}`);
}
for (const fragment of contract.forbidden_source_fragments) {
  if (provider.includes(fragment)) fail(`provider contains forbidden fragment ${fragment}`);
}

if (!blogCargo.includes('rustok-reactions-api = { path = "../rustok-reactions-api" }')) {
  fail("Blog must depend on the neutral rustok-reactions-api crate");
}
if (/^rustok-reactions\s*=/mu.test(blogCargo)) {
  fail("Blog must not depend on the Reactions owner crate");
}

for (const fragment of [
  "mod reaction_subject;",
  "register_reaction_subject_provider_factory",
  "BlogReactionSubjectProviderFactory",
  '&["content", "comments", "taxonomy", "outbox"]',
]) {
  if (!blogLib.includes(fragment)) fail(`Blog module registration is missing ${fragment}`);
}

for (const fragment of [
  'post.status != BLOG_POST_PUBLISHED_STATUS',
  'context.channel.as_deref()',
  'blog_post_channel_visibility::Column::TenantId.eq(subject.tenant_id())',
  'blog_post_channel_visibility::Column::PostId.eq(post.id)',
  'subject.subject_revision() != current_revision',
]) {
  if (!provider.includes(fragment)) fail(`provider owner authorization is missing ${fragment}`);
}

for (const fragment of [
  "| `REACTIONS-04` | `in_progress` |",
  "Blog `post` producer",
  "neutral-contract review",
]) {
  if (!reactionsPlan.includes(fragment)) fail(`Reactions plan is missing ${fragment}`);
}

console.log("Blog reaction subject provider source contract verified.");
