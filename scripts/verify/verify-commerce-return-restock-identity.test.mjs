// Maintainer-run mutation regressions. This module is not executed by the agent.
import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  sourceFiles,
  sourcePaths,
  verifyCommerceReturnRestockIdentity,
} from "./verify-commerce-return-restock-identity.mjs";

const repository = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
function fixture(run) {
  const root = fs.mkdtempSync(
    path.join(os.tmpdir(), "return-restock-identity-"),
  );
  try {
    for (const file of sourcePaths) {
      const target = path.join(root, file);
      fs.mkdirSync(path.dirname(target), { recursive: true });
      fs.copyFileSync(path.join(repository, file), target);
    }
    return run(root);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}
function replace(root, key, from, to) {
  const file = path.join(root, sourceFiles[key]);
  const source = fs.readFileSync(file, "utf8"),
    changed = source.replace(from, to);
  assert.notEqual(changed, source, `mutation did not change ${key}`);
  fs.writeFileSync(file, changed);
}
function rejected(root, label) {
  const result = verifyCommerceReturnRestockIdentity({ root });
  assert.equal(result.status, "failed");
  assert.ok(
    result.failures.some((failure) => failure.includes(label)),
    result.failures.join("\n"),
  );
}

test("current source has one bounded return-stock identity contract, not runtime certification", () => {
  fixture((root) =>
    assert.deepEqual(
      verifyCommerceReturnRestockIdentity({ root }).failures,
      [],
    ),
  );
});
test("tenant proof cannot be removed before tenant-less item locks", () => {
  fixture((root) => {
    replace(
      root,
      "inventory",
      "load_tenant_variant(&context, RESTOCK_OPERATION, &txn, tenant_id, *variant_id)",
      "load_unscoped_variant(&context, RESTOCK_OPERATION, &txn, *variant_id)",
    );
    rejected(root, "tenant proof before item lock");
  });
});
test("the native command must retain explicit saved Variant identity", () => {
  fixture((root) => {
    replace(
      root,
      "inventory",
      "pub variant_id: Option<Uuid>",
      "pub missing_variant: Option<Uuid>",
    );
    rejected(root, "explicit saved SKU binding");
  });
});
test("first-row reservation selection cannot replace a two-candidate ambiguity proof", () => {
  fixture((root) => {
    replace(root, "inventory", /\.all\(&txn\)/, ".one(&txn)");
    rejected(root, "bounded ambiguity proof");
  });
});
test("a missing tracked reservation cannot become an untracked success", () => {
  fixture((root) => {
    replace(
      root,
      "inventory",
      "if reservations.len() != 1 {",
      "if reservations.is_empty() { outcome.untracked.push(line.return_item_id); continue; } if reservations.len() != 1 {",
    );
    rejected(root, "only explicit SKU-less binding");
  });
});
test("reservation identity must be constrained by the tenant-proved item", () => {
  fixture((root) => {
    replace(
      root,
      "inventory",
      "reservation_item::Column::InventoryItemId.eq(item.id)",
      "reservation_item::Column::InventoryItemId.eq(unscoped_item.id)",
    );
    rejected(root, "Inventory identity/atomicity");
  });
});
test("changed selected quantity cannot count as an exact ledger replay", () => {
  fixture((root) => {
    replace(root, "inventory", "|| saved.quantity != line.quantity", "");
    rejected(root, "complete ledger replay correlation");
  });
});
test("receipt reads must remain tenant-scoped even behind a global primary key", () => {
  fixture((root) => {
    replace(
      root,
      "inventory",
      ".filter(ledger::Column::TenantId.eq(tenant_id))",
      "",
    );
    rejected(root, "tenant-scoped receipt reads");
  });
});
test("saved location ownership cannot disappear behind archived-location support", () => {
  fixture((root) => {
    replace(
      root,
      "inventory",
      ".filter(stock_location::Column::TenantId.eq(tenant_id))",
      "",
    );
    rejected(root, "saved location tenant proof");
  });
});
test("terminal release and disposal history cannot be ignored", () => {
  fixture((root) => {
    replace(
      root,
      "inventory",
      "|| has_terminal_disposition(&reservation.metadata)",
      "",
    );
    rejected(root, "retained allocation facts");
  });
});
test("Order-owned binding read cannot drop its joined tenant scope", () => {
  fixture((root) => {
    replace(
      root,
      "query",
      ".filter(order::Column::TenantId.eq(tenant_id))",
      "",
    );
    rejected(root, "tenant-proved bounded price-free saved projection");
  });
});
test("an embedded Order reader cannot replace the selected capability", () => {
  fixture((root) => {
    replace(
      root,
      "commerce",
      "order_reads.read_order_line_inventory_bindings(",
      "in_process_order_read_port().read_order_line_inventory_bindings(",
    );
    rejected(root, "no foreign rows, fallback, reprice or compensation");
  });
});
test("a partial owner reply cannot satisfy complete binding correlation", () => {
  fixture((root) => {
    replace(
      root,
      "commerce",
      "if snapshot.items.len() != expected.len() {",
      "if false {",
    );
    rejected(root, "complete saved SKU/quantity response correlation");
  });
});
test("a ready provider cannot bypass positive-deadline preflight", () => {
  fixture((root) => {
    replace(root, "commerce", "context.require_policy(policy)?;", "");
    rejected(root, "canonical positive-deadline policy");
  });
});
test("stock cannot be applied before the completed return matches the frozen plan", () => {
  fixture((root) => {
    replace(root, "commerce", "plan.validate_completed(completed)?;", "");
    rejected(root, "completion proof before stock effect");
  });
});
test("false owner retry advice cannot become terminal no-effect inference", () => {
  fixture((root) => {
    replace(
      root,
      "orchestration",
      "_ => FailureDisposition::Reconciliation,",
      "_ => FailureDisposition::Failed,",
    );
    rejected(root, "false advice parks same identity");
  });
});
test("source evidence cannot silently claim executed runtime or complete step eight", () => {
  fixture((root) => {
    const file = path.join(root, sourceFiles.evidence);
    const value = JSON.parse(fs.readFileSync(file, "utf8"));
    value.runtime_executed = true;
    value.step8_complete = true;
    fs.writeFileSync(file, JSON.stringify(value));
    rejected(root, "source-only and open step-8 status");
  });
});

test("omitted nullable Variant fields cannot silently become SKU-less owner proof", () => {
  fixture((root) => {
    replace(
      root,
      "order",
      '#[serde(deserialize_with = "deserialize_saved_variant_id")]',
      "",
    );
    rejected(root, "SKU-less null requires explicit field presence");
  });
});

test("financial-only returns cannot acquire an unnecessary stock-quota dependency", () => {
  fixture((root) => {
    replace(root, "commerce", "if explicit_no_goods(decision) {", "if false {");
    rejected(
      root,
      "explicit no-goods decision is not a stock-quota or availability dependency",
    );
  });
});

test("fresh stock cannot bypass receiving-location preflight behind replay ownership proof", () => {
  fixture((root) => {
    replace(
      root,
      "inventory",
      "lock_receiving_locations(&txn, &context, tenant_id, &movements).await?;",
      "",
    );
    rejected(
      root,
      "fresh receiving locations are proved before levels or receipts",
    );
  });
});

test("archived receiving locations cannot fall through to a retryable storage error", () => {
  fixture((root) => {
    replace(root, "inventory", "if !location.is_active() {", "if false {");
    rejected(
      root,
      "archived receiving locations require reconciliation before writes",
    );
  });
});

test("receiving-location archival must be serialized against the fresh stock write", () => {
  fixture((root) => {
    replace(
      root,
      "inventory",
      "receiving_query.lock_exclusive()",
      "receiving_query",
    );
    rejected(root, "receiving location lock serializes concurrent archival");
  });
});

test("receiving locations cannot acquire locks in caller-dependent duplicate order", () => {
  fixture((root) => {
    replace(
      root,
      "inventory",
      /\.map\(\|movement\| movement.location_id\)\s*\.collect::<BTreeSet<_>>\(\)/,
      ".map(|movement| movement.location_id).collect::<Vec<_>>()",
    );
    rejected(root, "receiving location locks have stable distinct UUID order");
  });
});
