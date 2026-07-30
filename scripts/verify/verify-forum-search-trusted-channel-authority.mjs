#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

const paths = {
  forumPlan: "crates/rustok-forum/docs/implementation-plan.md",
  searchPlan: "crates/rustok-search/docs/implementation-plan.md",
  contract: "crates/rustok-forum/contracts/forum-search-trusted-channel-authority.json",
  note: "crates/rustok-forum/docs/forum-23b2e1-trusted-channel-authority.md",
  owner: "crates/rustok-search/src/storefront_channel_authority.rs",
  searchLib: "crates/rustok-search/src/lib.rs",
  graphql: "crates/rustok-search/src/graphql/query.rs",
  forumGraphql: "crates/rustok-search/src/graphql/forum_storefront.rs",
  native: "crates/rustok-search/storefront/src/transport/native_server_adapter.rs",
  forumNative:
    "crates/rustok-search/storefront/src/transport/forum_native_server_adapter.rs",
  forumExecution: "crates/rustok-search/src/forum_storefront_execution.rs",
  projector: "crates/rustok-search/src/projector_legacy.rs",
  engine: "crates/rustok-search/src/pg_engine.rs",
};

function read(relativePath) {
  const target = path.join(root, relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target, "utf8");
}

function parseJson(relativePath) {
  try {
    return JSON.parse(read(relativePath));
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return null;
  }
}

function requireAll(source, markers, label) {
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
  }
}

function rejectAll(source, markers, label) {
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
  }
}

const forumPlan = read(paths.forumPlan);
const searchPlan = read(paths.searchPlan);
const contract = parseJson(paths.contract);
const note = read(paths.note);
const owner = read(paths.owner);
const searchLib = read(paths.searchLib);
const graphql = read(paths.graphql);
const forumGraphql = read(paths.forumGraphql);
const native = read(paths.native);
const forumNative = read(paths.forumNative);
const forumExecution = read(paths.forumExecution);
const projector = read(paths.projector);
const engine = read(paths.engine);

requireAll(
  owner,
  [
    "pub struct TrustedStorefrontChannel",
    "pub enum StorefrontChannelAuthorityError",
    "InvalidRequestedChannelId",
    "RequestedChannelMismatch",
    "RequestTenantMismatch",
    "IncompleteTrustedChannelContext",
    "InvalidTrustedChannelContext",
    "pub fn resolve_trusted_storefront_channel_input",
    "pub fn resolve_trusted_storefront_channel",
    "request_context.tenant_id != expected_tenant_id",
    "requested_channel_id.is_some() && requested_channel_id != trusted.channel_id",
    "mismatched_assertion_cannot_select_another_channel",
    "mismatched_tenant_fails_closed",
    "incomplete_context_fails_closed",
  ],
  paths.owner,
);
rejectAll(
  owner,
  ["rustok_channel", "ChannelService", "search_documents", "products"],
  paths.owner,
);

requireAll(
  searchLib,
  [
    "pub mod storefront_channel_authority;",
    "StorefrontChannelAuthorityError",
    "TrustedStorefrontChannel",
    "resolve_trusted_storefront_channel_input",
  ],
  paths.searchLib,
);

requireAll(
  graphql,
  [
    "let request_context = ctx.data::<RequestContext>()?;",
    "resolve_trusted_storefront_channel(",
    "request_context,",
    "tenant.id,",
    "input.channel_id,",
    "channel_id: trusted_channel.channel_id,",
  ],
  paths.graphql,
);

requireAll(
  native,
  [
    "leptos_axum::extract::<RequestContext>()",
    "resolve_trusted_storefront_channel(",
    "&request_context,",
    "tenant.id,",
    "input.channel_id,",
    "channel_id: trusted_channel.channel_id,",
  ],
  paths.native,
);

requireAll(
  forumGraphql,
  [
    "resolve_trusted_storefront_channel_input(",
    "&request_context,",
    "tenant.id,",
    "input.channel_id.as_deref(),",
    "channel_id: trusted_channel.channel_id.map(|value| value.to_string()),",
  ],
  paths.forumGraphql,
);

requireAll(
  forumNative,
  [
    "resolve_trusted_storefront_channel_input(",
    "filters.channel_id.as_deref(),",
    "channel_id: trusted_channel.channel_id.map(|value| value.to_string()),",
  ],
  paths.forumNative,
);

requireAll(
  forumExecution,
  [
    "resolve_trusted_storefront_channel(",
    "Forum storefront Search requires trusted request context",
    "channel_id: trusted_channel.channel_id,",
    "request_context: Some(request_context),",
  ],
  paths.forumExecution,
);

requireAll(
  forumPlan,
  [
    "FORUM-23B2E1",
    "trusted `RequestContext`",
    "verify-forum-search-trusted-channel-authority.mjs",
  ],
  paths.forumPlan,
);
requireAll(
  searchPlan,
  [
    "FORUM-23B2E1",
    "caller-provided `channel_id` is now only a compatibility assertion",
    "Product channel visibility remains blocked",
  ],
  paths.searchPlan,
);
requireAll(
  note,
  [
    "# FORUM-23B2E1 trusted storefront Search channel authority",
    "only an assertion",
    "does **not** claim product visibility completion",
  ],
  paths.note,
);

if (contract) {
  if (contract.task !== "FORUM-23B2E1") {
    failures.push(`${paths.contract}: unexpected task ${contract.task}`);
  }
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${paths.contract}: unexpected status ${contract.status}`);
  }
  if (!contract.authority?.request_context_tenant_must_match_search_tenant) {
    failures.push(`${paths.contract}: missing tenant authority invariant`);
  }
  if (!contract.caller_input?.public_channel_id_is_only_an_assertion) {
    failures.push(`${paths.contract}: caller assertion invariant is missing`);
  }
  if (!contract.caller_input?.mismatched_assertion_fails_closed) {
    failures.push(`${paths.contract}: mismatch must fail closed`);
  }
  if (contract.surfaces?.admin_search_preview_changed !== false) {
    failures.push(`${paths.contract}: admin preview must remain unchanged`);
  }
  if (contract.compatibility?.migration_required !== false) {
    failures.push(`${paths.contract}: migration claim drift`);
  }
}

// B2E1 intentionally does not claim the Product projection/filter work.
rejectAll(
  projector,
  ["allowed_channel_slugs'", "channel_visibility'"],
  `${paths.projector} B2E1 non-claim`,
);
if (engine.includes("allowed_channel_slugs")) {
  failures.push(`${paths.engine}: product visibility predicate moved into B2E1 unexpectedly`);
}

if (failures.length > 0) {
  console.error("FORUM-23B2E1 trusted channel authority verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("FORUM-23B2E1 trusted channel authority source contract is consistent.");
