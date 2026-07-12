#!/usr/bin/env node

import assert from "node:assert/strict";
import test from "node:test";
import { spawnSync } from "node:child_process";

test("forum runtime baseline static inventory is self-consistent", () => {
  const result = spawnSync(
    process.execPath,
    ["scripts/verify/verify-forum-runtime-baseline.mjs", "--static-only"],
    {
      cwd: process.cwd(),
      encoding: "utf8",
    },
  );

  assert.equal(
    result.status,
    0,
    `static verifier failed\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
  );
  assert.match(result.stdout, /7 tracked known defects/);
});
