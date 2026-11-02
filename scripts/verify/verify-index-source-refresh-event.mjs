#!/usr/bin/env node
// Supplementary read-only source contract check; never executes Rust or test callbacks.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { containsMarker, spaced } from "./lib/source-text.mjs";
import { rustFunctionBody } from "./verify-cart-priced-write-context.mjs";

const repository = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
export function verifyIndexSourceRefreshEvent({ root = repository } = {}) {
  const failures = [];
  const read = (name) => fs.readFileSync(path.join(root, name), "utf8");
  const require = (text, marker, label) => {
    if (!containsMarker(text, marker))
      failures.push(`${label}: missing ${marker}`);
  };
  const source = read(
    "crates/modules/rustok-index/src/application/source_refresh_event.rs",
  );
  const persist = rustFunctionBody(source, "persist_index_source_refresh");
  const process = rustFunctionBody(source, "process");
  if (!persist || !process)
    failures.push("missing canonical persistence/wrapper boundary");
  const code = spaced(persist);
  const ordered = [
    "event_registry.get(&event_domain)",
    "key.schema != expected_schema",
    ".source_for_schema(&key.schema)",
    "source.source_name() != expected_source_name",
    "IndexSourceLoadRequest::new(vec![key.clone()])",
    ".load(request)",
    "source_version < minimum_source_version",
    "mutation.event_id().is_nil()",
    ".apply_replay_mutation(schema_registry, &expected_source_name, &mutation)",
  ];
  let previous = -1;
  for (const marker of ordered) {
    const index = code.indexOf(spaced(marker));
    if (index <= previous) failures.push(`persistence order: ${marker}`);
    previous = index;
  }
  for (const marker of [
    "rebind_event_id",
    "acknowledge(",
    "Uuid::new_v4",
    "SELECT ",
    "INSERT INTO",
    "tokio::spawn",
    "Noop",
  ]) {
    if (containsMarker(persist, marker))
      failures.push(`persistence boundary forbidden: ${marker}`);
  }
  for (const marker of [
    "MissingSourceMutation",
    "AmbiguousSourceMutation",
    "SourceVersionBehind",
    "ReplaySourceMismatch",
    "NilSourceEventId",
  ]) {
    require(source, marker, "failure contract");
  }
  const wrapper = spaced(process);
  const persisted = wrapper.indexOf(spaced("persist_index_source_refresh("));
  const acknowledged = wrapper.indexOf(
    spaced(".acknowledge(delivery.acknowledgement_token())"),
  );
  if (
    persisted < 0 ||
    acknowledged <= persisted ||
    !wrapper.slice(persisted, acknowledged).includes(".await?")
  ) {
    failures.push(
      "wrapper must await durable persistence before original-token ACK",
    );
  }
  const application = read(
    "crates/modules/rustok-index/src/application/mod.rs",
  );
  const publicApi = read("crates/modules/rustok-index/src/lib.rs");
  for (const text of [application, publicApi])
    require(text, "persist_index_source_refresh", "canonical export");
  const tests = read(
    "crates/modules/rustok-index/src/application/source_refresh_event_tests.rs",
  );
  for (const marker of [
    "canonical_source_mutation_identity_is_preserved_before_acknowledgement",
    "missing_or_behind_source_state_suppresses_apply_and_ack",
    "schema_mismatch_fails_before_source_load_apply_or_ack",
    "delivery_rejects_invalid_identity_and_revision",
    "nil_loaded_owner_mutation_identity_is_refused_before_persistence_or_ack",
    "old_delivery_preserves_each_successively_loaded_owner_mutation_identity",
    "durable_persistence_failure_withholds_the_original_transport_ack",
    'vec!["apply", "ack"]',
    "Some(mutation(9).event_id())",
  ])
    require(tests, marker, "unrun regression definition");
  const docs = read(
    "crates/modules/rustok-index/docs/m5-source-refresh-event.md",
  );
  for (const marker of [
    "loaded owner mutation UUID is the durable Index inbox identity",
    "one bounded `IndexSourceLoadRequest` for exactly one key",
    "original opaque token",
  ])
    require(docs, marker, "owner documentation");
  if (
    !containsMarker(docs, /automatic backoff/i) ||
    !containsMarker(docs, "M6 - execute and admit concrete repair evidence")
  ) {
    failures.push("owner documentation must retain recovery/evidence gates");
  }
  return failures;
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const failures = verifyIndexSourceRefreshEvent();
  if (failures.length) {
    console.error(
      failures
        .map((failure) => `[verify-index-source-refresh-event] ${failure}`)
        .join("\n"),
    );
    process.exitCode = 1;
  } else
    console.log(
      "[verify-index-source-refresh-event] source contract only; no tests, compilation or runtime evidence",
    );
}
