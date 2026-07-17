#!/usr/bin/env node
// Enforces that every cargo-deny advisory ignore has a complete, time-bounded
// register entry and that expired exceptions cannot silently remain active.

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const failures = [];

function readRepo(relativePath) {
  return readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

function parseIgnoredAdvisories(source) {
  const advisorySection = /^\[advisories\]([\s\S]*?)(?=^\[|\Z)/m.exec(source)?.[1] ?? "";
  const ignoreBlock = /\bignore\s*=\s*\[([\s\S]*?)\]/m.exec(advisorySection)?.[1] ?? "";
  return new Set([...ignoreBlock.matchAll(/"(RUSTSEC-\d{4}-\d{4})"/g)].map((match) => match[1]));
}

function parseRegister(source) {
  const entries = new Map();
  const heading = /^###\s+(RUSTSEC-\d{4}-\d{4})\b.*$/gm;
  const matches = [...source.matchAll(heading)];

  matches.forEach((match, index) => {
    const id = match[1];
    const start = match.index ?? 0;
    const end = matches[index + 1]?.index ?? source.length;
    const block = source.slice(start, end);
    const fields = new Map();

    for (const row of block.matchAll(/^\|\s*([^|]+?)\s*\|\s*([^|]+?)\s*\|\s*$/gm)) {
      const key = row[1].trim();
      const value = row[2].trim();
      if (key !== "Field" && !/^[-:]+$/.test(key)) {
        fields.set(key, value);
      }
    }

    entries.set(id, fields);
  });

  return entries;
}

function utcDateOnly(date) {
  return Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate());
}

function parseIsoDate(value) {
  const match = /\b(\d{4})-(\d{2})-(\d{2})\b/.exec(value);
  if (!match) {
    return null;
  }
  const [, year, month, day] = match;
  return Date.UTC(Number(year), Number(month) - 1, Number(day));
}

const denySource = readRepo("deny.toml");
const registerSource = readRepo("docs/security/advisory-exceptions.md");
const ignored = parseIgnoredAdvisories(denySource);
const registered = parseRegister(registerSource);
const requiredFields = [
  "Severity",
  "Risk",
  "Patched version",
  "Repository policy location",
  "Accountable owner",
  "Dependency path",
  "Reachability",
  "Compensating controls",
  "Remediation",
  "Approved",
  "Expires",
  "Evidence required",
  "Upstream advisory",
];
const today = utcDateOnly(new Date());

for (const id of ignored) {
  const fields = registered.get(id);
  if (!fields) {
    fail(`deny.toml ignores ${id}, but the advisory exception register has no entry`);
    continue;
  }

  for (const field of requiredFields) {
    const value = fields.get(field);
    if (!value || value === "—" || /^TBD$/i.test(value)) {
      fail(`${id}: missing required register field ${field}`);
    }
  }

  const expiryText = fields.get("Expires") ?? "";
  const expiry = parseIsoDate(expiryText);
  if (expiry === null) {
    fail(`${id}: Expires must contain an ISO date (YYYY-MM-DD)`);
  } else if (expiry < today) {
    fail(`${id}: exception expired on ${expiryText}; remove the ignore or approve a new exception`);
  }

  if (fields.get("Repository policy location") !== "`deny.toml`") {
    fail(`${id}: Repository policy location must point to deny.toml`);
  }
}

for (const id of registered.keys()) {
  if (!ignored.has(id)) {
    fail(`${id}: active register entry is not present in deny.toml ignore list`);
  }
}

if (ignored.size === 0 && registered.size > 0) {
  fail("advisory register contains active entries while deny.toml has no ignored advisories");
}

if (failures.length > 0) {
  console.error("Security advisory exception check failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(`✔ ${ignored.size} advisory exception(s) are registered and unexpired`);
