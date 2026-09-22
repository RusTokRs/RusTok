import {
  existsSync,
  readdirSync,
  readFileSync,
  statSync,
} from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

const profiles = [
  {
    crate: "rustok-blog",
    source: "crates/modules/rustok-blog/src",
    requiredDirectories: [
      "controllers",
      "domain",
      "dto",
      "entities",
      "error",
      "graphql",
      "integrations",
      "migrations",
      "services",
      "tests",
    ],
    allowedRootRustFiles: new Set(["lib.rs", "module.rs"]),
    maxRustFileBytes: 32 * 1024,
  },
];


const uiProfiles = [
  {
    crate: "rustok-blog-admin",
    source: "crates/modules/rustok-blog/admin/src",
    requiredDirectories: ["core", "transport", "ui"],
    maxRustFileBytes: 40 * 1024,
  },
  {
    crate: "rustok-blog-storefront",
    source: "crates/modules/rustok-blog/storefront/src",
    requiredDirectories: ["core", "transport", "ui"],
    maxRustFileBytes: 40 * 1024,
  },
];

const failures = [];

function fail(message) {
  failures.push(message);
}

function rustFiles(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...rustFiles(path));
    } else if (entry.isFile() && entry.name.endsWith(".rs")) {
      files.push(path);
    }
  }
  return files;
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

function assertNoForbiddenImports(files, patterns, layer) {
  for (const file of files) {
    const source = readFileSync(file, "utf8");
    for (const [label, pattern] of patterns) {
      if (pattern.test(source)) {
        fail(
          `${relative(repoRoot, file)}: canonical ${layer} layer must not import ${label}`,
        );
      }
    }
  }
}

for (const profile of profiles) {
  const sourceRoot = join(repoRoot, profile.source);
  if (!existsSync(sourceRoot)) {
    fail(`${profile.crate}: missing source root ${profile.source}`);
    continue;
  }

  for (const directory of profile.requiredDirectories) {
    const path = join(sourceRoot, directory);
    if (!existsSync(path) || !statSync(path).isDirectory()) {
      fail(`${profile.crate}: missing canonical source slot ${directory}/`);
    }
  }

  for (const entry of readdirSync(sourceRoot, { withFileTypes: true })) {
    if (
      entry.isFile() &&
      entry.name.endsWith(".rs") &&
      !profile.allowedRootRustFiles.has(entry.name)
    ) {
      fail(
        `${profile.crate}: ${entry.name} is a responsibility module at src root; place it in a canonical source slot`,
      );
    }
  }

  const libPath = join(sourceRoot, "lib.rs");
  const modulePath = join(sourceRoot, "module.rs");
  const lib = readFileSync(libPath, "utf8");
  const moduleSource = readFileSync(modulePath, "utf8");

  for (const forbidden of [
    "impl RusToKModule",
    "impl MigrationSource",
    "register_runtime_extensions(",
    "register_event_listeners(",
  ]) {
    if (lib.includes(forbidden)) {
      fail(
        `${profile.crate}: lib.rs must remain a facade; move ${forbidden} to module.rs`,
      );
    }
  }

  if (lib.includes("pub mod entities;") || lib.includes("pub use entities::")) {
    fail(
      `${profile.crate}: persistence entities must remain private implementation details; expose DTOs/services/ports instead`,
    );
  }

  if (!lib.includes("mod module;") || !lib.includes("pub use module::BlogModule;")) {
    fail(
      `${profile.crate}: lib.rs must expose BlogModule through the canonical module.rs wiring slot`,
    );
  }
  if (
    !moduleSource.includes("impl RusToKModule for BlogModule") ||
    !moduleSource.includes("impl MigrationSource for BlogModule")
  ) {
    fail(
      `${profile.crate}: module.rs must own both RusToKModule and MigrationSource wiring`,
    );
  }

  const allRustFiles = rustFiles(sourceRoot);
  for (const file of allRustFiles) {
    const size = statSync(file).size;
    if (size > profile.maxRustFileBytes) {
      fail(
        `${relative(repoRoot, file)}: ${size} bytes exceeds the canonical reference-module limit of ${profile.maxRustFileBytes} bytes; split by responsibility`,
      );
    }
  }

  assertNoForbiddenImports(
    rustFiles(join(sourceRoot, "domain")),
    [
      ["SeaORM", /\bsea_orm(?:::|\s)/],
      ["Axum", /\baxum(?:::|\s)/],
      ["async-graphql", /\basync_graphql(?:::|\s)/],
      ["Leptos", /\bleptos(?:::|\s)/],
      ["rustok-web", /\brustok_web(?:::|\s)/],
    ],
    "domain",
  );

  assertNoForbiddenImports(
    rustFiles(join(sourceRoot, "services")),
    [
      ["Axum", /\baxum(?:::|\s)/],
      ["async-graphql", /\basync_graphql(?:::|\s)/],
      ["Leptos", /\bleptos(?:::|\s)/],
      ["rustok-web", /\brustok_web(?:::|\s)/],
    ],
    "service",
  );
}

for (const profile of uiProfiles) {
  const sourceRoot = join(repoRoot, profile.source);
  if (!existsSync(sourceRoot)) {
    fail(`${profile.crate}: missing UI source root ${profile.source}`);
    continue;
  }

  for (const directory of profile.requiredDirectories) {
    const path = join(sourceRoot, directory);
    if (!existsSync(path) || !statSync(path).isDirectory()) {
      fail(`${profile.crate}: missing canonical UI source slot ${directory}/`);
    }
  }

  for (const file of rustFiles(sourceRoot)) {
    const size = statSync(file).size;
    if (size > profile.maxRustFileBytes) {
      fail(
        `${relative(repoRoot, file)}: ${size} bytes exceeds the canonical module-owned UI limit of ${profile.maxRustFileBytes} bytes; split commands, presentation, components, tests, or transport responsibilities`,
      );
    }
  }
}

const adminCore = readFileSync(
  join(repoRoot, "crates/modules/rustok-blog/admin/src/core.rs"),
  "utf8",
);
for (const marker of ["mod commands;", "mod presentation;", "#[cfg(test)]\nmod tests;"]) {
  if (!adminCore.includes(marker)) {
    fail(`rustok-blog-admin: core.rs must remain a facade over canonical core responsibilities; missing ${marker}`);
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
  `Canonical module source-layout verification passed for ${profiles.map((profile) => profile.crate).join(", ")}.`,
);
