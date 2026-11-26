#!/usr/bin/env node

// Guards cursor paging on the forum Leptos storefront: the owner returns `nextCursor` for the topic
// and reply lists, so the storefront must be able to append a page, must gate its controls on the
// cursor (never on a fabricated total), must report how much it actually loaded, and must keep the
// thread timeline anchored to real in-document targets. See the paging section of
// crates/modules/rustok-forum/README.md and DECISIONS/2026-10-08-owner-routes-and-batched-conversion.md.

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

const storefront = "crates/modules/rustok-forum/storefront";
const paths = {
  model: `${storefront}/src/model.rs`,
  transport: `${storefront}/src/transport/mod.rs`,
  pages: `${storefront}/src/transport/native_server_adapter_pages.rs`,
  graphql: `${storefront}/src/transport/graphql_adapter.rs`,
  core: `${storefront}/src/core.rs`,
  ui: `${storefront}/src/ui/leptos.rs`,
  feed: `${storefront}/src/ui/topic_feed.rs`,
  timeline: `${storefront}/src/ui/timeline.rs`,
  i18n: `${storefront}/src/i18n.rs`,
  catalogEn: `${storefront}/locales/en.ftl`,
  catalogRu: `${storefront}/locales/ru.ftl`,
  catalogAr: `${storefront}/locales/ar.ftl`,
};

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireText(content, marker, label) {
  if (!content.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function forbidText(content, marker, label) {
  if (content.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

/** Returns the source of one `pub async fn` so assertions cannot pass on unrelated code. */
function functionBody(content, signature) {
  const start = content.indexOf(signature);
  if (start < 0) {
    failures.push(`missing function signature ${signature}`);
    return "";
  }
  const next = content.indexOf("\npub async fn", start + signature.length);
  return content.slice(start, next < 0 ? content.length : next);
}

/** Returns one GraphQL query constant so cursor plumbing is asserted where it is declared. */
function queryConst(content, name) {
  const start = content.indexOf(`const ${name}: &str = `);
  if (start < 0) {
    failures.push(`missing query constant ${name}`);
    return "";
  }
  const end = content.indexOf(";", start);
  return content.slice(start, end < 0 ? content.length : end);
}

// A page can only be extended by a model that owns the cursor, and a repeated keyset read must not
// duplicate a card: the append is id-deduplicated for both lists.
const model = read(paths.model);
for (const page of ["ForumTopicPage", "ForumReplyPage"]) {
  requireText(model, `impl ${page} {`, `${page} cursor contract`);
  requireText(model, "pub fn has_more(&self) -> bool {", `${page} has_more`);
  requireText(
    model,
    "pub fn append_page(&mut self, page: Self) {",
    `${page} append_page`
  );
}
requireText(
  model,
  "if !self.items.iter().any(|seen| seen.id == item.id) {",
  "append_page must skip items the surface already renders"
);

// One façade per list, selected by build profile, and paging is a canonical module (the transport
// still stitches some legacy files with include!, new ones must not).
const transport = read(paths.transport);
requireText(transport, "pub async fn fetch_storefront_forum_topic_page(", "topic page façade");
requireText(transport, "pub async fn fetch_storefront_forum_reply_page(", "reply page façade");
requireText(
  transport,
  "native_server_adapter_pages::fetch_storefront_forum_topic_page_server(",
  "topic page native transport"
);
requireText(
  transport,
  "graphql_adapter::fetch_storefront_forum_topic_page_graphql(",
  "topic page GraphQL transport"
);
requireText(
  transport,
  "native_server_adapter_pages::fetch_storefront_forum_reply_page_server(",
  "reply page native transport"
);
requireText(
  transport,
  "graphql_adapter::fetch_storefront_forum_reply_page_graphql(",
  "reply page GraphQL transport"
);
requireText(transport, "mod native_server_adapter_pages;", "paging transport module declaration");
forbidText(
  transport,
  'include!("native_server_adapter_pages.rs");',
  "paging transport must be a canonical module"
);

// The native endpoints page on the variant the first page came from and take their page size from
// owner settings. A principal that loses the read-state variant gets an error, never a silently
// reordered list, and no transport is swapped in on failure.
const pages = read(paths.pages);
requireText(pages, 'endpoint = "forum/storefront-topic-page"', "topic page server endpoint");
requireText(pages, 'endpoint = "forum/storefront-reply-page"', "reply page server endpoint");
requireText(
  pages,
  '"Forum read state is unavailable for this principal"',
  "topic page fail-closed read-state guard"
);
forbidText(pages, "fetch_storefront_forum_topic_page_graphql", "no cross-transport fallback");
requireText(pages, "default_topics_per_page(tenant.id)", "topic page size from owner settings");
requireText(pages, "default_replies_per_page(tenant.id)", "reply page size from owner settings");
requireText(pages, "after: Some(after),", "native endpoints pass the owner cursor");
forbidText(pages, "per_page: Some(", "native endpoints must not fix a page size");

const graphql = read(paths.graphql);
const topicQuery = functionBody(
  graphql,
  "pub async fn fetch_storefront_forum_topic_page_graphql("
);
requireText(topicQuery, "STOREFRONT_FORUM_UNREAD_TOPICS_QUERY", "topic page unread query");
requireText(topicQuery, "STOREFRONT_FORUM_AUDIENCE_TOPICS_QUERY", "topic page audience query");
requireText(topicQuery, "after: Some(after)", "topic page GraphQL cursor argument");
requireText(topicQuery, "limit: None", "topic page leaves the page size to the owner");
requireText(topicQuery, "per_page: None", "topic list leaves the page size to the owner");
forbidText(topicQuery, "after: None", "paging must not restart at the first page");
const replyQuery = functionBody(
  graphql,
  "pub async fn fetch_storefront_forum_reply_page_graphql("
);
requireText(replyQuery, "STOREFRONT_FORUM_REPLIES_QUERY", "reply page query");
requireText(replyQuery, "after: Some(after)", "reply page GraphQL cursor argument");
requireText(replyQuery, "per_page: None", "reply page leaves the page size to the owner");
forbidText(replyQuery, "after: None", "reply paging must not restart at the first page");
for (const name of [
  "STOREFRONT_FORUM_UNREAD_TOPICS_QUERY",
  "STOREFRONT_FORUM_AUDIENCE_TOPICS_QUERY",
  "STOREFRONT_FORUM_REPLIES_QUERY",
]) {
  const query = queryConst(graphql, name);
  requireText(query, "$after: String", `${name} declares the cursor variable`);
  requireText(query, "nextCursor", `${name} selects the next cursor`);
}

// The widget owns per-list paging state, appends pages through the model, and surfaces a failed
// page instead of leaving the list silently short. The header count stays owner-authored.
const ui = read(paths.ui);
requireText(ui, "let load_more_topics = Callback::new(move |_: ()| {", "topic page loader");
requireText(ui, "let load_more_replies = Callback::new(move |_: ()| {", "reply page loader");
requireText(
  ui,
  "topics_page.update(|state| state.append_page(page))",
  "topic pages append through the model"
);
requireText(
  ui,
  "replies_page.update(|state| state.append_page(page))",
  "reply pages append through the model"
);
requireText(
  ui,
  "set_topics_error.set(Some(error.to_string()))",
  "topic page failures are surfaced"
);
requireText(
  ui,
  "set_replies_error.set(Some(error.to_string()))",
  "reply page failures are surfaced"
);
// The initial result must retain the key used by its fetch, not adopt whatever
// key happens to be current when Suspense eventually mounts its subtree.
requireText(ui, "let (category_id, topic_id, locale, _, selection) = requested_key.clone();", "initial fetch captures request identity");
requireText(ui, ".map(|data| (requested_key, data))", "initial fetch returns request identity");
requireText(ui, "Ok((page_key, data))", "initial response consumes request identity");
requireText(ui, "if forum_page_is_current(resource_key, &page_key)", "read-state availability ignores stale initial results");
requireText(ui, "page_key: ForumResourceKey", "showcase receives the fetched identity");
requireText(ui, "let topic_page_key = page_key;", "paging uses the fetched identity");
forbidText(ui, "let topic_page_key = resource_key.get_untracked();", "a retained result cannot adopt a new request identity");
// A retained Suspense subtree is not necessarily disposed when its resource key
// changes. Both loaders must fence dispatch and completion against the entire key.
for (const [loader, end] of [
  ["topics", "    let load_more_replies ="],
  ["replies", "    view! {"],
]) {
  const start = ui.indexOf(`    let load_more_${loader} =`);
  const finish = ui.indexOf(end, start);
  const body = ui.slice(start, finish);
  requireText(body, "forum_page_is_current(resource_key, &requested_key)", `${loader} response scope fence`);
  requireText(body, `${loader}_page.try_get_untracked().is_none()`, `${loader} disposal fence`);
  requireText(body, `forum_page_is_current(resource_key, &${loader === "topics" ? "topic" : "reply"}_page_key)`, `${loader} dispatch scope fence`);
}
requireText(ui, "on_load_more=load_more_topics", "feed receives the loader");
requireText(ui, "on_load_more=load_more_replies", "thread receives the loader");
requireText(ui, "paging_busy=topics_busy", "feed busy state");
requireText(ui, "paging_error=replies_error", "thread error state");
requireText(
  ui,
  "total=selected_category_topic_count(",
  "feed count stays owner-authored per category"
);
requireText(ui, 'id="forum-thread-top"', "thread top anchor target");
requireText(ui, 'id="forum-thread-end"', "thread end anchor target");

// Both consumers render the control from the cursor, never from a guessed total, and cannot fire two
// reads at once.
const feed = read(paths.feed);
for (const [label, source] of [
  ["topic feed", feed],
  ["thread panel", ui],
]) {
  requireText(source, "paging_busy: ReadSignal<bool>", `${label} busy prop`);
  requireText(source, "paging_error: ReadSignal<Option<String>>", `${label} error prop`);
  requireText(source, "on_load_more: Callback<()>", `${label} loader prop`);
  requireText(source, "has_more.then(", `${label} cursor-gated control`);
  requireText(source, "disabled=move || paging_busy.get()", `${label} re-entrancy guard`);
  requireText(source, "on_load_more.run(())", `${label} loader call`);
}
requireText(feed, 'forum.feed.loaded', "feed loaded counter key");
requireText(ui, 'forum.thread.loaded', "thread loaded counter key");
requireText(feed, "topics: RwSignal<ForumTopicPage>", "feed reads the paged topic list");
requireText(ui, "replies: RwSignal<ForumReplyPage>", "thread reads the paged reply list");

// The timeline reports the loaded window, not a pretend position, and its jump controls are real
// anchors: an unhandled button is how this control shipped before.
const timeline = read(paths.timeline);
requireText(timeline, "loaded_posts: u64", "timeline loaded count");
forbidText(timeline, "current_post", "timeline must not claim a cursor position");
requireText(timeline, 'href="#forum-thread-top"', "timeline jump to first post");
requireText(timeline, 'href="#forum-thread-end"', "timeline jump to newest post");
requireText(timeline, 'role="progressbar"', "timeline exposes progress to assistive tech");
forbidText(timeline, "<button", "timeline controls must be actionable links");

// Count-bearing and slug-bearing catalog messages need bound Fluent arguments: resolved through `t`
// they fail formatting in every locale and silently render the English caller literal, so the
// storefront must go through the argument-bearing helpers. The repo-wide ratchet in
// scripts/verify/ui-i18n-argument-baseline.json is what forbids `t` on those keys; this pins that
// the forum storefront keeps the shape the ratchet was shrunk with.
const i18n = read(paths.i18n);
requireText(i18n, "pub fn count_message", "count message resolver");
requireText(i18n, 'arguments.set("count", count)', "count Fluent argument");
requireText(i18n, "Count: Into<rustok_ui_i18n::FluentValue<'static>> + ToString", "count argument type policy");
forbidText(i18n, "i64::MAX", "counts must not be clamped to i64");
requireText(i18n, "pub fn slug_message", "slug message resolver");
requireText(i18n, 'arguments.set("slug", slug)', "slug Fluent argument");
forbidText(timeline, "forum_storefront_count_label(", "timeline must resolve with arguments");
for (const [label, source] of [
  ["topic feed", feed],
  ["thread panel", ui],
]) {
  requireText(source, "count_message(", `${label} loaded count resolver`);
}
requireText(feed, "slug_message(", "topic feed slug resolver");
requireText(ui, "slug_message(", "thread panel slug resolver");

// A label template that no consumer reads is a phantom setting: the rail renders counts itself, so
// the builder must not carry a template, and the topic card takes the already resolved slug label.
const core = read(paths.core);
forbidText(core, "total_template", "category rail labels must not carry an unread template");
requireText(core, "slug_label: &str,", "topic card view model takes a resolved slug label");
requireText(core, "slug_label: slug_label.to_string(),", "topic card view model keeps the resolved label");

requireText(feed, "let applied_search = Memo::new(move |_| selection.get().search.unwrap_or_default());", "search draft observes only changes to the applied query");
requireText(feed, "set_search_draft.set(applied_search.get());", "mode-only changes preserve the unsubmitted search draft");
forbidText(feed, "set_search_draft.set(selection.get().search.unwrap_or_default());", "mode-only changes cannot reset search draft");
requireText(feed, '"forum.topic.solved"', "solved badge resolves through the catalog");
requireText(feed, "<span>{solved_label.clone()}</span>", "solved badge renders localized copy");
forbidText(feed, '<span>"Solved"</span>', "solved badge cannot hardcode English");

const pagingKeys = [
  "forum-topic-solved",
  "forum-feed-loadMore",
  "forum-feed-loading",
  "forum-feed-loaded",
  "forum-feed-pageFailed",
  "forum-thread-loadMore",
  "forum-thread-loading",
  "forum-thread-loaded",
  "forum-thread-pageFailed",
  "forum-timeline-toTop",
  "forum-timeline-toEnd",
  "forum-timeline-loaded",
  "forum-timeline-total",
];
for (const [locale, file] of [
  ["en", paths.catalogEn],
  ["ru", paths.catalogRu],
  ["ar", paths.catalogAr],
]) {
  const catalog = read(file);
  for (const key of pagingKeys) {
    requireText(catalog, `${key} =`, `${locale} catalog paging keys`);
  }
}

// Feed selectors are owner inputs on every page, not transforms of a loaded browser slice.
const ownerFilter = read("crates/modules/rustok-forum/src/dto/topic.rs");
for (const marker of ["pub search: Option<String>", "pub order: ForumTopicListOrder", "pub solved_only: bool", "selection_scope"]) {
  requireText(ownerFilter, marker, "canonical topic selection");
}
const listOwner = read("crates/modules/rustok-forum/src/services/topic.rs");
for (const marker of ["filter.normalize_selection()?", "filter.matches_selection(&topic)", "ReplyCount", "after.selection_scope != Some(scope)"]) {
  requireText(listOwner, marker, "paged selection owner");
}
for (const file of ["query_runtime.rs", "storefront_audience_topics.rs", "storefront_read_state.rs"]) {
  const adapter = read(`crates/modules/rustok-forum/src/graphql/${file}`);
  requireText(adapter, "search: Option<String>", `${file} search input`);
  requireText(adapter, "order: Option<crate::ForumTopicListOrder>", `${file} typed order`);
  requireText(adapter, "default_topic_order(tenant_id)", `${file} tenant ranking default`);
  requireText(adapter, "solved_only,", `${file} solved input delegation`);
}
const restTopics = read("crates/modules/rustok-forum/src/controllers/topics.rs");
for (const field of ["search", "solved_only"]) {
  requireText(restTopics, `${field}: query.${field}`, "REST owner selection");
}
requireText(restTopics, "let order = match query.order", "REST resolves explicit/default ranking");
requireText(restTopics, "default_topic_order(tenant.id)", "REST tenant ranking default");
requireText(restTopics, "        order,", "REST owner ranking delegation");
requireText(model, "pub topic_selection: ForumTopicSelection", "retained initial selection");
requireText(ui, "let requested_selection = topic_selection.clone()", "same selection on continuation");
requireText(read(`${storefront}/src/transport/native_server_adapter.rs`), "selected_topic_filter(topic_filter, selection.clone())", "initial native selection");
requireText(pages, "selected_topic_filter(filter, selection.clone())", "continuation native selection");
for (const field of ["search", "order", "solvedOnly"]) {
  requireText(graphql, `${field}: $${field}`, "GraphQL topic selection");
}
const nextFeed = read("apps/next-frontend/packages/rustok-forum/src/components/topic-feed.tsx");
for (const marker of ["topics.filter(", ".sort(", "filteredTopics", "filtersLoadedOnly"]) {
  forbidText(nextFeed, marker, "Next must preserve the owner selection and order");
}

for (const marker of ["list.retain(", "list.sort_by", "filtered_items", "TopicFilterMode"]) {
  forbidText(feed, marker, "Leptos must preserve owner selection and order");
}
requireText(ui, "selection.get()", "resource tracks owner selection");
requireText(ui, "category_override.get()", "resource tracks category override");
requireText(graphql, "unreadOnly: $unreadOnly", "unread mode is an owner predicate");
requireText(pages, "selection.unread_only", "native continuation unread predicate");

if (failures.length > 0) {
  console.error("forum storefront cursor paging verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum storefront cursor paging verification passed");
