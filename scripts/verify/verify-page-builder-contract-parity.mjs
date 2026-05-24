#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..");

const providerManifest = path.join(repoRoot, "crates", "rustok-page-builder", "rustok-module.toml");
const pagesManifest = path.join(repoRoot, "crates", "rustok-pages", "rustok-module.toml");

function readFile(filePath) {
  if (!fs.existsSync(filePath)) {
    throw new Error(`missing file: ${filePath}`);
  }
  return fs.readFileSync(filePath, "utf8");
}

function extractVersion(content, pattern, label) {
  const match = content.match(pattern);
  if (!match?.[1]) {
    throw new Error(`cannot extract ${label}`);
  }
  return match[1];
}

function compareSemverLike(left, right) {
  const parse = (value, label) =>
    value.split(".").map((chunk) => {
      if (!/^\d+$/.test(chunk)) {
        throw new Error(`invalid numeric version segment in ${label}: '${value}'`);
      }
      return Number.parseInt(chunk, 10);
    });
  const l = parse(left, "left version");
  const r = parse(right, "right version");
  const length = Math.max(l.length, r.length);
  for (let index = 0; index < length; index += 1) {
    const lv = Number.isFinite(l[index]) ? l[index] : 0;
    const rv = Number.isFinite(r[index]) ? r[index] : 0;
    if (lv > rv) return 1;
    if (lv < rv) return -1;
  }
  return 0;
}

try {
  const provider = readFile(providerManifest);
  const consumer = readFile(pagesManifest);

  const providerVersion = extractVersion(
    provider,
    /^\s*builder_contract_version\s*=\s*"([^"]+)"\s*$/m,
    "provider builder_contract_version",
  );
  const providerConsumerMinVersion = extractVersion(
    provider,
    /^\s*consumer_min_version\s*=\s*"([^"]+)"\s*$/m,
    "provider consumer_min_version",
  );
  const consumerVersion = extractVersion(
    consumer,
    /^\s*builder_contract_version\s*=\s*"([^"]+)"\s*$/m,
    "consumer builder_contract_version",
  );
  const consumerContractVersion = extractVersion(
    consumer,
    /^\s*contract_version\s*=\s*"([^"]+)"\s*$/m,
    "consumer contract_version",
  );

  const errors = [];

  if (providerVersion !== consumerVersion) {
    errors.push(
      `builder_contract_version mismatch: provider=${providerVersion}, consumer=${consumerVersion}`,
    );
  }

  if (compareSemverLike(consumerVersion, providerConsumerMinVersion) < 0) {
    errors.push(
      `consumer version below provider minimum: consumer=${consumerVersion}, provider_consumer_min_version=${providerConsumerMinVersion}`,
    );
  }

  if (providerVersion !== consumerContractVersion) {
    errors.push(
      `consumer contract_version mismatch: provider=${providerVersion}, consumer_contract_version=${consumerContractVersion}`,
    );
  }

  if (errors.length > 0) {
    console.error("[verify-page-builder-contract-parity] FAIL");
    errors.forEach((error) => console.error(`- ${error}`));
    process.exit(1);
  }

  console.log("[verify-page-builder-contract-parity] PASS");
  console.log(
    `provider=${providerVersion}; provider_consumer_min_version=${providerConsumerMinVersion}; consumer=${consumerVersion}; consumer_contract_version=${consumerContractVersion}`,
  );
} catch (error) {
  console.error("[verify-page-builder-contract-parity] FAIL");
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
