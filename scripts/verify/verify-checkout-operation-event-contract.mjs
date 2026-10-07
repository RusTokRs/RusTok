#!/usr/bin/env node

// The checkout operation journal is the only writer of `checkout_operations.status`, so the
// `checkout.operation.parked` / `checkout.operation.reconciled` events are the only way a consumer
// can observe a checkout that needs an operator or that an operator closed. This verifier locks the
// three things that make that guarantee true:
//
//   1. the typed family is registered in the canonical event contract (`rustok-events`);
//   2. the journal publishes the family through `publish_contract_in_tx` inside the same
//      transaction that writes the status, never after the commit;
//   3. the bounded park vocabulary has exactly one definition — the one shared with the
//      `rustok_checkout_reconciliation_parked_total` metric label — so a new reason cannot reach
//      the metric without reaching the event payload;
//   4. the operator decision that closes a parked operation is appended by the same transaction
//      that closes it, so the two states ("parked, no decision" and "closed, decision recorded")
//      are the only ones that can be observed.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");

const files = {
  family: "crates/libs/rustok-events/src/checkout_operation.rs",
  contract: "crates/libs/rustok-events/src/contract.rs",
  eventsLib: "crates/libs/rustok-events/src/lib.rs",
  journal: "crates/modules/rustok-commerce/src/services/checkout_operation.rs",
  reconciliation:
    "crates/modules/rustok-commerce/src/services/checkout_reconciliation.rs",
  telemetry: "crates/libs/rustok-telemetry/src/metrics.rs",
  admission:
    "crates/libs/rustok-events/docs/event-contract-digest-admission.md",
};

const failures = [];

const read = (file) => {
  const absolute = path.join(repoRoot, file);
  if (!fs.existsSync(absolute)) {
    failures.push(`${file}: missing`);
    return "";
  }
  return fs.readFileSync(absolute, "utf8");
};

const requireMarker = (source, marker, file) => {
  if (!source.includes(marker)) failures.push(`${file}: missing ${marker}`);
};

const forbidMatch = (source, pattern, file, description) => {
  if (pattern.test(source)) failures.push(`${file}: forbidden ${description}`);
};

const sources = Object.fromEntries(
  Object.entries(files).map(([key, file]) => [key, read(file)]),
);

// 1. The typed family and its bounded vocabulary.
requireMarker(
  sources.family,
  'pub const CHECKOUT_OPERATION_PARKED_EVENT_TYPE: &str = "checkout.operation.parked";',
  files.family,
);
requireMarker(
  sources.family,
  'pub const CHECKOUT_OPERATION_RECONCILED_EVENT_TYPE: &str = "checkout.operation.reconciled";',
  files.family,
);
for (const reason of ["manual_reconciliation", "attempts_exhausted"]) {
  requireMarker(
    sources.family,
    `pub const CHECKOUT_OPERATION_PARK_REASON_${reason.toUpperCase()}: &str = "${reason}";`,
    files.family,
  );
}
for (const outcome of ["compensation_required", "compensated", "failed"]) {
  requireMarker(
    sources.family,
    `pub const CHECKOUT_OPERATION_OUTCOME_${outcome.toUpperCase()}: &str = "${outcome}";`,
    files.family,
  );
}
for (const marker of [
  "impl EventContract for CheckoutOperationEvent",
  "fn into_contract_payload(self) -> ContractEventPayload",
  "impl ValidateEvent for CheckoutOperationEvent",
  "validators::validate_not_nil_uuid(\"operation_id\", operation_id)?",
  "validators::validate_not_nil_uuid(\"cart_id\", cart_id)?",
  "validators::validate_not_nil_uuid(\"operator_id\", operator_id)?",
  "validate_bounded_label(\"reason\", reason, CHECKOUT_OPERATION_PARK_REASONS)",
  "validate_bounded_label(\"outcome\", outcome, CHECKOUT_OPERATION_OUTCOMES)",
]) {
  requireMarker(sources.family, marker, files.family);
}

// 2. Registration in the sealed contract payload and the crate-wide schema surface.
requireMarker(
  sources.contract,
  "CheckoutOperation(CheckoutOperationEvent)",
  files.contract,
);
for (const arm of [
  "Self::CheckoutOperation(event) => event.event_type()",
  "Self::CheckoutOperation(event) => event.schema_version()",
  "Self::CheckoutOperation(event) => event.validate()",
]) {
  requireMarker(sources.contract, arm, files.contract);
}
requireMarker(sources.eventsLib, "mod checkout_operation;", files.eventsLib);
requireMarker(
  sources.eventsLib,
  ".or_else(|| checkout_operation_event_schema(event_type))",
  files.eventsLib,
);
requireMarker(
  sources.eventsLib,
  ".chain(CHECKOUT_OPERATION_EVENT_SCHEMAS.iter())",
  files.eventsLib,
);
requireMarker(
  sources.eventsLib,
  "checkout_operation_event_schema,",
  files.eventsLib,
);

