#!/usr/bin/env node
//
// PROD-EVENT-001 guard: product withdrawal and retirement are first-class lifecycle facts.
//
// The guard pins the published event contract (`product.unpublished`, `product.archived`), the owner
// commands that emit it, the port and transport surfaces that expose it, and every consumer route
// that must react to it. A missing consumer route means an unpublished or archived product would
// stay visible.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");
const failures = [];
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const slice = (source, start, end) => {
  const startIndex = source.indexOf(start);
  if (startIndex < 0) {
    failures.push(`missing slice anchor ${start}`);
    return "";
  }
  const endIndex = end === null ? -1 : source.indexOf(end, startIndex + 1);
  return source.slice(startIndex, endIndex < 0 ? source.length : endIndex);
};

const eventPath = "crates/libs/rustok-events/src/types/domain_event.rs";
const validationPath = "crates/libs/rustok-events/src/types/validation.rs";
const schemaPath = "crates/libs/rustok-events/src/schema.rs";
const canonicalTestPath = "crates/libs/rustok-events/tests/canonical_contracts.rs";
const commandsPath = "crates/modules/rustok-product/src/services/catalog/commands.rs";
const commandPortPath = "crates/modules/rustok-product/src/catalog_command_port.rs";
const mediaDecoratorPath = "crates/modules/rustok-product/src/media_asset_read_port.rs";
const graphqlPath = "crates/modules/rustok-commerce/src/graphql/mutations/catalog.rs";
const restSharedPath = "crates/modules/rustok-commerce/src/controllers/products.rs";
const restAdminPath = "crates/modules/rustok-commerce/src/controllers/admin/products.rs";
const restRouterPath = "crates/modules/rustok-commerce/src/controllers/admin/mod.rs";
const openapiPath = "crates/modules/rustok-commerce/src/openapi.rs";
const searchIngestionPath = "crates/modules/rustok-search/src/ingestion.rs";
const indexRefreshPath = "crates/modules/rustok-product/src/services/index_refresh.rs";
const indexPublicationPath =
  "crates/modules/rustok-product/src/services/index_refresh_publication.rs";
const lifecycleDocPath =
  "crates/modules/rustok-product/docs/product-unpublish-archive-events.md";
const auditPath = "docs/audits/product-module-engineering-audit-2026-10-07.md";

for (const relative of [
  eventPath,
  validationPath,
  schemaPath,
  canonicalTestPath,
  commandsPath,
  commandPortPath,
  mediaDecoratorPath,
  graphqlPath,
  restSharedPath,
  restAdminPath,
  restRouterPath,
  openapiPath,
  searchIngestionPath,
  indexRefreshPath,
  indexPublicationPath,
  lifecycleDocPath,
  auditPath,
]) {
  if (!fs.existsSync(path.join(root, relative))) failures.push(`missing ${relative}`);
}

