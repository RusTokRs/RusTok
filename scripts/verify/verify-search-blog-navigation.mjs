#!/usr/bin/env node
import { existsSync, readFileSync } from "node:fs";

const files = {
  transport: "crates/rustok-search/storefront/src/transport/mod.rs",
  navigation: "crates/rustok-search/storefront/src/transport/navigation.rs",
  projector: "crates/rustok-search/src/blog_projector.rs",
  model: "crates/rustok-search/storefront/src/model.rs",
};

function fail(message) {
  console.error("search blog navigation verification failed:");
  console.error(`- ${message}`);
  process.exit(1);
}

function read(path) {
  if (!existsSync(path)) fail(`${path}: expected file is missing`);
  return readFileSync(path, "utf8");
}

function hasAll(source, markers, label) {
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${label}: missing ${marker}`);
  }
}

const transport = read(files.transport);
const navigation = read(files.navigation);
const projector = read(files.projector);
const model = read(files.model);

hasAll(transport, [
  "mod navigation;",
  "execute_selected_transport(",
  "navigation::enrich_search_result_urls(&mut payload);",
], "transport parity");

hasAll(navigation, [
  "BLOG_SOURCE_MODULE: &str = \"blog\"",
  "BLOG_ENTITY_TYPE: &str = \"blog_post\"",
  "BLOG_STOREFRONT_ROUTE: &str = \"/modules/blog\"",
  "item.url.is_some()",
  "serde_json::from_str(payload)",
  "value.get(\"slug\")",
  "valid_blog_slug",
  "Some(format!(\"{BLOG_STOREFRONT_ROUTE}?slug={slug}\"))",
  "preserves_backend_url_and_rejects_invalid_slug",
], "navigation policy");

hasAll(projector, [
  "const BLOG_ENTITY_TYPE: &str = \"blog_post\"",
  "const BLOG_SOURCE_MODULE: &str = \"blog\"",
  "\"slug\": p.slug",
], "blog search projection");

hasAll(model, [
  "pub struct SearchPreviewResultItem",
  "pub url: Option<String>",
  "pub payload: String",
], "storefront result contract");

console.log("search blog navigation verification passed");
