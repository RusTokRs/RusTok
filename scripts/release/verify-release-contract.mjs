#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const CANONICAL_SEMVER =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*))*))?$/;
const RELEASE_HEADING = /^## \[([^\]]+)] - (\d{4}-\d{2}-\d{2})\s*$/gm;

function parseArguments(argv) {
  const options = { selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") {
      options.selfTest = true;
      continue;
    }
    if (["--tag", "--workspace", "--lock", "--changelog", "--github-output"].includes(argument)) {
      const value = argv[index + 1];
      if (!value) throw new Error(`${argument} requires a value`);
      options[argument.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())] = value;
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${argument}`);
  }
  return options;
}

function parseReleaseTag(tag) {
  if (typeof tag !== "string" || !tag.startsWith("v")) {
    throw new Error("release tag must start with v");
  }
  const version = tag.slice(1);
  const match = CANONICAL_SEMVER.exec(version);
  if (!match) {
    throw new Error(
      "release tag must be canonical SemVer vMAJOR.MINOR.PATCH with an optional prerelease and no build metadata",
    );
  }
  return {
    tag,
    version,
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
    prerelease: match[4] || "",
  };
}

function sectionValue(source, section, key) {
  const lines = source.replaceAll("\r\n", "\n").split("\n");
  const heading = `[${section}]`;
  const start = lines.findIndex((line) => line.trim() === heading);
  if (start === -1) throw new Error(`missing [${section}] section`);
  const duplicate = lines.findIndex((line, index) => index > start && line.trim() === heading);
  if (duplicate !== -1) throw new Error(`duplicate [${section}] section`);
  let end = lines.length;
  for (let index = start + 1; index < lines.length; index += 1) {
    if (/^\s*\[[^\]]+]\s*$/.test(lines[index])) {
      end = index;
      break;
    }
  }
  const body = lines.slice(start + 1, end).join("\n");
  const keyMatch = new RegExp(`^${key}\\s*=\\s*"([^"]+)"\\s*$`, "m").exec(body);
  if (!keyMatch) throw new Error(`[${section}] must define ${key} as a quoted string`);
  return keyMatch[1].trim();
}

function workspaceVersion(workspaceSource) {
  return sectionValue(workspaceSource, "workspace.package", "version");
}

function lockedPackageVersion(lockSource, packageName) {
  const packageBlocks = lockSource.split(/^\[\[package]]\s*$/m).slice(1);
  const matches = [];
  for (const block of packageBlocks) {
    const name = /^name\s*=\s*"([^"]+)"\s*$/m.exec(block)?.[1];
    if (name !== packageName) continue;
    const version = /^version\s*=\s*"([^"]+)"\s*$/m.exec(block)?.[1];
    if (!version) throw new Error(`Cargo.lock package ${packageName} has no version`);
    matches.push(version);
  }
  if (matches.length !== 1) {
    throw new Error(`Cargo.lock must contain exactly one ${packageName} package, found ${matches.length}`);
  }
  return matches[0];
}

function releaseSection(changelog, version) {
  const headings = [...changelog.matchAll(RELEASE_HEADING)].filter((match) => match[1] === version);
  if (headings.length !== 1) {
    throw new Error(`CHANGELOG.md must contain exactly one release heading for ${version}`);
  }
  const heading = headings[0];
  const start = heading.index + heading[0].length;
  RELEASE_HEADING.lastIndex = start;
  const next = RELEASE_HEADING.exec(changelog);
  RELEASE_HEADING.lastIndex = 0;
  const end = next?.index ?? changelog.length;
  const body = changelog.slice(start, end).trim();
  if (!body) throw new Error(`CHANGELOG.md release ${version} must not be empty`);
  if (!/^### (Added|Changed|Fixed|Deprecated|Removed|Security)\s*$/m.test(body)) {
    throw new Error(`CHANGELOG.md release ${version} must use standard release subsections`);
  }
  if (!/^-\s+\S/m.test(body)) {
    throw new Error(`CHANGELOG.md release ${version} must contain at least one bullet`);
  }
  if (/^-\s+_No .+ yet\._\s*$/m.test(body)) {
    throw new Error(`CHANGELOG.md release ${version} contains an unreplaced placeholder`);
  }
  const releaseDate = heading[2];
  const timestamp = Date.parse(`${releaseDate}T00:00:00Z`);
  if (
    !Number.isFinite(timestamp) ||
    new Date(timestamp).toISOString().slice(0, 10) !== releaseDate
  ) {
    throw new Error(`CHANGELOG.md release ${version} has an invalid date`);
  }
  return { body, releaseDate };
}

