import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath = path.join(
  repoRoot,
  "crates/modules/rustok-blog/contracts/blog-reaction-subject-provider.json",
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

const provider = read("crates/modules/rustok-blog/src/integrations/reaction_subject.rs");
const ownerRead = read(contract.owner_read.file);
const integrationModule = read("crates/modules/rustok-blog/src/integrations/mod.rs");
const moduleSource = read("crates/modules/rustok-blog/src/module.rs");
const blogCargo = read("crates/modules/rustok-blog/Cargo.toml");
const reactionsPlan = compact(read("crates/modules/rustok-reactions/docs/implementation-plan.md"));

for (const fragment of contract.required_source_fragments) {
  if (!provider.includes(fragment)) fail(`provider is missing ${fragment}`);
}
for (const fragment of contract.forbidden_source_fragments) {
  if (provider.includes(fragment)) fail(`provider contains forbidden fragment ${fragment}`);
}

if (!blogCargo.includes('rustok-reactions = { workspace = true, default-features = false }')) {
  fail("Blog must depend on rustok-reactions with default-features = false");
}

if (!integrationModule.includes("pub mod reaction_subject;")) {
  fail("Blog integration registry is missing pub mod reaction_subject;");
}
for (const fragment of [
  "register_reaction_subject_provider_factory",
  "BlogReactionSubjectProviderFactory",
]) {
  if (!moduleSource.includes(fragment)) fail(`Blog module registration is missing ${fragment}`);
}

// The registration runs under the module's declared dependency set, and the module
// manifest the host reads has to declare the same one. Blog reaches Comments through the
// host port, not as a module dependency, so `comments` must not appear here.
const declaredDependencies = moduleSource.match(
  /fn dependencies\(&self\) -> &\[&'static str\] \{\s*&\[([^\]]+)\]/u,
);
if (declaredDependencies === null) {
  fail("Blog module must declare its dependency list");
}
const dependencyNames = [...declaredDependencies[1].matchAll(/"([^"]+)"/gu)].map((m) => m[1]);
if (dependencyNames.join(",") !== "content,taxonomy,outbox,channel") {
  fail(`Blog module dependency set drifted: ${dependencyNames.join(",")}`);
}
const manifest = read("crates/modules/rustok-blog/rustok-module.toml");
const dependencySection = manifest.match(/\[dependencies\]\s*\n([\s\S]*?)(?:\n\[|$)/u);
if (dependencySection === null) {
  fail("Blog module manifest must declare a [dependencies] section");
}
const manifestDependencies = [...dependencySection[1].matchAll(/^([A-Za-z0-9_-]+)\s*=/gmu)]
  .map((m) => m[1])
  .sort();
if (manifestDependencies.join(",") !== [...dependencyNames].sort().join(",")) {
  fail(
    `Blog module manifest dependencies ${manifestDependencies.join(",")} do not match the module declaration ${dependencyNames.join(",")}`,
  );
}
if (dependencyNames.includes("comments")) {
  fail("Comments must not be a Blog module dependency");
}

// The provider authorizes against the owner snapshot: it must resolve the post through
// the shared loader and apply the lifecycle, channel and revision rules to it, instead of
// holding its own copy of the two queries.
for (const fragment of [
  'load_post_subject_snapshot(',
  'snapshot.status != BlogPostStatus::Published',
  'is_post_visible_for_channel(&snapshot.channel_slugs, context.channel.as_deref())',
  'subject.subject_revision() != current_revision',
  'tenant_id != request.subject.tenant_id()',
  'request.subject.source().as_str() != BLOG_REACTION_SOURCE',
]) {
  if (!provider.includes(fragment)) fail(`provider owner authorization is missing ${fragment}`);
}

// ...and the loader that resolves the snapshot must still read the owner row and its
// channel visibility rows with the tenant scope and the persisted-version check.
for (const fragment of contract.owner_read.required_fragments) {
  if (!ownerRead.includes(fragment)) {
    fail(`${contract.owner_read.file} is missing ${fragment}`);
  }
}

// The loader the provider imports has to be the one the contract describes; a rename that
// leaves the provider calling something else would otherwise pass the fragments above.
if (!provider.includes(`load_post_subject_snapshot`)) {
  fail("provider must resolve the subject through load_post_subject_snapshot");
}
if (!read("crates/modules/rustok-blog/src/services/mod.rs").includes(contract.owner_read.loader)) {
  fail(`rustok-blog services must re-export ${contract.owner_read.loader}`);
}

for (const fragment of [
  "| `REACTIONS-04` | `in_progress` |",
  "Blog `post` producer",
  "neutral-contract review",
]) {
  if (!reactionsPlan.includes(fragment)) fail(`Reactions plan is missing ${fragment}`);
}

console.log("Blog reaction subject provider source contract verified.");
