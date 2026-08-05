import fs from "node:fs";

const read = (path) => fs.readFileSync(path, "utf8");
const mustContain = (text, needle, label) => {
  if (!text.includes(needle)) {
    throw new Error(`${label} is missing: ${needle}`);
  }
};
const mustNotContain = (text, needle, label) => {
  if (text.includes(needle)) {
    throw new Error(`${label} must not contain: ${needle}`);
  }
};

const route = read("crates/rustok-forum/src/services/topic_route.rs");
const owner = read("crates/rustok-forum/src/services/topic_owner.rs");
const facade = read("crates/rustok-forum/src/services/topic_facade.rs");
const contract = JSON.parse(
  read("crates/rustok-forum/contracts/forum-topic-slug-rename-owner.json"),
);
const test = read("crates/rustok-forum/tests/topic_slug_rename_sqlite.rs");
const plan = read("crates/rustok-forum/docs/implementation-plan.md");

if (contract.task !== "FORUM-24D") {
  throw new Error("FORUM-24D contract task drifted");
}
if (contract.transaction.single_commit !== true) {
  throw new Error("slug rename must remain atomic");
}
if (contract.resolution.deleted_topic !== "gone") {
  throw new Error("deleted self-target alias behavior must remain explicit");
}

mustContain(route, "pub struct RenameForumTopicSlugInput", "rename input");
mustContain(route, "pub struct ForumTopicSlugRenameResult", "rename result");
mustContain(
  route,
  "pub(crate) async fn rename_topic_slug_in_tx",
  "route owner rename helper",
);
mustContain(route, "lock_topic_route_for_rename_in_tx", "localized route lock");
mustContain(route, "record_redirect_alias_in_tx(", "immutable alias delegation");
mustContain(route, '"Topic slug changed"', "stable alias reason");
mustContain(
  route,
  "canonical_descriptor_with_locale_fallback",
  "merged rename fallback",
);
mustContain(
  route,
  "ForumTopicRouteDisposition::Gone",
  "deleted rename alias resolution",
);

mustContain(owner, "pub async fn rename_slug(", "topic owner command");
mustContain(
  owner,
  "ForumTopicRouteService::rename_topic_slug_in_tx(",
  "topic owner route delegation",
);
mustContain(
  owner,
  "publish_forum_topic_projection_in_tx(",
  "topic projection invalidation",
);
mustNotContain(owner, "INSERT INTO forum_topic_route_aliases", "topic owner");
mustContain(facade, "pub async fn rename_slug(", "public owner facade");

mustContain(test, "RenameForumTopicSlugInput", "SQLite rename input");
mustContain(test, "assert!(!replay.changed)", "exact replay assertion");
mustContain(test, '"Topic slug changed"', "alias reason assertion");
mustContain(
  test,
  "ForumTopicRouteDisposition::Gone",
  "deleted old route assertion",
);
mustContain(plan, "### Delivered in FORUM-24D", "canonical plan synchronization");

console.log("FORUM-24D topic slug rename owner source contract is present");