if (failures.length > 0) {
  console.error("[verify-product-lifecycle-events] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const event = read(eventPath);
const validation = read(validationPath);
const registry = read(schemaPath);
const canonicalTest = read(canonicalTestPath);
const commands = read(commandsPath);
const commandPort = read(commandPortPath);
const mediaDecorator = read(mediaDecoratorPath);
const graphql = read(graphqlPath);
const restShared = read(restSharedPath);
const restAdmin = read(restAdminPath);
const restRouter = read(restRouterPath);
const openapi = read(openapiPath);
const searchIngestion = read(searchIngestionPath);
const indexRefresh = read(indexRefreshPath);
const indexPublication = read(indexPublicationPath);
const lifecycleDoc = read(lifecycleDocPath);
const audit = read(auditPath);

// ── Published contract ────────────────────────────────────────────────────────────────────────
need(
  event,
  `    ProductUnpublished {
        product_id: Uuid,
    },`,
  "DomainEvent variant",
);
need(
  event,
  `    ProductArchived {
        product_id: Uuid,
    },`,
  "DomainEvent variant",
);
need(event, 'Self::ProductUnpublished { .. } => "product.unpublished",', "event_type");
need(event, 'Self::ProductArchived { .. } => "product.archived",', "event_type");

const schemaVersion = slice(event, "    pub fn schema_version(&self) -> u16 {", "    pub fn affects_index");
need(schemaVersion, "Self::ProductUnpublished { .. } => 1,", "schema_version");
need(schemaVersion, "Self::ProductArchived { .. } => 1,", "schema_version");

const affectsIndex = slice(event, "    pub fn affects_index(&self) -> bool {", null);
need(affectsIndex, "Self::ProductUnpublished { .. }", "affects_index");
need(affectsIndex, "Self::ProductArchived { .. }", "affects_index");

need(
  validation,
  `            | Self::ProductUnpublished { product_id }
            | Self::ProductArchived { product_id }`,
  "root event validation",
);

for (const marker of [
  `        event_type: "product.unpublished",`,
  `        event_type: "product.archived",`,
  "fields: PRODUCT_ID_FIELDS,",
]) {
  need(registry, marker, "event registry");
}
// The sample carries each lifecycle event with its product id. rustfmt wraps the literal as soon as
// the line grows, so the rule is the shape of the sample entry, not one line of it.
for (const variant of ["ProductUnpublished", "ProductArchived"]) {
  const sample = new RegExp(`DomainEvent::${variant} \\{\\s*product_id: id\\(\\d+\\)`).test(
    canonicalTest,
  );
  if (!sample) {
    failures.push(
      `canonical contract sample: missing DomainEvent::${variant} { product_id: id(…) }`,
    );
  }
}

// ── Owner commands ───────────────────────────────────────────────────────────────────────────
const unpublish = slice(commands, "pub async fn unpublish_product(", "pub async fn archive_product(");
need(unpublish, "DomainEvent::ProductUnpublished { product_id },", "unpublish owner command");
forbid(unpublish, "DomainEvent::ProductUpdated { product_id },", "unpublish owner command");

const archive = slice(commands, "pub async fn archive_product(", "pub async fn delete_product(");
need(archive, "entities::product::ProductStatus::Archived", "archive owner command");
need(archive, "product_active.published_at = Set(None);", "archive owner command");
need(archive, "DomainEvent::ProductArchived { product_id },", "archive owner command");

const update = slice(commands, "pub async fn update_product(", "pub async fn delete_product(");
need(update, "let will_archive = match input.status.as_ref() {", "update owner command");
need(
  update,
  `        } else if will_archive {
            txn.publish(
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductArchived { product_id },
            )
            .await?;
        } else if will_deactivate {`,
  "update owner command lifecycle event routing",
);
need(update, "DomainEvent::ProductUnpublished { product_id },", "update owner command");

// ── Owner ports and surfaces ─────────────────────────────────────────────────────────────────
need(commandPort, "    async fn archive_product(\n        &self,\n        context: PortContext,\n        product_id: Uuid,\n    ) -> Result<ProductResponse, PortError>;", "command port trait");
need(commandPort, 'let operation = "archive_product";', "command port implementation");
need(mediaDecorator, "self.inner.archive_product(context, product_id).await", "command port decorator");

const graphqlUnpublish = slice(graphql, "async fn unpublish_product(", "async fn archive_product(");
need(graphqlUnpublish, "idempotency_key: String,", "unpublish GraphQL mutation");
need(graphqlUnpublish, ".unpublish_product(port_context.clone(), id)", "unpublish GraphQL mutation");
const graphqlArchive = slice(graphql, "async fn archive_product(", "async fn delete_product(");
need(graphqlArchive, "idempotency_key: String,", "archive GraphQL mutation");
need(graphqlArchive, ".archive_product(port_context.clone(), id)", "archive GraphQL mutation");

for (const marker of [
  "pub async fn archive_product(",
  '"archive_product",',
  ".archive_product(port_context.clone(), id)",
]) {
  need(restShared, marker, "shared REST archive handler");
}
need(restAdmin, "pub async fn archive_product(", "mounted REST archive handler");
need(restAdmin, 'path = "/admin/products/{id}/archive"', "mounted REST archive handler");
need(restAdmin, "super::super::products::archive_product(", "mounted REST archive handler");
need(
  restRouter,
  "axum::routing::post(products::archive_product),",
  "mounted REST archive route",
);
need(openapi, "crate::controllers::admin::archive_product,", "OpenAPI admin surface");

// ── Consumers ────────────────────────────────────────────────────────────────────────────────
const searchHandles = slice(
  searchIngestion,
  "fn handles(&self, event: &DomainEvent) -> bool {",
  "async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {",
);
need(searchHandles, "| DomainEvent::ProductUnpublished { .. }", "search ingestion handles");
need(searchHandles, "| DomainEvent::ProductArchived { .. }", "search ingestion handles");
need(
  searchIngestion,
  `                | DomainEvent::ProductUnpublished { product_id }
                | DomainEvent::ProductArchived { product_id } => {`,
  "search ingestion projection",
);
need(
  searchIngestion,
  `        | DomainEvent::ProductUnpublished { .. }
        | DomainEvent::ProductArchived { .. }`,
  "search ingestion plan",
);

const localeRefreshTarget = slice(
  indexRefresh,
  "pub(crate) fn product_locale_refresh_target(",
  "/// Captures the canonical live and retained-delete state",
);
need(localeRefreshTarget, "| DomainEvent::ProductUnpublished { product_id }", "index refresh target");
need(localeRefreshTarget, "| DomainEvent::ProductArchived { product_id }", "index refresh target");

const publication = slice(
  indexPublication,
  "let root_product_id = match &envelope.event {",
  "if root_product_id != product_id {",
);
need(publication, "| DomainEvent::ProductUnpublished { product_id }", "index refresh publication");
need(publication, "| DomainEvent::ProductArchived { product_id }", "index refresh publication");

// ── Release-process evidence ─────────────────────────────────────────────────────────────────
need(lifecycleDoc, "`product.unpublished`", "lifecycle event doc");
need(lifecycleDoc, "`product.archived`", "lifecycle event doc");
need(
  lifecycleDoc,
  "cargo run --locked -p rustok-events --example event_contract_digests -- --write",
  "lifecycle event doc release gate",
);
need(audit, "PROD-EVENT-001", "audit remediation log");
need(audit, "`ProductUnpublished`", "audit remediation log");

if (failures.length > 0) {
  console.error("[verify-product-lifecycle-events] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

console.log(
  "[verify-product-lifecycle-events] withdrawal and retirement are published as product.unpublished/product.archived, exposed through owner commands, the command port, GraphQL and REST, consumed by search and the index refresh family, and documented with the digest release gate",
);
