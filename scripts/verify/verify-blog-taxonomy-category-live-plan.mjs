#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(scriptPath), "../..");

function read(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) {
    throw new Error(`${label}: missing marker ${JSON.stringify(marker)}`);
  }
}

function rejectMarker(source, marker, label) {
  if (source.includes(marker)) {
    throw new Error(`${label}: stale marker must remain absent: ${JSON.stringify(marker)}`);
  }
}

function main() {
  const planPath = "crates/rustok-blog/docs/implementation-plan.md";
  const plan = read(planPath);

  for (const marker of [
    "`rustok-blog` owns localized posts, Blog Category membership/settings,",
    "Category localized identity, route history, and hierarchy are Taxonomy-owned",
    "### Blog Category Taxonomy ownership",
    "The former Blog-owned Category Translation pilot is retired.",
    "Category copy uses the registered `taxonomy/term` provider and its Taxonomy-owned",
    "Do not reintroduce a second `blog/category`\nprovider or direct Blog Category localized storage.",
  ]) {
    requireMarker(plan, marker, planPath);
  }

  for (const stale of [
    "### Blog category Translation target pilot",
    "`BlogCategoryTranslationTargetProvider` is registered by the server",
    "The focused SQLite suite in `src/translation_target_tests.rs`",
    "This is a registered pilot, not a production-enablement claim.",
    "`blog_translation_changes` owner journal",
  ]) {
    rejectMarker(plan, stale, planPath);
  }

  console.log("OK  Blog live plan keeps canonical Category Translation ownership in Taxonomy");
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
