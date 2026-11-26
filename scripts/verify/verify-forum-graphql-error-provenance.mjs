#!/usr/bin/env node
// Bounded ForumError propagation checks; these do not replace resolver tests.
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spaced } from "./lib/forum-source-verification.mjs";

export const DOMAIN_ERROR_REGIONS = [
  {
    file: "query_runtime.rs",
    start: "    async fn forum_category(",
    end: "    async fn forum_replies(",
    rules: [
      ["typed read failures", /Err\(error\) => return Err\(error\.into\(\)\)/g, 2],
      ["category absence", /Err\(ForumError::CategoryNotFound\(_\)\) => return Ok\(None\)/g, 1],
      ["topic absence", /Err\(ForumError::TopicNotFound\(_\)\) => return Ok\(None\)/g, 1],
    ],
  },
  {
    file: "topic_route_query.rs",
    start: "async fn resolve_authorized_public_route(",
    end: "fn map_legacy_public_route_resolution(",
    rules: [
      ["typed route failures", /Err\(error\) => (?:return )?Err\(error\.into\(\)\)/g, 2],
      ["typed tombstone disclosure", /\.can_disclose_public_gone\([^;]*?\.await\?/g, 1],
      ["route absence", /Err\(ForumError::TopicRouteNotFound\) => (?:return )?Ok\(None\)/g, 2],
    ],
  },
  {
    file: "category_route_query.rs",
    start: "async fn resolve_authorized_public_category_route(",
    end: "fn map_public_category_route_resolution(",
    rules: [
      ["typed category route failures", /Err\(error\) => (?:return )?Err\(error\.into\(\)\)/g, 2],
      ["typed category audience context", /let audience_context = category_read_audience_port_context\([^;]*?\)\s*\?/g, 1],
      ["category route absence", /Err\(ForumError::CategoryRouteNotFound\) => (?:\{\s*return )?Ok\(None\)/g, 2],
    ],
  },
  {
    file: "member_card_query.rs",
    start: "    async fn forum_member_cards(",
    end: "fn require_member_card_permission(",
    rules: [
      ["typed member validation", /ForumMemberCardService::normalize_user_ids\(&user_ids\)\s*\?/g, 1],
      ["typed loaded member composition", /\.compose_admitted_profiles\([^;]*?\.await\?/g, 1],
      ["typed member read", /\.read_for_audience\([^;]*?\.await\?/g, 1],
    ],
  },
];

export function inspectDomainErrors(source, region) {
  const start = source.indexOf(region.start);
  const end = source.indexOf(region.end, start + region.start.length);
  if (start < 0 || end <= start) return [`${region.file}: missing resolver boundaries`];
  const body = spaced(source.slice(start, end));
  const errors = [];
  if (/(?:Error::new|internal_error)\(\s*\w+\.to_string\(\)\s*\)/.test(body)) errors.push(`${region.file}: ForumError source must not be erased into display text`);
  for (const [label, pattern, count] of region.rules) {
    const actual = [...body.matchAll(pattern)].length;
    if (actual !== count) errors.push(`${region.file}: expected ${count} ${label} branches, found ${actual}`);
  }
  return errors;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const root = process.env.RUSTOK_VERIFY_REPO_ROOT || path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
  const errors = DOMAIN_ERROR_REGIONS.flatMap((region) => inspectDomainErrors(
    readFileSync(path.join(root, "crates/modules/rustok-forum/src/graphql", region.file), "utf8"), region,
  ));
  if (errors.length) {
    console.error(errors.join("\n"));
    process.exitCode = 1;
  } else console.log("Forum selected-read/route/member errors preserve typed sources and existing null semantics (source evidence only).");
}
