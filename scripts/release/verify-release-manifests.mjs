#!/usr/bin/env node

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { resolveReleaseHistory } from "./resolve-release-history.mjs";

const manifestName = "release-manifest.json";

function runGitHub(args, directory) {
  const result = spawnSync("gh", args, {
    cwd: directory,
    env: { ...process.env, GH_PROMPT_DISABLED: "1", GH_NO_UPDATE_NOTIFIER: "1" },
    stdio: "ignore",
    timeout: 60_000,
    windowsHide: true,
  });
  if (result.error) throw new Error(`could not run gh ${args[0]} ${args[1]} (${result.error.code})`);
  if (result.status !== 0) {
    throw new Error(`gh ${args[0]} ${args[1]} failed (${result.signal ?? result.status})`);
  }
}

function readManifest(file) {
  const stat = fs.lstatSync(file);
  if (!stat.isFile() || stat.size === 0 || stat.size > 4 * 1024 * 1024) {
    throw new Error(`${manifestName} must be a nonempty regular file of at most 4 MiB`);
  }
  return fs.readFileSync(file);
}

function checkUnchanged(file, bytes) {
  if (!bytes.equals(readManifest(file))) throw new Error(`${manifestName} changed during verification`);
}

function verifiedArtifactIndex(bytes, repository, release) {
  const manifest = JSON.parse(bytes.toString("utf8"));
  if (manifest?.schema_version !== 1) throw new Error("unsupported release manifest schema version");
  const identity = manifest.release;
  if (identity?.tag !== release.tag || identity?.version !== release.tag.slice(1)
    || identity?.commit !== release.commit || typeof identity?.repository !== "string"
    || identity.repository.toLowerCase() !== repository.toLowerCase()) {
    throw new Error("manifest identity differs from the signed release tag");
  }
  if (!Array.isArray(manifest.artifacts) || manifest.artifacts.length === 0) {
    throw new Error("release manifest must contain an artifact index");
  }

  const names = new Set();
  return manifest.artifacts.map((asset) => {
    if (typeof asset?.name !== "string" || !/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(asset.name)
      || names.has(asset.name) || typeof asset.sha256 !== "string" || !/^[0-9a-f]{64}$/.test(asset.sha256)
      || !Number.isSafeInteger(asset.size_bytes) || asset.size_bytes < 1) {
      throw new Error("release manifest contains an invalid or repeated artifact");
    }
    names.add(asset.name);
    return { name: asset.name, size_bytes: asset.size_bytes, sha256: asset.sha256 };
  });
}

function verifyManifest(repository, release, directory, run) {
  const file = path.join(directory, manifestName);
  const repo = ["--repo", `github.com/${repository}`];
  try {
    run(["release", "download", release.tag, ...repo, "--pattern", manifestName, "--output", file], directory);
    const bytes = readManifest(file);
    run(["release", "verify-asset", release.tag, file, ...repo], directory);
    checkUnchanged(file, bytes);
    run([
      "attestation", "verify", file,
      "--hostname", "github.com",
      "--repo", repository,
      "--signer-workflow", `${repository}/.github/workflows/release.yml`,
      "--signer-digest", release.commit,
      "--source-ref", `refs/tags/${release.tag}`,
      "--source-digest", release.commit,
      "--predicate-type", "https://slsa.dev/provenance/v1",
      "--deny-self-hosted-runners",
    ], directory);
    checkUnchanged(file, bytes);
    return {
      ...release,
      manifest_sha256: createHash("sha256").update(bytes).digest("hex"),
      artifacts: verifiedArtifactIndex(bytes, repository, release),
    };
  } catch (error) {
    throw new Error(`${release.tag}: ${error.message}`);
  } finally {
    fs.rmSync(file, { force: true });
  }
}

function verifyManifests(history, scratch, run = runGitHub) {
  const directory = fs.mkdtempSync(path.join(fs.realpathSync(scratch), "rustok-release-manifests-"));
  try {
    const releases = history.releases.map((release) => verifyManifest(history.repository, release, directory, run));
    return { repository: history.repository, releases };
  } finally {
    fs.rmdirSync(directory);
  }
}

