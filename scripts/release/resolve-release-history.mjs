#!/usr/bin/env node

import assert from "node:assert/strict";
import process from "node:process";
import { parseReleaseTag } from "./verify-release-contract.mjs";

const repository = "RusTokRs/RusTok";
const pageSize = 100;

async function readGitHub(endpoint) {
  const token = process.env.GH_TOKEN?.trim() || process.env.GITHUB_TOKEN?.trim();
  const headers = {
    Accept: "application/vnd.github+json",
    "User-Agent": "rustok-release-history/1",
    "X-GitHub-Api-Version": "2022-11-28",
  };
  if (token) headers.Authorization = `Bearer ${token}`;
  const response = await fetch(`https://api.github.com/repos/${repository}/${endpoint}`, {
    headers,
    redirect: "error",
    signal: AbortSignal.timeout(15_000),
  });
  if (!response.ok) throw new Error(`GitHub ${endpoint} returned HTTP ${response.status}`);
  return await response.json();
}

async function stableReleases(read) {
  const ids = new Set();
  const publishedTags = new Set();
  const stable = [];
  // Read through an empty page so a short page cannot silently truncate history.
  for (let page = 1; ; page += 1) {
    const releases = await read(`releases?per_page=${pageSize}&page=${page}`);
    if (!Array.isArray(releases) || releases.length > pageSize) {
      throw new Error(`invalid GitHub releases page ${page}`);
    }
    if (releases.length === 0) return stable;

    for (const release of releases) {
      if (!Number.isSafeInteger(release?.id) || release.id < 1 || ids.has(release.id)
        || typeof release.draft !== "boolean" || typeof release.prerelease !== "boolean") {
        throw new Error(`invalid or repeated release record on page ${page}`);
      }
      ids.add(release.id);
      if (release.draft) continue;

      const { tag, prerelease } = parseReleaseTag(release.tag_name);
      if (publishedTags.has(tag) || Boolean(prerelease) !== release.prerelease) {
        throw new Error(`inconsistent published release classification for ${tag}`);
      }
      publishedTags.add(tag);
      if (prerelease) continue;
      if (release.immutable !== true || typeof release.published_at !== "string"
        || !Number.isFinite(Date.parse(release.published_at))) {
        throw new Error(`stable release ${tag} must be published and immutable`);
      }
      stable.push({ id: release.id, tag });
    }
  }
}

function isGitObjectId(value) {
  return typeof value === "string" && /^[0-9a-f]{40}$/.test(value);
}

async function peelTag(read, tag, tagObject, target) {
  // Like git's ^{commit}, follow any nested annotated tag objects by hash.
  const seen = new Set([tagObject]);
  while (target?.type === "tag" && isGitObjectId(target.sha) && !seen.has(target.sha)) {
    const oid = target.sha;
    seen.add(oid);
    const nestedTag = await read(`git/tags/${oid}`);
    if (nestedTag?.sha !== oid) throw new Error(`stable release ${tag} has a mismatched nested tag object`);
    target = nestedTag.object;
  }
  if (target?.type !== "commit" || !isGitObjectId(target.sha)) {
    throw new Error(`stable release ${tag} has no exact commit target`);
  }
  return target.sha;
}

async function resolveRelease(read, release) {
  const { tag } = release;
  const ref = await read(`git/ref/tags/${encodeURIComponent(tag)}`);
  const tagObject = ref?.object?.sha;
  if (ref?.ref !== `refs/tags/${tag}` || ref?.object?.type !== "tag" || !isGitObjectId(tagObject)) {
    throw new Error(`stable release ${tag} must point to an annotated Git tag`);
  }
  const annotation = await read(`git/tags/${tagObject}`);
  if (annotation?.tag !== tag || annotation?.sha !== tagObject
    || annotation?.verification?.verified !== true || annotation?.verification?.reason !== "valid") {
    throw new Error(`stable release ${tag} must have a verified tag signature`);
  }
  const commit = await peelTag(read, tag, tagObject, annotation.object);
  return { id: release.id, tag, tag_object: tagObject, commit };
}

async function resolveReleaseHistory(read) {
  const stable = await stableReleases(read);
  if (stable.length === 0) {
    throw new Error(`no published stable releases found for ${repository}; no release boundary established`);
  }

  const releases = [];
  for (const release of stable) {
    releases.push(await resolveRelease(read, release));
  }
  return { repository, releases };
}

