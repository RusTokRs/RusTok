#!/usr/bin/env node
/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

/**
 * Category-schema authoring reachability guard (SCHEMA-UI-001).
 *
 * Five owner commands (schema mode, schema groups, category groups, schema
 * attribute bindings, category attribute bindings) were reachable only through
 * raw GraphQL/REST before this guard existed: the mounted admin never rendered
 * them. The guard pins the whole chain end to end and rejects the raw-write and
 * discarded-result regressions that caused the gap.
 */

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? path.resolve(configuredRoot)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const read = (relativePath) => readFileSync(path.join(root, relativePath), "utf8");
const failures = [];

const assertContains = (source, value, label) => {
  if (!source.includes(value)) {
    failures.push(`${label}: missing ${value}`);
  }
};

const assertAbsent = (source, value, label) => {
  if (source.includes(value)) {
    failures.push(`${label}: forbidden ${value}`);
  }
};

const paths = {
  ownerCategories: "crates/modules/rustok-product/src/services/catalog_schema_service/categories.rs",
  ownerSchemas: "crates/modules/rustok-product/src/services/catalog_schema_service/schemas.rs",
  ownerPort: "crates/modules/rustok-product/src/catalog_schema_write_port.rs",
  commerceMutations: "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs",
  facade: "crates/modules/rustok-product/admin/src/catalog_transport.rs",
  fallbackWrappers: "crates/modules/rustok-product/admin/src/transport/graphql_fallback_mutations.rs",
  legacy: "crates/modules/rustok-product/admin/src/transport.rs",
  card: "crates/modules/rustok-product/admin/src/ui/leptos.rs",
  mountedPage: "crates/modules/rustok-product/admin/src/ui/attributes.rs",
  core: "crates/modules/rustok-product/admin/src/core.rs",
};

const ownerCategories = read(paths.ownerCategories);
const ownerSchemas = read(paths.ownerSchemas);
const ownerPort = read(paths.ownerPort);
const commerceMutations = read(paths.commerceMutations);
const facade = read(paths.facade);
const fallbackWrappers = read(paths.fallbackWrappers);
const legacy = read(paths.legacy);
const card = read(paths.card);
const mountedPage = read(paths.mountedPage);
const core = read(paths.core);

// 1. Owner layer: the five commands exist and are name-stable.
for (const marker of [
  "pub async fn set_category_schema_mode(",
  "pub async fn create_category_group(",
  "pub async fn bind_category_attribute(",
]) {
  assertContains(ownerCategories, marker, paths.ownerCategories);
}
for (const marker of [
  "pub async fn create_schema_group(",
  "pub async fn bind_schema_attribute(",
]) {
  assertContains(ownerSchemas, marker, paths.ownerSchemas);
}
for (const marker of [
  "async fn set_category_schema_mode(",
  "async fn create_category_group(",
  "async fn bind_category_attribute(",
  "async fn create_schema_group(",
  "async fn bind_schema_attribute(",
]) {
  assertContains(ownerPort, marker, paths.ownerPort);
}

// 2. Commerce GraphQL: the mutations stay permission-gated and idempotent.
for (const marker of [
  "async fn set_catalog_category_schema_mode(",
  "async fn create_product_attribute_schema_group(",
  "async fn create_catalog_category_attribute_group(",
  "async fn bind_product_attribute_schema_attribute(",
  "async fn bind_catalog_category_attribute(",
  "idempotency_key: String,",
  "product_schema_write_context(",
  '"create_category_group",',
  '"set_category_schema_mode",',
]) {
  assertContains(commerceMutations, marker, paths.commerceMutations);
}

