#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../../..");
const fixturePath = path.join(
  repoRoot,
  "apps/next-frontend/contracts/seo/runtime-parity-fixtures.json",
);
const backendPath = path.join(repoRoot, "crates/rustok-forum/src/seo_targets.rs");

const fixtures = JSON.parse(readFileSync(fixturePath, "utf8"));
const backend = readFileSync(backendPath, "utf8");
const failures = [];

function assert(condition, message) {
  if (!condition) failures.push(message);
}

const forumRoute = (fixtures.routeOwnership ?? []).find(
  (entry) => entry.targetKind === "forum_topic",
);
const categoryId = "11111111-1111-4111-8111-111111111111";
const topicId = "22222222-2222-4222-8222-222222222222";
const expectedPatterns = [
  "/modules/forum?category=:category_id",
  "/modules/forum?topic=:topic_id",
  "/modules/forum?category=:category_id&topic=:topic_id",
];

assert(forumRoute, "Forum route ownership entry is missing");
assert(
  JSON.stringify(forumRoute?.routePatterns) === JSON.stringify(expectedPatterns),
  "Forum fixture must advertise only the accepted ID-based compatibility routes",
);
assert(
  forumRoute?.nextSmokeRoute?.query?.category === categoryId &&
    forumRoute?.nextSmokeRoute?.query?.topic === topicId,
  "Forum fixture must use deterministic UUID category/topic values",
);
assert(
  forumRoute?.rustStorefrontRoute ===
    `/modules/forum?category=${categoryId}&topic=${topicId}`,
  "Forum Rust storefront fixture must use the canonical category+topic ID route",
);
assert(
  !(forumRoute?.routePatterns ?? []).some((pattern) => pattern.includes(":slug")),
  "Forum fixture must not advertise slug routes before FORUM-24",
);
assert(
  !JSON.stringify(forumRoute).includes("welcome"),
  "Forum fixture must not use a slug-like smoke value",
);
for (const marker of [
  "forum_route_contract_remains_id_based",
  'parse_forum_route("/modules/forum?topic=welcome")',
  'parse_forum_route("/forum/welcome")',
  'format!("/modules/forum?topic={topic_id}")',
  'format!("/en/modules/forum?category={category_id}")',
]) {
  assert(
    backend.includes(marker),
    `Forum backend ID-route contract marker is missing: ${marker}`,
  );
}

if (failures.length > 0) {
  console.error("forum route fixture verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum route fixture verification passed");
