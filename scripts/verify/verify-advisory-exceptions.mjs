#!/usr/bin/env node
// Enforces that every advisory ignore in cargo-deny or cargo-audit has a
// complete, time-bounded active register entry.

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

function tomlSection(source, sectionName) {
  const marker = `[${sectionName}]`;
  const start = source.indexOf(marker);
  if (start === -1) {
    return "";
  }
  const nextSection = source.indexOf("\n[", start + marker.length);
  return source.slice(start + marker.length, nextSection === -1 ? source.length : nextSection);
}

function parseIgnoredAdvisories(source) {
  const advisorySection = tomlSection(source, "advisories");
  const ignoreBlock = /\bignore\s*=\s*\[([\s\S]*?)\]/m.exec(advisorySection)?.[1] ?? "";
  return new Set([...ignoreBlock.matchAll(/"(RUSTSEC-\d{4}-\d{4})"/g)].map((match) => match[1]));
}

function markdownSection(source, heading, nextHeadings) {
  const start = source.indexOf(heading);
  if (start === -1) {
    return "";
  }
  const candidates = nextHeadings
    .map((nextHeading) => source.indexOf(nextHeading, start + heading.length))
    .filter((index) => index !== -1);
  const end = candidates.length > 0 ? Math.min(...candidates) : source.length;
  return source.slice(start + heading.length, end);
}

function parseActiveRegister(source) {
  const activeSection = markdownSection(source, "## Active Exceptions", [
    "## Closed Exceptions",
    "## Required Verification",
  ]);
  const entries = new Map();
  const heading = /^###\s+(RUSTSEC-\d{4}-\d{4})\b.*$/gm;
  const matches = [...activeSection.matchAll(heading)];

  matches.forEach((match, index) => {
    const id = match[1];
    const start = match.index ?? 0;
    const end = matches[index + 1]?.index ?? activeSection.length;
    const block = activeSection.slice(start, end);
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

const policyFiles = ["deny.toml", ".cargo/audit.toml"];
const ignoredByPolicy = new Map(
  policyFiles.map((relativePath) => [
    relativePath,
    parseIgnoredAdvisories(readRepo(relativePath)),
  ]),
);
const ignored = new Set([...ignoredByPolicy.values()].flatMap((ids) => [...ids]));
const active = parseActiveRegister(readRepo("docs/security/advisory-exceptions.md"));
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
  const fields = active.get(id);
  const policyLocations = [...ignoredByPolicy.entries()]
    .filter(([, ids]) => ids.has(id))
    .map(([relativePath]) => relativePath);
  if (!fields) {
    fail(`${policyLocations.join(", ")} ignore ${id}, but the active exception register has no entry`);
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

  const policyLocation = fields.get("Repository policy location") ?? "";
  for (const relativePath of policyLocations) {
    if (!policyLocation.includes(relativePath)) {
      fail(`${id}: Repository policy location must mention ${relativePath}`);
    }
  }
}

if (failures.length > 0) {
  console.error("Security advisory exception check failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(`✔ ${ignored.size} advisory exception(s) are registered and unexpired`);
