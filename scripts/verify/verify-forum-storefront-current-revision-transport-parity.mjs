#!/usr/bin/env node

import fs from "node:fs";

const read = (path) => fs.readFileSync(path, "utf8");
const requireContains = (text, needle, message) => {
  if (!text.includes(needle)) throw new Error(message);
};
const requireAbsent = (text, needle, message) => {
  if (text.includes(needle)) throw new Error(message);
};

const facade = read("crates/rustok-forum/storefront/src/transport/mod.rs");
const graphql = read("crates/rustok-forum/storefront/src/transport/revision_graphql_adapter.rs");
const native = read("crates/rustok-forum/storefront/src/transport/native_server_adapter_revision.rs");
const cargo = read("crates/rustok-forum/storefront/Cargo.toml");
const contract = JSON.parse(
  read("crates/rustok-forum/contracts/forum-storefront-current-revision-transport-parity.json"),
);

for (const fact of ["topic", "reply"]) {
  requireContains(
    facade,
    `fetch_storefront_${fact}_current_revision`,
    `Forum storefront facade must expose ${fact} current revision`,
  );
  requireContains(
    facade,
    `fetch_storefront_${fact}_current_revision_server`,
    `Forum storefront facade must select native ${fact} revision transport`,
  );
  requireContains(
    facade,
    `fetch_storefront_${fact}_current_revision_graphql`,
    `Forum storefront facade must select GraphQL ${fact} revision transport`,
  );
}

requireContains(
  graphql,
  "forumStorefrontTopicCurrentRevision",
  "GraphQL adapter must use the Forum topic owner-revision field",
);
requireContains(
  graphql,
  "forumStorefrontReplyCurrentRevision",
  "GraphQL adapter must use the Forum reply owner-revision field",
);
requireContains(
  native,
  "current_topic_revision(tenant.id, topic.id)",
  "Native topic path must read Forum RevisionService after visibility",
);
requireContains(
  native,
  "current_reply_revision(tenant.id, reply.id)",
  "Native reply path must read Forum RevisionService after visibility",
);
requireContains(
  native,
  "get_authenticated_storefront_visible_with_audience_context",
  "Native authenticated reply path must reuse exact Forum audience ownership",
);
requireContains(
  native,
  "get_public_storefront_visible_with_locale_fallback",
  "Native public reply path must reuse exact Forum audience ownership",
);
requireContains(
  native,
  "Some(&approved_statuses)",
  "Native reply revision must stay limited to approved storefront replies",
);

for (const source of [graphql, native]) {
  for (const forbidden of [
    "rustok_reactions",
    "ReactionSubject",
    "ReactionBar",
    "reactionSnapshot",
    "applyReaction",
  ]) {
    requireAbsent(
      source,
      forbidden,
      `Forum current-revision transport must not absorb Reactions functionality: ${forbidden}`,
    );
  }
}
for (const forbidden of ["rustok-reactions-storefront", "rustok-reactions ="]) {
  requireAbsent(
    cargo,
    forbidden,
    `Forum storefront transport parity must not add a Reactions dependency: ${forbidden}`,
  );
}
if (contract.owner !== "rustok-forum") {
  throw new Error("revision transport parity contract must remain Forum-owned");
}
if (!contract.not_claimed.includes("ReactionBar embedding")) {
  throw new Error("revision transport parity must not claim Reactions composition");
}

console.log("forum storefront current revision transport parity: ok");
