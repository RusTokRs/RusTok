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
    "confirmed",
    "retryable",
  ]);
  requireMarkers(files.ports, ['"GroupApplicationBulkReviewCommandPort"']);
  requireMarkers(files.contract, [
    "partial-result batch",
    "between 1 and 50 unique application IDs",
    "independent of request order",
    "admin FFA confirmation/results UI remains open",
  ]);
}

if (failures.length > 0) {
  console.error("Groups application bulk-review boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Groups bounded partial-result bulk review and authorization-order source checks passed.");
