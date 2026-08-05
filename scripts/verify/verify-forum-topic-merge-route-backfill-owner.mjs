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

const owner = read("crates/rustok-forum/src/services/topic_route_backfill.rs");
const route = read("crates/rustok-forum/src/services/topic_route.rs");
const migration = read(
  "crates/rustok-forum/src/migrations/m20260805_000024_add_forum_topic_route_aliases.rs",
);
const contract = JSON.parse(
  read("crates/rustok-forum/contracts/forum-topic-merge-route-backfill-owner.json"),
);
const test = read("crates/rustok-forum/tests/topic_merge_route_backfill_sqlite.rs");
const docs = read("crates/rustok-forum/docs/forum-24e-topic-merge-route-backfill.md");
const readme = read("crates/rustok-forum/docs/README.md");

if (contract.task !== "FORUM-24E") {
  throw new Error("FORUM-24E contract task drifted");
}
if (contract.input.maximum_limit !== 100) {
  throw new Error("merge route backfill limit must remain bounded at 100");
}
if (contract.write_policy.single_page_transaction !== true) {
  throw new Error("one merge route backfill page must remain atomic");
}
if (contract.write_policy.exact_page_replay_idempotent !== true) {
  throw new Error("merge route backfill replay must remain idempotent");
}

mustContain(
  owner,
  "pub struct ForumTopicMergeRouteBackfillCursor",
  "typed merge receipt cursor",
);
mustContain(
  owner,
  "pub struct BackfillForumTopicMergeRouteAliasesInput",
  "backfill input",
);
mustContain(
  owner,
  "pub struct ForumTopicMergeRouteBackfillResult",
  "backfill result",
);
mustContain(
  owner,
  "pub async fn backfill_merge_route_aliases(",
  "backfill owner command",
);
mustContain(owner, "Action::Manage", "manager authorization");
mustContain(
  owner,
  "MAX_FORUM_TOPIC_MERGE_ROUTE_BACKFILL_OPERATIONS: u32 = 100",
  "bounded operation page",
);
mustContain(
  owner,
  "ForumTopicRouteService::record_merge_redirect_aliases_in_tx(",
  "canonical route owner delegation",
);
mustContain(owner, ".checked_add(operation_alias_count)", "checked alias count");
mustContain(owner, "txn.commit().await?", "single page commit");
mustNotContain(owner, "INSERT INTO forum_topic_route_aliases", "backfill owner");
mustNotContain(owner, "UPDATE forum_topic_merge_operations", "backfill owner");

mustContain(
  route,
  "ON CONFLICT (tenant_id, locale, short_id, slug) DO NOTHING",
  "exact replay insertion guard",
);
mustContain(
  migration,
  "idx_forum_topic_merge_operations_route_backfill",
  "merge receipt cursor index",
);
mustContain(
  migration,
  "tenant_id, merged_at, operation_id",
  "merge receipt cursor index order",
);

mustContain(
  test,
  "historical_merge_aliases_backfill_in_bounded_replay_safe_pages",
  "SQLite historical backfill coverage",
);
mustContain(test, "assert_eq!(replay, first)", "exact page replay assertion");
mustContain(test, "assert_eq!(alias_count(&db, tenant_id).await?, 2)", "complete alias assertion");
mustContain(docs, "# FORUM-24E historical topic-merge route backfill", "task documentation");
mustContain(
  readme,
  "[FORUM-24E historical merge route backfill](./forum-24e-topic-merge-route-backfill.md)",
  "documentation index",
);

console.log("FORUM-24E historical merge route backfill source contract is present");
