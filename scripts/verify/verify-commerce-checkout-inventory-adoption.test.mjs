// Maintainer-run regression sources only. The agent does not execute this test module.
import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  sourceFiles,
  sourcePaths,
  verifyCommerceCheckoutInventoryAdoption,
} from "./verify-commerce-checkout-inventory-adoption.mjs";
import { rustFunctionBody } from "./verify-cart-priced-write-context.mjs";

const repository = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
function fixture(run) {
  const root = fs.mkdtempSync(
    path.join(os.tmpdir(), "inventory-adoption-contract-"),
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
  const result = verifyCommerceCheckoutInventoryAdoption({ root });
  assert.equal(result.status, "failed");
  assert.ok(
    result.failures.some((failure) => failure.includes(label)),
    result.failures.join("\n"),
  );
}

test("current source has the owner-local adoption contract without claiming runtime execution", () => {
  fixture((root) =>
    assert.deepEqual(
      verifyCommerceCheckoutInventoryAdoption({ root }).failures,
      [],
    ),
  );
});

test("Commerce cannot reintroduce direct Inventory row access", () => {
  fixture((root) => {
    replace(
      root,
      "commerce",
      "self.request_owner_adoption(context, &request).await?;",
      "reservation_item::Entity::find(); self.request_owner_adoption(context, &request).await?;",
    );
    rejected(
      root,
      "Commerce must not resolve or mutate foreign Inventory rows",
    );
  });
});

test("owner tenant proof cannot disappear behind item locks", () => {
  fixture((root) => {
    replace(
      root,
      "inventory",
      "load_tenant_variant(&context, ADOPT_OPERATION, &txn, tenant_id, *variant_id).await?;",
      "load_unscoped_variant(&context, ADOPT_OPERATION, &txn, *variant_id).await?;",
    );
    rejected(root, "tenant proof before item locks");
  });
});

test("order creation cannot replace the supplied Inventory capability with an embedded factory", () => {
  fixture((root) => {
    replace(
      root,
      "creation",
      /adoption_service: CheckoutInventoryOrderAdoptionService::new\(\s*db,\s*event_bus,\s*inventory_port,?\s*\)/,
      "adoption_service: CheckoutInventoryOrderAdoptionService::new(db.clone(), event_bus, in_process_inventory_reservation_identity_port(db))",
    );
    rejected(root, "provider threaded through order creation");
  });
});

test("a partial or changed owner reply cannot stand in for complete correlation", () => {
  fixture((root) => {
    replace(root, "commerce", "|| response.items != request.items", "");
    rejected(root, "complete response, not inferred success");
  });
});

test("false owner retry advice cannot be used to authorize automatic compensation", () => {
  fixture((root) => {
    const file = path.join(root, sourceFiles.staged);
    const source = fs.readFileSync(file, "utf8");
    const body = rustFunctionBody(source, "pipeline_failure_disposition");
    assert.ok(body.includes("FailureDisposition::ReconciliationRequired"));
    fs.writeFileSync(
      file,
      source.replace(
        body,
        body.replaceAll(
          "FailureDisposition::ReconciliationRequired",
          "FailureDisposition::CompensationRequired",
        ),
      ),
    );
    rejected(root, "false is not permission for compensation");
  });
});

test("missing provenance is tolerated only for explicitly marked known compaction", () => {
  fixture((root) => {
    replace(
      root,
      "inventory",
      /metadata\s*\.get\("metadata_truncated"\)\s*\.and_then\(Value::as_bool\)\s*== Some\(true\)/,
      "true",
    );
    rejected(root, "only known compaction permits missing labels");
  });
});

test("parking remains an executing-lease transition instead of an unfenced state write", () => {
  fixture((root) => {
    const file = path.join(root, sourceFiles.journal);
    const source = fs.readFileSync(file, "utf8");
    const body = rustFunctionBody(source, "mark_reconciliation_required");
    const changed = body.replace(
      "expected_status: CheckoutOperationStatus::Executing",
      "expected_status: CheckoutOperationStatus::Pending",
    );
    assert.notEqual(changed, body);
    fs.writeFileSync(file, source.replace(body, changed));
    rejected(root, "lease-fenced existing state");
  });
});

test("static adoption declarations cannot become a fabricated execution receipt", () => {
  fixture((root) => {
    const file = path.join(root, sourceFiles.matrix);
    const data = JSON.parse(fs.readFileSync(file, "utf8"));
    data.cases.find(
      (entry) => entry.operation === "adopt_inventory_by_identity",
    ).execution_status = "executed";
    fs.writeFileSync(file, JSON.stringify(data));
    rejected(
      root,
      "adoption source evidence must not become runtime/readiness evidence",
    );
  });
});

test("all changed metadata is measured before the first owner binding write", () => {
  fixture((root) => {
    replace(
      root,
      "inventory",
      "validate_adoption_metadata_capacity(&context, &txn, &changes).await?;",
      "",
    );
    rejected(root, "whole metadata batch fits before any binding writes");
  });
});

test("physical metadata bounds cannot silently become character-count limits", () => {
  fixture((root) => {
    replace(root, "inventory", "length(CAST(value AS BLOB))", "length(value)");
    rejected(root, "backend-native physical byte representation");
  });
});

test("a zero timeout cannot replace canonical deadline admission before provider polling", () => {
  fixture((root) => {
    replace(
      root,
      "commerce",
      ".require_deadline_semantics()",
      ".require_no_deadline_semantics()",
    );
    rejected(
      root,
      "missing or zero deadline refuses before the selected owner can be polled",
    );
  });
});

test("successful parking cannot erase the original typed error in recovery", () => {
  fixture((root) => {
    replace(
      root,
      "recovery",
      /\.is_some_and\(\s*super::CheckoutInventoryOrderAdoptionError::requires_owner_recovery\s*,?\s*\)/,
      ".is_some_and(|_| false)",
    );
    rejected(
      root,
      "preserve typed same-identity failure before compensation lookup",
    );
  });
});

test("nested journal wrappers cannot discard the original adoption cause", () => {
  fixture((root) => {
    replace(
      root,
      "staged",
      "Self::PipelineAndJournal { pipeline, .. } => pipeline.inventory_adoption_failure()",
      "Self::PipelineAndJournal { .. } => None",
    );
    rejected(root, "typed wrapper body retains the original adoption cause");
  });
});

test("local pre-call read failures are not proof that a prior attempt never committed", () => {
  fixture((root) => {
    replace(root, "commerce", "Some(local_adoption_failure(self))", "None");
    rejected(
      root,
      "a local pre-call failure cannot prove a prior attempt never committed",
    );
  });
});

test("positive quantities cannot hide retained terminal disposition or supersession", () => {
  fixture((root) => {
    replace(
      root,
      "inventory",
      "|| has_terminal_disposition(&reservation.metadata)",
      "",
    );
    rejected(
      root,
      "terminal metadata is not an active allocation or repair authority",
    );
  });
});
