#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  const target = repoPath(relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target, "utf8");
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function rejectMarker(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

const projectorPath = "crates/rustok-search/src/blog_projector.rs";
const routingTestPath = "crates/rustok-search/tests/blog_ingestion_contract_test.rs";
const postgresTestPath = "crates/rustok-search/tests/blog_projection_postgres_test.rs";
const evidencePath = "crates/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json";
const planPath = "crates/rustok-search/docs/implementation-plan.md";

const projector = read(projectorPath);
const routingTest = read(routingTestPath);
const postgresTest = read(postgresTestPath);
const plan = read(planPath);
let evidence = null;
try {
  evidence = JSON.parse(read(evidencePath));
} catch (error) {
  failures.push(`${evidencePath}: invalid JSON: ${error.message}`);
}

for (const table of [
  "blog_posts",
  "blog_post_translations",
  "blog_post_channel_visibility",
  "blog_category_translations",
]) {
  requireMarker(projector, `to_regclass('${table}')`, projectorPath);
}
rejectMarker(projector, "to_regclass('public.blog_", projectorPath);
requireMarker(projector, "FROM blog_posts p", projectorPath);
requireMarker(projector, "INSERT INTO search_documents", projectorPath);
requireMarker(projector, "DELETE FROM search_documents", projectorPath);

for (const marker of [
  "DomainEvent::BlogPostCreated",
  "DomainEvent::BlogPostPublished",
  "DomainEvent::BlogPostUnpublished",
  "DomainEvent::BlogPostUpdated",
  "DomainEvent::BlogPostArchived",
  "DomainEvent::BlogPostDeleted",
  'target_type: "blog".to_string()',
]) {
  requireMarker(routingTest, marker, routingTestPath);
}

for (const marker of [
  "RUSTOK_SEARCH_TEST_DATABASE_URL",
  "SearchModule.migrations()",
  "SearchIngestionHandler::new",
  "ContractEventEnvelope::new",
  'SET search_path TO "{schema_name}", public',
  "full_blog_reindex_replaces_only_current_tenant_blog_documents",
  "blog_events_upsert_publish_archive_and_delete_search_document",
]) {
  requireMarker(postgresTest, marker, postgresTestPath);
}

if (evidence) {
  if (evidence.schema_version !== 1) failures.push(`${evidencePath}: schema_version must be 1`);
  if (evidence.module !== "search" || evidence.surface !== "blog_post_projection") {
    failures.push(`${evidencePath}: module/surface identity drift`);
  }
  if (evidence.status !== "executable_no_run" || evidence.compile_policy !== "not_run_by_request") {
    failures.push(`${evidencePath}: execution status drift`);
  }
  const targets = evidence.test_targets ?? [];
  for (const target of [routingTestPath, postgresTestPath]) {
    if (!targets.includes(target)) failures.push(`${evidencePath}: missing test target ${target}`);
  }
}

for (const marker of [
  "search-blog-projection-postgres-harness.json",
  "RUSTOK_SEARCH_TEST_DATABASE_URL",
  "search_path",
]) {
  requireMarker(plan, marker, planPath);
}

if (failures.length > 0) {
  console.error("search Blog projection verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("search Blog projection verification passed");
