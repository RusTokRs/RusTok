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
const audience = read("crates/rustok-forum/src/services/reply_audience_read.rs");
const graphql = read("crates/rustok-forum/src/graphql/reply_audience_query.rs");
const storefrontCargo = read("crates/rustok-forum/storefront/Cargo.toml");
const contract = JSON.parse(
  read("crates/rustok-forum/contracts/forum-storefront-reply-current-revision.json"),
);

requireContains(
  revision,
  "pub async fn current_reply_revision",
  "Forum must expose one generic current reply owner revision fact",
);
requireContains(
  audience,
  "get_authenticated_storefront_visible_with_audience_context",
  "Authenticated exact reply revision must reuse Forum storefront audience ownership",
);
requireContains(
  audience,
  "is_topic_visible",
  "Exact reply visibility must remain derived from the parent topic owner",
);
requireContains(
  graphql,
  "forum_storefront_reply_current_revision",
  "Storefront GraphQL must expose the generic Forum reply current revision fact",
);
requireContains(
  graphql,
  "ForumReplyReadOperation::SelectedReply",
  "Authenticated revision reads must use the exact selected-reply caller context",
);
requireContains(
  graphql,
  "Some(&PUBLIC_REPLY_STATUSES)",
  "Storefront reply revision must be limited to approved replies",
);
requireContains(
  graphql,
  "current_reply_revision(tenant_id, reply.id)",
  "GraphQL must read the Forum-owned revision only after visibility succeeds",
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
    `Forum reply revision GraphQL must not absorb Reactions functionality: ${forbidden}`,
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
  throw new Error("reply current revision contract must remain Forum-owned");
}
if (!contract.not_claimed.includes("ReactionBar embedding")) {
  throw new Error("reply revision contract must not claim Reactions UI composition");
}

console.log("forum storefront reply current revision ownership: ok");