// 3. In-transaction publication from the journal that owns the status write.
requireMarker(
  sources.journal,
  "CHECKOUT_OPERATION_PARK_REASON_MANUAL_RECONCILIATION as PARK_REASON_MANUAL_RECONCILIATION",
  files.journal,
);
requireMarker(
  sources.journal,
  "CHECKOUT_OPERATION_PARK_REASON_ATTEMPTS_EXHAUSTED as PARK_REASON_ATTEMPTS_EXHAUSTED",
  files.journal,
);
requireMarker(
  sources.journal,
  "CHECKOUT_OPERATION_OUTCOME_COMPENSATION_REQUIRED as RECONCILED_OUTCOME_COMPENSATION_REQUIRED",
  files.journal,
);
requireMarker(
  sources.journal,
  "CheckoutOperationEvent::Parked {",
  files.journal,
);
requireMarker(
  sources.journal,
  "CheckoutOperationEvent::Reconciled {",
  files.journal,
);
requireMarker(
  sources.journal,
  "self.event_bus\n            .publish_contract_in_tx(",
  files.journal,
);
const methodRanges = [];
{
  const methodPattern = /^ {4}(?:pub )?(?:async )?fn (\w+)/gm;
  let match = methodPattern.exec(sources.journal);
  while (match) {
    methodRanges.push({ name: match[1], start: match.index });
    match = methodPattern.exec(sources.journal);
  }
}
const methodBody = (name) => {
  const index = methodRanges.findIndex((method) => method.name === name);
  if (index === -1) return undefined;
  const start = methodRanges[index].start;
  const end =
    index + 1 < methodRanges.length
      ? methodRanges[index + 1].start
      : sources.journal.length;
  return sources.journal.slice(start, end);
};
const publishInTransaction = /\.publish_(?:parked|reconciled)\(\s*&txn/s;
if (
  !publishInTransaction.test(sources.journal) &&
  !sources.journal.includes("CheckoutOperationEvent::Parked {")
) {
  failures.push(`${files.journal}: missing in-transaction publication helpers`);
}
// The status write, the event write and the commit are ordered: every writer that publishes an
// event must commit after the publication, so a rolled-back status transition cannot be announced.
for (const writer of [
  "park_exhausted_compensation",
  "resolve_reconciliation_required",
  "request_compensation_retry",
]) {
  const writerBody = methodBody(writer);
  if (writerBody === undefined) {
    failures.push(`${files.journal}: missing writer ${writer}`);
    continue;
  }
  const commitIndex = writerBody.indexOf("txn.commit().await?");
  if (commitIndex === -1) {
    failures.push(`${files.journal}: ${writer} must commit the transaction`);
    continue;
  }
  const publishMatch = publishInTransaction.exec(writerBody);
  if (!publishMatch) {
    failures.push(`${files.journal}: ${writer} must publish inside the transaction`);
    continue;
  }
  if (publishMatch.index > commitIndex) {
    failures.push(
      `${files.journal}: ${writer} publishes after the commit, so a rollback would not retract the event`,
    );
  }
}
if (!publishInTransaction.test(methodBody("release_lease_with_error") || "")) {
  failures.push(
    `${files.journal}: the manual-reconciliation park path must publish the parked event in its transaction`,
  );
}

// 4. The decision row and the transition it explains are one unit of work.
for (const writer of ["resolve_reconciliation_required", "request_compensation_retry"]) {
  const writerBody = methodBody(writer);
  if (writerBody === undefined) {
    failures.push(`${files.journal}: missing writer ${writer}`);
    continue;
  }
  const recordIndex = writerBody.indexOf("append_reconciliation_decision(&txn, &");
  const commitIndex = writerBody.indexOf("txn.commit().await?");
  if (recordIndex === -1) {
    failures.push(
      `${files.journal}: ${writer} must append the operator decision in the same transaction`,
    );
  } else if (commitIndex !== -1 && recordIndex > commitIndex) {
    failures.push(
      `${files.journal}: ${writer} appends the operator decision after the commit, so a rolled-back close would leave the row behind`,
    );
  }
}
requireMarker(
  sources.journal,
  "async fn append_reconciliation_decision<C>(",
  files.journal,
);
// Single writer: the service that drives the operator workflow closes the
// operation through the journal instead of writing the append-only table itself.
forbidMatch(
  sources.reconciliation,
  /checkout_reconciliation_action::ActiveModel/,
  files.reconciliation,
  "the reconciliation service writing the decision table directly",
);
for (const delegation of [
  ".resolve_reconciliation_required(",
  ".request_compensation_retry(",
]) {
  requireMarker(sources.reconciliation, delegation, files.reconciliation);
}

// 4. One vocabulary: the metric label must be the contract label, never a local literal.
requireMarker(
  sources.telemetry,
  '"rustok_checkout_reconciliation_parked_total"',
  files.telemetry,
);
forbidMatch(
  sources.journal,
  /record_checkout_reconciliation_parked\(\s*"/,
  files.journal,
  "a hand-written park label; the metric label must be the rustok-events park reason constant",
);
forbidMatch(
  sources.journal,
  /const PARK_REASON_[A-Z_]+: &str = "/,
  files.journal,
  "a local park-reason constant; the bounded vocabulary lives in rustok-events",
);

// 5. A new family changes the digest artifact, so the admission doc must name the generator run.
requireMarker(
  sources.admission,
  "cargo run --locked -p rustok-events --example event_contract_digests -- --write",
  files.admission,
);

if (failures.length > 0) {
  console.error("checkout operation event contract verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("checkout operation event contract verified");
