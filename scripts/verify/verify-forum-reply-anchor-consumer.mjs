#!/usr/bin/env node

// Guards the reply-anchor route contract: the forum module authors `?reply=<uuid>` links, the Next.js
// storefront consumes them, and both hosts serve the owner's canonical topic route prefix. See
// DECISIONS/2026-10-11-forum-reply-anchor-route-contract.md.

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

const paths = {
  routeOwner: "crates/modules/rustok-forum/src/services/topic_route.rs",
  notificationSource: "crates/modules/rustok-forum/src/notification_source.rs",
  searchProjection: "crates/modules/rustok-forum/src/search_projection.rs",
  routeReader: "crates/modules/rustok-forum/src/seo_targets.rs",
  anchor: "apps/next-frontend/packages/rustok-forum/src/components/reply-anchor.ts",
  section: "apps/next-frontend/packages/rustok-forum/src/components/forum-section.tsx",
  threadPanel: "apps/next-frontend/packages/rustok-forum/src/components/thread-panel.tsx",
  detail: "apps/next-frontend/packages/rustok-forum/src/components/topic-detail-view.tsx",
  feed: "apps/next-frontend/packages/rustok-forum/src/components/topic-feed.tsx",
  sectionPage: "apps/next-frontend/src/app/[locale]/modules/forum/page.tsx",
  topicPage: "apps/next-frontend/src/app/[locale]/forum/t/[...slug]/page.tsx",
  staleTopicPage: "apps/next-frontend/src/app/[locale]/modules/forum/t/[...slug]/page.tsx",
  decision: "DECISIONS/2026-10-11-forum-reply-anchor-route-contract.md",
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

// Producers: the anchor is authored by the owner, on the canonical route each host serves.
requireText(
  read(paths.routeOwner),
  'format!("/{locale}/forum/t/{short_id}/{slug}")',
  "owner topic route builder"
);
requireText(
  read(paths.notificationSource),
  '"/modules/forum?category={}&topic={}&reply={}"',
  "reply notification target route"
);
requireText(
  read(paths.searchProjection),
  'format!("{topic_route}?reply={reply_id}")',
  "reply search document route"
);

// An anchor selects a post to present; it must never change which topic a route resolves to.
const routeReader = read(paths.routeReader);
requireText(routeReader, '"topic" => topic_id = Uuid::parse_str', "canonical route reader stays topic-scoped");
forbidText(routeReader, '"reply" =>', "canonical route reader must not resolve a topic from an anchor");

// Consumers: the id is untrusted URL input, the walk is bounded, and a miss is reported.
const anchor = read(paths.anchor);
requireText(anchor, "FORUM_REPLY_ID_PATTERN", "anchor helper validates the reply id");
requireText(anchor, '[data-forum-post][data-target-kind="REPLY"]', "anchor helper resolves rendered posts");
requireText(anchor, "MAX_ANCHOR_REPLY_PAGES", "anchor paging is bounded");

const section = read(paths.section);
requireText(section, "initialReplyId", "forum section consumes the reply anchor");
requireText(section, "MAX_ANCHOR_REPLY_PAGES", "forum section bounds the anchor walk");

const threadPanel = read(paths.threadPanel);
requireText(threadPanel, "focusForumReplyPost", "thread panel scrolls to the linked post");
requireText(threadPanel, "focusReplyMissing", "thread panel reports an unreachable linked post");

const detail = read(paths.detail);
requireText(detail, "focusForumReplyPost", "topic view scrolls to the linked post");
requireText(detail, "setAnchorMissing(anchoredIndex < 0)", "topic view reports an unreachable linked post");

const feed = read(paths.feed);
requireText(feed, "/forum/t/${topic.id}", "topic feed links use the canonical route prefix");
forbidText(feed, "modules/forum/t/", "topic feed must not link a prefix the owner does not publish");

const sectionPage = read(paths.sectionPage);
requireText(sectionPage, "resolvedSearchParams.reply", "section route reads the reply anchor");

const topicPage = read(paths.topicPage);
requireText(topicPage, "query.reply", "topic route reads the reply anchor");
requireText(topicPage, "initialRepliesCursor", "topic route hands the reply cursor to the view");
requireText(topicPage, "encodeURIComponent(replyParam)", "canonical redirect preserves the anchor");
if (existsSync(path.join(repoRoot, paths.staleTopicPage))) {
  failures.push(`${paths.staleTopicPage}: a second live mount for one canonical topic route is forbidden`);
}

requireText(read(paths.decision), "## Decision", "reply anchor route contract decision");

if (failures.length > 0) {
  console.error("forum reply anchor consumption verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum reply anchor consumption verification passed");
