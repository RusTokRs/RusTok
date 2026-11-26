import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const sourcePath = "crates/modules/rustok-distribution/tests/forum_moderation_lost_response_postgres.rs";
function fixture(mutate) {
  const temporary = mkdtempSync(path.join(tmpdir(), "forum-lost-response-"));
  try {
    for (const file of [sourcePath,
      "crates/modules/rustok-distribution/Cargo.toml",
      "crates/modules/rustok-moderation/docs/forum-lost-response-postgres-contract.md",
    ]) {
      const target = path.join(temporary, file);
      mkdirSync(path.dirname(target), { recursive: true });
      const source = readFileSync(path.join(root, file), "utf8");
      writeFileSync(target, file === sourcePath ? mutate(source) : source);
    }
    return spawnSync(process.execPath, [path.join(root, "scripts/verify/verify-forum-moderation-lost-response-postgres.mjs")], {
      cwd: temporary, encoding: "utf8",
    });
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
}
const assertion = /assert_eq!\(\s*reply_revision\(&database\.db, &seed\)\.await\?,\s*revision_after_first\s*,?\s*\)/;
test("lost-response guard accepts the rustfmt-wrapped replay revision assertion", () => {
  const result = fixture((source) => source);
  assert.equal(result.status, 0, result.stderr);
});
test("lost-response guard accepts the same revision assertion on one line", () => {
  const result = fixture((source) => {
    assert.match(source, assertion);
    return source.replace(assertion, "assert_eq!(reply_revision(&database.db, &seed).await?, revision_after_first)");
  });
  assert.equal(result.status, 0, result.stderr);
});
test("lost-response guard still rejects removal of the replay revision assertion", () => {
  const result = fixture((source) => {
    assert.match(source, assertion);
    return source.replace(assertion, "assert!(true)");
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /contract is missing.*revision_after_first/);
});
