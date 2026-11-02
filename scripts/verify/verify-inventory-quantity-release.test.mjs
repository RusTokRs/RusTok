// Regression definitions only. The maintainer runs this file; source checks must not import it.
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  sourceFiles,
  sourcePaths,
  verifyInventoryQuantityRelease,
} from "./verify-inventory-quantity-release.mjs";

const repository = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
function fixture(run) {
  const root = fs.mkdtempSync(
    path.join(os.tmpdir(), "quantity-release-source-"),
  );
  try {
    for (const file of sourcePaths) {
      const target = path.join(root, file);
      fs.mkdirSync(path.dirname(target), { recursive: true });
      fs.copyFileSync(path.join(repository, file), target);
    }
    run(root);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}
function mutate(root, key, before, after) {
  const file = path.join(root, sourceFiles[key]);
  const source = fs.readFileSync(file, "utf8");
  assert.ok(
    typeof before === "string" ? source.includes(before) : source.match(before),
    `mutation anchor missing: ${before}`,
  );
  fs.writeFileSync(file, source.replace(before, after));
}
function refused(root) {
  assert.ok(verifyInventoryQuantityRelease({ root }).failures.length > 0);
}

test("current anonymous quantity source contract is explicit, not runtime evidence", () =>
  fixture((root) => {
    const result = verifyInventoryQuantityRelease({ root });
    assert.deepEqual(result.failures, []);
    assert.equal(result.tests_executed, false);
    assert.equal(result.sql_executed, false);
    assert.equal(result.step8_complete, false);
  }));

test("unkeyed pool query cannot include external-bound reservations", () =>
  fixture((root) => {
    mutate(
      root,
      "service",
      ".filter(entities::reservation_item::Column::ExternalId.is_null())",
      "",
    );
    refused(root);
  }));

test("terminal false and zero cannot be reinterpreted as missing history", () =>
  fixture((root) => {
    mutate(
      root,
      "plan",
      "crate::reservation_adoption::has_terminal_disposition(&row.metadata)",
      "false",
    );
    refused(root);
  }));

test("shared-level protection covers all claims rather than just released quantity", () =>
  fixture((root) => {
    mutate(
      root,
      "plan",
      "i64::from(level.reserved_quantity) < current_claims",
      "i64::from(level.reserved_quantity) < i64::from(released)",
    );
    refused(root);
  }));

test("original-level binding cannot become another location selection", () =>
  fixture((root) => {
    mutate(
      root,
      "service",
      "Column::LocationId.is_in(location_ids.clone())",
      "Column::LocationId.is_in(Vec::<Uuid>::new())",
    );
    refused(root);
  }));

test("scoped compatibility commands cannot silently discard Cart identity", () =>
  fixture((root) => {
    mutate(
      root,
      "ports",
      /require_anonymous_quantity_scope\(\s*request\.cart_id\.is_some\(\),\s*request\.line_item_id\.is_some\(\),?\s*\)\?;/,
      "",
    );
    refused(root);
  }));

test("suppressed level updates cannot count as release success", () =>
  fixture((root) => {
    mutate(
      root,
      "plan",
      "updated.rows_affected != 1",
      "updated.rows_affected > 1",
    );
    refused(root);
  }));

test("checked signed response conversion cannot become truncation", () =>
  fixture((root) => {
    mutate(root, "service", "i32::try_from(total)", "Ok(total as i32)");
    refused(root);
  }));

test("fresh anonymous reservations cannot exceed the release pool bound", () =>
  fixture((root) => {
    mutate(
      root,
      "plan",
      "rows.len() >= QUANTITY_RESERVATION_ROW_LIMIT",
      "rows.len() > QUANTITY_RESERVATION_ROW_LIMIT",
    );
    refused(root);
  }));

test("source evidence cannot promote notification delivery or step eight", () =>
  fixture((root) => {
    const file = path.join(root, sourceFiles.evidence);
    const record = JSON.parse(fs.readFileSync(file, "utf8"));
    record.notification_delivery_certified = true;
    record.step8_complete = true;
    fs.writeFileSync(file, JSON.stringify(record, null, 2));
    refused(root);
  }));
