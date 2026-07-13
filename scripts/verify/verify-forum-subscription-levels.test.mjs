#!/usr/bin/env node

import { mkdtempSync, mkdirSync, writeFileSync, cpSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

const root = mkdtempSync(join(tmpdir(), "forum-subscription-verifier-"));
cpSync("crates/rustok-forum", join(root, "crates/rustok-forum"), { recursive: true });
mkdirSync(join(root, "scripts/verify"), { recursive: true });
cpSync(
  "scripts/verify/verify-forum-subscription-levels.mjs",
  join(root, "scripts/verify/verify-forum-subscription-levels.mjs"),
);
writeFileSync(
  join(root, "crates/rustok-forum/src/subscription.rs"),
  "pub enum ForumSubscriptionLevel { Watching, Tracking, Normal }\n",
);
const result = spawnSync(
  process.execPath,
  ["scripts/verify/verify-forum-subscription-levels.mjs", "--static-only"],
  { cwd: root, encoding: "utf8" },
);
if (result.status === 0) {
  console.error("verifier fixture unexpectedly passed after removing Muted");
  process.exit(1);
}
if (!`${result.stdout}${result.stderr}`.includes("Muted")) {
  console.error("verifier fixture did not report the missing Muted contract");
  process.exit(1);
}
console.log("forum subscription-level verifier fixture passed");
