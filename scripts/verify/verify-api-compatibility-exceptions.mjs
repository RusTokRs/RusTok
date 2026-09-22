#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

function parseArguments(argv) {
  let file;
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--file") {
      file = argv[index + 1];
      if (!file) throw new Error("--file requires a path");
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${argument}`);
  }
  if (!file) throw new Error("usage: verify-api-compatibility-exceptions.mjs --file FILE");
  return path.resolve(file);
}

function requireNonEmpty(value, label, failures) {
  if (typeof value !== "string" || value.trim() === "") {
    failures.push(`${label} must be a non-empty string`);
  }
}

function parseDate(value, label, failures) {
  const timestamp = Date.parse(`${value}T23:59:59Z`);
  if (!Number.isFinite(timestamp)) failures.push(`${label} must be an ISO date`);
  return timestamp;
}

function verificationTimestamp() {
  if (!process.env.VERIFICATION_DATE) return Date.now();
  const timestamp = Date.parse(`${process.env.VERIFICATION_DATE}T00:00:00Z`);
  if (!Number.isFinite(timestamp)) throw new Error("VERIFICATION_DATE must be an ISO date");
  return timestamp;
}

function main() {
  const file = parseArguments(process.argv.slice(2));
  const register = JSON.parse(fs.readFileSync(file, "utf8"));
  const failures = [];

  if (register.schema_version !== 1) failures.push(`${file}: schema_version must be 1`);
  requireNonEmpty(register.policy?.owner, `${file}: policy.owner`, failures);
  requireNonEmpty(register.policy?.review_by, `${file}: policy.review_by`, failures);
  requireNonEmpty(register.policy?.exit_criteria, `${file}: policy.exit_criteria`, failures);

  const reviewBy = parseDate(register.policy?.review_by, `${file}: policy.review_by`, failures);
  const now = verificationTimestamp();
  if (Number.isFinite(reviewBy) && now > reviewBy) {
    failures.push(`${file}: policy review expired on ${register.policy.review_by}`);
  }

  if (!Array.isArray(register.exceptions)) {
    failures.push(`${file}: exceptions must be an array`);
  } else {
    const ids = new Set();
    for (const [index, entry] of register.exceptions.entries()) {
      const label = `${file}: exceptions[${index}]`;
      for (const field of ["id", "owner", "reason", "expires_on"]) {
        requireNonEmpty(entry[field], `${label}.${field}`, failures);
      }
      if (typeof entry.id === "string" && !/^(?:openapi|graphql):/.test(entry.id)) {
        failures.push(`${label}.id must use an openapi: or graphql: change id`);
      }
      if (ids.has(entry.id)) failures.push(`${label}.id duplicates ${entry.id}`);
      ids.add(entry.id);

      const expiresOn = parseDate(entry.expires_on, `${label}.expires_on`, failures);
      if (Number.isFinite(expiresOn) && now > expiresOn) {
        failures.push(`${label} expired on ${entry.expires_on}`);
      }
      if (Number.isFinite(expiresOn) && Number.isFinite(reviewBy) && expiresOn > reviewBy) {
        failures.push(`${label}.expires_on must not exceed policy.review_by`);
      }
    }
  }

  if (failures.length > 0) {
    console.error("API compatibility exception verification failed:");
    failures.forEach((failure) => console.error(`✗ ${failure}`));
    process.exit(Math.min(failures.length, 255));
  }

  console.log(
    `✔ API compatibility exception register is valid (${register.exceptions.length} active exception(s), review due ${register.policy.review_by})`,
  );
}

try {
  main();
} catch (error) {
  console.error(`API compatibility exception verification failed: ${error.message}`);
  process.exit(1);
}
