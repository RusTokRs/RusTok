#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function dependencySpec(source, name) {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = new RegExp(`^${escaped}\\s*=\\s*\\{([^\\n]+)\\}$`, "m").exec(source);
  return match?.[1] ?? null;
}

function requireSpec(spec, marker, dependency) {
  if (!spec?.includes(marker)) {
    failures.push(`${dependency}: dependency specification must include ${marker}`);
  }
}

function forbidSpec(spec, marker, dependency) {
  if (spec?.includes(marker)) {
    failures.push(`${dependency}: dependency specification must not include ${marker}`);
  }
}

function walkFiles(relativeRoot, predicate) {
  const absoluteRoot = path.join(repoRoot, relativeRoot);
  if (!fs.existsSync(absoluteRoot)) {
    return [];
  }

  const files = [];
  const stack = [absoluteRoot];
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      if (entry.name === "target" || entry.name === "node_modules" || entry.name === ".git") {
        continue;
      }
      const absolute = path.join(current, entry.name);
      if (entry.isDirectory()) {
        stack.push(absolute);
      } else if (entry.isFile() && predicate(entry.name)) {
        files.push(absolute);
      }
    }
  }
  return files;
}

const cargo = read("Cargo.toml");
const migration = dependencySpec(cargo, "sea-orm-migration");
const postcard = dependencySpec(cargo, "postcard");

if (!migration) {
  failures.push("Cargo.toml: sea-orm-migration workspace dependency not found");
} else {
  requireSpec(migration, "default-features = false", "sea-orm-migration");
  requireSpec(migration, '"sqlx-postgres"', "sea-orm-migration");
  requireSpec(migration, '"sqlx-sqlite"', "sea-orm-migration");
  requireSpec(migration, '"runtime-tokio-rustls"', "sea-orm-migration");
  forbidSpec(migration, '"sqlx-mysql"', "sea-orm-migration");
  forbidSpec(migration, '"cli"', "sea-orm-migration");
}

if (!postcard) {
  failures.push("Cargo.toml: postcard workspace dependency not found");
} else {
  requireSpec(postcard, "default-features = false", "postcard");
  requireSpec(postcard, '"use-std"', "postcard");
  forbidSpec(postcard, '"heapless"', "postcard");
  forbidSpec(postcard, '"heapless-cas"', "postcard");
}

const memberManifests = [
  ...walkFiles("apps", (name) => name === "Cargo.toml"),
  ...walkFiles("crates", (name) => name === "Cargo.toml"),
  ...walkFiles("xtask", (name) => name === "Cargo.toml"),
  ...walkFiles("tests", (name) => name === "Cargo.toml"),
  ...walkFiles("ops", (name) => name === "Cargo.toml"),
  ...walkFiles("UI", (name) => name === "Cargo.toml"),
];

for (const absolutePath of memberManifests) {
  const source = fs.readFileSync(absolutePath, "utf8");
  const relativePath = path.relative(repoRoot, absolutePath);
  for (const dependency of ["sea-orm-migration", "postcard"]) {
    const escaped = dependency.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const specs = [...source.matchAll(new RegExp(`^${escaped}\\s*=\\s*\\{([^\\n]+)\\}$`, "gm"))];
    for (const match of specs) {
      const spec = match[1];
      if (!spec.includes("workspace = true")) {
        failures.push(`${relativePath}: ${dependency} must inherit the workspace dependency policy`);
      }
      for (const forbidden of [
        "default-features = true",
        '"cli"',
        '"sqlx-mysql"',
        '"heapless"',
        '"heapless-cas"',
      ]) {
        if (spec.includes(forbidden)) {
          failures.push(`${relativePath}: ${dependency} member override must not include ${forbidden}`);
        }
      }
    }
  }
}

const forbiddenRustPatterns = [
  ["sea_orm_migration::cli", "SeaORM migration CLI API"],
  ["sea_orm_cli", "sea-orm-cli API"],
  ["postcard::to_vec", "Postcard heapless vector API"],
  ["postcard::to_vec_cobs", "Postcard heapless COBS API"],
];

for (const absolutePath of [
  ...walkFiles("apps", (name) => name.endsWith(".rs")),
  ...walkFiles("crates", (name) => name.endsWith(".rs")),
  ...walkFiles("xtask", (name) => name.endsWith(".rs")),
  ...walkFiles("tests", (name) => name.endsWith(".rs")),
]) {
  const source = fs.readFileSync(absolutePath, "utf8");
  const relativePath = path.relative(repoRoot, absolutePath);
  for (const [pattern, description] of forbiddenRustPatterns) {
    if (source.includes(pattern)) {
      failures.push(`${relativePath}: ${description} is forbidden by workspace feature policy`);
    }
  }
}

if (failures.length > 0) {
  console.error("Dependency feature hygiene verification failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ unused SeaORM CLI/MySQL and Postcard heapless defaults remain disabled");
