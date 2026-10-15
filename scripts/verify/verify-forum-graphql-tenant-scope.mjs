#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const query = readFileSync(path.join(root, 'crates/modules/rustok-forum/src/graphql/query_runtime.rs'), 'utf8');
const mutation = readFileSync(path.join(root, 'crates/modules/rustok-forum/src/graphql/mutation.rs'), 'utf8');

function requireEarlyTenantScope(source, method, before) {
  const start = source.indexOf(`async fn ${method}(`);
  if (start < 0) throw new Error(`missing GraphQL method: ${method}`);
  const end = source.indexOf('\n    async fn ', start + 1);
  const body = source.slice(start, end < 0 ? undefined : end);
  const scope = body.indexOf('resolve_tenant_scope(tenant, Some(tenant_id))?;');
  const effect = body.indexOf(before);
  if (scope < 0 || effect < 0 || scope > effect) {
    throw new Error(`${method} must bind tenantId to the request before ${before}`);
  }
}

requireEarlyTenantScope(query, 'forum_user_stats', 'UserStatsService::new');
requireEarlyTenantScope(mutation, 'clear_forum_topic_vote', '.clear_topic_vote(');
requireEarlyTenantScope(mutation, 'clear_forum_reply_vote', '.clear_reply_vote(');
console.log('forum GraphQL stats and vote tenant scope verification passed');
