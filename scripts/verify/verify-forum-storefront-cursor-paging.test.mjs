import assert from "node:assert/strict";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const storefront = "crates/modules/rustok-forum/storefront";
const ui = `${storefront}/src/ui/leptos.rs`;
const forum = "crates/modules/rustok-forum";
const files = [
  ...["src/model.rs", "src/transport/mod.rs", "src/transport/native_server_adapter_pages.rs", "src/transport/native_server_adapter.rs", "src/transport/graphql_adapter.rs", "src/core.rs", "src/ui/leptos.rs", "src/ui/topic_feed.rs", "src/ui/timeline.rs", "src/i18n.rs", "locales/en.ftl", "locales/ru.ftl", "locales/ar.ftl"].map(file => `${storefront}/${file}`),
  ...["src/dto/topic.rs", "src/services/topic.rs", "src/graphql/query_runtime.rs", "src/graphql/storefront_audience_topics.rs", "src/graphql/storefront_read_state.rs", "src/controllers/topics.rs"].map(file => `${forum}/${file}`),
  "apps/next-frontend/packages/rustok-forum/src/components/topic-feed.tsx",
];

function runMutation(before, after, target = ui) {
  const fixture = mkdtempSync(path.join(tmpdir(), "forum-initial-page-"));
  try {
    for (const file of files) {
      mkdirSync(path.dirname(path.join(fixture, file)), { recursive: true });
      copyFileSync(path.join(root, file), path.join(fixture, file));
    }
    if (before) {
      const text = readFileSync(path.join(fixture, target), "utf8");
      assert.ok(text.includes(before), `fixture must mutate ${before}`);
      writeFileSync(path.join(fixture, target), text.replace(before, after));
    }
    return spawnSync(process.execPath, [path.join(root, "scripts/verify/verify-forum-storefront-cursor-paging.mjs")], {
      cwd: fixture, env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: fixture }, encoding: "utf8",
    });
  } finally { rmSync(fixture, { recursive: true, force: true }); }
}
test("paging contract accepts current initial and continuation request fencing", () => {
  const result = runMutation();
  assert.equal(result.status, 0, result.stderr || result.stdout);
});
for (const [before, after, diagnostic] of [
  [".map(|data| (requested_key, data))", ".map(|data| (resource_key.get_untracked(), data))", /initial fetch returns request identity/],
  ["let topic_page_key = page_key;", "let topic_page_key = resource_key.get_untracked();", /paging uses the fetched identity/],
  ["if forum_page_is_current(resource_key, &page_key)", "if true", /read-state availability ignores stale initial results/],
]) {
  test(`paging guard rejects ${after}`, () => {
    const result = runMutation(before, after);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, diagnostic);
  });
}

for (const [before, after, diagnostic] of [
  ["set_search_draft.set(applied_search.get());", "set_search_draft.set(selection.get().search.unwrap_or_default());", /mode-only changes preserve the unsubmitted search draft/],
  ["<span>{solved_label.clone()}</span>", '<span>"Solved"</span>', /solved badge renders localized copy/],
]) {
  test(`feed guard rejects ${after}`, () => {
    const result = runMutation(before, after, `${storefront}/src/ui/topic_feed.rs`);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, diagnostic);
  });
}


for (const binding of ["", "selection\n                                selection=selection"]) {
  test(`paging guard rejects missing/duplicate selection prop: ${binding || 'missing'}`, () => {
    const result = runMutation(
      "<ForumTopicFeed\n                                topics=topics_page\n                                selection\n",
      `<ForumTopicFeed\n                                topics=topics_page\n                                ${binding}\n`,
    );
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /topic feed must bind selection exactly once/);
  });
}
