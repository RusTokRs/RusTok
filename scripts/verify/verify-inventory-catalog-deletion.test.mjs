// Regression definitions only. The maintainer runs these; the source guard never does.
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  sourceFiles,
  sourcePaths,
  historicalMigrations,
  verifyInventoryCatalogDeletion,
} from "./verify-inventory-catalog-deletion.mjs";

const repository = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
function fixture(run) {
  const root = fs.mkdtempSync(
    path.join(os.tmpdir(), "catalog-deletion-source-"),
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
    typeof before === "string" ? source.includes(before) : before.test(source),
    `missing mutation anchor: ${before}`,
  );
  fs.writeFileSync(file, source.replace(before, after));
}
function refused(root) {
  assert.ok(verifyInventoryCatalogDeletion({ root }).failures.length > 0);
}

test("source guard makes no compiler, migration, test or Index promotion", () => {
  fixture((root) => {
    const result = verifyInventoryCatalogDeletion({ root });
    assert.deepEqual(result.failures, []);
    assert.equal(result.rust_definitions, 31);
    assert.equal(result.tests_executed, false);
    assert.equal(result.generic_index_complete, false);
  });
  for (const promote of [
    (packet) => {
      packet.compiled = true;
    },
    (packet) => {
      packet.source_regressions.compiled = true;
    },
    (packet) => {
      packet.source_regressions.executed = true;
    },
    (packet) => {
      packet.status = "runtime_certified";
    },
    (packet) => {
      packet.source_regressions.rust_definitions = 0;
    },
    (packet) => {
      packet.source_regressions.node_definitions = 0;
    },
  ]) {
    fixture((root) => {
      const file = path.join(root, sourceFiles.packet);
      const packet = JSON.parse(fs.readFileSync(file, "utf8"));
      promote(packet);
      fs.writeFileSync(file, JSON.stringify(packet));
      refused(root);
    });
  }
});

test("native deletion cannot accept an autocommit connection", () =>
  fixture((root) => {
    mutate(
      root,
      "bootstrap",
      /delete_records_for_variants_in_tx\s*\(\s*txn:\s*&sea_orm::DatabaseTransaction/,
      "delete_records_for_variants_in_tx(\n        txn: &sea_orm::DatabaseConnection",
    );
    refused(root);
  }));

test("the complete SKU request must remain bounded", () =>
  fixture((root) => {
    mutate(
      root,
      "plan",
      "variant_ids.len() > MAX_CATALOG_DELETE_VARIANTS",
      "false",
    );
    refused(root);
  }));

test("zero or soft-deleted reservation history cannot be filtered away", () =>
  fixture((root) => {
    mutate(
      root,
      "plan",
      ".column(reservation_item::Column::Id)",
      ".column(reservation_item::Column::Id).filter(reservation_item::Column::Quantity.gt(0))",
    );
    refused(root);
  }));

test("current levels cannot bypass original-location lock proof", () =>
  fixture((root) => {
    mutate(
      root,
      "plan",
      /current_read\(\s*txn,\s*level_query\(&item_ids\)\s*\)/,
      "level_query(&item_ids)",
    );
    refused(root);
  }));

test("incoming quantities cannot be mistaken for empty inventory", () =>
  fixture((root) => {
    mutate(root, "plan", "self.incoming_quantity == 0", "true");
    refused(root);
  }));

test("both conditional deletion families require exact-one admission", () =>
  fixture((root) => {
    mutate(
      root,
      "plan",
      "deleted.rows_affected != 1",
      "deleted.rows_affected > 1",
    );
    refused(root);
  }));

test("Product cannot drop the current tenant at the native hook", () =>
  fixture((root) => {
    mutate(
      root,
      "commands",
      /delete_records_for_variants_in_tx\(\s*&txn,\s*tenant_id,/,
      "delete_records_for_variants_in_tx(&txn,",
    );
    refused(root);
  }));

test("known owner code still needs the declared error kind", () =>
  fixture((root) => {
    mutate(
      root,
      "productError",
      "error.kind == kind && error.code == code",
      "error.code == code",
    );
    refused(root);
  }));

test("new lookup indexes cannot justify changing historical migrations", () =>
  fixture((root) => {
    fs.appendFileSync(
      path.join(root, historicalMigrations[0]),
      "\n// forbidden historical rewrite\n",
    );
    refused(root);
  }));