async function runSelfTest() {
  const release = (id, tag, fields = {}) => ({
    id, tag_name: tag, draft: false, prerelease: false, immutable: true,
    published_at: "2026-11-20T00:00:00Z", ...fields,
  });
  const oldTag = "v1.2.0";
  const newTag = "v1.3.0";
  const oldObject = "a".repeat(40);
  const newObject = "b".repeat(40);
  const commit = "c".repeat(40);
  const ref = (tag, sha, type = "tag") => ({ ref: `refs/tags/${tag}`, object: { type, sha } });
  const annotation = (tag, sha) => ({
    tag, sha, object: { type: "commit", sha: commit }, verification: { verified: true, reason: "valid" },
  });
  const results = {
    "releases?per_page=100&page=1": [
      release(4, "release draft", { draft: true }),
      release(3, "v1.4.0-rc.1", { prerelease: true }),
      release(2, newTag),
    ],
    "releases?per_page=100&page=2": [release(1, oldTag)],
    "releases?per_page=100&page=3": [],
    [`git/ref/tags/${newTag}`]: ref(newTag, newObject),
    [`git/ref/tags/${oldTag}`]: ref(oldTag, oldObject),
    [`git/tags/${newObject}`]: annotation(newTag, newObject),
    [`git/tags/${oldObject}`]: annotation(oldTag, oldObject),
  };
  const withResults = (changes = {}) => {
    const data = { ...results, ...changes };
    return async (endpoint) => {
      assert(Object.hasOwn(data, endpoint), `unexpected request to ${endpoint}`);
      if (data[endpoint] instanceof Error) throw data[endpoint];
      return data[endpoint];
    };
  };
  assert.deepEqual(await resolveReleaseHistory(withResults()), {
    repository,
    releases: [
      { id: 2, tag: newTag, tag_object: newObject, commit },
      { id: 1, tag: oldTag, tag_object: oldObject, commit },
    ],
  });
  await assert.rejects(resolveReleaseHistory(withResults({
    "releases?per_page=100&page=1": [release(3, "v1.4.0-rc.1", { prerelease: true })],
    "releases?per_page=100&page=2": [],
  })), /no release boundary established/);
  const nestedObject = "d".repeat(40);
  const nestedCommit = "e".repeat(40);
  const nestedTarget = { ...annotation(oldTag, oldObject), object: { type: "tag", sha: nestedObject } };
  const nested = await resolveReleaseHistory(withResults({
    [`git/tags/${oldObject}`]: nestedTarget,
    [`git/tags/${nestedObject}`]: { sha: nestedObject, object: { type: "commit", sha: nestedCommit } },
  }));
  assert.deepEqual(nested.releases[1], { id: 1, tag: oldTag, tag_object: oldObject, commit: nestedCommit });
  await assert.rejects(resolveReleaseHistory(withResults({
    [`git/tags/${oldObject}`]: nestedTarget,
    [`git/tags/${nestedObject}`]: { sha: nestedObject, object: { type: "tag", sha: oldObject } },
  })), /no exact commit target/);

  const cases = [
    ["releases?per_page=100&page=1", [], /no release boundary established/],
    ["releases?per_page=100&page=1", { message: "unexpected payload" }, /invalid GitHub releases page/],
    ["releases?per_page=100&page=2", new Error("GitHub request failed"), /GitHub request failed/],
    ["releases?per_page=100&page=2", [release(2, newTag)], /repeated release record/],
    ["releases?per_page=100&page=2", [release(1, newTag)], /inconsistent published release classification/],
    ["releases?per_page=100&page=2", [release(1, oldTag, { prerelease: true })], /inconsistent published release classification/],
    ["releases?per_page=100&page=2", [release(1, oldTag, { immutable: false })], /published and immutable/],
    ["releases?per_page=100&page=2", [release(1, oldTag, { published_at: null })], /published and immutable/],
    [`git/ref/tags/${oldTag}`, ref(oldTag, oldObject, "commit"), /annotated Git tag/],
    [`git/tags/${oldObject}`, annotation(newTag, oldObject), /verified tag signature/],
    [`git/tags/${oldObject}`, annotation(oldTag, newObject), /verified tag signature/],
    [`git/tags/${oldObject}`, { ...annotation(oldTag, oldObject), object: { type: "tree", sha: commit } }, /no exact commit target/],
    [`git/tags/${oldObject}`, {
      ...annotation(oldTag, oldObject), verification: { verified: false, reason: "unsigned" },
    }, /verified tag signature/],
  ];
  for (const [endpoint, payload, error] of cases) {
    await assert.rejects(resolveReleaseHistory(withResults({ [endpoint]: payload })), error);
  }
  console.log("✔ published release history resolver self-test passed");
}

try {
  const args = process.argv.slice(2);
  if (args.length === 1 && args[0] === "--self-test") {
    await runSelfTest();
  } else if (args.length === 0) {
    const history = await resolveReleaseHistory(readGitHub);
    console.log(JSON.stringify(history, null, 2));
  } else {
    throw new Error("usage: resolve-release-history.mjs [--self-test]");
  }
} catch (error) {
  console.error(`release history resolution failed: ${error.message}`);
  process.exitCode = 1;
}
