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

const composition = read("apps/storefront/src/modules/forum_reactions_composition.rs");
const forumUi = read("crates/rustok-forum/storefront/src/ui/leptos.rs");
const forumCargo = read("crates/rustok-forum/Cargo.toml");
const forumStorefrontCargo = read("crates/rustok-forum/storefront/Cargo.toml");
const forumStorefrontLib = read("crates/rustok-forum/storefront/src/lib.rs");
const forumTransport = read("crates/rustok-forum/storefront/src/transport/mod.rs");
const contract = JSON.parse(
  read("apps/storefront/contracts/forum-reply-reactions-composition.json"),
);

requireContains(
  forumStorefrontLib,
  "fetch_storefront_reply_current_revision",
  "Forum storefront must expose the neutral reply current-revision fact for host extensions",
);
requireContains(
  forumTransport,
  "pub async fn fetch_storefront_reply_current_revision",
  "Reply host-extension fact must stay on the dual-path Forum storefront transport facade",
);
requireContains(
  composition,
  'query_value("reply")',
  "Reply reaction composition must use only the explicit Forum route reply selection",
);
requireContains(
  composition,
  "explicit_forum_reply_id(&route, topic_id.as_ref())",
  "Reply selection must require an explicit Forum topic context",
);
requireContains(
  composition,
  "fetch_storefront_reply_current_revision",
  "Host composition must consume the generic Forum reply revision facade",
);
requireContains(
  composition,
  'ReactionSubjectUiRef::new(\n                            "forum",\n                            "reply"',
  "Only the host render may construct the neutral Forum reply Reactions subject",
);
requireContains(
  composition,
  'data-storefront-composition="forum-reply-reactions"',
  "Selected reply composition must keep an explicit host marker",
);
requireContains(
  composition,
  "let Some(reply_id) = reply_id else",
  "Reply composition must fail closed when no explicit selected reply exists",
);
requireContains(
  composition,
  "<ReactionBar subject />",
  "Selected reply composition must reuse the module-owned Reactions presentation",
);

for (const forbidden of [
  "forumStorefrontReplyCurrentRevision",
  "GraphqlRequest",
  "reactionSnapshot",
  "applyReaction",
  "forum-reply-reactions-unavailable",
  "Reactions are temporarily unavailable for this reply.",
]) {
  requireAbsent(
    composition,
    forbidden,
    `Reply host composition must not duplicate Reactions transport/presentation: ${forbidden}`,
  );
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
requireAbsent(
  forumUi,
  "ReactionBar",
  "Forum reply cards must stay reaction-agnostic; host composition owns the cross-module mount",
);
requireAbsent(
  forumUi,
  "fetch_storefront_reply_current_revision",
  "Forum reply rendering must not fan out revision lookups per visible reply",
);

if (contract.owner !== "apps/storefront host composition") {
  throw new Error("Forum/Reactions reply composition must remain host-owned");
}
if (!contract.boundedness.includes("only the one explicitly selected reply is composed")) {
  throw new Error("Reply composition contract must remain selected-reply only");
}
if (!contract.boundedness.includes("the host never requests revisions for every visible reply")) {
  throw new Error("Reply composition contract must prohibit per-visible-reply revision fan-out");
}
if (!contract.not_claimed.includes("ReactionBars for every visible reply")) {
  throw new Error("Reply composition must not claim all-visible-reply controls");
}

console.log("forum selected reply Reactions storefront host composition ownership: ok");
