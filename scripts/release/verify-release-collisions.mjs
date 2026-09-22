#!/usr/bin/env node

import assert from "node:assert/strict";
import process from "node:process";

const ACCEPT_MANIFEST = [
  "application/vnd.oci.image.index.v1+json",
  "application/vnd.oci.image.manifest.v1+json",
  "application/vnd.docker.distribution.manifest.list.v2+json",
  "application/vnd.docker.distribution.manifest.v2+json",
].join(", ");

function parseArguments(argv) {
  const options = { selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") {
      options.selfTest = true;
      continue;
    }
    if (argument === "--github-release") {
      if (options.kind) throw new Error("choose exactly one collision probe kind");
      options.kind = "github-release";
      continue;
    }
    if (argument === "--container-tag") {
      if (options.kind) throw new Error("choose exactly one collision probe kind");
      options.kind = "container-tag";
      continue;
    }
    if (["--repository", "--tag", "--image", "--actor"].includes(argument)) {
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

function requireRepository(value) {
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(value || "")) {
    throw new Error("--repository must be owner/name");
  }
  return value;
}

function requireTag(value, label = "--tag") {
  if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/.test(value || "")) {
    throw new Error(`${label} must be a safe tag name`);
  }
  return value;
}

function requireImage(value) {
  const match = /^ghcr\.io\/([a-z0-9_.-]+)\/([a-z0-9_.-]+)$/.exec(value || "");
  if (!match) throw new Error("--image must be a lowercase ghcr.io/owner/package reference");
  return { image: value, repositoryPath: `${match[1]}/${match[2]}` };
}

function requiredSecret(...names) {
  for (const name of names) {
    const value = process.env[name]?.trim();
    if (value) return value;
  }
  throw new Error(`one of ${names.join(", ")} must be set`);
}

function requestOptions(headers = {}) {
  return {
    headers,
    redirect: "error",
    signal: AbortSignal.timeout(15_000),
  };
}

async function checkedFetch(url, options, label) {
  try {
    return await fetch(url, options);
  } catch (error) {
    throw new Error(`${label} request failed: ${error.message}`);
  }
}

function assertAbsentStatus(status, label) {
  if (status === 404) return;
  if (status >= 200 && status < 300) throw new Error(`${label} already exists`);
  throw new Error(`${label} collision probe returned unexpected HTTP ${status}`);
}

async function verifyGithubReleaseAbsent(options) {
  const repository = requireRepository(options.repository);
  const tag = requireTag(options.tag);
  const token = requiredSecret("GITHUB_TOKEN", "GH_TOKEN");
  const response = await checkedFetch(
    `https://api.github.com/repos/${repository}/releases/tags/${encodeURIComponent(tag)}`,
    requestOptions({
      Accept: "application/vnd.github+json",
      Authorization: `Bearer ${token}`,
      "User-Agent": "rustok-release-collision-probe/1",
      "X-GitHub-Api-Version": "2022-11-28",
    }),
    "GitHub Release",
  );
  assertAbsentStatus(response.status, `GitHub Release ${tag}`);
  console.log(`✔ GitHub Release ${tag} does not exist`);
}

function parseBearerChallenge(value) {
  const match = /^Bearer\s+(.+)$/i.exec(value || "");
  if (!match) throw new Error("GHCR authentication challenge is missing a Bearer scheme");
  const parameters = {};
  for (const item of match[1].matchAll(/([A-Za-z][A-Za-z0-9_-]*)="([^"]*)"/g)) {
    parameters[item[1].toLowerCase()] = item[2];
  }
  if (!parameters.realm || !parameters.service || !parameters.scope) {
    throw new Error("GHCR authentication challenge is incomplete");
  }
  let realm;
  try {
    realm = new URL(parameters.realm);
  } catch {
    throw new Error("GHCR authentication realm is not a valid URL");
  }
  if (realm.protocol !== "https:" || realm.hostname !== "ghcr.io") {
    throw new Error("GHCR authentication realm must be https://ghcr.io");
  }
  return { ...parameters, realm };
}

