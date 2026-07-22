import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const failures = [];
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");
const files = {
  composition: "crates/rustok-groups/src/applications.rs",
  review: "crates/rustok-groups/src/applications_review.rs",
  owner: "crates/rustok-groups/src/applications_bulk_review.rs",
  graphqlRoot: "crates/rustok-groups/src/graphql_application_cas.rs",
  graphql: "crates/rustok-groups/src/graphql_application_bulk_review.rs",
  ports: "crates/rustok-groups/src/ports.rs",
  adminCore: "crates/rustok-groups/admin/src/application_bulk_core.rs",
  adminModel: "crates/rustok-groups/admin/src/application_bulk_model.rs",
  adminTransport: "crates/rustok-groups/admin/src/application_bulk_transport.rs",
  nativeAdapter: "crates/rustok-groups/admin/src/transport/native_application_bulk_review_adapter.rs",
  graphqlAdapter: "crates/rustok-groups/admin/src/transport/graphql_application_bulk_review_adapter.rs",
  adminUi: "crates/rustok-groups/admin/src/ui/application_bulk_review.rs",
  adminRoot: "crates/rustok-groups/admin/src/ui/root.rs",
  localeEn: "crates/rustok-groups/admin/locales/en.json",
  localeRu: "crates/rustok-groups/admin/locales/ru.json",
  contract: "crates/rustok-groups/docs/bulk-review-contract.md",
};

for (const relative of Object.values(files)) {
  if (!fs.existsSync(path.join(root, relative))) {
    failures.push(`missing Groups bulk-review artifact: ${relative}`);
  }
}

const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) {
      failures.push(`${relative}: missing marker ${JSON.stringify(marker)}`);
    }
  }
};

if (failures.length === 0) {
  requireMarkers(files.composition, ['include!("applications_bulk_review.rs")']);
  requireMarkers(files.owner, [
    "MAX_BULK_REVIEW_ITEMS: usize = 50",
    "GroupApplicationBulkReviewCommandPort",
    "confirmed",
    "groups.bulk_review_confirmation_required",
    "groups.bulk_review_limit_exceeded",
    "groups.bulk_review_duplicate_application",
    "normalize_optional_note",
    "review_application_authorized_owned",
    "BulkReviewGroupMembershipApplicationItemResult",
    "succeeded",
    "failed",
    "bulk_review_item_idempotency_key",
    "application_id.as_bytes()",
  ]);
  for (const forbidden of [
    "review_application_owned",
    "index.to_be_bytes()",
    "DatabaseTransaction",
    "transaction.commit",
  ]) {
    if (read(files.owner).includes(forbidden)) {
      failures.push(`${files.owner}: forbidden unsafe batch marker ${JSON.stringify(forbidden)}`);
    }
  }

  const reviewSource = read(files.review);
  const reviewStart = reviewSource.indexOf("async fn review_application_authorized_owned");
  const authorize = reviewSource.indexOf("authorize_application_review", reviewStart);
  const statusCheck = reviewSource.indexOf(
    "application_model.status != GroupApplicationStatus::Pending.as_str()",
    reviewStart,
  );
  if (!(reviewStart >= 0 && authorize > reviewStart && statusCheck > authorize)) {
    failures.push(
      `${files.review}: manager authorization must occur before pending-status disclosure`,
    );
  }

  requireMarkers(files.graphqlRoot, [
    'include!("graphql_application_bulk_review.rs")',
    "GroupsApplicationBulkReviewMutation",
  ]);
  requireMarkers(files.graphql, [
    "bulk_review_group_membership_applications",
    "GroupApplicationBulkReviewCommandPort",
    "BulkReviewGroupMembershipApplicationsInputGql",
    "BULK_REVIEW_PORT_DEADLINE",
    "Duration::from_secs(30)",
    "bulk_review_port_context",
    "confirmed",
    "retryable",
  ]);
  requireMarkers(files.ports, ['"GroupApplicationBulkReviewCommandPort"']);

  requireMarkers(files.adminCore, [
    "MAX_BULK_REVIEW_ITEMS: usize = 50",
    "ConfirmationRequired",
    "prepare_bulk_review_group_membership_applications",
    "groups-admin-bulk-review-",
  ]);
  requireMarkers(files.adminModel, [
    "BulkReviewGroupMembershipApplicationsCommand",
    "GroupsAdminBulkReviewApplicationItemResult",
    "GroupsAdminBulkReviewApplicationsResult",
  ]);
  requireMarkers(files.adminTransport, [
    '"groups.admin.applications.bulk_review"',
    "execute_selected_transport",
    "UiTransportPath::NativeServer",
    "UiTransportPath::Graphql",
  ]);
  requireMarkers(files.nativeAdapter, [
    'endpoint = "groups/admin/applications/bulk-review"',
    "GroupApplicationBulkReviewCommandPort",
    "with_idempotency_key",
    "Duration::from_secs(30)",
    "let succeeded = result.succeeded",
    "let failed = result.failed",
    "let items = result.items",
  ]);
  requireMarkers(files.graphqlAdapter, [
    "bulkReviewGroupMembershipApplications",
    "BulkReviewGroupMembershipApplicationsInputGql",
    "applicationIds",
    "let BulkReviewWire",
    "retryable",
  ]);
  requireMarkers(files.adminUi, [
    "GroupsApplicationsBulkReviewAdmin",
    "MAX_BULK_REVIEW_ITEMS: usize = 50",
    "event_target_checked",
    "confirmed",
    "selected_ids.get().is_empty()",
    "aria-live=\"polite\"",
    "GroupsAdminBulkReviewApplicationItemResult",
    "groups.admin.applications.bulk.selectApplication",
  ]);
  requireMarkers(files.adminRoot, ["<GroupsApplicationsBulkReviewAdmin />"]);
  for (const locale of [files.localeEn, files.localeRu]) {
    requireMarkers(locale, [
      '"groups.admin.applications.bulk.title"',
      '"groups.admin.applications.bulk.selectApplication"',
      '"groups.admin.applications.bulk.confirm"',
      '"groups.admin.applications.bulk.succeeded"',
      '"groups.admin.applications.bulk.failed"',
    ]);
  }

  requireMarkers(files.contract, [
    "partial-result batch",
    "between 1 and 50 unique application IDs",
    "independent of request order",
    "The module-owned admin package provides",
    "runtime, parity, replay, concurrency, accessibility, and recovery evidence remains open",
  ]);
}

if (failures.length > 0) {
  console.error("Groups application bulk-review boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Groups bounded partial-result bulk review, deadline parity, FFA composition, localization, and authorization-order source checks passed.");
