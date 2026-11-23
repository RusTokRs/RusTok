#!/usr/bin/env node

import assert from "node:assert/strict";
import process from "node:process";
import path from "node:path";
import { fileURLToPath } from "node:url";

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
  if (realm.origin !== "https://ghcr.io" || realm.username || realm.password
    || realm.pathname !== "/token" || realm.search || realm.hash) {
    throw new Error("GHCR authentication realm must be https://ghcr.io");
  }
  return { ...parameters, realm };
}

async function requestRegistryToken(challenge, basic, request) {
  const url = new URL(challenge.realm);
  url.searchParams.set("service", challenge.service);
  url.searchParams.set("scope", challenge.scope);
  const response = await request(
    url,
    requestOptions({
      Accept: "application/json",
      Authorization: basic,
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
  const bearer = payload?.token ?? payload?.access_token;
  if (typeof bearer !== "string" || !/^\S+$/.test(bearer)) {
    throw new Error("GHCR token response did not include a registry token");
  }
  return bearer;
}

function registryRequester(repositoryPath, actor, token, request = checkedFetch) {
  const basic = `Basic ${Buffer.from(`${actor}:${token}`, "utf8").toString("base64")}`;
  let authorization = basic;
  return async (url, method, accept, label) => {
    const fetchResource = () => request(url, {
      ...requestOptions({
        Accept: accept,
        Authorization: authorization,
        "User-Agent": "rustok-release-collision-probe/1",
      }),
      method,
    }, label);
    const first = await fetchResource();
    if (first.status !== 401) return first;

    const challenge = parseBearerChallenge(first.headers.get("www-authenticate"));
    if (challenge.scope !== `repository:${repositoryPath}:pull` || challenge.service !== "ghcr.io") {
      throw new Error("GHCR authentication challenge does not match the requested repository");
    }
    const bearer = await requestRegistryToken(challenge, basic, request);
    authorization = `Bearer ${bearer}`;
    return fetchResource();
  };
}

function authenticatedRegistry(repositoryPath, login) {
  const actor = String(login || "").trim();
  if (!/^[A-Za-z0-9-]+$/.test(actor)) throw new Error("GHCR requires a GitHub login in --actor or GITHUB_ACTOR");
  const token = requiredSecret("GHCR_TOKEN", "GITHUB_TOKEN");
  return registryRequester(repositoryPath, actor, token);
}

async function verifyContainerTagAbsent(options) {
  const { image, repositoryPath } = requireImage(options.image);
  const tag = requireTag(options.tag, "--tag");
  const request = authenticatedRegistry(repositoryPath, options.actor);
  const response = await request(
    `https://ghcr.io/v2/${repositoryPath}/manifests/${encodeURIComponent(tag)}`,
    "HEAD", ACCEPT_MANIFEST, "GHCR manifest",
  );
  assertAbsentStatus(response.status, `container tag ${image}:${tag}`);
  console.log(`✔ container tag ${image}:${tag} does not exist`);
}

function nextTagsPage(link, url, lastTag) {
  const match = /^\s*<([^>]+)>\s*;\s*rel\s*=\s*(?:"next"|next)\s*$/i.exec(link);
  if (!match || !lastTag) throw new Error("invalid GHCR tags pagination link");
  const next = new URL(match[1], url);
  const pageSizes = next.searchParams.getAll("n");
  const size = pageSizes[0] ?? url.searchParams.get("n");
  if (next.origin !== url.origin || next.pathname !== url.pathname || next.username || next.password || next.hash
    || next.searchParams.getAll("last").length !== 1 || next.searchParams.get("last") !== lastTag
    || pageSizes.length > 1 || !/^(?:[1-9]\d?|100)$/.test(size)
    || [...next.searchParams.keys()].some((key) => key !== "last" && key !== "n")) {
    throw new Error("GHCR tags pagination link does not match the current repository and cursor");
  }
  next.searchParams.set("n", size);
  return next;
}

async function readRegistryTags(repositoryPath, request) {
  const pageSize = 100;
  let url = new URL(`https://ghcr.io/v2/${repositoryPath}/tags/list?n=${pageSize}`);
  const tags = new Set();
  for (;;) {
    const response = await request(url, "GET", "application/json", "GHCR tag list");
    if (response.status !== 200) throw new Error(`GHCR tag list returned HTTP ${response.status}`);
    const payload = await response.json();
    const requested = Number(url.searchParams.get("n"));
    if (payload?.name !== repositoryPath || !Array.isArray(payload.tags) || payload.tags.length > requested) {
      throw new Error("invalid GHCR tag list for the requested repository");
    }
    for (const tag of payload.tags) {
      if (typeof tag !== "string" || !/^\w[\w.-]{0,127}$/.test(tag) || tags.has(tag)) {
        throw new Error("invalid or repeated GHCR tag");
      }
      tags.add(tag);
    }
    const link = response.headers.get("link");
    if (link !== null) {
      url = nextTagsPage(link, url, payload.tags.at(-1));
    } else if (payload.tags.length === requested) {
      url.searchParams.set("last", payload.tags.at(-1));
    } else {
      return [...tags].sort();
    }
  }
}

export async function listContainerTags(image, actor) {
  const { repositoryPath } = requireImage(image);
  return readRegistryTags(repositoryPath, authenticatedRegistry(repositoryPath, actor));
}

async function runSelfTest() {
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
  const repositoryPath = "rustokrs/rustok";
  const endpoint = `/v2/${repositoryPath}/tags/list`;
  const page = (tags, link) => new Response(JSON.stringify({ name: repositoryPath, tags }), {
    headers: link ? { Link: link } : {},
  });
  let calls = 0;
  const authenticated = registryRequester(repositoryPath, "release-bot", "fixture-secret", async (url, options) => {
    assert.equal(new URL(url).origin, "https://ghcr.io");
    assert.equal(options.redirect, "error");
    const request = new URL(url);
    switch (calls++) {
      case 0:
        assert.equal(request.pathname, endpoint);
        assert.match(options.headers.Authorization, /^Basic /);
        return new Response(null, { status: 401, headers: {
          "www-authenticate": 'Bearer realm="https://ghcr.io/token",service="ghcr.io",scope="repository:rustokrs/rustok:pull"',
        } });
      case 1:
        assert.equal(request.pathname, "/token");
        assert.equal(request.searchParams.get("scope"), "repository:rustokrs/rustok:pull");
        assert.match(options.headers.Authorization, /^Basic /);
        return new Response('{"token":"fixture-bearer"}');
      case 2:
        assert.equal(options.headers.Authorization, "Bearer fixture-bearer");
        return page(["1.0.0-rc.1"], `<${endpoint}?last=1.0.0-rc.1>; rel="next"`);
      case 3:
        assert.equal(options.headers.Authorization, "Bearer fixture-bearer");
        assert.equal(request.searchParams.get("last"), "1.0.0-rc.1");
        assert.equal(request.searchParams.get("n"), "100");
        return page(["latest"]);
      default:
        assert.fail("unexpected GHCR request");
    }
  });
  assert.deepEqual(await readRegistryTags(repositoryPath, authenticated), ["1.0.0-rc.1", "latest"]);
  const fullPage = Array.from({ length: 100 }, (_, i) => `tag-${String(i).padStart(2, "0")}`);
  assert.deepEqual(await readRegistryTags(repositoryPath, async (url) => {
    return url.searchParams.has("last") ? page([]) : page(fullPage);
  }), fullPage);
  await assert.rejects(readRegistryTags(repositoryPath, async () => page(["same", "same"])), /repeated GHCR tag/);
  for (const status of [401, 403, 404, 429]) {
    await assert.rejects(readRegistryTags(repositoryPath, async () => new Response(null, { status })), new RegExp(`HTTP ${status}`));
  }
  for (const next of [
    `https://example.invalid${endpoint}?last=sample&n=100`,
    "https://ghcr.io/v2/other/package/tags/list?last=sample&n=100",
    `${endpoint}?last=unvisited&n=100`,
  ]) {
    await assert.rejects(readRegistryTags(repositoryPath, async () => page(["sample"], `<${next}>; rel="next"`)), /pagination link does not match/);
  }
  const badChallenge = registryRequester(repositoryPath, "release-bot", "fixture-secret", async (url) => {
    assert.equal(new URL(url).pathname, endpoint);
    return new Response(null, { status: 401, headers: {
      "www-authenticate": 'Bearer realm="https://ghcr.io/token",service="ghcr.io",scope="repository:other/package:pull"',
    } });
  });
  await assert.rejects(readRegistryTags(repositoryPath, badChallenge), /challenge does not match/);
  console.log("✔ release collision probe self-test passed");
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.selfTest) {
    await runSelfTest();
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

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    await main();
  } catch (error) {
    console.error(`release collision verification failed: ${error.message}`);
    process.exitCode = 1;
  }
}
