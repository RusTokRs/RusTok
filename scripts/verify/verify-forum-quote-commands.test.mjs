import assert from "node:assert/strict";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const forum = "crates/modules/rustok-forum";
const boundary = `${forum}/contracts/forum-mention-write-boundary.json`;
const service = `${forum}/src/services/quote_command.rs`;
const composition = `${forum}/src/services/mod.rs`;
const resolver = `${forum}/src/services/relation_quote_input.rs`;
const graphqlRuntime = `${forum}/src/graphql/runtime_data.rs`;
const restRuntime = `${forum}/src/controllers/mod.rs`;
const contract = JSON.parse(readFileSync(path.join(repoRoot, boundary), "utf8"));
const files = new Set([
  boundary, service, composition, resolver, graphqlRuntime, restRuntime,
  `${forum}/src/services/topic_engagement_guard.rs`,
  `${forum}/CRATE_API.md`, `${forum}/README.md`, `${forum}/src/mentions.rs`,
  `${forum}/src/services/mention_relation.rs`, `${forum}/src/services/mention_relation_tests.rs`,
  `${forum}/src/services/topic_facade.rs`, `${forum}/src/services/reply_facade.rs`,
  `${forum}/src/controllers/quote_commands.rs`, `${forum}/src/controllers/content_commands.rs`,
  `${forum}/src/graphql/mod.rs`, `${forum}/src/graphql/quote_commands.rs`,
  `${forum}/src/graphql/content_commands.rs`, `${forum}/src/openapi.rs`,
  "crates/modules/rustok-profiles/src/reader.rs", "crates/modules/rustok-profiles/src/services.rs",
  contract.quote_command.dto, contract.inline_quote_commands.topic_dto,
  contract.inline_quote_commands.reply_dto, ...Object.values(contract.source_entrypoints),
  ...Object.values(contract.owner_entrypoints).flatMap(({ owner, implementation }) => [owner, implementation]),
]);

function withFixture(change, assertion) {
  const root = mkdtempSync(path.join(tmpdir(), "forum-quote-contract-"));
  try {
    const write = (file, text) => {
      const target = path.join(root, file);
      mkdirSync(path.dirname(target), { recursive: true });
      writeFileSync(target, text);
    };
    for (const file of files) {
      const target = path.join(root, file);
      mkdirSync(path.dirname(target), { recursive: true });
      copyFileSync(path.join(repoRoot, file), target);
    }
    const replace = (file, before, after) => {
      const text = readFileSync(path.join(root, file), "utf8");
      assert.ok(text.includes(before), `fixture mutation must apply: ${before}`);
      write(file, text.replace(before, after));
    };
    change({ replace, write });
    assertion((name = "verify-forum-quote-commands") => spawnSync(process.execPath,
      [path.join(repoRoot, `scripts/verify/${name}.mjs`)], {
        cwd: root, env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root }, encoding: "utf8",
      }));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

function rejects(change, message) {
  withFixture(change, (run) => {
    const result = run();
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, message);
  });
}

test("quote and mention integration guards accept current owner modules without deleted handoff files", () => {
  withFixture(() => {}, (run) => {
    for (const guard of ["verify-forum-quote-commands", "verify-forum-mention-integration"]) {
      const result = run(guard);
      assert.equal(result.status, 0, result.stderr || result.stdout);
    }
  });
});

test("quote guard rejects obsolete fragment paths in the machine contract", () => {
  rejects(({ replace }) => replace(boundary, "services/topic_owner.rs", "services/nonexistent_owner.rs"), /required file is missing/);
});

test("quote guard requires the transactional admission call before persistence", () => {
  rejects(({ replace }) => replace(service,
    ".enforce_source_edit_in_tx(&txn, tenant_id, source, &security)", ".removed_admission()"), /missing or out-of-order marker/);
});

test("quote admission rejects checking the reply policy before acquiring its source lock", () => {
  rejects(({ replace }) => replace(service,
    "ReplyService::find_reply_for_update_in_tx(", "ReplyService::find_reply_in_tx("), /quote admission must lock parent and reply/);
});

test("quote guard requires the in-transaction settings read, not just the preflight", () => {
  rejects(({ replace }) => replace(service, "module_settings_in_tx", "module_settings_outside_tx"), /transactional quote admission/);
});

for (const [label, file] of [["REST", restRuntime], ["GraphQL", graphqlRuntime]]) {
  test(`quote guard detects lost settings in the ${label} quote factory despite other configured owners`, () => {
    withFixture(({ write }) => {
      const source = readFileSync(path.join(repoRoot, file), "utf8");
      const start = source.indexOf("fn quote_command_service(");
      const end = source.indexOf("\n    }", start);
      const factory = source.slice(start, end);
      assert.ok(factory.includes(".with_settings_providers(self.settings_providers.clone())"));
      write(file, source.slice(0, start) + factory.replace(".with_settings_providers(self.settings_providers.clone())", "") + source.slice(end));
    }, (run) => {
      const result = run();
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, new RegExp(`${label} runtime must pass configured settings`));
    });
  });
}

test("quote guard still rejects source persistence access from a new transport", () => {
  rejects(({ write }) => write(`${forum}/src/controllers/private_relation_access.rs`, "MentionRelationService"), /transport must not access private symbol/);
});

test("quote guard rejects removing the source lock from preserved-quote CAS", () => {
  rejects(({ replace }) => replace(resolver, "lock_active_source_in_tx(txn, tenant_id, source).await?;", "removed_source_lock();"), /preserved quotes must lock the source/);
});

test("quote guard pins the current exact-locale replacement contract", () => {
  rejects(({ replace }) => replace(`${forum}/CRATE_API.md`, "exact source locale", "unspecified source locale"), /canonical quote API contract/);
});


test("quote guard requires both edits to use the canonical engagement policy", () => {
  rejects(({ replace }) => replace(service,
    "enforce_topic_engagement(\n", "removed_engagement_admission(\n"), /transactional quote admission/);
});

test("quote guard rejects losing the shared archived-parent refusal", () => {
  rejects(({ replace }) => replace(`${forum}/src/services/topic_engagement_guard.rs`,
    "TopicStatus::Archived => return Err(ForumError::TopicArchived)",
    "TopicStatus::Archived => return Ok(())"), /canonical engagement admission/);
});
