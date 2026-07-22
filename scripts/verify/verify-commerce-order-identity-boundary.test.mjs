import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  CommerceOrderIdentityBoundaryError,
  verifyCommerceOrderIdentityBoundary,
} from "./verify-commerce-order-identity-boundary.mjs";

const writeFixture = ({ creation, compensation }) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "rustok-order-identity-guard-"));
  const files = {
    creation: "crates/rustok-commerce/src/services/checkout_order_creation.rs",
    compensation: "crates/rustok-commerce/src/services/checkout_compensation.rs",
    ports: "crates/rustok-order/src/ports.rs",
    registry: "crates/rustok-order/contracts/order-fba-registry.json",
  };
  for (const file of Object.values(files)) {
    fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
  }
  fs.writeFileSync(path.join(root, files.creation), creation);
  fs.writeFileSync(path.join(root, files.compensation), compensation);
  fs.writeFileSync(
    path.join(root, files.ports),
    "trait CheckoutOrderIdentityPort { async fn read_by_operation(); async fn read_by_cart(); async fn bind(); async fn adopt_legacy(); } order_checkout_identities",
  );
  fs.writeFileSync(
    path.join(root, files.registry),
    JSON.stringify({
      ports: [
        {
          name: "CheckoutOrderIdentityPort",
          operations: ["read_by_operation", "read_by_cart", "bind", "adopt_legacy"],
        },
      ],
    }),
  );
  return root;
};

const compliantConsumer = [
  "CheckoutOrderIdentityPort",
  "ReadCheckoutOrderIdentityByOperationRequest",
  "AdoptLegacyCheckoutOrderIdentityRequest",
].join("\n");

test("accepts typed order identity consumers", () => {
  const root = writeFixture({
    creation: compliantConsumer,
    compensation: compliantConsumer,
  });
  assert.equal(verifyCommerceOrderIdentityBoundary({ root }).status, "ok");
});

test("rejects direct commerce order storage lookup", () => {
  const root = writeFixture({
    creation: `${compliantConsumer}\nSELECT id FROM orders`,
    compensation: compliantConsumer,
  });
  assert.throws(
    () => verifyCommerceOrderIdentityBoundary({ root }),
    CommerceOrderIdentityBoundaryError,
  );
});
