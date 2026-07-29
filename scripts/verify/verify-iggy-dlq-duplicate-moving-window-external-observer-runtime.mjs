#!/usr/bin/env node
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL("../../", import.meta.url));
const sourcePath = "crates/rustok-iggy/contracts/evidence/dlq-duplicate-moving-window-external-observer-runtime-source.json";
const executionPath = "crates/rustok-iggy/contracts/evidence/dlq-duplicate-moving-window-external-observer-execution-contract.json";
const s = JSON.parse(readFileSync(resolve(root, sourcePath), "utf8"));
const e = JSON.parse(readFileSync(resolve(root, executionPath), "utf8"));
const read = (path) => readFileSync(resolve(root, path), "utf8");
const test = read(s.test), observer = read(s.observer_source), moving = read(s.moving_scanner_source), server = read(s.server_source), runner = read(s.retained_execution.runner), retained = read(s.retained_execution.verifier), docs = read(s.documentation), checkpoint = read(s.profiles_checkpoint);
const failures = []; const fail = (m) => failures.push(m); const same = (a,b) => JSON.stringify(a) === JSON.stringify(b);
const need = (name, text, marker) => { if (!text.includes(marker)) fail(`${name} missing: ${marker}`); };
if (s.schema_version !== 1 || s.packet !== "dlq-duplicate-moving-window-external-observer-runtime-source" || s.status !== "source_complete_runtime_execution_pending" || s.execution_status !== "not_run" || s.case !== "moving_observer_retains_duplicate_across_advancing_cycles") fail("source identity drift");
if (s.retained_execution.contract !== executionPath || s.retained_execution.canonical_packet_present !== false || s.retained_execution.no_clobber_write !== true) fail("retained relationship drift");
if (e.source_contract !== sourcePath || e.case !== s.case || e.evidence_status !== "runtime_execution_pending" || !same(e.moving_configuration, { partition_count:1, initial_offset:0, per_partition_messages:1, batch_size:1, rolling_max_cycles:3, rolling_max_observations_per_cycle:1, production_defaults:false })) fail("execution relationship drift");
if (s.fixture?.same_id_split_across_partitions !== false || s.fixture?.replacement_observer_starts_from_initial_offset !== true || s.required_observations?.stored_consumer_offset_count_every_checkpoint !== 0 || s.required_observations?.checkpoints !== 5) fail("fixture boundary drift");
for (const marker of ["fn moving_observer_retains_duplicate_across_advancing_cycles()", "IggyDlqDuplicateAlertObserver::connect_moving_window", "assert_first_summary", "assert_second_summary", "assert_no_stored_offset", s.required_observations.runtime_marker]) need("test", test, marker);
for (const marker of ["MovingWindow", "connect_moving_window", "preserves_process_local_state_after_scan_error", "Ok(*snapshot.rolling().summary())"]) need("observer", observer, marker);
for (const marker of ["PollingStrategy::offset(next_offset)", "auto_commit = false", "state.apply_complete_cycle(partitions)"]) need("moving scanner", moving, marker);
for (const marker of ["RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE", "moving_window", "required_u64_env(START_OFFSET_ENV)?"]) need("server", server, marker);
for (const marker of ["working tree must be clean", "sourceHashes()", "runtime marker missing", "writeNoClobber", "linkSync(temp, outputPath)"]) need("runner", runner, marker);
for (const marker of ["canonical evidence is absent", "source_sha256", "reviewed_reset", "privacy exclusions"]) need("retained verifier", retained, marker);
for (const marker of ["source-complete; external execution pending", "same production-selected partition", "replacement observer", "does not start the full server process"]) need("owner guide", docs, marker);
for (const marker of ["Profiles never authorizes", "initial_offset = 0", "restart_continuity_required = false", "canonical packet is pending"]) need("Profiles checkpoint", checkpoint, marker);
for (const forbidden of ["pub broker_message_id", "pub payload", "pub offset", "store_consumer_offset(", ".acknowledge(", ".delete_topic(", ".purge_topic("]) if (test.includes(forbidden)) fail(`test contains forbidden marker: ${forbidden}`);
if (existsSync(resolve(root, s.retained_execution.evidence_path))) fail("canonical packet unexpectedly present in source-complete slice");
if (failures.length) { console.error("Moving-observer runtime source verification failed:"); failures.forEach((x)=>console.error(`- ${x}`)); process.exit(1); }
console.log("Iggy moving-observer runtime source verified: a same-partition cross-cycle duplicate, empty-cycle retention, replacement reset, absent stored offsets, reviewed inputs, privacy exclusions, and pending no-clobber execution are locked.");
