import {
  existsSync,
  readdirSync,
  readFileSync,
  statSync,
} from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

import { lintNativeModule, resolveNativeModule } from "./native-module-layout.mjs";

const parameters = process.argv.slice(2);
const [selector, slug] = parameters;
const embedded = selector === "--embedded";
if (parameters.length !== 0 &&
    (parameters.length !== 2 || !["--module", "--embedded", "--cli"].includes(selector) || !slug)) {
  console.error("Usage: node scripts/verify/verify-module-source-layout.mjs [--module <slug> | --embedded <slug> | --cli <slug>]");
  process.exit(2);
}
const selectedModule = slug ?? "blog";
let modulePath;
try {
  modulePath = resolveNativeModule(repoRoot, selectedModule, { embedded });
} catch (error) {
  console.error(error.message);
  process.exit(1);
}
const architectureFailures = lintNativeModule(repoRoot, modulePath, selectedModule, {
  embedded,
  cli: selectedModule === "blog" || selector === "--cli",
});
if (architectureFailures.length) {
  console.error("Native module architecture verification failed:");
  for (const failure of architectureFailures) console.error("- " + failure);
  process.exit(1);
}
if (selectedModule !== "blog") {
  console.log("Native module architecture verification passed for " + selectedModule + ".");
  process.exit(0);
}

const failures = [];
const blog = "crates/modules/rustok-blog";

function fail(message) {
  failures.push(message);
}

const requiredSlots = [
  ["src", [
    "controllers", "domain", "dto", "entities", "error", "graphql", "integrations",
    "migrations", "services", "services/category/commands", "tests",
  ]],
  ["admin/src", ["core", "core/tests", "transport", "ui/leptos", "tests"]],
  ["storefront/src", ["core", "transport", "ui/leptos", "tests"]],
  ["tests", ["integration"]],
];
for (const [source, slots] of requiredSlots) {
  for (const slot of slots) {
    const path = join(repoRoot, blog, source, slot);
    if (!existsSync(path) || !statSync(path).isDirectory()) {
      fail(`${blog}/${source}: missing Blog reference slot ${slot}/`);
    }
  }
}
for (const entry of readdirSync(join(repoRoot, blog, "src"), { withFileTypes: true })) {
  if (entry.isFile() && entry.name.endsWith(".rs") && !["lib.rs", "module.rs"].includes(entry.name)) {
    fail(`${blog}/src/${entry.name}: place the responsibility in a Blog source slot`);
  }
}

function textContractFiles(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...textContractFiles(path));
    } else if (
      entry.isFile() &&
      (entry.name.endsWith(".mjs") ||
        entry.name.endsWith(".json") ||
        entry.name.endsWith(".yml") ||
        entry.name.endsWith(".yaml"))
    ) {
      files.push(path);
    }
  }
  return files;
}

const adminCore = readFileSync(
  join(repoRoot, "crates/modules/rustok-blog/admin/src/core/mod.rs"),
  "utf8",
).replaceAll("\r\n", "\n");
for (const marker of ["mod commands;", "mod presentation;", "#[cfg(test)]\nmod tests;"]) {
  if (!adminCore.includes(marker)) {
    fail(`rustok-blog-admin: core/mod.rs must remain a facade over canonical core responsibilities; missing ${marker}`);
  }
}
const adminUi = readFileSync(
  join(repoRoot, "crates/modules/rustok-blog/admin/src/ui/mod.rs"),
  "utf8",
);
if (!adminUi.includes("mod components;")) {
  fail("rustok-blog-admin: reusable render components must live outside the primary Leptos controller");
}

const retiredBlogPhysicalPaths = [
  blog + "/src/services/category.rs",
  blog + "/src/services/category_owner.rs",
  blog + "/src/services/category_command.rs",
  blog + "/src/services/category_delete.rs",
  blog + "/src/services/category_name_projection.rs",
  blog + "/src/services/category_taxonomy_sync.rs",
  blog + "/admin/src/core.rs",
  blog + "/admin/src/core/tests.rs",
  blog + "/admin/src/ui/leptos.rs",
  blog + "/storefront/src/core.rs",
  blog + "/storefront/src/ui/leptos.rs",
  blog + "/tests/integration.rs",
  "crates/modules/rustok-blog/src/" + "error.rs",
  "crates/modules/rustok-blog/src/" + "public_error.rs",
  "crates/modules/rustok-blog/src/" + "openapi.rs",
  "crates/modules/rustok-blog/src/" + "richtext.rs",
  "crates/modules/rustok-blog/src/" + "state_machine.rs",
  "crates/modules/rustok-blog/src/" + "reaction_subject.rs",
  "crates/modules/rustok-blog/src/" + "seo_targets.rs",
  "crates/modules/rustok-blog/src/" + "public_comments_snapshot.rs",
  "crates/modules/rustok-blog/src/services/" + "post.rs",
  "crates/modules/rustok-blog/src/" + "contract_tests.rs",
  "crates/modules/rustok-blog/src/" + "tag_tenant_integrity_tests.rs",
];

for (const root of [
  join(repoRoot, "scripts/verify"),
  join(repoRoot, "crates/modules/rustok-blog/contracts"),
  join(repoRoot, ".github/workflows"),
]) {
  for (const file of textContractFiles(root)) {
    const source = readFileSync(file, "utf8");
    for (const retiredPath of retiredBlogPhysicalPaths) {
      if (source.includes(retiredPath)) {
        fail(
          `${relative(repoRoot, file)}: active verifier/evidence/workflow still references retired Blog path ${retiredPath}`,
        );
      }
    }
  }
}

if (failures.length > 0) {
  console.error("Canonical module source-layout verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "Canonical module source-layout verification passed for rustok-blog.",
);
