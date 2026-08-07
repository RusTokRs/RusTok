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

const revision = read("crates/rustok-forum/src/services/revision.rs");
const graphql = read("crates/rustok-forum/src/graphql/storefront_audience_topic.rs");
const storefrontCargo = read("crates/rustok-forum/storefront/Cargo.toml");
const contract = JSON.parse(
  read("crates/rustok-forum/contracts/forum-storefront-topic-current-revision.json"),
);

requireContains(
  revision,
  "pub async fn current_topic_revision",
  "Forum must expose one generic current topic owner revision fact",
);
requireContains(
  revision,
  "forum_topic_revision::Column::Id",
  "Forum owner revision must come from captured Forum topic revision history",
);
requireContains(
  revision,
  "revision.checked_add(1)",
  "Forum current revision must remain latest captured revision id plus one",
);
requireContains(
  graphql,
  "forum_storefront_topic_current_revision",
  "Storefront GraphQL must expose the generic Forum current revision fact",
);
requireContains(
  graphql,
  "load_storefront_audience_topic",
  "Revision exposure must reuse the selected-topic audience path",
);
requireContains(
  graphql,
  "current_topic_revision(tenant_id, topic.id)",
  "GraphQL must read the Forum-owned revision service after visibility succeeds",
);

for (const forbidden of [
  "rustok_reactions",
  "ReactionSubject",
  "ReactionBar",
  "applyReaction",
  "reactionSnapshot",
]) {
  requireAbsent(
    graphql,
    forbidden,
    `Forum revision GraphQL must not absorb Reactions functionality: ${forbidden}`,
  );
}
for (const forbidden of ["rustok-reactions-storefront", "rustok-reactions ="]) {
  requireAbsent(
    storefrontCargo,
    forbidden,
    `Forum storefront must not depend on optional Reactions presentation/owner: ${forbidden}`,
  );
}
if (contract.owner !== "rustok-forum") {
  throw new Error("current revision contract must remain Forum-owned");
}
if (!contract.not_claimed.includes("ReactionBar embedding")) {
  throw new Error("contract must not claim Reactions UI composition");
}

console.log("forum storefront topic current revision ownership: ok");
