#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function reject(source, pattern, message) {
  if (pattern.test(source)) failures.push(message);
}

const contractPath = "crates/rustok-forum/src/category_presentation.rs";
const entityPath = "crates/rustok-forum/src/entities/forum_category.rs";
const dtoPath = "crates/rustok-forum/src/dto/category.rs";
const treeDtoPath = "crates/rustok-forum/src/dto/category_tree.rs";
const categoryServicePath = "crates/rustok-forum/src/services/category.rs";
const categoryOwnerPath = "crates/rustok-forum/src/services/category_owner.rs";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const crateApiPath = "crates/rustok-forum/CRATE_API.md";

const contract = read(contractPath);
const entity = read(entityPath);
const plan = read(planPath);
const crateApi = read(crateApiPath);
const categoryBoundary = [
  dtoPath,
  treeDtoPath,
  entityPath,
  categoryServicePath,
  categoryOwnerPath,
  contractPath,
]
  .map((filePath) => `${filePath}\n${read(filePath)}`)
  .join("\n");

requireText(
  contract,
  "pub struct CategoryCoverMediaCandidate",
  `${contractPath}: transport-neutral cover candidate is missing`,
);
for (const field of [
  "media_id",
  "tenant_id",
  "mime_type",
  "size",
  "width",
  "height",
  "descriptor",
]) {
  requireText(contract, `pub ${field}:`, `${contractPath}: cover candidate misses ${field}`);
}
requireText(
  contract,
  "normalize_category_icon_key",
  `${contractPath}: category icon token normalization is missing`,
);
requireText(
  contract,
  "should_emit_to_public_metadata",
  `${contractPath}: direct-public descriptor policy is not enforced`,
);
requireText(
  contract,
  "Quarantine/deletion state is not currently published",
  `${contractPath}: unresolved Media owner state must remain explicit`,
);
requireText(
  entity,
  "normalize_category_icon_key",
  `${entityPath}: database write boundary does not validate icon tokens`,
);

reject(
  categoryBoundary,
  /rustok_media::entities|MediaService::new|storage_path|storage_driver/,
  "forum category presentation must not access Media persistence or storage internals",
);
reject(
  categoryBoundary,
  /\b(?:cover|image)_(?:url|path)\b/i,
  "forum category presentation must not store an arbitrary image URL or path",
);

requireText(plan, "Delivered in `FORUM-13A`", `${planPath}: FORUM-13A delivery is not recorded`);
requireText(
  plan,
  "quarantine/deletion",
  `${planPath}: remaining Media state dependency is not recorded`,
);
requireText(
  crateApi,
  "CategoryCoverMediaCandidate",
  `${crateApiPath}: category presentation contract is not documented`,
);

if (failures.length > 0) {
  console.error("forum category presentation verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum category presentation verification passed");