// 3. Admin transport facade re-exports the eleven typed fallback wrappers, which
//    live in transport/graphql_fallback_mutations.rs. Each command carries a typed
//    error policy and delegates through the legacy transport instead of bypassing it.
assertContains(facade, "pub(crate) use graphql_fallback_mutations::{", paths.facade);
const reexportBlock = facade.slice(
  facade.indexOf("pub(crate) use graphql_fallback_mutations::{"),
  facade.indexOf("};", facade.indexOf("pub(crate) use graphql_fallback_mutations::{")),
);
for (const name of [
  "set_category_schema_mode",
  "create_product_attribute_schema_group",
  "create_category_attribute_group",
  "bind_schema_attribute",
  "bind_category_attribute",
]) {
  assertContains(reexportBlock, name, paths.facade);
}
for (const marker of [
  "pub(crate) async fn set_category_schema_mode(",
  "pub(crate) async fn create_product_attribute_schema_group(",
  "pub(crate) async fn create_category_attribute_group(",
  "pub(crate) async fn bind_schema_attribute(",
  "pub(crate) async fn bind_category_attribute(",
  "GraphqlFallbackMutationContext::for_set_category_schema_mode(",
  "GraphqlFallbackMutationContext::for_create_product_attribute_schema_group(",
  "GraphqlFallbackMutationContext::for_create_category_attribute_group(",
  "GraphqlFallbackMutationContext::for_bind_schema_attribute(",
  "GraphqlFallbackMutationContext::for_bind_category_attribute(",
  "legacy::set_category_schema_mode(token, tenant_slug, tenant_id, user_id, draft)",
  "legacy::bind_category_attribute(token, tenant_slug, tenant_id, user_id, draft)",
]) {
  assertContains(fallbackWrappers, marker, paths.fallbackWrappers);
}
const facadeFallbackMappers = (fallbackWrappers.match(/\|fallback_mutation_error\| context\.map_error\(fallback_mutation_error\)/g) ?? []).length;
if (facadeFallbackMappers !== 11) {
  failures.push(
    `${paths.fallbackWrappers}: expected 11 typed fallback mutation error mappers, found ${facadeFallbackMappers}`
  );
}

// 4. Legacy transport: native-first with a GraphQL fallback for each command.
for (const marker of [
  "native_server_adapter::set_category_schema_mode(",
  "native_server_adapter::create_product_attribute_schema_group(",
  "native_server_adapter::create_category_attribute_group(",
  "native_server_adapter::bind_schema_attribute(",
  "native_server_adapter::bind_category_attribute(",
  "graphql_adapter::set_category_schema_mode(",
  "graphql_adapter::bind_category_attribute(",
]) {
  assertContains(legacy, marker, paths.legacy);
}
// Whitespace-insensitive: the minted caller key must reach both the native owner call and the
// GraphQL fallback for the same logical operation.
const legacyCompact = legacy.replace(/\s+/g, "");
for (const call of [
  "native_server_adapter::set_category_schema_mode(tenant_id.clone(),draft.clone(),idempotency_key.clone()",
  "native_server_adapter::bind_category_attribute(tenant_id.clone(),draft.clone(),idempotency_key.clone()",
  "graphql_adapter::set_category_schema_mode(token,tenant_slug,draft,idempotency_key",
  "graphql_adapter::bind_category_attribute(token,tenant_slug,draft,idempotency_key",
]) {
  if (!legacyCompact.includes(call)) {
    failures.push(`${paths.legacy}: missing ${call}`);
  }
}

// 5. The authoring card: one result handler, one failure copy, no raw discard.
for (const marker of [
  "fn apply_schema_authoring_result(",
  "pub fn ProductSchemaAuthoringCard(locale: Option<String>) -> impl IntoView",
  "catalog_transport::set_category_schema_mode(",
  "catalog_transport::create_product_attribute_schema_group(",
  "catalog_transport::create_category_attribute_group(",
  "catalog_transport::bind_schema_attribute(",
  "catalog_transport::bind_category_attribute(",
  "SetCategorySchemaModeDraft {",
  "CategoryAttributeGroupDraft {",
  "ProductAttributeSchemaGroupDraft {",
  "BindSchemaAttributeDraft {",
  "BindCategoryAttributeDraft {",
  "binding_kind: category_binding_kind.get_untracked(),",
  "error_copy.save_product_failure(detail)",
  "ProductAdminErrorCopy",
]) {
  assertContains(card, marker, paths.card);
}
assertAbsent(card, "let _ = catalog_transport::", paths.card);

// 6. The card is mounted on the routed attribute page, so the commands are
//    reachable from the surface the host code generator mounts.
assertContains(mountedPage, "use super::leptos::ProductSchemaAuthoringCard;", paths.mountedPage);
assertContains(mountedPage, "<ProductSchemaAuthoringCard locale=", paths.mountedPage);

// 7. Operator copy stays in the framework-agnostic core.
assertContains(core, "struct ProductAttributeValuesSectionCopy", paths.core);
assertContains(core, "fn build_product_attribute_values_section_copy(", paths.core);
assertContains(core, "product.attributes.valuesMissingOption", paths.core);

if (failures.length > 0) {
  console.error("Product admin category-schema authoring verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "[verify-product-admin-schema-authoring] Schema mode, groups and attribute bindings are authorable from the mounted admin through the typed transport"
);