async function requestRegistryToken(challenge, actor, token) {
  const url = new URL(challenge.realm);
  url.searchParams.set("service", challenge.service);
  url.searchParams.set("scope", challenge.scope);
  const basic = Buffer.from(`${actor}:${token}`, "utf8").toString("base64");
  const response = await checkedFetch(
    url,
    requestOptions({
      Accept: "application/json",
      Authorization: `Basic ${basic}`,
      "User-Agent": "rustok-release-collision-probe/1",
    }),
    "GHCR token",
  );
  if (response.status !== 200) {
    throw new Error(`GHCR token request returned HTTP ${response.status}`);
  }
  let payload;
  try {
    payload = await response.json();
  } catch (error) {
    throw new Error(`GHCR token response is not valid JSON: ${error.message}`);
  }
  const bearer = String(payload.token || payload.access_token || "").trim();
  if (!bearer) throw new Error("GHCR token response did not include a registry token");
  return bearer;
}

async function registryManifestResponse(repositoryPath, tag, authorization) {
  return checkedFetch(
    `https://ghcr.io/v2/${repositoryPath}/manifests/${encodeURIComponent(tag)}`,
    {
      ...requestOptions({
        Accept: ACCEPT_MANIFEST,
        Authorization: authorization,
        "User-Agent": "rustok-release-collision-probe/1",
      }),
      method: "HEAD",
    },
    "GHCR manifest",
  );
}

async function verifyContainerTagAbsent(options) {
  const { image, repositoryPath } = requireImage(options.image);
  const tag = requireTag(options.tag, "--tag");
  const actor = String(options.actor || "").trim();
  if (!/^[A-Za-z0-9-]+$/.test(actor)) throw new Error("--actor must be a GitHub login");
  const token = requiredSecret("GHCR_TOKEN", "GITHUB_TOKEN");
  const basic = Buffer.from(`${actor}:${token}`, "utf8").toString("base64");
  let response = await registryManifestResponse(repositoryPath, tag, `Basic ${basic}`);
  if (response.status === 401) {
    const challenge = parseBearerChallenge(response.headers.get("www-authenticate"));
    if (challenge.scope !== `repository:${repositoryPath}:pull`) {
      throw new Error(`GHCR challenge requested unexpected scope ${challenge.scope}`);
    }
    const bearer = await requestRegistryToken(challenge, actor, token);
    response = await registryManifestResponse(repositoryPath, tag, `Bearer ${bearer}`);
  }
  assertAbsentStatus(response.status, `container tag ${image}:${tag}`);
  console.log(`✔ container tag ${image}:${tag} does not exist`);
}

function runSelfTest() {
  assert.doesNotThrow(() => assertAbsentStatus(404, "sample"));
  assert.throws(() => assertAbsentStatus(200, "sample"), /already exists/);
  assert.throws(() => assertAbsentStatus(401, "sample"), /unexpected HTTP 401/);
  const challenge = parseBearerChallenge(
    'Bearer realm="https://ghcr.io/token",service="ghcr.io",scope="repository:rustokrs/rustok:pull"',
  );
  assert.equal(challenge.realm.href, "https://ghcr.io/token");
  assert.equal(challenge.service, "ghcr.io");
  assert.equal(challenge.scope, "repository:rustokrs/rustok:pull");
  assert.throws(
    () => parseBearerChallenge('Bearer realm="https://example.com/token",service="ghcr.io",scope="x"'),
    /must be https:\/\/ghcr.io/,
  );
  assert.deepEqual(requireImage("ghcr.io/rustokrs/rustok"), {
    image: "ghcr.io/rustokrs/rustok",
    repositoryPath: "rustokrs/rustok",
  });
  console.log("✔ release collision probe self-test passed");
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.selfTest) {
    runSelfTest();
    return;
  }
  if (options.kind === "github-release") {
    await verifyGithubReleaseAbsent(options);
    return;
  }
  if (options.kind === "container-tag") {
    await verifyContainerTagAbsent(options);
    return;
  }
  throw new Error("choose --github-release or --container-tag");
}

try {
  await main();
} catch (error) {
  console.error(`release collision verification failed: ${error.message}`);
  process.exit(1);
}
