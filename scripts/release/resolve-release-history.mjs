#!/usr/bin/env node

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { parseReleaseTag } from "./verify-release-contract.mjs";

const repository = "RusTokRs/RusTok";
const pageSize = 100;
const bootstrapPath = "scripts/release/migration-bootstrap.json";

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

async function* pages(read, endpoint) {
  // Read through an empty page so a short page cannot silently truncate history.
  for (let page = 1; ; page += 1) {
    const records = await read(`${endpoint}?per_page=${pageSize}&page=${page}`);
    if (!Array.isArray(records) || records.length > pageSize) {
      throw new Error(`invalid GitHub ${endpoint} page ${page}`);
    }
    if (records.length === 0) return;
    yield { records, page };
  }
}

async function stableReleases(read) {
  const ids = new Set();
  const publishedTags = new Set();
  const stable = [];
  for await (const { records, page } of pages(read, "releases")) {
    for (const release of records) {
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
  return stable;
}

function isGitObjectId(value) {
  return typeof value === "string" && /^[0-9a-f]{40}$/.test(value);
}

function bootstrapRecord(bytes) {
  const expected = {
    schema_version: 1,
    repository,
    scope: "stable_distribution",
    state: "before_first_stable_release",
    seal_generation: 0,
  };
  const record = JSON.parse(bytes.toString("utf8"));
  if (record === null || typeof record !== "object" || Array.isArray(record)
    || Object.keys(record).length !== Object.keys(expected).length
    || Object.entries(expected).some(([name, value]) => record[name] !== value)) {
    throw new Error("invalid first stable distribution bootstrap record");
  }
  return record;
}

function contentBlobId(bytes) {
  return createHash("sha1").update(`blob ${bytes.length}\0`).update(bytes).digest("hex");
}

async function mainCommit(read) {
  const ref = await read("git/ref/heads/main");
  if (ref?.ref !== "refs/heads/main" || ref.object?.type !== "commit" || !isGitObjectId(ref.object.sha)) {
    throw new Error("invalid official main branch identity");
  }
  return ref.object.sha;
}

async function checkBootstrap(read = readGitHub) {
  const commit = await mainCommit(read);
  const file = await read(`contents/${bootstrapPath}?ref=${commit}`);
  if (file?.type !== "file" || file.path !== bootstrapPath || file.encoding !== "base64"
    || typeof file.content !== "string" || !isGitObjectId(file.sha)
    || !Number.isSafeInteger(file.size) || file.size < 1 || file.size > 4096) {
    throw new Error("bootstrap record is missing or invalid in official main");
  }
  const encoded = file.content.replace(/\r?\n/g, "");
  const bytes = Buffer.from(encoded, "base64");
  if (bytes.length !== file.size || bytes.toString("base64") !== encoded || contentBlobId(bytes) !== file.sha) {
    throw new Error("bootstrap record bytes do not match its Git blob identity");
  }
  const record = bootstrapRecord(bytes);
  if ((await stableReleases(read)).length > 0) {
    throw new Error("stable distribution has already been published");
  }

  const seen = new Set();
  for await (const { records, page } of pages(read, "tags")) {
    for (const entry of records) {
      const tag = entry?.name;
      if (typeof tag !== "string" || !tag || seen.has(tag)) {
        throw new Error(`invalid or repeated GitHub tag on page ${page}`);
      }
      seen.add(tag);
      // Match the release workflow's tag trigger. A stable tag can have started
      // container publication before GitHub Releases exposes a record.
      if (!/^v[^/]*\.[^/]*\.[^/]*$/.test(tag)) continue;
      if (!parseReleaseTag(tag).prerelease) throw new Error(`stable distribution tag ${tag} already exists`);
    }
  }
  if (await mainCommit(read) !== commit) {
    throw new Error("official main changed during the bootstrap check");
  }
  return {
    ...record,
    main_commit: commit,
    record_blob: file.sha,
    observation: "github_release_history_and_tag_refs",
    replacement_admission: "blocked",
  };
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

export async function resolveReleaseHistory(read = readGitHub) {
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

  const recordBytes = fs.readFileSync(new URL("./migration-bootstrap.json", import.meta.url));
  const recordEndpoint = `contents/${bootstrapPath}?ref=${commit}`;
  const mainEndpoint = "git/ref/heads/main";
  const tagsFirstPage = "tags?per_page=100&page=1";
  const tagsSecondPage = "tags?per_page=100&page=2";
  const recordFile = {
    type: "file", path: bootstrapPath, encoding: "base64",
    content: recordBytes.toString("base64"), size: recordBytes.length, sha: contentBlobId(recordBytes),
  };
  const prerelease = "v1.4.0-rc.1";
  const bootstrapResults = {
    [mainEndpoint]: { ref: "refs/heads/main", object: { type: "commit", sha: commit } },
    [recordEndpoint]: recordFile,
    [tagsFirstPage]: [{ name: prerelease }, { name: "v" }],
    [tagsSecondPage]: [],
    "releases?per_page=100&page=1": [release(3, prerelease, { prerelease: true })],
    "releases?per_page=100&page=2": [],
  };
  assert.deepEqual(await checkBootstrap(withResults(bootstrapResults)), {
    ...bootstrapRecord(recordBytes), main_commit: commit, record_blob: recordFile.sha,
    observation: "github_release_history_and_tag_refs", replacement_admission: "blocked",
  });
  for (const [endpoint, payload, error] of [
    [recordEndpoint, new Error("GitHub contents returned HTTP 404"), /HTTP 404/],
    [recordEndpoint, { ...recordFile, sha: oldObject }, /bytes do not match/],
    [recordEndpoint, { ...recordFile, type: "symlink" }, /record is missing or invalid/],
    ["releases?per_page=100&page=2", [release(1, oldTag)], /already been published/],
    [tagsSecondPage, [{ name: newTag }], /stable distribution tag v1\.3\.0 already exists/],
    [tagsFirstPage, { refs: [] }, /invalid GitHub tags page/],
    [tagsSecondPage, [{ name: prerelease }], /repeated GitHub tag/],
    [tagsSecondPage, new Error("GitHub tag request returned HTTP 403"), /HTTP 403/],
  ]) {
    await assert.rejects(checkBootstrap(withResults({ ...bootstrapResults, [endpoint]: payload })), error);
  }
  const current = withResults(bootstrapResults);
  let mainReads = 0;
  await assert.rejects(checkBootstrap(async (endpoint) => {
    if (endpoint === mainEndpoint && ++mainReads === 2) {
      return { ref: "refs/heads/main", object: { type: "commit", sha: nestedCommit } };
    }
    return current(endpoint);
  }), /official main changed/);
  for (const field of ["schema_version", "repository", "scope", "state", "seal_generation"]) {
    assert.throws(() => bootstrapRecord(Buffer.from(JSON.stringify({
      ...bootstrapRecord(recordBytes), [field]: "unexpected",
    }))), /invalid first stable distribution/);
  }
  console.log("✔ published release history resolver self-test passed");
}

async function main() {
  const args = process.argv.slice(2);
  if (args.length === 1 && args[0] === "--self-test") {
    await runSelfTest();
  } else if (args.length === 1 && args[0] === "--check-bootstrap") {
    console.log(JSON.stringify(await checkBootstrap(), null, 2));
  } else if (args.length === 0) {
    const history = await resolveReleaseHistory();
    console.log(JSON.stringify(history, null, 2));
  } else {
    throw new Error("usage: resolve-release-history.mjs [--self-test | --check-bootstrap]");
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    await main();
  } catch (error) {
    console.error(`release history resolution failed: ${error.message}`);
    process.exitCode = 1;
  }
}
