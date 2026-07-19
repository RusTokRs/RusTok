#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const API_VERSION = "2026-03-10";
const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const defaultContract = path.resolve(scriptDir, "../../docs/ci/repository-ruleset-contract.json");

function parseArguments(argv) {
  const options = { contract: defaultContract, selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") {
      options.selfTest = true;
      continue;
    }
    if (["--contract", "--rules-json", "--repository", "--branch"].includes(argument)) {
      const value = argv[index + 1];
      if (!value) throw new Error(`${argument} requires a value`);
      options[argument.slice(2).replace(/-([a-z])/g, (_, character) => character.toUpperCase())] = value;
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${argument}`);
  }
  return options;
}

function readJsonFile(file, label) {
  const stats = fs.lstatSync(file, { throwIfNoEntry: false });
  if (!stats) throw new Error(`${label} is missing: ${file}`);
  if (!stats.isFile() || stats.isSymbolicLink()) {
    throw new Error(`${label} must be a regular non-symlink file: ${file}`);
  }
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"));
  } catch (error) {
    throw new Error(`${label} is not valid JSON: ${error.message}`);
  }
}

function nonEmptyString(value, label) {
  if (typeof value !== "string" || value.trim() === "") {
    throw new Error(`${label} must be a non-empty string`);
  }
  return value.trim();
}

function validateContract(raw) {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
    throw new Error("ruleset contract must be an object");
  }
  if (raw.schema_version !== 1) throw new Error("ruleset contract schema_version must be 1");

  const repository = nonEmptyString(raw.repository, "repository");
  if (!/^[^/\s]+\/[^/\s]+$/.test(repository)) {
    throw new Error("repository must use owner/name form");
  }
  if (!Number.isSafeInteger(raw.repository_id) || raw.repository_id <= 0) {
    throw new Error("repository_id must be a positive integer");
  }
  const branch = nonEmptyString(raw.branch, "branch");
  if (raw.do_not_enforce_on_create !== false) {
    throw new Error("do_not_enforce_on_create must be false");
  }
  if (!Array.isArray(raw.required_status_checks) || raw.required_status_checks.length === 0) {
    throw new Error("required_status_checks must be a non-empty array");
  }

  const contexts = new Set();
  const requiredStatusChecks = raw.required_status_checks.map((check, index) => {
    if (!check || typeof check !== "object" || Array.isArray(check)) {
      throw new Error(`required_status_checks[${index}] must be an object`);
    }
    const context = nonEmptyString(check.context, `required_status_checks[${index}].context`);
    if (contexts.has(context)) throw new Error(`required status check context duplicates ${context}`);
    contexts.add(context);
    const integrationSlug = nonEmptyString(
      check.integration_slug,
      `required_status_checks[${index}].integration_slug`,
    );
    if (integrationSlug !== "github-actions") {
      throw new Error(`required_status_checks[${index}].integration_slug must be github-actions`);
    }
    if (!Number.isSafeInteger(check.integration_id) || check.integration_id <= 0) {
      throw new Error(`required_status_checks[${index}].integration_id must be a positive integer`);
    }
    if (check.strict !== true) {
      throw new Error(`required_status_checks[${index}].strict must be true`);
    }
    return {
      context,
      integration_slug: integrationSlug,
      integration_id: check.integration_id,
      strict: true,
    };
  });

  return {
    schema_version: 1,
    repository,
    repository_id: raw.repository_id,
    branch,
    do_not_enforce_on_create: false,
    required_status_checks: requiredStatusChecks,
  };
}

function verifyRules(contract, rawRules) {
  if (!Array.isArray(rawRules)) throw new Error("active branch rules response must be an array");
  const statusRules = rawRules.filter((rule) => rule?.type === "required_status_checks");
  if (statusRules.length === 0) {
    throw new Error(`branch ${contract.branch} has no active required_status_checks rule`);
  }

  const failures = [];
  for (const expected of contract.required_status_checks) {
    const matches = [];
    for (const rule of statusRules) {
      const parameters = rule?.parameters ?? {};
      const checks = Array.isArray(parameters.required_status_checks)
        ? parameters.required_status_checks
        : [];
      for (const actual of checks) {
        if (actual?.context === expected.context) matches.push({ rule, parameters, actual });
      }
    }

    if (matches.length === 0) {
      failures.push(`missing required status check ${expected.context}`);
      continue;
    }
    if (matches.length !== 1) {
      failures.push(`required status check ${expected.context} appears ${matches.length} times`);
      continue;
    }

    const [{ parameters, actual }] = matches;
    if (actual.integration_id !== expected.integration_id) {
      failures.push(
        `required status check ${expected.context} must originate from integration ${expected.integration_id}, got ${actual.integration_id ?? "any source"}`,
      );
    }
    if (parameters.strict_required_status_checks_policy !== true) {
      failures.push(`required status check ${expected.context} must use strict branch freshness`);
    }
    if (parameters.do_not_enforce_on_create !== contract.do_not_enforce_on_create) {
      failures.push(`required status check ${expected.context} must enforce checks on branch creation`);
    }
  }

  if (failures.length > 0) {
    throw new Error(`repository ruleset contract failed:\n${failures.map((failure) => `✗ ${failure}`).join("\n")}`);
  }
  return contract.required_status_checks.map((check) => check.context);
}

async function fetchActiveBranchRules(contract, token) {
  const [owner, repository] = contract.repository.split("/");
  const url = `https://api.github.com/repos/${encodeURIComponent(owner)}/${encodeURIComponent(repository)}/rules/branches/${encodeURIComponent(contract.branch)}`;
  const headers = {
    Accept: "application/vnd.github+json",
    "X-GitHub-Api-Version": API_VERSION,
    "User-Agent": "rustok-ruleset-contract-audit",
  };
  if (token) headers.Authorization = `Bearer ${token}`;

  const response = await fetch(url, { headers, redirect: "error" });
  if (!response.ok) {
    const body = (await response.text()).slice(0, 500).replaceAll("\n", " ");
    throw new Error(`GitHub rules API returned ${response.status}: ${body}`);
  }
  return response.json();
}

function runSelfTest() {
  const contract = validateContract({
    schema_version: 1,
    repository: "RusTokRs/RusTok",
    repository_id: 1144063896,
    branch: "main",
    do_not_enforce_on_create: false,
    required_status_checks: [
      {
        context: "Migration harness approval",
        integration_slug: "github-actions",
        integration_id: 15368,
        strict: true,
      },
      {
        context: "Repository ruleset contract",
        integration_slug: "github-actions",
        integration_id: 15368,
        strict: true,
      },
    ],
  });
  const validRules = [
    {
      type: "required_status_checks",
      parameters: {
        do_not_enforce_on_create: false,
        strict_required_status_checks_policy: true,
        required_status_checks: [
          { context: "Migration harness approval", integration_id: 15368 },
          { context: "Repository ruleset contract", integration_id: 15368 },
        ],
      },
    },
  ];
  assert.deepEqual(verifyRules(contract, validRules), [
    "Migration harness approval",
    "Repository ruleset contract",
  ]);

  const missing = structuredClone(validRules);
  missing[0].parameters.required_status_checks.pop();
  assert.throws(() => verifyRules(contract, missing), /missing required status check/);

  const wrongSource = structuredClone(validRules);
  wrongSource[0].parameters.required_status_checks[0].integration_id = null;
  assert.throws(() => verifyRules(contract, wrongSource), /must originate from integration 15368/);

  const loose = structuredClone(validRules);
  loose[0].parameters.strict_required_status_checks_policy = false;
  assert.throws(() => verifyRules(contract, loose), /strict branch freshness/);

  const duplicate = structuredClone(validRules);
  duplicate.push(structuredClone(validRules[0]));
  assert.throws(() => verifyRules(contract, duplicate), /appears 2 times/);

  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "rustok-ruleset-contract-"));
  try {
    const contractFile = path.join(fixtureRoot, "contract.json");
    fs.writeFileSync(contractFile, JSON.stringify(contract));
    assert.equal(readJsonFile(contractFile, "contract").schema_version, 1);
    fs.symlinkSync("contract.json", path.join(fixtureRoot, "contract-link.json"));
    assert.throws(
      () => readJsonFile(path.join(fixtureRoot, "contract-link.json"), "contract"),
      /regular non-symlink file/,
    );
  } finally {
    fs.rmSync(fixtureRoot, { recursive: true, force: true });
  }
  console.log("✔ repository ruleset contract self-test passed");
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.selfTest) {
    runSelfTest();
    return;
  }

  const contract = validateContract(readJsonFile(path.resolve(options.contract), "ruleset contract"));
  if (options.repository && options.repository !== contract.repository) {
    throw new Error(`runtime repository ${options.repository} does not match contract ${contract.repository}`);
  }
  if (options.branch && options.branch !== contract.branch) {
    throw new Error(`runtime branch ${options.branch} does not match contract ${contract.branch}`);
  }

  const rules = options.rulesJson
    ? readJsonFile(path.resolve(options.rulesJson), "active branch rules fixture")
    : await fetchActiveBranchRules(
        contract,
        process.env.RULESET_AUDIT_TOKEN || process.env.GITHUB_TOKEN || process.env.GH_TOKEN,
      );
  const verified = verifyRules(contract, rules);
  console.log(
    `✔ ${contract.repository}:${contract.branch} enforces ${verified.join(", ")} through GitHub Actions with strict required status checks`,
  );
}

try {
  await main();
} catch (error) {
  console.error(`repository ruleset contract verification failed: ${error.message}`);
  process.exit(1);
}
