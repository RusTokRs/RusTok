#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  migration:
    "crates/modules/rustok-pages/src/migrations/m20260806_000013_create_page_publish_rebuild_sources.rs",
  migrations: "crates/modules/rustok-pages/src/migrations/mod.rs",
  entity: "crates/modules/rustok-pages/src/entities/page_publish_rebuild_source.rs",
  entities: "crates/modules/rustok-pages/src/entities/mod.rs",
  publishManifest: "crates/modules/rustok-pages/src/services/page/publish_manifest.rs",
  evidence:
    "crates/modules/rustok-pages/contracts/evidence/pages-publish-rebuild-provenance-source.json",
  packet: "crates/modules/rustok-pages/docs/immutable-artifact-rebuild-provenance.md",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(absolute(relativePath))) {
    failures.push(`${label}: missing ${relativePath}`);
    continue;
  }
  const stats = fs.lstatSync(absolute(relativePath));
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${label}: ${relativePath} must be a regular non-symlink file`);
  }
}
if (failures.length > 0) {
  console.error("[verify-pages-publish-rebuild-provenance] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);

if (evidence.format !== "pages_publish_rebuild_provenance_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_publish_rebuild_provenance_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`evidence validation.${key} must remain false`);
}
, marker, "packet");
}
, marker, "actualization");
}
, marker, "continuation");
}

if (failures.length > 0) {
  console.error("[verify-pages-publish-rebuild-provenance] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

console.log("[verify-pages-publish-rebuild-provenance] PASS");
