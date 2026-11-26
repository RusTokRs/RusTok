import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = fileURLToPath(new URL("../../", import.meta.url));
const forum = "crates/modules/rustok-forum";
const projection = "verify-forum-projection-invalidation.mjs";
const seo = "verify-forum-public-discovery-seo.mjs";

function fixture(t, script) {
  const directory = mkdtempSync(path.join(os.tmpdir(), "forum-owner-verifier-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const scriptPath = path.join(root, "scripts/verify", script);
  const text = readFileSync(scriptPath, "utf8");
  // Include every explicitly required input; an absent canonical file must fail,
  // rather than being replaced by an old include-layout compatibility fixture.
  const files = new Set([...text.matchAll(/["'](crates\/[^"']+)["']/g)].map((match) => match[1]));
  assert.ok(files.size > 10);
  for (const relative of files) {
    const destination = path.join(directory, relative);
    mkdirSync(path.dirname(destination), { recursive: true });
    writeFileSync(destination, readFileSync(path.join(root, relative), "utf8"));
  }
  return {
    mutate(relative, before, after) {
      const destination = path.join(directory, relative);
      const source = readFileSync(destination, "utf8");
      assert.ok(source.includes(before), `fixture must change ${relative}: ${before}`);
      writeFileSync(destination, source.replaceAll(before, after));
    },
    remove(relative) { rmSync(path.join(directory, relative)); },
    run() {
      const result = spawnSync(process.execPath, [scriptPath], {
        cwd: root,
        env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: directory },
        encoding: "utf8",
      });
      assert.ifError(result.error);
      return { status: result.status, output: result.stdout + result.stderr };
    },
  };
}

for (const script of [projection, seo]) {
  test(`${script} accepts the current owner composition`, (t) => {
    const result = fixture(t, script).run();
    assert.equal(result.status, 0, result.output);
  });
}

for (const [name, file, before, after] of [
  ["category publication", "src/services/category.rs", "publish_forum_projection_scope_direct_in_tx", "missing_publication"],
  ["category move publication", "src/services/category_command.rs", "publish_forum_projection_scope_direct_in_tx", "missing_publication"],
  ["category lifecycle publication", "src/services/category_lifecycle.rs", "publish_forum_projection_scope_direct_in_tx", "missing_publication"],
  ["topic audience publication", "src/services/topic_audience.rs", "publish_forum_topic_projection_direct_in_tx", "missing_publication"],
  ["reply edit publication", "src/services/reply.rs", "publish_forum_topic_projection_in_tx", "missing_publication"],
  ["reply edit transaction binding", "src/services/reply.rs", "&self.event_bus,\n            &txn,", "&self.event_bus,\n            &different_transaction,"],
  ["module declaration", "src/services/mod.rs", "mod category_command;", "// removed category command module"],
  ["root event identity", "contracts/forum-projection-invalidation.json", '"index.reindex_requested"', '"search.reindex_requested"'],
  ["outbox owner path", "contracts/forum-projection-invalidation.json", "crates/libs/rustok-outbox-storage/src/transactional.rs", "retired/outbox.rs"],
]) {
  test(`projection guard rejects loss of ${name}`, (t) => {
    const current = fixture(t, projection);
    current.mutate(`${forum}/${file}`, before, after);
    const result = current.run();
    assert.equal(result.status, 1, result.output);
    assert.match(result.output, /projection invalidation verification failed/);
  });
}

test("projection guard rejects a missing canonical contract note", (t) => {
  const current = fixture(t, projection);
  current.remove(`${forum}/docs/forum-20bk-projection-invalidation.md`);
  const result = current.run();
  assert.equal(result.status, 1, result.output);
  assert.match(result.output, /expected file is missing/);
});

for (const provider of ["ForumCategorySeoTargetProvider", "ForumTopicSeoTargetProvider"]) {
  test(`SEO guard requires actual ${provider} registration in the module owner`, (t) => {
    const current = fixture(t, seo);
    current.mutate(`${forum}/src/module.rs`, `seo_audience_targets::${provider}`, "retired_seo::Provider");
    const result = current.run();
    assert.equal(result.status, 1, result.output);
    assert.match(result.output, /missing exact (category|topic) SEO registration/);
  });
}

test("SEO guard rejects a registration function being replaced even when provider names remain", (t) => {
  const current = fixture(t, seo);
  current.mutate(`${forum}/src/module.rs`, "register_seo_target_provider(", "skipped_registration(");
  const result = current.run();
  assert.equal(result.status, 1, result.output);
  assert.match(result.output, /missing exact category SEO registration/);
});
