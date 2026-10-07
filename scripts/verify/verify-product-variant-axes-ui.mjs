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
 * Product variant-axis reachability guard (PROD-AXES-001).
 *
 * The ADR identity model is only usable when an operator can configure axes.
 * This guard pins the whole chain: owner command, Commerce GraphQL mutation,
 * Product admin transport (idempotent retry identity + typed error policy), the
 * mounted Leptos editor, and the Next admin surface. It also forbids the
 * read-only placeholder that used to sit where the axes editor belongs.
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
  ownerCommands: "crates/modules/rustok-product/src/services/catalog/commands.rs",
  commerceMutations: "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs",
  lifecycleGraphql:
    "crates/modules/rustok-product/admin/src/transport/product_lifecycle_graphql.rs",
  retry: "crates/modules/rustok-product/admin/src/transport/retry.rs",
  retryIdentity: "crates/modules/rustok-product/admin/src/lifecycle_retry_identity.rs",
  legacy: "crates/modules/rustok-product/admin/src/transport.rs",
  facade: "crates/modules/rustok-product/admin/src/catalog_transport.rs",
  leptosAdapter: "crates/modules/rustok-product/admin/src/ui/leptos.rs",
  mountedEditor: "crates/modules/rustok-product/admin/src/ui/product_editor.rs",
  nextApi: "apps/next-admin/packages/rustok-product/src/api/products.ts",
  nextCard:
    "apps/next-admin/packages/rustok-product/src/components/products/product-variant-axes-card.tsx",
  nextPage: "apps/next-admin/packages/rustok-product/src/pages/product-editor-page.tsx",
  nextActions: "apps/next-admin/src/app/dashboard/product/actions.ts",
  nextRoute: "apps/next-admin/src/app/dashboard/product/[productId]/page.tsx",
};

const ownerCommands = read(paths.ownerCommands);
const commerceMutations = read(paths.commerceMutations);
const lifecycleGraphql = read(paths.lifecycleGraphql);
const retry = read(paths.retry);
const retryIdentity = read(paths.retryIdentity);
const legacy = read(paths.legacy);
const facade = read(paths.facade);
const leptosAdapter = read(paths.leptosAdapter);
const mountedEditor = read(paths.mountedEditor);
const nextApi = read(paths.nextApi);
const nextCard = read(paths.nextCard);
const nextPage = read(paths.nextPage);
const nextActions = read(paths.nextActions);
const nextRoute = read(paths.nextRoute);

// 1. Owner command keeps validating the schema policy and the existing assignments.
for (const marker of [
  "pub async fn set_variant_axes(",
  "input\n            .validate()",
  "validate_variant_axis_configuration_in(",
  "validate_existing_variant_axis_assignments_in(",
]) {
  assertContains(ownerCommands, marker, paths.ownerCommands);
}

// 2. Commerce GraphQL mutation stays permission-gated and idempotent.
for (const marker of [
  "async fn set_product_variant_axes(",
  "idempotency_key: String,",
  "Permission::PRODUCTS_UPDATE",
  "product_command_context(",
  "product_command_port_error(&port_context, error, \"set_product_variant_axes\")",
]) {
  assertContains(commerceMutations, marker, paths.commerceMutations);
}

// 3. Admin transport: one idempotent mutation with typed classification.
for (const marker of [
  "const SET_VARIANT_AXES_MUTATION",
  "$idempotencyKey: String!",
  "$productId: UUID!",
  "$input: SetVariantAxesInput!",
  "setProductVariantAxes(idempotencyKey: $idempotencyKey, productId: $productId, input: $input)",
  "set_product_variant_axes: Vec<crate::model::VariantAxisConfig>",
  "pub(crate) async fn set_variant_axes(",
  "\"set_variant_axes\",",
  ".with_resource(&product_id)",
]) {
  assertContains(lifecycleGraphql, marker, paths.lifecycleGraphql);
}

// 4. Retry identity: the axis write is a retained logical invocation.
for (const marker of [
  "SetVariantAxes,",
  'Self::SetVariantAxes => "set-variant-axes",',
]) {
  assertContains(retryIdentity, marker, paths.retryIdentity);
}
for (const marker of [
  "pub(crate) async fn set_variant_axes(",
  "ProductAdminLifecycleOperation::SetVariantAxes",
  "retained_caller_key(&slot, operation, intent)",
  "variant_axes_intent(",
  "mark_lifecycle_succeeded(&slot)",
]) {
  assertContains(retry, marker, paths.retry);
}

// 5. Both admin import paths expose the write.
assertContains(legacy, "set_variant_axes,", paths.legacy);
assertContains(facade, "set_variant_axes,", paths.facade);

// 6. Mounted Leptos surface owns the editor and drops the placeholder.
for (const marker of [
  "pub fn ProductVariantAxesSection(",
  "catalog_transport::set_variant_axes(",
  'attribute.variant_axis_policy != "forbidden"',
  "attribute.default_variant_axis",
  "SetVariantAxesDraft { axes }",
]) {
  assertContains(leptosAdapter, marker, paths.leptosAdapter);
}
assertContains(
  mountedEditor,
  "<ProductVariantAxesSection",
  paths.mountedEditor
);
assertAbsent(mountedEditor, "variant_axes_str", paths.mountedEditor);
assertAbsent(
  mountedEditor,
  "Comma-separated attribute axes",
  paths.mountedEditor
);

// 7. Next admin: mutation, action, and the mounted card.
for (const marker of [
  "export const SET_VARIANT_AXES_MUTATION",
  "export async function setVariantAxes(",
  "crypto.randomUUID()",
  "setProductVariantAxes: VariantAxisConfig[]",
]) {
  assertContains(nextApi, marker, paths.nextApi);
}
for (const marker of [
  "export async function setVariantAxesAction(",
  "await setVariantAxes(opts, productId, input)",
  "revalidatePath(`/dashboard/product/${productId}`)",
]) {
  assertContains(nextActions, marker, paths.nextActions);
}
for (const marker of [
  "export function ProductVariantAxesCard(",
  "variantAxisPolicy !== 'forbidden'",
  "defaultVariantAxis",
  "onSaveAxes",
  "allowedOptionIds",
]) {
  assertContains(nextCard, marker, paths.nextCard);
}
assertContains(nextPage, "<ProductVariantAxesCard", paths.nextPage);
assertContains(nextPage, "onSetVariantAxes", paths.nextPage);
assertContains(nextRoute, "onSetVariantAxes={", paths.nextRoute);
assertContains(
  nextRoute,
  "setVariantAxesAction",
  paths.nextRoute
);

if (failures.length > 0) {
  console.error("Product variant-axis UI verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "[verify-product-variant-axes-ui] Variant axes are configurable on both admin surfaces through the idempotent owner command"
);
