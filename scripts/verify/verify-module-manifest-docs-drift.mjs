#!/usr/bin/env node
// Verifies that the canonical module manifest, its example, and the central
// module overview describe the same module topology.

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

function addRecord(records, record, sourceName) {
  if (records.has(record.slug)) {
    fail(`${sourceName}: duplicate module ${record.slug}`);
    return;
  }
  records.set(record.slug, record);
}

function parseManifest(relativePath) {
  const source = readRepo(relativePath);
  const modulesHeading = "[modules]";
  const modulesStart = source.indexOf(modulesHeading);
  const settingsStart = source.indexOf("[settings]", modulesStart);
  if (modulesStart === -1 || settingsStart === -1 || settingsStart <= modulesStart) {
    fail(`${relativePath}: expected ordered [modules] and [settings] sections`);
    return new Map();
  }

  const records = new Map();
  const moduleBlock = source.slice(modulesStart + modulesHeading.length, settingsStart);
  for (const rawLine of moduleBlock.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) {
      continue;
    }

    const match = /^([a-zA-Z0-9_]+)\s*=\s*\{(.+)\}\s*$/.exec(line);
    if (!match) {
      fail(`${relativePath}: unsupported module declaration: ${line}`);
      continue;
    }

    const [, slug, body] = match;
    const crateMatch = /\bcrate\s*=\s*"([^"]+)"/.exec(body);
    if (!crateMatch) {
      fail(`${relativePath}: module ${slug} is missing crate`);
      continue;
    }

    const dependsMatch = /\bdepends_on\s*=\s*\[([^\]]*)\]/.exec(body);
    const dependencies = dependsMatch
      ? [...dependsMatch[1].matchAll(/"([^"]+)"/g)].map((entry) => entry[1])
      : [];

    addRecord(
      records,
      {
        slug,
        crate: crateMatch[1],
        required: /\brequired\s*=\s*true\b/.test(body),
        runtime: /\bruntime\s*=\s*"([^"]+)"/.exec(body)?.[1] ?? null,
        dependencies,
      },
      relativePath,
    );
  }

  return records;
}

function parseOverviewTable(source, startHeading, endHeading, category) {
  const start = source.indexOf(startHeading);
  const end = source.indexOf(endHeading, start + startHeading.length);
  if (start === -1 || end === -1) {
    fail(`docs/modules/overview.md: missing ${startHeading} table`);
    return new Map();
  }

  const records = new Map();
  for (const rawLine of source.slice(start, end).split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line.startsWith("| `")) {
      continue;
    }

    const cells = line
      .split("|")
      .slice(1, -1)
      .map((cell) => cell.trim());
    if (cells.length < 2) {
      continue;
    }

    const slug = /^`([^`]+)`$/.exec(cells[0])?.[1];
    const crateName = /^`([^`]+)`$/.exec(cells[1])?.[1];
    if (!slug || !crateName) {
      continue;
    }

    const dependencies = (category === "core" || category === "optional") && cells[2] && cells[2] !== "—"
      ? [...cells[2].matchAll(/`([^`]+)`/g)].map((entry) => entry[1])
      : [];
    const runtime = category === "extension"
      ? /^`([^`]+)`$/.exec(cells[2] ?? "")?.[1] ?? null
      : null;

    addRecord(
      records,
      {
        slug,
        crate: crateName,
        required: category === "core",
        runtime,
        dependencies,
      },
      `docs/modules/overview.md ${startHeading}`,
    );
  }

  return records;
}

function parseOverview() {
  const source = readRepo("docs/modules/overview.md");
  const records = new Map();
  const sections = [
    parseOverviewTable(source, "### Core", "### Optional", "core"),
    parseOverviewTable(
      source,
      "### Optional",
      "### Capability Extensions",
      "optional",
    ),
    parseOverviewTable(
      source,
      "### Capability Extensions",
      "## What Lives Next to Modules",
      "extension",
    ),
  ];

  for (const section of sections) {
    for (const record of section.values()) {
      addRecord(records, record, "docs/modules/overview.md");
    }
  }
  return records;
}

function normalized(record) {
  return {
    crate: record.crate,
    required: record.required,
    runtime: record.runtime,
    dependencies: [...record.dependencies].sort(),
  };
}

function compareMaps(leftName, left, rightName, right) {
  const slugs = new Set([...left.keys(), ...right.keys()]);
  for (const slug of [...slugs].sort()) {
    const leftRecord = left.get(slug);
    const rightRecord = right.get(slug);
    if (!leftRecord) {
      fail(`${leftName}: missing module ${slug} present in ${rightName}`);
      continue;
    }
    if (!rightRecord) {
      fail(`${rightName}: missing module ${slug} present in ${leftName}`);
      continue;
    }

    const leftJson = JSON.stringify(normalized(leftRecord));
    const rightJson = JSON.stringify(normalized(rightRecord));
    if (leftJson !== rightJson) {
      fail(
        `${slug}: topology mismatch between ${leftName} ${leftJson} and ${rightName} ${rightJson}`,
      );
    }
  }
}

const canonical = parseManifest("modules.toml");
const example = parseManifest("modules.toml.example");
const overview = parseOverview();

compareMaps("modules.toml", canonical, "modules.toml.example", example);
compareMaps("modules.toml", canonical, "docs/modules/overview.md", overview);

if (failures.length > 0) {
  console.error("Module manifest documentation drift check failed:");
  console.error(
    `Parsed module counts: modules.toml=${canonical.size}, modules.toml.example=${example.size}, docs/modules/overview.md=${overview.size}`,
  );
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  `✔ Module topology is synchronized: modules.toml=${canonical.size}, modules.toml.example=${example.size}, docs/modules/overview.md=${overview.size}`,
);
