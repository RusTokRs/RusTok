#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..");
const read = (relativePath) =>
  fs.readFileSync(path.join(repoRoot, relativePath), "utf8");

const graphqlMutation = read("crates/rustok-pages/src/graphql/mutation.rs");
const graphqlTypes = read("crates/rustok-pages/src/graphql/types.rs");
const createService = read("crates/rustok-pages/src/services/page/create.rs");
const http = read("crates/rustok-pages/src/http.rs");
const openapi = read("crates/rustok-pages/src/openapi.rs");
const manifest = read("crates/rustok-pages/rustok-module.toml");
const adminModel = read("crates/rustok-pages/admin/src/model.rs");
const adminTransport = read(
  "crates/rustok-pages/admin/src/transport/graphql_adapter.rs",
);
const adminTransportModule = read(
  "crates/rustok-pages/admin/src/transport/mod.rs",
);

function fail(message) {
  console.error(`[verify-page-builder-publish-transport-cutover] ${message}`);
  process.exit(1);
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) fail(`${label} is missing ${marker}`);
}

function forbidMarker(source, marker, label) {
  if (source.includes(marker)) fail(`${label} still contains ${marker}`);
}

function sliceBetween(source, start, end, label) {
  const startIndex = source.indexOf(start);
  if (startIndex < 0) fail(`${label} is missing ${start}`);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (endIndex < 0) fail(`${label} is missing ${end}`);
  return source.slice(startIndex, endIndex);
}

for (const marker of [
  "pub struct PublishGqlPageInput",
  "pub struct GqlPageBodyRevisionInput",
  "pub struct ReviewedGqlPagePublishRuntimeInput",
  "pub struct GqlPublishPageResult",
  "impl From<crate::PublishPageResult> for GqlPublishPageResult",
]) {
  requireMarker(graphqlTypes, marker, "GraphQL reviewed publish types");
}

const graphqlPublish = sliceBetween(
  graphqlMutation,
  "async fn publish_page",
  "async fn unpublish_page",
  "GraphQL publish mutation",
);
for (const marker of [
  "input: PublishGqlPageInput",
  "Result<GqlPublishPageResult>",
  ".publish_reviewed(",
  "publish_page_input(input)",
]) {
  requireMarker(graphqlPublish, marker, "GraphQL reviewed publish mutation");
}
forbidMarker(graphqlPublish, "publish_if_current", "GraphQL publish mutation");

const graphqlCreate = sliceBetween(
  graphqlMutation,
  "async fn create_page",
  "async fn create_menu",
  "GraphQL create mutation",
);
for (const marker of [
  "if input.publish.unwrap_or(false)",
  "create_publish_bypass_error()",
  "publish: false",
]) {
  requireMarker(graphqlCreate, marker, "GraphQL create fail-closed path");
}

for (const marker of [
  "if input.publish",
  "create the draft, review a runtime scenario, then use the atomic publish command",
  "ContentStatus::Draft",
  "published_at: Set(None)",
]) {
  requireMarker(createService, marker, "Pages create service");
}
for (const forbidden of [
  "PageBuilderArtifactService::compile_source",
  "ContentStatus::Published",
  "DomainEvent::NodePublished",
  "ensure_builder_publish_enabled",
]) {
  forbidMarker(createService, forbidden, "Pages create service");
}

for (const marker of [
  "$input: PublishGqlPageInput!",
  "publishPage(id: $id, input: $input)",
  "expectedBodyRevisions",
  "PageBuilderReviewedPublishRuntime::new",
  "select_single_reviewed_scenario",
  "ProjectHash::from_bytes(&bytes).hex()",
  "PublishPageReceipt",
]) {
  requireMarker(adminTransport, marker, "Pages admin publish transport");
}
forbidMarker(
  adminTransport,
  "publishPage(id: $id) {",
  "Pages admin publish transport",
);
for (const marker of [
  "pub struct PublishPageReceipt",
  "pub available_locales: Vec<String>",
]) {
  requireMarker(adminModel, marker, "Pages admin publish model");
}
requireMarker(
  adminTransportModule,
  "Result<PublishPageReceipt, TransportError>",
  "Pages admin transport facade",
);

for (const marker of [
  'path = "/api/admin/pages/{id}/publish"',
  "request_body = PublishPageInput",
  "HttpResult<Json<PublishPageResult>>",
  ".publish_reviewed(",
  '"/api/admin/pages/{id}/publish"',
]) {
  requireMarker(http, marker, "Pages reviewed publish HTTP route");
}
requireMarker(
  manifest,
  'axum_router = "http::axum_router"',
  "Pages HTTP manifest",
);
requireMarker(openapi, "crate::http::publish_page", "Pages OpenAPI publish path");
for (const marker of [
  "crate::PublishPageInput",
  "crate::PublishPageResult",
  "crate::PageBodyRevisionInput",
  "crate::ReviewedPagePublishRuntimeInput",
]) {
  requireMarker(openapi, marker, "Pages OpenAPI publish schemas");
}

console.log("[verify-page-builder-publish-transport-cutover] PASS");
