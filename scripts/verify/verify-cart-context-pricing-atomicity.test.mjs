import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  sourceFiles,
  sourcePaths,
  verifyCartContextPricingAtomicity,
} from "./verify-cart-context-pricing-atomicity.mjs";

const repository = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
function mutate(key, from, to) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "cart-context-source-"));
  try {
    for (const file of sourcePaths) {
      const target = path.join(root, file);
      fs.mkdirSync(path.dirname(target), { recursive: true });
      fs.copyFileSync(path.join(repository, file), target);
    }
    const file = path.join(root, sourceFiles[key]);
    const source = fs.readFileSync(file, "utf8");
    assert.ok(source.includes(from), `fixture marker must exist: ${from}`);
    fs.writeFileSync(file, source.replace(from, to));
    assert.ok(verifyCartContextPricingAtomicity({ root }).failures.length > 0);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

test("current atomic context source satisfies its bounded owner contract", () => {
  assert.deepEqual(verifyCartContextPricingAtomicity().failures, []);
});
test("rebasing the price observation to target context is detected", () => {
  mutate(
    "planner",
    "CartPricingContextGuard::capture(cart)?",
    "CartPricingContextGuard::capture(&target)?",
  );
});
test("aggregating or flattening tier quantities is detected", () => {
  mutate(
    "planner",
    ".entry((line.product_id, line.quantity))",
    ".entry((line.product_id, 1))",
  );
});
test("single-read fallback instead of selected batch preparation is detected", () => {
  mutate(
    "planner",
    ".resolve_product_prices(read_context.clone(), request)",
    ".resolve_product_price(read_context.clone(), request)",
  );
});
test("omitting complete target line coverage is detected", () => {
  mutate(
    "pricing",
    "PricingCoverage::Complete && updates_map.len() != lines.len()",
    "false",
  );
});
test("dropping the normalized target scope check is detected", () => {
  mutate("context", "plan.scope != target", "false");
});
test("resurrecting old shipping after explicit scalar clear is detected", () => {
  mutate(
    "helpers",
    "let option_id = input.selected_shipping_option_id;",
    "let option_id = input.selected_shipping_option_id.or(current_selected_shipping_option_id);",
  );
});
test("snapshot adapter cannot return to split begin-then-context preparation", () => {
  mutate(
    "snapshot",
    ".prepare_checkout(tenant_id, request)",
    ".begin_checkout(tenant_id, request.cart_id)",
  );
});
