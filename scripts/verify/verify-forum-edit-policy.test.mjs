import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { GRAPHQL_SETTINGS_DEFAULT_SITES, SETTINGS_COMPOSITION_SITES, inspectGraphqlSettingsErrors, inspectSettingsComposition, inspectTopicEditPolicy, inspectTransactionalSettingsPolicy } from "./verify-forum-edit-policy.mjs";

for (const [file, count] of GRAPHQL_SETTINGS_DEFAULT_SITES) {
  const source = readFileSync(new URL(`../../crates/modules/rustok-forum/${file}`, import.meta.url), "utf8");
  test(`${file} preserves typed settings errors in every default branch`, () => {
    assert.deepEqual(inspectGraphqlSettingsErrors(source, file, count), []);
  });
  const calls = [...source.matchAll(/\.default_(?:topics_per_page|replies_per_page|topic_order)\(tenant_id\)\s*\.await\s*\?/g)];
  assert.equal(calls.length, count);
  for (const [index, call] of calls.entries()) {
    test(`${file} rejects string erasure at settings default ${index}`, () => {
      const changed = call[0].replace(/\.await\s*\?$/, '.await.map_err(|error| async_graphql::Error::new(error.to_string()))?');
      const mutated = source.slice(0, call.index) + changed + source.slice(call.index + call[0].length);
      assert.notDeepEqual(inspectGraphqlSettingsErrors(mutated, file, count), []);
    });
  }
}

for (const [file, label, count] of SETTINGS_COMPOSITION_SITES) {
  const source = readFileSync(new URL(`../../crates/modules/rustok-forum/${file}`, import.meta.url), "utf8");
  test(`${label} preserves both optional settings capabilities for owner admission`, () => {
    assert.deepEqual(inspectSettingsComposition(source, label, count), []);
  });
  for (const reader of ["SharedStaticModuleSettingsReader", "SharedStaticModuleSettingsTransactionReader"]) {
    test(`${label} cannot discard ${reader} into default-only configuration`, () => {
      const mutated = source.replace(new RegExp(`\\w+\\.shared_get::<${reader}>\\(\\)`), "None");
      assert.notEqual(mutated, source);
      assert.notDeepEqual(inspectSettingsComposition(mutated, label, count), []);
    });
  }
}

const settings = readFileSync(new URL("../../crates/modules/rustok-forum/src/services/engagement_mode.rs", import.meta.url), "utf8");
test("configured transactional settings fail closed without breaking default-only services", () => {
  assert.deepEqual(inspectTransactionalSettingsPolicy(settings), []);
});
test("settings guard rejects a constructor that discards the supplied reader pair", () => {
  const mutated = settings.replace(/Self\s*\{\s*static_reader,\s*transactional_reader,?\s*\}/, "Self::default()");
  assert.notEqual(mutated, settings);
  assert.notDeepEqual(inspectTransactionalSettingsPolicy(mutated), []);
});
for (const [before, after] of [
  ["(None, true) => Err(ForumError::capability_unavailable(", "(None, true) => Ok(None), removed("],
  ["self.transactional_settings_reader()?", "self.transactional_reader.as_ref()"],
  ["(None, false) => Ok(None)", "(None, false) => panic!()"],
  ['"FORUM_TRANSACTIONAL_SETTINGS_UNAVAILABLE"', '"UNSPECIFIED"'],
  ['"FORUM_STATIC_SETTINGS_UNAVAILABLE"', '"UNSPECIFIED"'],
  ["self.static_settings_reader()?;", "removed_static_admission();"],
  ["let Some(reader) = self.static_settings_reader()?", "let Some(reader) = self.static_reader.as_ref()"],
  ["providers.static_settings_reader()?", "providers.static_reader.as_ref()"],
]) {
  test(`settings guard rejects ${after}`, () => {
    assert.ok(settings.includes(before));
    const start = before === "(None, true) => Err(ForumError::capability_unavailable(" || before === "(None, false) => Ok(None)"
      ? settings.indexOf("fn transactional_settings_reader(") : 0;
    const mutated = settings.slice(0, start) + settings.slice(start).replace(before, after);
    assert.notDeepEqual(inspectTransactionalSettingsPolicy(mutated), []);
  });
}

const source = readFileSync(new URL("../../crates/modules/rustok-forum/src/services/topic.rs", import.meta.url), "utf8");
const start = source.indexOf("    pub(crate) async fn update_with_inline_relations(");
const txStart = source.indexOf("        let txn = self.db.begin().await?;", start);
const mutateTx = (before, after) => {
  const transaction = source.slice(txStart);
  assert.ok(transaction.includes(before), `fixture must mutate ${before}`);
  return source.slice(0, txStart) + transaction.replace(before, after);
};

test("topic edit checks locked ownership/lifecycle/settings/time after claim", () => {
  assert.deepEqual(inspectTopicEditPolicy(source), []);
});
for (const marker of ["Self::claim_topic_update_in_tx(", "Self::find_topic_in_tx(", "enforce_owned_scope(", "topic.status == TopicStatus::Archived", "enforce_author_edit_window(", ".module_settings_in_tx(", "Utc::now()"] ) {
  test(`topic edit guard rejects missing transactional ${marker}`, () => {
    assert.notDeepEqual(inspectTopicEditPolicy(mutateTx(marker, "removed_check(")), []);
  });
}
test("topic edit guard rejects moving claim after the edit-window check", () => {
  const claimStart = source.indexOf("        let claimed_updated_at =", txStart);
  const claimEnd = source.indexOf("\n", source.indexOf(".await?;", claimStart)) + 1;
  const claim = source.slice(claimStart, claimEnd);
  const without = source.slice(0, claimStart) + source.slice(claimEnd);
  const insertAt = without.indexOf("        // A content edit", claimStart);
  assert.notDeepEqual(inspectTopicEditPolicy(without.slice(0, insertAt) + claim + without.slice(insertAt)), []);
});