function runSelfTest() {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "rustok-verified-manifest-fixture-"));
  const repository = "RusTokRs/RusTok";
  const release = { id: 1, tag: "v1.2.3", commit: "c".repeat(40), tag_object: "d".repeat(40) };
  const history = { repository, releases: [release] };
  const version = "1.2.3";
  const image = path.join(directory, "container-image.json");
  const archive = path.join(directory, `rustok-server-${version}-linux-x86_64.tar.gz`);
  const sbom = path.join(directory, `rustok-server-${version}.spdx.json`);
  const manifest = path.join(directory, manifestName);
  const checksums = path.join(directory, "SHA256SUMS");
  try {
    fs.writeFileSync(archive, "test archive");
    fs.writeFileSync(sbom, "{}\n");
    fs.writeFileSync(image, JSON.stringify({
      schema_version: 1, version, commit: release.commit, image: "ghcr.io/rustokrs/rustok",
      digest: `sha256:${"a".repeat(64)}`, tags: [`ghcr.io/rustokrs/rustok:${version}`],
    }));
    const finalizer = fileURLToPath(new URL("./finalize-release-artifacts.mjs", import.meta.url));
    const result = spawnSync(process.execPath, [
      finalizer, "--directory", directory, "--version", version, "--tag", release.tag,
      "--commit", release.commit, "--repository", repository, "--created-epoch", "1784332800",
      "--image-metadata", image,
    ], { encoding: "utf8" });
    assert.ifError(result.error);
    assert.equal(result.status, 0, result.stderr);
    const bytes = fs.readFileSync(manifest);

    const calls = [];
    const fixture = (args, scratch) => {
      calls.push(args);
      if (args[1] === "download") fs.writeFileSync(path.join(scratch, manifestName), bytes);
    };
    const verified = verifyManifests(history, directory, fixture);
    assert.deepEqual(calls.map((args) => args.slice(0, 2)), [
      ["release", "download"], ["release", "verify-asset"], ["attestation", "verify"],
    ]);
    assert.equal(calls[0][2], release.tag);
    assert.equal(calls[1][2], release.tag);
    const attest = calls[2];
    assert.equal(attest[attest.indexOf("--signer-workflow") + 1], `${repository}/.github/workflows/release.yml`);
    assert.equal(attest[attest.indexOf("--signer-digest") + 1], release.commit);
    assert.equal(attest[attest.indexOf("--source-ref") + 1], `refs/tags/${release.tag}`);
    assert.equal(attest[attest.indexOf("--source-digest") + 1], release.commit);
    assert.deepEqual(verified.releases[0].artifacts, JSON.parse(bytes).artifacts);
    assert(fs.readFileSync(checksums, "utf8").includes(`${verified.releases[0].manifest_sha256}  ${manifestName}`));

    for (const failure of ["download", "verify-asset", "verify"]) {
      assert.throws(() => verifyManifests(history, directory, (args, scratch) => {
        fixture(args, scratch);
        if (args[1] === failure) throw new Error("test rejection");
      }), /v1\.2\.3: test rejection/);
    }
    assert.throws(() => verifyManifests(history, directory, (args, scratch) => {
      fixture(args, scratch);
      if (args[1] === "verify-asset") fs.appendFileSync(path.join(scratch, manifestName), "\n");
    }), /changed during verification/);
    const wrongCommit = { repository, releases: [{ ...release, commit: "e".repeat(40) }] };
    assert.throws(() => verifyManifests(wrongCommit, directory, fixture), /identity differs from the signed release tag/);
    const duplicate = JSON.parse(bytes);
    duplicate.artifacts.push(duplicate.artifacts[0]);
    assert.throws(() => verifiedArtifactIndex(Buffer.from(JSON.stringify(duplicate)), repository, release), /repeated artifact/);
    assert.equal(fs.readdirSync(directory).length, 5);
  } finally {
    for (const file of [image, archive, sbom, manifest, checksums]) fs.rmSync(file, { force: true });
    fs.rmdirSync(directory);
  }
  console.log("✔ published release manifest verifier self-test passed");
}

try {
  const args = process.argv.slice(2);
  if (args.length === 1 && args[0] === "--self-test") {
    runSelfTest();
  } else if (args.length === 2 && args[0] === "--scratch") {
    const scratch = fs.realpathSync(args[1]);
    if (!fs.statSync(scratch).isDirectory()) throw new Error("--scratch must be an existing directory");
    const history = await resolveReleaseHistory();
    const result = verifyManifests(history, scratch);
    console.log(JSON.stringify(result, null, 2));
  } else {
    throw new Error("usage: verify-release-manifests.mjs --scratch DIRECTORY | --self-test");
  }
} catch (error) {
  console.error(`release manifest verification failed: ${error.message}`);
  process.exitCode = 1;
}
