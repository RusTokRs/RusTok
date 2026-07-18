#!/usr/bin/env node

import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";

function parseArguments(argv) {
  const options = {
    selfTest: false,
    manifestName: "release-manifest.json",
    checksumsName: "SHA256SUMS",
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") {
      options.selfTest = true;
      continue;
    }
    if (
      [
        "--directory",
        "--version",
        "--tag",
        "--commit",
        "--repository",
        "--created-epoch",
        "--image-metadata",
        "--manifest-name",
        "--checksums-name",
      ].includes(argument)
    ) {
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

function sha256(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function releaseTimestamp(raw) {
  if (!/^(?:0|[1-9]\d*)$/.test(String(raw || ""))) {
    throw new Error("--created-epoch must be a non-negative integer Unix timestamp");
  }
  const timestamp = Number(raw) * 1000;
  if (!Number.isSafeInteger(timestamp)) throw new Error("--created-epoch is out of range");
  const created = new Date(timestamp);
  if (!Number.isFinite(created.getTime())) throw new Error("--created-epoch is invalid");
  return created.toISOString().replace(".000Z", "Z");
}

function validateFileName(name, label) {
  if (typeof name !== "string" || !/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(name)) {
    throw new Error(`${label} must be a safe basename`);
  }
}

function readImageMetadata(file, version, commit) {
  const metadata = JSON.parse(fs.readFileSync(file, "utf8"));
  if (metadata.schema_version !== 1) throw new Error("container metadata schema_version must be 1");
  if (metadata.version !== version) throw new Error("container metadata version does not match release");
  if (metadata.commit !== commit) throw new Error("container metadata commit does not match release");
  if (!/^ghcr\.io\/[a-z0-9_.-]+\/[a-z0-9_.-]+$/.test(metadata.image || "")) {
    throw new Error("container metadata image must be a lowercase ghcr.io owner/name reference");
  }
  if (!/^sha256:[0-9a-f]{64}$/.test(metadata.digest || "")) {
    throw new Error("container metadata digest must be sha256:<64 lowercase hex characters>");
  }
  if (!Array.isArray(metadata.tags) || metadata.tags.length === 0) {
    throw new Error("container metadata tags must be a non-empty array");
  }
  const tags = [...new Set(metadata.tags)];
  if (tags.length !== metadata.tags.length) throw new Error("container metadata tags must be unique");
  for (const tag of tags) {
    if (typeof tag !== "string" || !tag.startsWith(`${metadata.image}:`)) {
      throw new Error(`container metadata tag ${JSON.stringify(tag)} is not scoped to ${metadata.image}`);
    }
  }
  return { image: metadata.image, digest: metadata.digest, tags: [...tags].sort() };
}

function regularFiles(directory, excludedNames) {
  const entries = fs.readdirSync(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    if (excludedNames.has(entry.name)) continue;
    validateFileName(entry.name, "release asset name");
    const file = path.join(directory, entry.name);
    const stats = fs.lstatSync(file);
    if (stats.isSymbolicLink()) throw new Error(`release asset ${entry.name} must not be a symlink`);
    if (!entry.isFile() || !stats.isFile()) {
      throw new Error(`release artifact directory must contain regular files only: ${entry.name}`);
    }
    if (stats.size === 0) throw new Error(`release asset ${entry.name} must not be empty`);
    files.push({ name: entry.name, file, size: stats.size, sha256: sha256(file) });
  }
  return files.sort((left, right) => left.name.localeCompare(right.name));
}

function finalize(options) {
  const directory = path.resolve(options.directory);
  if (!fs.statSync(directory).isDirectory()) throw new Error("--directory must be a directory");
  validateFileName(options.manifestName, "--manifest-name");
  validateFileName(options.checksumsName, "--checksums-name");
  if (options.manifestName === options.checksumsName) {
    throw new Error("manifest and checksums names must be different");
  }
  if (options.tag !== `v${options.version}`) throw new Error("--tag must equal v<version>");
  if (!/^[0-9a-f]{40}$/i.test(options.commit)) {
    throw new Error("--commit must be a full 40-character Git commit SHA");
  }
  const repository = options.repository.replace(/^https?:\/\/github\.com\//, "").replace(/\/$/, "");
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) {
    throw new Error("--repository must be owner/name or a canonical github.com repository URL");
  }

  const manifestPath = path.join(directory, options.manifestName);
  const checksumsPath = path.join(directory, options.checksumsName);
  fs.rmSync(manifestPath, { force: true });
  fs.rmSync(checksumsPath, { force: true });

  const image = readImageMetadata(path.resolve(options.imageMetadata), options.version, options.commit);
  const assets = regularFiles(
    directory,
    new Set([options.manifestName, options.checksumsName]),
  );
  if (!assets.some((asset) => asset.name.endsWith(".tar.gz"))) {
    throw new Error("release artifact directory must contain a .tar.gz binary archive");
  }
  if (!assets.some((asset) => asset.name.endsWith(".spdx.json"))) {
    throw new Error("release artifact directory must contain an SPDX JSON SBOM");
  }

  const manifest = {
    schema_version: 1,
    release: {
      tag: options.tag,
      version: options.version,
      commit: options.commit.toLowerCase(),
      repository,
      created: releaseTimestamp(options.createdEpoch),
    },
    container: image,
    artifacts: assets.map(({ name, size, sha256: digest }) => ({
      name,
      size_bytes: size,
      sha256: digest,
    })),
  };
  fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);

  const checksumAssets = regularFiles(directory, new Set([options.checksumsName]));
  const checksumText = checksumAssets
    .map((asset) => `${asset.sha256}  ${asset.name}`)
    .join("\n");
  fs.writeFileSync(checksumsPath, `${checksumText}\n`);
  return { manifest, checksumAssets };
}

function runSelfTest() {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "rustok-release-finalize-"));
  try {
    fs.writeFileSync(path.join(directory, "rustok-server-1.2.3-linux-x86_64.tar.gz"), "archive");
    fs.writeFileSync(path.join(directory, "rustok-server-1.2.3.spdx.json"), "{\"spdxVersion\":\"SPDX-2.3\"}\n");
    const imageMetadata = path.join(directory, "container-image.json");
    fs.writeFileSync(
      imageMetadata,
      `${JSON.stringify({
        schema_version: 1,
        version: "1.2.3",
        commit: "0123456789abcdef0123456789abcdef01234567",
        image: "ghcr.io/rustokrs/rustok",
        digest: `sha256:${"a".repeat(64)}`,
        tags: ["ghcr.io/rustokrs/rustok:1.2.3"],
      })}\n`,
    );
    const options = {
      directory,
      version: "1.2.3",
      tag: "v1.2.3",
      commit: "0123456789abcdef0123456789abcdef01234567",
      repository: "RusTokRs/RusTok",
      createdEpoch: "1784332800",
      imageMetadata,
      manifestName: "release-manifest.json",
      checksumsName: "SHA256SUMS",
    };
    const first = finalize(options);
    const firstChecksums = fs.readFileSync(path.join(directory, "SHA256SUMS"), "utf8");
    const second = finalize(options);
    const secondChecksums = fs.readFileSync(path.join(directory, "SHA256SUMS"), "utf8");
    assert.deepEqual(first.manifest, second.manifest);
    assert.equal(firstChecksums, secondChecksums);
    assert(firstChecksums.includes("release-manifest.json"));
    assert.equal(first.manifest.container.digest, `sha256:${"a".repeat(64)}`);
    assert.throws(
      () => readImageMetadata(imageMetadata, "1.2.4", options.commit),
      /version does not match/,
    );
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
  console.log("✔ release artifact finalizer self-test passed");
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.selfTest) {
    runSelfTest();
    return;
  }
  for (const name of [
    "directory",
    "version",
    "tag",
    "commit",
    "repository",
    "createdEpoch",
    "imageMetadata",
  ]) {
    if (!options[name]) throw new Error(`--${name.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)} is required`);
  }
  const result = finalize(options);
  console.log(
    `✔ finalized ${result.checksumAssets.length} release asset(s) with manifest and SHA256SUMS`,
  );
}

try {
  main();
} catch (error) {
  console.error(`release artifact finalization failed: ${error.message}`);
  process.exit(1);
}
