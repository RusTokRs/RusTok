#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");

const checks = [
  {
    file: "apps/next-admin/src/features/blog/components/post-form.tsx",
    forbidden: ["Legacy content warning", "Legacy markdown detected"],
  },
  {
    file: "apps/next-admin/src/features/blog/components/page-builder.tsx",
    forbidden: ["hasLegacyBlocks", "Legacy block payload"],
  },
  {
    file: "apps/next-admin/src/features/blog/api/posts.ts",
    forbidden: ["Example (legacy):"],
  },
  {
    file: "crates/rustok-pages/admin/src/lib.rs",
    forbidden: [
      "pages.surface.tree.legacyBlocks",
      "pages.surface.tree.noLegacyBlocks",
      "pages.compat.legacyBlocks",
      "Legacy blocks",
      "legacy blocks",
    ],
  },
  {
    file: "crates/rustok-pages/admin/locales/en.json",
    forbidden: [
      '"pages.surface.tree.legacyBlocks"',
      '"pages.surface.tree.noLegacyBlocks"',
      '"pages.compat.legacyBlocks"',
      "Legacy blocks",
      "legacy blocks",
    ],
  },
  {
    file: "crates/rustok-pages/admin/locales/ru.json",
    forbidden: [
      '"pages.surface.tree.legacyBlocks"',
      '"pages.surface.tree.noLegacyBlocks"',
      '"pages.compat.legacyBlocks"',
      "Legacy blocks",
      "legacy blocks",
      "existing blocks",
      "Existing blocks",
    ],
  },
];

function fail(message) {
  console.error("[verify-page-builder-terminology] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
}

for (const check of checks) {
  const fullPath = path.join(repoRoot, check.file);
  if (!fs.existsSync(fullPath)) {
    fail(`missing file: ${check.file}`);
  }

  const content = fs.readFileSync(fullPath, "utf8");
  for (const token of check.forbidden) {
    if (content.includes(token)) {
      fail(`${check.file} contains forbidden token '${token}'`);
    }
  }
}

console.log("[verify-page-builder-terminology] PASS");
