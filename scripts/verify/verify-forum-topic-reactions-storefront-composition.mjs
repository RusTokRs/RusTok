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

const appCargo = read("apps/storefront/Cargo.toml");
const appBuild = read("apps/storefront/build.rs");
const appModules = read("apps/storefront/src/modules/mod.rs");
const composition = read("apps/storefront/src/modules/forum_reactions_composition.rs");
const forumCargo = read("crates/rustok-forum/Cargo.toml");
const forumStorefrontCargo = read("crates/rustok-forum/storefront/Cargo.toml");
const forumStorefrontLib = read("crates/rustok-forum/storefront/src/lib.rs");
const forumTransport = read("crates/rustok-forum/storefront/src/transport/mod.rs");

requireContains(
  appCargo,
  'rustok-reactions-storefront = { path = "../../crates/rustok-reactions-storefront", default-features = false, optional = true }',
  "Storefront host must own the optional Reactions presentation dependency",
);
requireContains(
  appCargo,
  '"rustok-reactions-storefront/ssr"',
  "Storefront SSR composition must enable the Reactions presentation SSR feature",
);
requireContains(
  appBuild,
  'entry.slug == "forum"',
  "Generated Forum mounting must route through the storefront host composition",
);
requireContains(
  appBuild,
  'crate::modules::ForumStorefrontComposition',
  "Generated Forum mounting must use the host-owned Forum composition component",
);
requireContains(
  appModules,
  "forum_reactions_composition",
  "Storefront host must register the Forum/Reactions composition module",
);
requireContains(
  forumStorefrontLib,
  "fetch_storefront_topic_current_revision",
  "Forum storefront must expose the neutral current-revision fact for host extensions",
);
requireContains(
  forumTransport,
  "pub async fn fetch_storefront_topic_current_revision",
  "Host extension fact must stay on the dual-path Forum storefront transport facade",
);
requireContains(
  composition,
  'use_is_module_enabled("reactions")',
  "Forum topic reaction composition must remain disabled when Reactions is disabled",
);
requireContains(
  composition,
  'route_segment.as_deref() != Some(FORUM_ROUTE_SEGMENT)',
  "Topic reaction composition must run only on the Forum module route, never the shared home slot",
);
requireContains(
  composition,
  'query_value("topic")',
  "Topic reaction composition must consume the shared route-query helper for an explicit topic",
);
requireContains(
  composition,
  "fetch_storefront_topic_current_revision",
  "Host composition must consume the generic Forum storefront revision facade",
);
requireContains(
  composition,
  'ReactionSubjectUiRef::new("forum", "topic"',
  "Only the storefront host may combine Forum identity/revision with the neutral Reactions subject UI ref",
);
requireContains(
  composition,
  "<ReactionBar subject />",
  "Host composition must render the separate module-owned Reactions control",
);
requireContains(
  composition,
  "Ok(None) | Err(_) => ().into_any()",
  "Optional Reactions composition failures must degrade without replacing Forum-owned UI",
);

for (const forbidden of [
  "forumStorefrontTopicCurrentRevision",
  "GraphqlRequest",
  "reactionSnapshot",
  "applyReaction",
  "forum-topic-reactions-unavailable",
  "Reactions are temporarily unavailable for this topic.",
]) {
  requireAbsent(
    composition,
    forbidden,
    `Host composition must use owner facades and must not duplicate Reactions transport/presentation: ${forbidden}`,
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

console.log("forum topic Reactions storefront host composition ownership: ok");
