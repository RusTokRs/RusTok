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

const owner = read("crates/rustok-forum/src/services/topic_owner.rs");
const route = read("crates/rustok-forum/src/services/topic_route.rs");
const contract = JSON.parse(
  read("crates/rustok-forum/contracts/forum-topic-delete-route-tombstone-owner.json"),
);
const test = read("crates/rustok-forum/tests/topic_delete_route_tombstone_sqlite.rs");
const mergeRegression = read("crates/rustok-forum/tests/topic_merge_route_alias_sqlite.rs");
const plan = read("crates/rustok-forum/docs/implementation-plan.md");

if (contract.task !== "FORUM-24C") {
  throw new Error("FORUM-24C contract task drifted");
}
if (contract.owner.same_transaction !== true) {
  throw new Error("delete tombstones must remain in the owner transaction");
}
if (contract.existing_history.merged_source_delete_keeps_redirect !== true) {
  throw new Error("merged-source redirect preservation must remain explicit");
}

mustContain(
  owner,
  "ForumTopicRouteService::record_delete_tombstones_in_tx(",
  "topic delete owner delegation",
);
mustContain(owner, "FORUM_TOPIC_DELETED_ROUTE_REASON", "stable delete reason");
mustContain(
  owner,
  "delete_attached_localized_values",
  "existing localized cleanup",
);
if (
  owner.indexOf("record_delete_tombstones_in_tx") >
  owner.indexOf("delete_attached_localized_values")
) {
  throw new Error("route tombstones must be recorded before localized cleanup");
}
mustNotContain(owner, "INSERT INTO forum_topic_route_aliases", "topic delete owner");

mustContain(
  route,
  "pub(crate) async fn record_delete_tombstones_in_tx",
  "route owner batch helper",
);
mustContain(route, "StoredRouteDisposition::Redirect => {}", "redirect preservation");
mustContain(route, "StoredRouteDisposition::Gone", "gone idempotency");
mustContain(route, "Self::record_gone_alias_in_tx(", "gone insertion delegation");
mustContain(route, "load_topic_translation_routes_in_tx", "localized route source");

mustContain(test, "ForumTopicRouteDisposition::Gone", "gone resolution assertion");
mustContain(test, '"Topic deleted"', "fixed reason assertion");
mustContain(test, "target_topic_id", "null target assertion");
mustContain(
  mergeRegression,
  ".delete(tenant_id, source_topic_id, admin)",
  "merged-source delete regression",
);
mustContain(
  mergeRegression,
  "ForumTopicRouteDisposition::Redirect",
  "merged-source redirect regression",
);
mustContain(plan, "### Delivered in FORUM-24C", "canonical plan synchronization");

console.log("FORUM-24C delete route tombstone source contract is present");
