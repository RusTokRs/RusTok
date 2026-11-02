import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  sourcePaths,
  verifyCartPricedWriteContext,
} from "./verify-cart-priced-write-context.mjs";

const repository = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
const fixture = () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "cart-priced-source-"));
  for (const file of sourcePaths) {
    const target = path.join(root, file);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(path.join(repository, file), target);
  }
  return root;
};
const mutate = (file, from, to) => {
  const root = fixture();
  try {
    const target = path.join(root, file);
    const source = fs.readFileSync(target, "utf8");
    assert.ok(source.includes(from), `fixture marker must exist: ${from}`);
    fs.writeFileSync(target, source.replace(from, to));
    assert.ok(verifyCartPricedWriteContext({ root }).failures.length > 0);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
};

test("current priced-write source satisfies the scoped contract", () => {
  assert.deepEqual(verifyCartPricedWriteContext().failures, []);
});
test("dropping the actual owner add guard is detected", () => {
  mutate(
    "crates/modules/rustok-cart/src/owner_ports.rs",
    "Some(request.pricing_context)",
    "None",
  );
});
test("rebasing observation contents during guest authorization is detected", () => {
  mutate(
    "crates/modules/rustok-cart/src/pricing_context_guard.rs",
    "self.guest_access_fingerprint = guest_access_fingerprint(&cart.metadata);",
    "self.line_items = Vec::new();",
  );
});
test("unquoted public quantity writes cannot return", () => {
  mutate(
    "crates/modules/rustok-cart/src/ports.rs",
    "pub trait CartStorefrontPort",
    "// update_storefront_line_item_quantity\npub trait CartStorefrontPort",
  );
});
test("omitting duplicate batch validation is detected", () => {
  mutate(
    "crates/modules/rustok-cart/src/services/cart/pricing.rs",
    "updates_map.insert(update.line_item_id, update).is_some()",
    "false",
  );
});
test("scaling a tier quote for a different quantity is detected", () => {
  mutate(
    "crates/modules/rustok-cart/src/services/cart/pricing.rs",
    "*quantity != update.quantity",
    "false",
  );
});
test("double-counting the replaced target in stock demand is detected", () => {
  mutate(
    "crates/modules/rustok-commerce/src/storefront_cart_runtime.rs",
    "Some(line.id) != replaced_line",
    "true",
  );
});
test("removing the guarded native decrement workflow is detected", () => {
  mutate(
    "crates/modules/rustok-cart/storefront/src/transport/native_server_adapter_ssr.rs",
    ".update_line_item_quantity(",
    ".update_without_owner_quote(",
  );
});

test("marketplace snapshot writers cannot bypass the shared Cart lock", () => {
  mutate(
    "crates/modules/rustok-cart/src/services/marketplace_snapshot.rs",
    "load_cart_for_update_in_tx(&transaction, tenant_id, cart_id)",
    "load_cart_in_tx(&transaction, tenant_id, cart_id)",
  );
});
