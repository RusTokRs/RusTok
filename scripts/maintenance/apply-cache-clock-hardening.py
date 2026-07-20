#!/usr/bin/env python3
from pathlib import Path


def replace_once(path: Path, old: str, new: str) -> None:
    source = path.read_text()
    if source.count(old) != 1:
        raise SystemExit(f"{path}: expected exactly one replacement marker")
    path.write_text(source.replace(old, new, 1))


clock = Path("crates/rustok-cache/src/clock.rs")
if clock.exists():
    raise SystemExit("cache clock module already exists")
clock.write_text(
    """use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) fn unix_time_millis() -> rustok_core::Result<u64> {
    unix_time_millis_at(SystemTime::now())
}

fn unix_time_millis_at(now: SystemTime) -> rustok_core::Result<u64> {
    let duration = now.duration_since(UNIX_EPOCH).map_err(|error| {
        rustok_core::Error::Cache(format!(
            "system clock is before the Unix epoch by {} ms",
            duration_millis_saturated(error.duration())
        ))
    })?;
    Ok(duration_millis_saturated(duration))
}

fn duration_millis_saturated(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_time_millis_preserves_post_epoch_milliseconds() {
        assert_eq!(
            unix_time_millis_at(UNIX_EPOCH + Duration::from_millis(1_234)).unwrap(),
            1_234
        );
    }

    #[test]
    fn unix_time_millis_rejects_pre_epoch_clock() {
        let error = unix_time_millis_at(UNIX_EPOCH - Duration::from_millis(1)).unwrap_err();
        match error {
            rustok_core::Error::Cache(message) => {
                assert!(message.contains("before the Unix epoch"));
                assert!(message.contains("1 ms"));
            }
            other => panic!("unexpected clock error: {other}"),
        }
    }
}
"""
)

lib = Path("crates/rustok-cache/src/lib.rs")
replace_once(lib, "mod cas_observability;\n", "mod cas_observability;\nmod clock;\n")

negative = Path("crates/rustok-cache/src/negative.rs")
replace_once(
    negative,
    "use std::time::{Duration, SystemTime, UNIX_EPOCH};",
    "use std::time::Duration;",
)
replace_once(
    negative,
    "use crate::{\n    CacheEnvelope,",
    "use crate::{\n    clock::unix_time_millis, CacheEnvelope,",
)
replace_once(
    negative,
    "        self.get_negative_at(backend, key, policy, current_unix_ms())\n            .await",
    "        let now_unix_ms = unix_time_millis()?;\n        self.get_negative_at(backend, key, policy, now_unix_ms)\n            .await",
)
replace_once(
    negative,
    "fn current_unix_ms() -> u64 {\n    SystemTime::now()\n        .duration_since(UNIX_EPOCH)\n        .unwrap_or_default()\n        .as_millis()\n        .min(u128::from(u64::MAX)) as u64\n}\n\n",
    "",
)

typed = Path("crates/rustok-cache/src/typed.rs")
replace_once(typed, "use std::time::{SystemTime, UNIX_EPOCH};\n\n", "")
replace_once(
    typed,
    "use crate::{\n    CacheEnvelope,",
    "use crate::{\n    clock::unix_time_millis, CacheEnvelope,",
)
replace_once(
    typed,
    "                now_unix_ms: current_unix_ms(),",
    "                now_unix_ms: unix_time_millis()?,",
)
replace_once(
    typed,
    "fn current_unix_ms() -> u64 {\n    SystemTime::now()\n        .duration_since(UNIX_EPOCH)\n        .unwrap_or_default()\n        .as_millis()\n        .min(u128::from(u64::MAX)) as u64\n}\n\n",
    "",
)

refresh = Path("crates/rustok-cache/src/refresh.rs")
replace_once(refresh, "use std::time::{SystemTime, UNIX_EPOCH};\n\n", "")
replace_once(
    refresh,
    "use crate::{\n    CacheEnvelope,",
    "use crate::{\n    clock::unix_time_millis, CacheEnvelope,",
)
replace_once(
    refresh,
    "                let envelope = loader().await?;\n                if envelope.is_hard_expired(current_unix_ms()) {",
    "                let envelope = loader().await?;\n                let completed_at_unix_ms = unix_time_millis()?;\n                if envelope.is_hard_expired(completed_at_unix_ms) {",
)
replace_once(
    refresh,
    "\n        self.load_enveloped_stale_while_revalidate_with_limit_at(\n",
    "\n        let now_unix_ms = unix_time_millis()?;\n        self.load_enveloped_stale_while_revalidate_with_limit_at(\n",
)
replace_once(
    refresh,
    "                now_unix_ms: current_unix_ms(),",
    "                now_unix_ms,",
)
replace_once(
    refresh,
    "fn current_unix_ms() -> u64 {\n    SystemTime::now()\n        .duration_since(UNIX_EPOCH)\n        .unwrap_or_default()\n        .as_millis()\n        .min(u128::from(u64::MAX)) as u64\n}\n\n",
    "",
)

