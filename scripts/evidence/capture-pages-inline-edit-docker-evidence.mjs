#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, renameSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-pages/contracts/evidence/pages-inline-edit-artifact-http-execution-contract.json";

function fail(message) {
  throw new Error(`Pages inline edit Docker evidence capture failed: ${message}`);
}

function parseArguments(argv) {
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--help" || argument === "-h") {
      console.log(
        "usage: capture-pages-inline-edit-docker-evidence.mjs " +
          "--image IMAGE --output FILE [--source-commit SHA]",
      );
      process.exit(0);
    }
    if (["--image", "--output", "--source-commit"].includes(argument)) {
      const value = argv[index + 1];
      if (!value) fail(`${argument} requires a value`);
      const key = argument
        .slice(2)
        .replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
      options[key] = value;
      index += 1;
      continue;
    }
    fail(`unknown argument ${argument}`);
  }
  return options;
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function requireCommit(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{40}$/u.test(value)) {
    fail(`${label} must be a lowercase 40-character git commit`);
  }
  return value;
}

function currentCommit() {
  return requireCommit(
    execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: repoRoot,
      encoding: "utf8",
    }).trim(),
    "git HEAD",
  );
}

function inspectImage(image) {
  let stdout;
  try {
    stdout = execFileSync("docker", ["image", "inspect", image], {
      cwd: repoRoot,
      encoding: "utf8",
      maxBuffer: 32 * 1024 * 1024,
      stdio: ["ignore", "pipe", "pipe"],
    });
  } catch (error) {
    fail(`docker image inspect failed: ${error.message}`);
  }
  let parsed;
  try {
    parsed = JSON.parse(stdout);
  } catch (error) {
    fail(`docker image inspect returned invalid JSON: ${error.message}`);
  }
  if (!Array.isArray(parsed) || parsed.length !== 1) {
    fail("docker image inspect must return exactly one image");
  }
  return { image: parsed[0], stdout };
}

function writeAtomic(output, document) {
  const absolute = path.isAbsolute(output) ? path.resolve(output) : path.resolve(repoRoot, output);
  mkdirSync(path.dirname(absolute), { recursive: true });
  const temporary = `${absolute}.tmp-${process.pid}`;
  rmSync(temporary, { force: true });
  writeFileSync(temporary, `${JSON.stringify(document, null, 2)}\n`, "utf8");
  renameSync(temporary, absolute);
  return absolute;
}

const options = parseArguments(process.argv.slice(2));
if (!options.image) fail("--image is required");
if (!options.output) fail("--output is required");
if (options.image.length > 512 || /[\u0000-\u001f\u007f]/u.test(options.image)) {
  fail("--image is outside the bounded evidence input");
}

const contract = JSON.parse(
  execFileSync("cat", [path.join(repoRoot, contractPath)], { encoding: "utf8" }),
);
const head = currentCommit();
const sourceCommit = options.sourceCommit
  ? requireCommit(options.sourceCommit, "--source-commit")
  : head;
if (sourceCommit !== head) {
  fail(`--source-commit ${sourceCommit} does not match git HEAD ${head}`);
}

const inspected = inspectImage(options.image);
const image = inspected.image;
const config = image.Config ?? {};
const labels = config.Labels ?? {};
const repoDigests = [...(image.RepoDigests ?? [])].sort();
if (repoDigests.length === 0 || repoDigests.some((value) => !/@sha256:[0-9a-f]{64}$/u.test(value))) {
  fail("image must have at least one immutable sha256 RepoDigest");
}
if (!/^sha256:[0-9a-f]{64}$/u.test(image.Id ?? "")) {
  fail("image Id must be a canonical sha256 digest");
}
const platform = `${image.Os ?? ""}/${image.Architecture ?? ""}`;
if (platform !== contract.docker_capture.required_platform) {
  fail(`image platform ${platform} does not match ${contract.docker_capture.required_platform}`);
}
if (config.User !== contract.docker_capture.required_user) {
  fail(`image user ${config.User ?? "<unset>"} does not match ${contract.docker_capture.required_user}`);
}
const entrypoint = Array.isArray(config.Entrypoint) ? config.Entrypoint : [];
if (!entrypoint.includes(contract.docker_capture.required_entrypoint)) {
  fail(`image entrypoint does not include ${contract.docker_capture.required_entrypoint}`);
}
const revision = labels["org.opencontainers.image.revision"];
if (revision !== sourceCommit) {
  fail(`OCI revision ${revision ?? "<missing>"} does not match source commit ${sourceCommit}`);
}
if (!Number.isSafeInteger(image.Size) || image.Size <= 0) {
  fail("image Size must be a positive safe integer");
}

const document = {
  format: contract.docker_capture.format,
  status: "passed",
  source_commit: sourceCommit,
  captured_at: new Date().toISOString(),
  requested_image: options.image,
  image_id: image.Id,
  repo_digests: repoDigests,
  size_bytes: image.Size,
  platform,
  runtime: {
    user: config.User,
    entrypoint,
    working_dir: config.WorkingDir ?? null,
  },
  oci: {
    source: labels["org.opencontainers.image.source"] ?? null,
    version: labels["org.opencontainers.image.version"] ?? null,
    revision,
    base_name: labels["org.opencontainers.image.base.name"] ?? null,
    base_digest: labels["org.opencontainers.image.base.digest"] ?? null,
  },
  inspect_output: {
    bytes: Buffer.byteLength(inspected.stdout),
    sha256: sha256(inspected.stdout),
    raw_document_persisted: false,
  },
  privacy: {
    docker_inspect_document_persisted: false,
    environment_values_persisted: false,
    credentials_persisted: false,
  },
};

const output = writeAtomic(options.output, document);
console.log(
  `[capture-pages-inline-edit-docker-evidence] PASS image_id=${image.Id} output=${output}`,
);
