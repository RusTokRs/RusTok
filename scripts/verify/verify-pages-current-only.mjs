#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

const forbiddenFiles = [
  "crates/rustok-pages/src/entities/page_block.rs",
  "crates/rustok-pages/src/dto/block.rs",
  "crates/rustok-pages/src/services/block.rs",
  "crates/rustok-pages/src/services/page/update.rs",
  "apps/next-admin/packages/blog/src/api/pages.ts",
  "apps/next-admin/packages/blog/src/components/page-builder.tsx",
  "apps/next-admin/src/app/dashboard/blog/page-builder/page.tsx",
];

const scannedRoots = [
  "crates/rustok-pages/src",
  "crates/rustok-pages/admin/src",
  "crates/rustok-pages/storefront/src",
  "apps/next-admin/packages/blog/src",
  "apps/next-admin/src/app/dashboard/blog",
];

const forbiddenPatterns = [
  [/(^|[^A-Za-z0-9_])PageBlock([^A-Za-z0-9_]|$)/, "PageBlock runtime model"],
  [/(^|[^A-Za-z0-9_])BlockService([^A-Za-z0-9_]|$)/, "BlockService runtime service"],
  [/(^|[^A-Za-z0-9_])page_blocks([^A-Za-z0-9_]|$)/, "page_blocks database table"],
  [/addPageBlock|updatePageBlock|deletePageBlock|reorderPageBlocks/, "block mutation API"],
  [/(^|[^A-Za-z0-9_])UpdatePageInput([^A-Za-z0-9_]|$)/, "universal page update DTO"],
  [/(^|[^A-Za-z0-9_])UpdateGqlPageInput([^A-Za-z0-9_]|$)/, "universal GraphQL page update DTO"],
  [/\bupdatePage\s*\(/, "universal GraphQL page mutation"],
  [/\.update\(\s*tenant_id[\s\S]{0,160}page_id/, "universal PageService update"],
  [/frames\s*:\s*\[\s*\{\s*component/, "frame component-tree mirror"],
];

for (const relativePath of forbiddenFiles) {
  if (existsSync(path.join(repoRoot, relativePath))) {
    failures.push(`${relativePath}: obsolete Pages file must stay deleted`);
  }
}

for (const root of scannedRoots) {
  const absoluteRoot = path.join(repoRoot, root);
  if (!existsSync(absoluteRoot)) continue;
  for (const filePath of walk(absoluteRoot)) {
    if (!/\.(rs|ts|tsx|js|jsx|mjs)$/.test(filePath)) continue;
    const source = readFileSync(filePath, "utf8");
    const relativePath = path.relative(repoRoot, filePath);
    for (const [pattern, label] of forbiddenPatterns) {
      if (pattern.test(source)) failures.push(`${relativePath}: forbidden ${label}`);
    }
  }
}

if (failures.length > 0) {
  console.error("Pages current-only verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Pages current-only verification passed");

function* walk(directory) {
  for (const entry of readdirSync(directory)) {
    const filePath = path.join(directory, entry);
    const stats = statSync(filePath);
    if (stats.isDirectory()) yield* walk(filePath);
    else if (stats.isFile()) yield filePath;
  }
}
