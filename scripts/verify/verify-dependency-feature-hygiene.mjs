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

function walkRustFiles(relativeRoot) {
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
      } else if (entry.isFile() && entry.name.endsWith(".rs")) {
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

const forbiddenRustPatterns = [
  ["sea_orm_migration::cli", "SeaORM migration CLI API"],
  ["sea_orm_cli", "sea-orm-cli API"],
  ["postcard::to_vec", "Postcard heapless vector API"],
  ["postcard::to_vec_cobs", "Postcard heapless COBS API"],
];

for (const absolutePath of [
  ...walkRustFiles("apps"),
  ...walkRustFiles("crates"),
  ...walkRustFiles("xtask"),
  ...walkRustFiles("tests"),
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
