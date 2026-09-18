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
      "graphql",
      "integrations",
      "migrations",
      "services",
      "tests",
    ],
    allowedRootRustFiles: new Set(["error.rs", "lib.rs", "module.rs"]),
    maxRustFileBytes: 32 * 1024,
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
      (entry.name.endsWith(".mjs") || entry.name.endsWith(".json"))
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

const retiredBlogPhysicalPaths = [
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
]) {
  for (const file of textContractFiles(root)) {
    const source = readFileSync(file, "utf8");
    for (const retiredPath of retiredBlogPhysicalPaths) {
      if (source.includes(retiredPath)) {
        fail(
          `${relative(repoRoot, file)}: active verifier/evidence still references retired Blog path ${retiredPath}`,
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
