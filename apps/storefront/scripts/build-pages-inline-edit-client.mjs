#!/usr/bin/env node

import { cpSync, mkdirSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..", "..");
const profile = process.env.RUSTOK_PAGES_INLINE_EDIT_PROFILE?.trim() || "release";
const targetRoot = path.resolve(
  repoRoot,
  process.env.RUSTOK_PAGES_INLINE_EDIT_ASSET_DIR?.trim() ||
    "target/site/assets/pages-inline-edit",
);
const assetRoot = path.dirname(targetRoot);
const cargoArgs = [
  "build",
  "-p",
  "rustok-storefront",
  "--lib",
  "--target",
  "wasm32-unknown-unknown",
  "--no-default-features",
  "--features",
  "pages-inline-edit-hydrate",
];
if (profile === "release") cargoArgs.push("--release");

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    env: process.env,
    stdio: "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${command} exited with status ${result.status}`);
  }
}

rmSync(targetRoot, { force: true, recursive: true });
mkdirSync(targetRoot, { recursive: true });
run("cargo", cargoArgs);

const wasmInput = path.join(
  repoRoot,
  "target",
  "wasm32-unknown-unknown",
  profile === "release" ? "release" : "debug",
  "rustok_storefront.wasm",
);
run("wasm-bindgen", [
  wasmInput,
  "--target",
  "web",
  "--out-dir",
  targetRoot,
  "--out-name",
  "rustok_storefront",
  "--no-typescript",
]);

mkdirSync(assetRoot, { recursive: true });
cpSync(
  path.join(repoRoot, "apps/storefront/public/assets/pages-inline-edit-bootstrap.js"),
  path.join(assetRoot, "pages-inline-edit-bootstrap.js"),
);
console.log(`[pages-inline-edit-client] assets written to ${targetRoot}`);
