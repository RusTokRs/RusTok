import fs from "node:fs";

const read = (path) => fs.readFileSync(path, "utf8");

function requireTokens(path, tokens) {
  const source = read(path);
  for (const token of tokens) {
    if (!source.includes(token)) {
      throw new Error(`${path} is missing required token: ${token}`);
    }
  }
}

function rejectTokens(path, tokens) {
  const source = read(path);
  for (const token of tokens) {
    if (source.includes(token)) {
      throw new Error(`${path} contains forbidden token: ${token}`);
    }
  }
}

const routeOwner = "crates/rustok-forum/src/services/topic_route.rs";
const mergeOwner = "crates/rustok-forum/src/services/topic_merge.rs";
const contract = "crates/rustok-forum/contracts/forum-topic-merge-route-alias-owner.json";
const documentation = "crates/rustok-forum/docs/forum-24b-topic-merge-route-aliases.md";
const plan = "crates/rustok-forum/docs/implementation-plan.md";
const test = "crates/rustok-forum/tests/topic_merge_route_alias_sqlite.rs";

requireTokens(routeOwner, [
  "PLATFORM_FALLBACK_LOCALE",
  "record_merge_redirect_aliases_in_tx",
  "all source topic translations",
  "Forum topic merge target must provide at least one localized route",
  "record_redirect_alias_in_tx",
  "ORDER BY locale, id",
]);
requireTokens(mergeOwner, [
  "use super::topic_route::ForumTopicRouteService;",
  "ForumTopicRouteService::record_merge_redirect_aliases_in_tx",
  "input.source_topic_id",
  "target_topic_id",
  "&reason",
]);
requireTokens(contract, [
  '"task": "FORUM-24B"',
  '"same_transaction": true',
  '"receipt_schema_changed": false',
  '"event_schema_changed": false',
  '"platform_fallback_locale"',
  '"historical_merge_backfill_included": false',
]);
requireTokens(documentation, [
  "FORUM-24B topic merge route aliases",
  "same database transaction",
  "platform fallback locale",
  "lexicographically first available target locale",
  "No command above was run by the implementation agent",
]);
requireTokens(plan, [
  "### Delivered in FORUM-24B",
  "record_merge_redirect_aliases_in_tx",
  "Topic rename aliases and deletion tombstones remain",
]);
requireTokens(test, [
  "merge_persists_one_redirect_alias_and_replay_does_not_duplicate_it",
  "ForumTopicRouteDisposition::Redirect",
  "assert_eq!(count, 1)",
  "TopicService::new",
]);
rejectTokens(mergeOwner, [
  "INSERT INTO forum_topic_route_aliases",
  "UPDATE forum_topic_route_aliases",
  "DELETE FROM forum_topic_route_aliases",
]);

console.log("FORUM-24B topic merge route alias owner source contract is present");
