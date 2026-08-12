#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");

const checks = [
  {
    file: "crates/rustok-pages/tests/integration.rs",
    label: "page CRUD and body sanitization contract tests",
    tokens: [
      "page_create_emits_domain_event",
      "document: serde_json::json!",
    ],
  },
  {
    file: "crates/rustok-pages/src/dto/page.rs",
    label: "single Page Builder document write DTO",
    tokens: [
      "pub struct PageBodyInput {\n    pub locale: String,\n    pub document: Value,\n}",
    ],
  },
  {
    file: "crates/rustok-pages/src/graphql/types.rs",
    label: "single Page Builder document GraphQL input",
    tokens: [
      "pub struct GqlPageBodyInput {\n    pub locale: String,\n    pub document: Value,\n}",
    ],
  },
  {
    file: "crates/rustok-pages/admin/src/transport/graphql_adapter.rs",
    label: "single Page Builder document admin transport",
    tokens: [
      "struct PageBodyWriteInput {\n    locale: String,\n    document: Value,\n}",
      "document: draft.document",
      "document: project_data",
    ],
  },
  {
    file: "crates/rustok-pages/tests/page_builder_roundtrip.rs",
    label: "visual builder and page content bridge contract tests",
    tokens: [
      "pages_write_api_is_split_by_owner_and_revision",
      "document_and_metadata_services_cannot_cross_write",
      "non_builder_publish_checks_locked_document_revisions_before_transition",
      "current_fly_tree_remains_the_only_document_authority",
    ],
  },
  {
    file: "crates/rustok-pages/tests/page_service_kind_guard.rs",
    label: "Pages lifecycle and Page Builder ownership guard tests",
    tokens: [
      "lifecycle_operations_reject_unknown_page_ids",
      "explicit_non_builder_publish_and_unpublish_advance_metadata_version",
      "non_builder_lifecycle_rejects_builder_documents_with_stable_code",
      "published_pages_must_be_unpublished_before_delete",
    ],
  },
  {
    file: "crates/rustok-pages/tests/page_locale_fallback.rs",
    label: "page locale fallback contract tests",
    tokens: [
      "get_by_slug_falls_back_to_platform_locale",
      "get_by_slug_respects_explicit_fallback_locale",
      "get_with_locale_fallback_normalizes_requested_and_fallback_locale",
    ],
  },
  {
    file: "crates/rustok-pages/tests/rbac.rs",
    label: "RBAC and channel visibility contract tests",
    tokens: [
      "manager_cannot_publish_during_create_or_non_builder_lifecycle_transition",
      "customer_reads_only_published_pages",
    ],
  },
  {
    file: "crates/rustok-pages/tests/contract_surface.rs",
    label: "manifest and external builder contract drift tests",
    tokens: [
      "module_manifest_declares_fba_builder_consumer_contract",
      "builder_degraded_modes_bind_to_typed_error_catalog",
      "pages_consumer_version_satisfies_provider_minimum",
    ],
  },
];

function fail(message) {
  console.error("[verify-page-builder-pages-contract-surface] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
}

for (const check of checks) {
  const filePath = path.join(repoRoot, check.file);
  if (!fs.existsSync(filePath)) {
    fail(`${check.label}: missing file ${check.file}`);
  }
  const content = fs.readFileSync(filePath, "utf8").replace(/\r\n/g, "\n");
  for (const token of check.tokens) {
    if (!content.includes(token)) {
      fail(`${check.label}: ${check.file} missing token '${token}'`);
    }
  }
  console.log(`[verify-page-builder-pages-contract-surface] ${check.label}: PASS`);
}

console.log("[verify-page-builder-pages-contract-surface] PASS");
