#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const RELEASE_HEADING = /^## \[([^\]]+)] - (\d{4}-\d{2}-\d{2})\s*$/gm;

function parseArguments(argv) {
  const options = { selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") {
      options.selfTest = true;
      continue;
    }
    if (["--changelog", "--version", "--output"].includes(argument)) {
      const value = argv[index + 1];
      if (!value) throw new Error(`${argument} requires a value`);
      options[argument.slice(2)] = value;
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${argument}`);
  }
  return options;
}

function extractReleaseNotes(changelog, version) {
  const headings = [...changelog.matchAll(RELEASE_HEADING)].filter((match) => match[1] === version);
  if (headings.length !== 1) {
    throw new Error(`CHANGELOG.md must contain exactly one release heading for ${version}`);
  }
  const heading = headings[0];
  const bodyStart = heading.index + heading[0].length;
  RELEASE_HEADING.lastIndex = bodyStart;
  const next = RELEASE_HEADING.exec(changelog);
  RELEASE_HEADING.lastIndex = 0;
  const bodyEnd = next?.index ?? changelog.length;
  const body = changelog.slice(bodyStart, bodyEnd).trim();
  if (!body) throw new Error(`CHANGELOG.md release ${version} is empty`);
  return `# RusToK ${version}\n\nReleased ${heading[2]}.\n\n${body}\n`;
}

function runSelfTest() {
  const changelog = `# Changelog\n\n## [Unreleased]\n\n### Added\n- Pending.\n\n## [1.2.3] - 2026-07-18\n\n### Added\n- Signed artifacts.\n\n## [1.2.2] - 2026-07-01\n\n### Fixed\n- Previous fix.\n`;
  const notes = extractReleaseNotes(changelog, "1.2.3");
  assert(notes.includes("# RusToK 1.2.3"));
  assert(notes.includes("Released 2026-07-18."));
  assert(notes.includes("Signed artifacts."));
  assert(!notes.includes("Previous fix."));
  assert.throws(() => extractReleaseNotes(changelog, "1.2.4"), /exactly one/);
  console.log("✔ release notes extractor self-test passed");
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.selfTest) {
    runSelfTest();
    return;
  }
  for (const name of ["changelog", "version", "output"]) {
    if (!options[name]) throw new Error(`--${name} is required`);
  }
  const notes = extractReleaseNotes(
    fs.readFileSync(path.resolve(options.changelog), "utf8"),
    options.version,
  );
  const output = path.resolve(options.output);
  fs.mkdirSync(path.dirname(output), { recursive: true });
  fs.writeFileSync(output, notes);
  console.log(`✔ extracted release notes for ${options.version} to ${output}`);
}

try {
  main();
} catch (error) {
  console.error(`release notes extraction failed: ${error.message}`);
  process.exit(1);
}
