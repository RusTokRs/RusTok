#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

function readJson(relativePath) {
  const file = path.join(repoRoot, relativePath);
  const stats = fs.lstatSync(file, { throwIfNoEntry: false });
  if (!stats) throw new Error(`${relativePath}: required file is missing`);
  if (!stats.isFile() || stats.isSymbolicLink()) {
    throw new Error(`${relativePath}: must be a regular non-symlink file`);
  }
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"));
  } catch (error) {
    throw new Error(`${relativePath}: invalid JSON: ${error.message}`);
  }
}

function exactlyOne(items, predicate, label) {
  const matches = items.filter(predicate);
  if (matches.length !== 1) throw new Error(`${label}: expected exactly one match, found ${matches.length}`);
  return matches[0];
}

function verify() {
  const contract = readJson("docs/ci/repository-ruleset-contract.json");
  const payload = readJson("docs/ci/repository-ruleset-admin-payload.json");

  if (payload.target !== "branch") throw new Error("payload target must be branch");
  if (payload.enforcement !== "active") throw new Error("payload enforcement must be active");
  if (!Array.isArray(payload.bypass_actors) || payload.bypass_actors.length !== 0) {
    throw new Error("payload must not define permanent bypass actors");
  }

  const include = payload.conditions?.ref_name?.include;
  const exclude = payload.conditions?.ref_name?.exclude;
  if (!Array.isArray(include) || include.length !== 1 || include[0] !== `refs/heads/${contract.branch}`) {
    throw new Error(`payload must target only refs/heads/${contract.branch}`);
  }
  if (!Array.isArray(exclude) || exclude.length !== 0) {
    throw new Error("payload must not exclude refs from main protection");
  }
  if (!Array.isArray(payload.rules)) throw new Error("payload rules must be an array");

  exactlyOne(payload.rules, (rule) => rule?.type === "deletion", "deletion rule");
  exactlyOne(payload.rules, (rule) => rule?.type === "non_fast_forward", "non-fast-forward rule");

  const pullRequest = exactlyOne(
    payload.rules,
    (rule) => rule?.type === "pull_request",
    "pull request rule",
  );
  const pr = pullRequest.parameters ?? {};
  if (pr.required_approving_review_count !== 1) {
    throw new Error("pull request rule must require exactly one approving review");
  }
  if (pr.dismiss_stale_reviews_on_push !== true) {
    throw new Error("pull request rule must dismiss stale reviews on push");
  }
  if (pr.require_last_push_approval !== true) {
    throw new Error("pull request rule must require approval after the last push");
  }
  if (pr.required_review_thread_resolution !== true) {
    throw new Error("pull request rule must require conversation resolution");
  }
  if (pr.require_code_owner_review !== false) {
    throw new Error("CODEOWNERS review must remain disabled until CODEOWNERS is formally governed");
  }

  const statusRule = exactlyOne(
    payload.rules,
    (rule) => rule?.type === "required_status_checks",
    "required status checks rule",
  );
  const parameters = statusRule.parameters ?? {};
  if (parameters.strict_required_status_checks_policy !== true) {
    throw new Error("required status checks must use strict branch freshness");
  }
  if (parameters.do_not_enforce_on_create !== contract.do_not_enforce_on_create) {
    throw new Error("branch-creation enforcement differs from the ruleset contract");
  }
  if (!Array.isArray(parameters.required_status_checks)) {
    throw new Error("required_status_checks must be an array");
  }
  if (parameters.required_status_checks.length !== contract.required_status_checks.length) {
    throw new Error("admin payload required checks differ from the ruleset contract");
  }

  for (const expected of contract.required_status_checks) {
    const actual = exactlyOne(
      parameters.required_status_checks,
      (check) => check?.context === expected.context,
      `required status check ${expected.context}`,
    );
    if (actual.integration_id !== expected.integration_id) {
      throw new Error(
        `required status check ${expected.context} integration must be ${expected.integration_id}`,
      );
    }
  }

  console.log(
    "✔ active main-only PR ruleset payload blocks deletion/force-push, requires review resolution and matches the strict migration approval contract",
  );
}

try {
  verify();
} catch (error) {
  console.error(`repository ruleset admin payload verification failed: ${error.message}`);
  process.exit(1);
}
