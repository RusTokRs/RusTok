import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const decisionsDir = join(root, "DECISIONS");
const registryPath = join(decisionsDir, "README.md");
const activationDate = "2026-09-18";

const decisionStatuses = new Set(["Proposed", "Accepted", "Superseded", "Rejected"]);
const implementationStatuses = new Set([
  "Not started",
  "In progress",
  "Implemented",
  "Not applicable",
  "Not tracked",
]);

const requiredNewHeadings = [
  "## Context",
  "## Decision",
  "## Sources of truth and ownership",
  "## Invariants",
  "## Non-goals",
  "## Data, transaction, and concurrency boundary",
  "## Context dimensions",
  "## Events and projections",
  "## Failure semantics",
  "## Migration and cutover",
  "## Alternatives considered",
  "## Verification",
  "## Consequences",
];

const errors = [];
const fail = (message) => errors.push(message);
const datedName = /^\d{4}-\d{2}-\d{2}-[a-z0-9]+(?:-[a-z0-9]+)*\.md$/;

const decisionFiles = readdirSync(decisionsDir)
  .filter((name) => datedName.test(name))
  .sort((a, b) => b.localeCompare(a));

const registry = readFileSync(registryPath, "utf8");
const registryLines = registry
  .split(/\r?\n/)
  .filter((line) => /^\| \[\d{4}-\d{2}-\d{2}\]\(\.\/[^)]+\.md\) \|/.test(line));

const rows = [];
for (const line of registryLines) {
  const cells = line.split("|").slice(1, -1).map((cell) => cell.trim());
  if (cells.length !== 5) {
    fail(`registry row must contain 5 columns: ${line}`);
    continue;
  }

  const link = cells[0].match(/^\[(\d{4}-\d{2}-\d{2})\]\(\.\/([^)]+\.md)\)$/);
  if (!link) {
    fail(`invalid ADR link cell: ${cells[0]}`);
    continue;
  }

  const [, displayDate, file] = link;
  const row = {
    displayDate,
    file,
    title: cells[1],
    decisionStatus: cells[2],
    implementationStatus: cells[3],
    relations: cells[4],
  };
  rows.push(row);

  if (!datedName.test(file)) fail(`invalid ADR filename in registry: ${file}`);
  if (displayDate !== file.slice(0, 10)) fail(`registry date does not match filename: ${file}`);
  if (!decisionStatuses.has(row.decisionStatus)) {
    fail(`unknown decision status for ${file}: ${row.decisionStatus}`);
  }
  if (!implementationStatuses.has(row.implementationStatus)) {
    fail(`unknown implementation status for ${file}: ${row.implementationStatus}`);
  }
  if (file >= `${activationDate}-` && row.implementationStatus === "Not tracked") {
    fail(`new ADR cannot use historical implementation status "Not tracked": ${file}`);
  }
  if (!existsSync(join(decisionsDir, file))) fail(`registry points to missing ADR: ${file}`);

  for (const match of row.relations.matchAll(/\(\.\/([^)]+\.md)\)/g)) {
    if (!existsSync(join(decisionsDir, match[1]))) {
      fail(`relation from ${file} points to missing ADR: ${match[1]}`);
    }
  }
}

const indexed = new Set();
for (const row of rows) {
  if (indexed.has(row.file)) fail(`ADR is indexed more than once: ${row.file}`);
  indexed.add(row.file);
}
for (const file of decisionFiles) {
  if (!indexed.has(file)) fail(`ADR is missing from DECISIONS/README.md: ${file}`);
}
for (const file of indexed) {
  if (!decisionFiles.includes(file)) fail(`registry contains non-ADR or stale entry: ${file}`);
}

const listedOrder = rows.map((row) => row.file);
const expectedOrder = [...listedOrder].sort((a, b) => b.localeCompare(a));
if (listedOrder.some((file, index) => file !== expectedOrder[index])) {
  fail("ADR registry must be sorted by filename/date descending");
}

for (const row of rows.filter((entry) => entry.file >= `${activationDate}-`)) {
  const content = readFileSync(join(decisionsDir, row.file), "utf8");

  if (/^- Status:/m.test(content)) {
    fail(`${row.file}: use separate Decision status and Implementation status metadata`);
  }

  const metadata = new Map();
  for (const match of content.matchAll(/^- ([A-Za-z ]+):\s*(.+)$/gm)) {
    metadata.set(match[1], match[2].trim());
  }

  for (const key of ["Date", "Decision status", "Implementation status", "Owners", "Extends", "Supersedes", "Superseded by"]) {
    if (!metadata.has(key)) fail(`${row.file}: missing metadata field "${key}"`);
  }

  if (metadata.get("Date") !== row.file.slice(0, 10)) {
    fail(`${row.file}: Date metadata must match filename`);
  }
  if (metadata.get("Decision status") !== row.decisionStatus) {
    fail(`${row.file}: Decision status metadata does not match registry`);
  }
  if (metadata.get("Implementation status") !== row.implementationStatus) {
    fail(`${row.file}: Implementation status metadata does not match registry`);
  }
  if (!metadata.get("Owners") || metadata.get("Owners") === "owning module/team") {
    fail(`${row.file}: Owners metadata must name the canonical owner`);
  }
  if (row.decisionStatus === "Superseded" && metadata.get("Superseded by") === "None") {
    fail(`${row.file}: superseded ADR must identify its replacement`);
  }

  for (const heading of requiredNewHeadings) {
    if (!content.includes(heading)) fail(`${row.file}: missing required heading "${heading}"`);
  }
}

if (errors.length > 0) {
  console.error("ADR governance verification failed:");
  for (const error of errors) console.error(`- ${error}`);
  process.exit(1);
}

console.log(`ADR governance verification passed for ${decisionFiles.length} decisions.`);
