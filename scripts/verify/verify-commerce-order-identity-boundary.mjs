import fs from "node:fs";
import path from "node:path";
import process from "node:process";

export class CommerceOrderIdentityBoundaryError extends Error {
  constructor(message) {
    super(message);
    this.name = "CommerceOrderIdentityBoundaryError";
  }
}

const defaultRoot = process.cwd();

export function verifyCommerceOrderIdentityBoundary({ root = defaultRoot } = {}) {
  const files = {
    creation: "crates/rustok-commerce/src/services/checkout_order_creation.rs",
    compensation: "crates/rustok-commerce/src/services/checkout_compensation.rs",
    orderPorts: "crates/rustok-order/src/ports.rs",
    orderRegistry: "crates/rustok-order/contracts/order-fba-registry.json",
  };
  const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
  const creation = read(files.creation);
  const compensation = read(files.compensation);
  const orderPorts = read(files.orderPorts);
  const registry = JSON.parse(read(files.orderRegistry));
  const failures = [];

  for (const [name, source] of [
    [files.creation, creation],
    [files.compensation, compensation],
  ]) {
    if (/\bFROM\s+orders\b/i.test(source)) {
      failures.push(`${name}: direct orders SQL is forbidden`);
    }
    if (source.includes("find_order_id_by_operation")) {
      failures.push(`${name}: legacy local order lookup helper is forbidden`);
    }
    if (/\border\s*\.\s*metadata\b/.test(source)) {
      failures.push(`${name}: order metadata identity reads are forbidden`);
    }
    for (const marker of [
      "CheckoutOrderIdentityPort",
      "ReadCheckoutOrderIdentityByOperationRequest",
      "AdoptLegacyCheckoutOrderIdentityRequest",
    ]) {
      if (!source.includes(marker)) failures.push(`${name}: missing ${marker}`);
    }
  }

  for (const marker of [
    "trait CheckoutOrderIdentityPort",
    "async fn read_by_operation(",
    "async fn read_by_cart(",
    "async fn bind(",
    "async fn adopt_legacy(",
    "order_checkout_identities",
  ]) {
    if (!orderPorts.includes(marker)) {
      failures.push(`${files.orderPorts}: missing ${marker}`);
    }
  }

  const identityPort = registry.ports?.find(
    (port) => port.name === "CheckoutOrderIdentityPort",
  );
  if (!identityPort) {
    failures.push(`${files.orderRegistry}: CheckoutOrderIdentityPort is not registered`);
  } else {
    for (const operation of ["read_by_operation", "read_by_cart", "bind", "adopt_legacy"]) {
      if (!identityPort.operations?.includes(operation)) {
        failures.push(`${files.orderRegistry}: missing ${operation}`);
      }
    }
  }

  if (failures.length > 0) {
    throw new CommerceOrderIdentityBoundaryError(failures.join("\n"));
  }

  return {
    checked_files: Object.values(files),
    status: "ok",
  };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  try {
    const result = verifyCommerceOrderIdentityBoundary();
    console.log(JSON.stringify(result, null, 2));
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