function verifyContract({ tag, workspaceSource, lockSource, changelog }) {
  const parsedTag = parseReleaseTag(tag);
  const declaredVersion = workspaceVersion(workspaceSource);
  if (declaredVersion !== parsedTag.version) {
    throw new Error(
      `workspace version ${declaredVersion} does not match release tag ${parsedTag.version}`,
    );
  }
  const lockedVersion = lockedPackageVersion(lockSource, "rustok-server");
  if (lockedVersion !== parsedTag.version) {
    throw new Error(
      `Cargo.lock rustok-server version ${lockedVersion} does not match release tag ${parsedTag.version}`,
    );
  }
  const notes = releaseSection(changelog, parsedTag.version);
  return { ...parsedTag, ...notes };
}

function writeGithubOutput(file, result) {
  if (!file) return;
  const lines = [
    `tag=${result.tag}`,
    `version=${result.version}`,
    `major=${result.major}`,
    `minor=${result.minor}`,
    `patch=${result.patch}`,
    `prerelease=${result.prerelease ? "true" : "false"}`,
    `release_date=${result.releaseDate}`,
  ];
  fs.appendFileSync(file, `${lines.join("\n")}\n`);
}

function runSelfTest() {
  const workspace = `[workspace]\nmembers = []\n\n[workspace.package]\nversion = "1.2.3-rc.1"\nedition = "2021"\n`;
  const lock = `version = 4\n\n[[package]]\nname = "rustok-server"\nversion = "1.2.3-rc.1"\n`;
  const changelog = `# Changelog\n\n## [Unreleased]\n\n### Added\n- Pending.\n\n## [1.2.3-rc.1] - 2026-07-18\n\n### Security\n- Signed release artifacts.\n`;
  const result = verifyContract({
    tag: "v1.2.3-rc.1",
    workspaceSource: workspace,
    lockSource: lock,
    changelog,
  });
  assert.equal(result.version, "1.2.3-rc.1");
  assert.equal(result.prerelease, "rc.1");
  assert.equal(result.releaseDate, "2026-07-18");
  assert.equal(workspaceVersion("[workspace.package]\nversion = \"9.8.7\"\n"), "9.8.7");
  assert.throws(
    () => workspaceVersion("[workspace.package]\nversion = \"1.0.0\"\n[workspace.package]\nversion = \"2.0.0\"\n"),
    /duplicate/,
  );
  assert.throws(() => parseReleaseTag("1.2.3"), /start with v/);
  assert.throws(() => parseReleaseTag("v01.2.3"), /canonical SemVer/);
  assert.throws(() => parseReleaseTag("v1.2.3+build.1"), /canonical SemVer/);
  assert.throws(
    () =>
      verifyContract({
        tag: "v1.2.4",
        workspaceSource: workspace,
        lockSource: lock,
        changelog,
      }),
    /does not match release tag/,
  );
  assert.throws(
    () =>
      releaseSection(
        `# Changelog\n\n## [1.2.3] - 2026-07-18\n\n### Added\n- _No additions yet._\n`,
        "1.2.3",
      ),
    /placeholder/,
  );
  assert.throws(
    () =>
      releaseSection(
        `# Changelog\n\n## [1.2.3] - 2026-02-31\n\n### Fixed\n- Invalid date fixture.\n`,
        "1.2.3",
      ),
    /invalid date/,
  );
  console.log("✔ release contract verifier self-test passed");
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.selfTest) {
    runSelfTest();
    return;
  }
  for (const name of ["tag", "workspace", "lock", "changelog"]) {
    if (!options[name]) throw new Error(`--${name} is required`);
  }
  const result = verifyContract({
    tag: options.tag,
    workspaceSource: fs.readFileSync(path.resolve(options.workspace), "utf8"),
    lockSource: fs.readFileSync(path.resolve(options.lock), "utf8"),
    changelog: fs.readFileSync(path.resolve(options.changelog), "utf8"),
  });
  writeGithubOutput(options.githubOutput, result);
  console.log(
    `✔ release ${result.tag} matches workspace, Cargo.lock and CHANGELOG.md (${result.releaseDate})`,
  );
}

try {
  main();
} catch (error) {
  console.error(`release contract verification failed: ${error.message}`);
  process.exit(1);
}
