#!/usr/bin/env node

import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function requireContains(text, needle, message) {
  if (!text.includes(needle)) throw new Error(message);
}

function requireAbsent(text, needle, message) {
  if (text.includes(needle)) throw new Error(message);
}

const testPath = "tests/e2e-rust/tests/leptos_storefront_forum_reactions.rs";
const contractPath = "crates/rustok-forum/contracts/forum-reactions-storefront-browser-evidence.json";
const readmePath = "tests/e2e-rust/README.md";
const testSource = read(testPath);
const contract = JSON.parse(read(contractPath));
const readme = read(readmePath);
const forumCargo = read("crates/rustok-forum/Cargo.toml");
const forumStorefrontCargo = read("crates/rustok-forum/storefront/Cargo.toml");

for (const marker of [
  "RUSTOK_FORUM_TOPIC_REACTIONS_E2E_URL",
  "RUSTOK_FORUM_REPLY_REACTIONS_E2E_URL",
  "data-storefront-composition='forum-topic-reactions'",
  "data-storefront-composition='forum-reply-reactions'",
  ":not(:has([data-storefront-composition='forum-reply-reactions']))",
  ":not(:has([data-storefront-composition='forum-topic-reactions']))",
  "PLAYWRIGHT_CHROMIUM_EXECUTABLE",
]) {
  requireContains(testSource, marker, `browser harness missing required marker: ${marker}`);
}

requireContains(
  testSource,
  "status < 400",
  "browser harness must reject failed storefront navigation",
);
requireContains(
  testSource,
  "topic evidence URL must not select a reply",
  "topic evidence must reject an accidental reply-selection fixture",
);
requireContains(
  testSource,
  "reply evidence URL must carry the canonical reply query selection",
  "reply evidence must require an explicit reply selection",
);

for (const forbidden of [
  "reactionSnapshot",
  "applyReaction",
  "GraphqlRequest",
  "forumStorefrontReplyCurrentRevision",
  "forumStorefrontTopicCurrentRevision",
]) {
  requireAbsent(
    testSource,
    forbidden,
    `browser evidence must observe the mounted UI instead of reimplementing transport: ${forbidden}`,
  );
}

if (contract.status !== "source_ready_maintainer_execution_pending") {
  throw new Error("browser evidence contract must not claim execution");
}
if (contract.runner !== testPath) {
  throw new Error("browser evidence contract must point to the Rust Playwright harness");
}
if (!contract.not_claimed.includes("browser execution")) {
  throw new Error("browser evidence contract must keep execution explicitly pending");
}
if (!contract.boundaries.includes("production Forum and Reactions source is unchanged")) {
  throw new Error("browser evidence contract must remain evidence-only");
}

for (const envName of [
  "RUSTOK_FORUM_TOPIC_REACTIONS_E2E_URL",
  "RUSTOK_FORUM_REPLY_REACTIONS_E2E_URL",
]) {
  requireContains(readme, envName, `E2E README must document ${envName}`);
}

for (const cargo of [forumCargo, forumStorefrontCargo]) {
  requireAbsent(
    cargo,
    "rustok-reactions-storefront",
    "Forum owner/storefront packages must not depend on Reactions presentation",
  );
  requireAbsent(
    cargo,
    "rustok-reactions =",
    "Forum owner/storefront packages must not depend on the Reactions owner",
  );
}

console.log("forum Reactions storefront browser evidence source: ok");
