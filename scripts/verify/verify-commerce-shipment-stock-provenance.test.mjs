// Maintainer-run source mutations only. The agent has not executed this module.
import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  sourceFiles,
  sourcePaths,
  verifyCommerceShipmentStockProvenance,
} from "./verify-commerce-shipment-stock-provenance.mjs";

const repository = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
function fixture(run) {
  const root = fs.mkdtempSync(
    path.join(os.tmpdir(), "shipment-stock-provenance-"),
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
  const source = fs.readFileSync(file, "utf8");
  const changed = source.replace(from, to);
  assert.notEqual(changed, source, `mutation did not change ${key}`);
  fs.writeFileSync(file, changed);
}
function rejected(root, label) {
  const result = verifyCommerceShipmentStockProvenance({ root });
  assert.equal(result.status, "failed");
  assert.ok(
    result.failures.some((failure) => failure.includes(label)),
    result.failures.join("\n"),
  );
}

test("current source has one bounded owner contract, including the mounted manual workflow, without runtime certification", () => {
  fixture((root) =>
    assert.deepEqual(
      verifyCommerceShipmentStockProvenance({ root }).failures,
      [],
    ),
  );
});
test("the consumer protocol cannot silently lose its complete-batch bound", () => {
  fixture((root) => {
    replace(
      root,
      "port",
      "MAX_SHIPMENT_STOCK_ITEMS: usize = 1024",
      "MAX_SHIPMENT_STOCK_ITEMS: usize = 4096",
    );
    rejected(root, "consumer-owned bounded protocol");
  });
});
test("selected peer preparation cannot move under a Fulfillment write transaction", () => {
  fixture((root) => {
    replace(
      root,
      "stock",
      "let mut context = context;",
      "let txn = self.db.begin().await?; let mut context = context;",
    );
    rejected(root, "peer I/O outside F write transaction");
  });
});
test("the whole Variant batch must be tenant-proved before tenant-less item reads", () => {
  fixture((root) => {
    replace(root, "scope", /load_tenant_variant\(/g, "load_unscoped_variant(");
    rejected(root, "prove the whole Variant batch first");
  });
});
test("native proof cannot accept a changed saved Variant", () => {
  fixture((root) => {
    replace(root, "scope", "saved.variant_id != line.variant_id", "false");
    rejected(root, "complete native provenance, allocation and backend checks");
  });
});
test("an unavailable selected Order reader cannot become an embedded native fallback", () => {
  fixture((root) => {
    replace(root, "adapter", /^/, "in_process_order_read_port(db);\n");
    rejected(root, "no embedded selected O fallback");
  });
});
test("Pending claim CAS cannot ignore retained commit evidence", () => {
  fixture((root) => {
    replace(
      root,
      "journal",
      ".filter(provider_operation::Column::CommittedAt.is_null())",
      "",
    );
    rejected(root, "claim excludes every retained outcome marker");
  });
});
test("legacy ERROR cannot regain automatic carrier execution in the private transition matrix", () => {
  fixture((root) => {
    replace(
      root,
      "journal",
      "PROVIDER_OPERATION_EXECUTING => from == PROVIDER_OPERATION_PENDING",
      "PROVIDER_OPERATION_EXECUTING => matches!(from, PROVIDER_OPERATION_PENDING | PROVIDER_OPERATION_ERROR)",
    );
    rejected(root, "legacy ERROR is not permission to execute");
  });
});
test("malformed Pending cannot hide a retained committed marker from the shared fence", () => {
  fixture((root) => {
    replace(root, "journal", ".add(Column::CommittedAt.is_not_null())", "");
    rejected(
      root,
      "malformed Pending and incomplete Committed cannot hide effects",
    );
  });
});
test("local progress cannot replace the shared owner fence with a status-only shortcut", () => {
  fixture((root) => {
    replace(
      root,
      "stock",
      ".filter(super::super::provider_operation::unresolved_provider_effects())",
      "",
    );
    rejected(root, "same owner predicate fences local progress");
  });
});
test("explicit no-effect resolution must requeue the same identity Pending, not ambiguous ERROR", () => {
  fixture((root) => {
    replace(
      root,
      "journalRecovery",
      "Expr::value(PROVIDER_OPERATION_PENDING)",
      "Expr::value(PROVIDER_OPERATION_ERROR)",
    );
    rejected(
      root,
      "explicit no-effect recovery retains identity and requeues Pending",
    );
  });
});
test("operator no-effect resolution cannot discard a retained provider reference", () => {
  fixture((root) => {
    replace(
      root,
      "journalRecovery",
      ".filter(provider_operation::Column::ProviderReference.is_null())",
      "",
    );
    rejected(root, "operator no-effect cannot erase evidence or cross scope");
  });
});
test("success resolution cannot overwrite a concurrently observed provider reference", () => {
  fixture((root) => {
    replace(root, "journalRecovery", ".filter(reference_match)", "");
    rejected(
      root,
      "success resolution preserves and CASes observed outcome evidence",
    );
  });
});
test("recovery listing cannot silently include executing attempts without explicit quarantine", () => {
  fixture((root) => {
    replace(
      root,
      "journalRecovery",
      ".filter(provider_operation::Column::Status.ne(PROVIDER_OPERATION_EXECUTING))",
      "",
    );
    rejected(
      root,
      "bounded unresolved worklist keeps explicit stale-execution quarantine",
    );
  });
});
test("the new provider admission guard must be in the canonical migration registry", () => {
  fixture((root) => {
    replace(
      root,
      "fulfillmentRegistry",
      "Box::new(m20261101_000121_enforce_provider_effect_admission::Migration),",
      "",
    );
    rejected(root, "forward-only admission migration registered");
  });
});
test("the PostgreSQL admission correction cannot stop preserving retained provider references", () => {
  fixture((root) => {
    replace(
      root,
      "admissionMigration",
      "NEW.provider_reference IS DISTINCT FROM OLD.provider_reference",
      "FALSE",
    );
    rejected(root, "forward-only SQL admission/evidence guard");
  });
});
test("raw Commerce fixtures cannot omit the current forward-only provider admission schema", () => {
  fixture((root) => {
    replace(
      root,
      "support",
      /m20261101_000121_enforce_provider_effect_admission/g,
      "m20260713_000113_allow_provider_execution_reconciliation",
    );
    rejected(
      root,
      "raw fixtures install the current forward-only admission guard",
    );
  });
});
test("local recovery cannot refresh the current operator budget after reading the journal", () => {
  fixture((root) => {
    replace(
      root,
      "recovery",
      /remaining_stock_context\(\s*&preserved_context,\s*started\s*,?\s*\)/g,
      "remaining_stock_context(&preserved_context, std::time::Instant::now())",
    );
    rejected(
      root,
      "local recovery does not refresh the current operator budget",
    );
  });
});
test("native provider-result wrappers cannot refresh the budget after journal validation and replay reads", () => {
  fixture((root) => {
    replace(
      root,
      "fulfillment",
      "remaining_stock_context(&context, started)",
      "remaining_stock_context(&context, std::time::Instant::now())",
    );
    rejected(root, "native recovery budget includes journal replay reads");
  });
});
test("source evidence cannot promote unrun regressions into execution evidence", () => {
  fixture((root) => {
    replace(root, "evidence", '"executed": false', '"executed": true');
    rejected(
      root,
      "source evidence must not claim completion or regression/runtime execution",
    );
  });
});

test("Cancelled and Shipped projections cannot replace original key and payload admission", () => {
  for (const operation of ["cancel", "reship"]) {
    fixture((root) => {
      replace(
        root,
        "admin",
        new RegExp(
          `(const OPERATION: &str = "${operation}_admin_fulfillment";[\\s\\S]*?)(let provider_request = operation_request\\()`,
        ),
        '$1if current.status == "cancelled" { return Ok(current); } $2',
      );
      rejected(root, "terminal status cannot authorize key replay");
    });
  }
});
test("retained provider lookup cannot be bypassed after a mutable shipping option edit", () => {
  fixture((root) => {
    replace(
      root,
      "admin",
      ".find_fulfillment_key(tenant_id, fulfillment.id, key)",
      ".find_by_mutable_provider(tenant_id, fulfillment.id, key)",
    );
    rejected(
      root,
      "retained provider identity precedes mutable provider selection",
    );
  });
});
test("historical cross-provider key lookup must remain bounded and refuse ambiguity", () => {
  fixture((root) => {
    replace(root, "journal", ".limit(2)", ".limit(1)");
    rejected(
      root,
      "bounded original-key lookup refuses historical provider ambiguity",
    );
  });
});
test("late provider checkpoint cannot overwrite a concurrently observed reference", () => {
  fixture((root) => {
    replace(root, "journal", ".filter(reference_match)", "");
    rejected(
      root,
      "provider outcomes remain monotonic across late success and duplicate callbacks",
    );
  });
  fixture((root) => {
    replace(
      root,
      "journalRecovery",
      'Expr::cust("COALESCE(provider_completed_at, CURRENT_TIMESTAMP)")',
      "Expr::current_timestamp()",
    );
    rejected(
      root,
      "stale execution quarantine preserves an already saved completion timestamp",
    );
  });
});
test("a positive entry deadline cannot replace elapsed budget gates after native lock waits", () => {
  fixture((root) => {
    replace(
      root,
      "inventory",
      /require_remaining_budget\(context, started\)\?;/g,
      "",
    );
    rejected(root, "native elapsed budget before stock proof or writes");
  });
});
test("reopening Pending cancellation must inspect the nullable timestamp instead of its ActiveValue wrapper", () => {
  fixture((root) => {
    replace(
      root,
      "fulfillment",
      "fulfillment.shipped_at.is_some()",
      "fulfillment.shipped_at.clone().take().is_some()",
    );
    rejected(root, "nullable timestamp is not an ActiveValue receipt");
  });
});
test("HTTP and GraphQL owner scopes must reject a contradictory supplied request tenant", () => {
  fixture((root) => {
    replace(root, "http", "request_context.tenant_id != tenant.id", "false");
    rejected(
      root,
      "canonical request tenant matches authenticated owner scope",
    );
  });
  fixture((root) => {
    replace(
      root,
      "graphql",
      ".is_some_and(|request| request.tenant_id != tenant.id)",
      ".is_some_and(|_| false)",
    );
    rejected(
      root,
      "canonical request tenant matches authenticated owner scope",
    );
  });
});
test("receipt completion cannot refresh the original final commit budget", () => {
  fixture((root) => {
    replace(
      root,
      "stock",
      "remaining_stock_context(context, started)",
      "remaining_stock_context(context, std::time::Instant::now())",
    );
    rejected(root, "same budget survives receipt completion and final commit");
  });
});
test("literal saved keys cannot be trimmed into a second journal identity", () => {
  fixture((root) => {
    replace(
      root,
      "journal",
      "input.provider_id = input.provider_id.trim().to_string();",
      "input.provider_id = input.provider_id.trim().to_string(); input.idempotency_key = input.idempotency_key.trim().to_string();",
    );
    rejected(
      root,
      "retained literal identity is never trimmed into a different key",
    );
  });
});

test("explicit late success resolution must not be blocked by the new SQL guard when its retained facts are unchanged", () => {
  fixture((root) => {
    replace(
      root,
      "admissionMigration",
      "AND OLD.committed_at IS NULL AND NEW.status = 'provider_succeeded'",
      "AND OLD.provider_result IS NULL AND OLD.committed_at IS NULL AND NEW.status = 'provider_succeeded'",
    );
    rejected(
      root,
      "explicit known success promotion preserves retained SQL facts",
    );
  });
  fixture((root) => {
    replace(
      root,
      "createAdmin",
      "if operation.status == PROVIDER_OPERATION_COMMITTED {\n            return Ok(fulfillment);",
      "if matches!(operation.status.as_str(), PROVIDER_OPERATION_COMMITTED | PROVIDER_OPERATION_SUCCEEDED) { return Ok(fulfillment);",
    );
    rejected(
      root,
      "create success checkpoint is not native completion receipt",
    );
  });
});