verifier = Path("scripts/verify/verify-cache-clock-contract.mjs")
if verifier.exists():
    raise SystemExit("cache clock verifier already exists")
verifier.write_text(
    r'''#!/usr/bin/env node
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
  forbidMatch(name, /fn current_unix_ms\(/, `${name} must not own a duplicate system clock helper`);
  forbidMatch(name, /duration_since\(UNIX_EPOCH\)/, `${name} must not read the system clock directly`);
  forbidMatch(name, /unwrap_or_default\(\)/, `${name} must not turn a clock failure into timestamp zero`);
}
requireMatch("negative", /let now_unix_ms = unix_time_millis\(\)\?;/, "negative cache reads must propagate clock failure");
requireMatch("typed", /now_unix_ms: unix_time_millis\(\)\?,/, "typed cache reads must propagate clock failure");
requireMatch("refresh", /let completed_at_unix_ms = unix_time_millis\(\)\?;/, "refresh completion must recheck the fallible clock");
requireMatch("refresh", /let now_unix_ms = unix_time_millis\(\)\?;/, "SWR request freshness must use the fallible clock");
requireMatch("hardening", /Verify cache clock fail-closed contract[\s\S]*verify-cache-clock-contract\.mjs/, "Hardening Gates must run the cache clock verifier");
requireMatch("master", /verify-cache-clock-contract\.mjs:Cache Clock Fail-closed Contract/, "master verification must include the cache clock verifier");
requireMatch("readme", /verify-cache-clock-contract\.mjs/, "verification README must document the cache clock verifier");

if (failures.length) {
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(1);
}
console.log("✔ cache freshness paths fail closed when the system clock violates the Unix epoch invariant");
'''
)

hardening = Path(".github/workflows/hardening-gates.yml")
replace_once(
    hardening,
    "      - name: Verify dependency feature hygiene\n        run: node scripts/verify/verify-dependency-feature-hygiene.mjs\n",
    "      - name: Verify dependency feature hygiene\n        run: node scripts/verify/verify-dependency-feature-hygiene.mjs\n      - name: Verify cache clock fail-closed contract\n        run: node scripts/verify/verify-cache-clock-contract.mjs\n",
)

master = Path("scripts/verify/verify-all.sh")
replace_once(
    master,
    '    echo "  dependency-feature-hygiene  Verify unused vulnerable dependency defaults stay disabled"\n',
    '    echo "  dependency-feature-hygiene  Verify unused vulnerable dependency defaults stay disabled"\n    echo "  cache-clock-contract  Verify cache freshness fails closed on invalid system time"\n',
)
replace_once(
    master,
    '    "verify-dependency-feature-hygiene.mjs:Dependency Feature Hygiene"\n',
    '    "verify-dependency-feature-hygiene.mjs:Dependency Feature Hygiene"\n    "verify-cache-clock-contract.mjs:Cache Clock Fail-closed Contract"\n',
)

readme = Path("scripts/verify/README.md")
replace_once(
    readme,
    "node scripts/verify/verify-module-build-worker-isolation.mjs\n",
    "node scripts/verify/verify-module-build-worker-isolation.mjs\nnode scripts/verify/verify-cache-clock-contract.mjs\n",
)
replace_once(
    readme,
    "| Runtime-context/cache-key invariants check | `node scripts/verify/verify-runtime-context-invariants.mjs` |\n",
    "| Runtime-context/cache-key invariants check | `node scripts/verify/verify-runtime-context-invariants.mjs` |\n| Cache freshness/system-clock fail-closed contract | `node scripts/verify/verify-cache-clock-contract.mjs` |\n",
)
replace_once(
    readme,
    "### `verify-runtime-context-invariants.mjs`\n",
    """### `verify-cache-clock-contract.mjs`
**Cache clock guardrail** — verifies that cache freshness decisions fail closed when system time is before the Unix epoch.

What it checks:
- `rustok-cache` owns one fallible Unix-millisecond clock helper;
- negative, typed and stale-while-revalidate paths propagate clock failures instead of using timestamp zero;
- request-time and refresh-completion freshness checks use the canonical helper;
- pre-epoch and post-epoch behavior has Rust regression coverage.

Example:

```bash
node scripts/verify/verify-cache-clock-contract.mjs
./scripts/verify/verify-all.sh cache-clock-contract
```

---

### `verify-runtime-context-invariants.mjs`
""",
)
