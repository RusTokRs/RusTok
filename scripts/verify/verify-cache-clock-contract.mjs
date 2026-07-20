#!/usr/bin/env node
import fs from "node:fs";

const files = {
  lib: "crates/rustok-cache/src/lib.rs",
  clock: "crates/rustok-cache/src/clock.rs",
  negative: "crates/rustok-cache/src/negative.rs",
  typed: "crates/rustok-cache/src/typed.rs",
  refresh: "crates/rustok-cache/src/refresh.rs",
  hardening: ".github/workflows/hardening-gates.yml",
  master: "scripts/verify/verify-all.sh",
  readme: "scripts/verify/README.md",
};
const source = Object.fromEntries(
  Object.entries(files).map(([name, path]) => [name, fs.readFileSync(path, "utf8")]),
);
const failures = [];
const requireMatch = (name, pattern, message) => {
  if (!pattern.test(source[name])) failures.push(message);
};
const forbidMatch = (name, pattern, message) => {
  if (pattern.test(source[name])) failures.push(message);
};

requireMatch("lib", /mod clock;/, "rustok-cache must own one private clock module");
requireMatch("clock", /pub\(crate\) fn unix_time_millis\(\) -> rustok_core::Result<u64>/, "cache clock must be fallible");
requireMatch("clock", /duration_since\(UNIX_EPOCH\)\.map_err/, "pre-epoch system time must map to a cache error");
requireMatch("clock", /system clock is before the Unix epoch/, "clock errors must explain the failed epoch invariant");
requireMatch("clock", /unix_time_millis_rejects_pre_epoch_clock/, "pre-epoch behavior needs a regression test");
requireMatch("clock", /unix_time_millis_preserves_post_epoch_milliseconds/, "post-epoch conversion needs a regression test");
for (const name of ["negative", "typed", "refresh"]) {
  requireMatch(name, /clock::unix_time_millis/, `${name} cache path must use the canonical fallible clock`);
  forbidMatch(name, /current_unix_ms\(/, `${name} must not reference the removed infallible clock helper`);
  forbidMatch(name, /duration_since\(UNIX_EPOCH\)/, `${name} must not read the system clock directly`);
  forbidMatch(name, /unwrap_or_default\(\)/, `${name} must not turn a clock failure into timestamp zero`);
}
requireMatch("negative", /let now_unix_ms = unix_time_millis\(\)\?;/, "negative cache reads must propagate clock failure");
requireMatch("typed", /now_unix_ms: unix_time_millis\(\)\?,/, "typed cache reads must propagate clock failure");
requireMatch("refresh", /let completed_at_unix_ms = unix_time_millis\(\)\?;/, "refresh completion must recheck the fallible clock");
requireMatch("refresh", /let now_unix_ms = unix_time_millis\(\)\?;/, "SWR request freshness must use the fallible clock");
const refreshTestClockCalls = source.refresh.match(/unix_time_millis\(\)\.unwrap\(\)/g) ?? [];
if (refreshTestClockCalls.length !== 5) {
  failures.push(`refresh tests must use the canonical clock at exactly five existing call sites; found ${refreshTestClockCalls.length}`);
}
requireMatch("hardening", /Verify cache clock fail-closed contract[\s\S]*verify-cache-clock-contract\.mjs/, "Hardening Gates must run the cache clock verifier");
requireMatch("master", /verify-cache-clock-contract\.mjs:Cache Clock Fail-closed Contract/, "master verification must include the cache clock verifier");
requireMatch("readme", /verify-cache-clock-contract\.mjs/, "verification README must document the cache clock verifier");

if (failures.length) {
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(1);
}
console.log("✔ cache freshness paths fail closed when the system clock violates the Unix epoch invariant");
