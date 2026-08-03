#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (path) => readFileSync(path, "utf8");
const contract = JSON.parse(
  read("crates/rustok-forum/contracts/forum-topic-canonical-resolution.json"),
);

assert.equal(contract.latest_transport_slice, "FORUM-21J");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");

console.log("FORUM-21J redirect source is ready.");
