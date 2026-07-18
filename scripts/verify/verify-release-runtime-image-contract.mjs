#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const dockerfilePath = "apps/server/Dockerfile.release";
const workflowPath = ".github/workflows/release.yml";
const failures = [];

function readRegularFile(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!fs.existsSync(absolutePath)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  const stats = fs.lstatSync(absolutePath);
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${relativePath}: must be a regular non-symlink file`);
    return "";
  }
  return fs.readFileSync(absolutePath, "utf8");
}

function requireMarkers(source, relativePath, markers) {
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${relativePath}: missing marker ${marker}`);
  }
}

function forbidMarkers(source, relativePath, markers) {
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${relativePath}: forbidden marker ${marker}`);
  }
}

const dockerfile = readRegularFile(dockerfilePath);
const workflow = readRegularFile(workflowPath);
const baseDigest = "sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818";
const snapshot = "20260713T000000Z";

requireMarkers(dockerfile, dockerfilePath, [
  `FROM debian:bookworm-20260713-slim@${baseDigest}`,
  `org.opencontainers.image.base.name="docker.io/library/debian:bookworm-20260713-slim"`,
  `org.opencontainers.image.base.digest="${baseDigest}"`,
  `http://snapshot.debian.org/archive/debian/${snapshot} bookworm main`,
  `http://snapshot.debian.org/archive/debian/${snapshot} bookworm-updates main`,
  `http://snapshot.debian.org/archive/debian-security/${snapshot} bookworm-security main`,
  'Acquire::Check-Valid-Until "false";',
  "apt-get install --yes --no-install-recommends",
  "ca-certificates",
  "libssl3",
  "rm -rf /var/lib/apt/lists/*",
  "USER 10001:10001",
]);
forbidMarkers(dockerfile, dockerfilePath, [
  "FROM debian:bookworm-slim",
  "FROM debian:${DEBIAN_VERSION}-slim",
  "deb.debian.org",
  "security.debian.org",
  "apt-get upgrade",
  "apt-get dist-upgrade",
  "ARG DEBIAN_",
  "USER root",
]);

requireMarkers(workflow, workflowPath, [
  "--file release-image-context/Dockerfile",
  "--platform linux/amd64",
  "--provenance=mode=max",
  "--sbom=true",
]);
forbidMarkers(workflow, workflowPath, [
  "--build-arg DEBIAN_",
  "--build-arg BASE_IMAGE",
  "--build-arg RUNTIME_IMAGE",
]);

if (failures.length > 0) {
  console.error("Release runtime image contract verification failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  `✔ release runtime uses pinned Debian ${baseDigest}, snapshot ${snapshot}, non-root execution, SBOM and max provenance`,
);
