#!/usr/bin/env node
import { createHash } from "node:crypto";
import { existsSync, linkSync, mkdirSync, readFileSync, statSync, unlinkSync, writeFileSync } from "node:fs";
import { dirname, isAbsolute, resolve, sep } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const contractPath = "crates/rustok-iggy/contracts/evidence/dlq-duplicate-moving-window-external-observer-execution-contract.json";
const contract = JSON.parse(readFileSync(resolve(root, contractPath), "utf8"));
const outputPath = resolve(root, contract.evidence_path);
const expectedCase = "moving_observer_retains_duplicate_across_advancing_cycles";
const fail = (message) => { throw new Error(message); };
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const sha256 = (value) => createHash("sha256").update(value).digest("hex");

function run(program, args, env = process.env) {
  const result = spawnSync(program, args, { cwd: root, env, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
  if (result.error) fail(`${program} could not start: ${result.error.message}`);
  if ((result.status ?? -1) !== 0) fail(`${program} exited with status ${result.status}; no evidence written`);
  return { stdout: result.stdout ?? "", stderr: result.stderr ?? "" };
}
function oneLine(value, field, max = 256) {
  if (typeof value !== "string" || !value || value.trim() !== value || value.length > max || /[\r\n\u0000-\u001f\u007f]/u.test(value)) fail(`${field} is not a bounded one-line value`);
  return value;
}
function commandLine(program, args, field) { return oneLine(run(program, args).stdout.replace(/\r?\n$/u, ""), field); }
function externalPath(value, field) {
  const supplied = oneLine(value, field, 4096);
  if (!isAbsolute(supplied)) fail(`${field} must be an absolute path outside the repository`);
  const path = resolve(supplied); const repo = resolve(root);
  if (path === repo || path.startsWith(`${repo}${sep}`)) fail(`${field} must point outside the repository`);
  if (!existsSync(path) || !statSync(path).isFile()) fail(`${field} must point to an existing file`);
  return path;
}
function artifact(value, field) {
  const label = oneLine(value, field);
  if (label.includes("://") || label.includes("@") || /^\[[0-9a-fA-F:]+\]:\d+$/u.test(label) || /^[A-Za-z0-9._-]+:\d+$/u.test(label)) fail(`${field} must be a reviewed non-endpoint label`);
  return label;
}
function validateAddress(value, field) {
  const address = oneLine(value, field, 255);
  if (address.includes("://") || address.includes("@") || address.includes("?") || address.includes("#") || !/^\[[0-9a-fA-F:]+\]:\d+$|^[A-Za-z0-9._-]+:\d+$/u.test(address)) fail(`${field} must be host:port`);
  const port = Number(address.slice(address.lastIndexOf(":") + 1));
  if (!Number.isInteger(port) || port < 1 || port > 65535) fail(`${field} port is invalid`);
}
function validateCredentials() {
  const [u, p] = contract.optional_environment; const username = process.env[u] ?? ""; const password = process.env[p] ?? "";
  if ((!username) !== (!password)) fail("username and password must both be set or both be empty");
  for (const [value, name] of [[username, u], [password, p]]) if (value) { oneLine(value, name, 191); if (/[:@]/u.test(value)) fail(`${name} contains a connection delimiter`); }
}
function parseDedup(path) {
  const text = readFileSync(path, "utf8");
  const section = /(?:^|\n)\s*\[system\.message_deduplication\]\s*\n([\s\S]*?)(?=\n\s*\[|$)/u.exec(text)?.[1];
  if (!section || !/(?:^|\n)\s*enabled\s*=\s*false\s*(?:#.*)?(?:\n|$)/u.test(section)) fail("reviewed Iggy config must disable message deduplication");
  const canonical = { section: "system.message_deduplication", enabled: false };
  return { ...canonical, canonical_sha256: sha256(JSON.stringify(canonical)) };
}
function parseReset(path) {
  let value; try { value = JSON.parse(readFileSync(path, "utf8")); } catch { fail("reset review must be valid JSON"); }
  const fields = contract.reviewed_reset.required_fields.slice().sort();
  if (!value || Array.isArray(value) || !same(Object.keys(value).sort(), fields)) fail("reset review keys drift");
  if (value.schema_version !== 1 || value.initial_offset !== 0 || value.restart_continuity_required !== false) fail("reset review must approve initial_offset=0 without restart continuity");
  const canonical = { schema_version: 1, initial_offset: 0, acceptable_reset_frequency: artifact(value.acceptable_reset_frequency, "acceptable_reset_frequency"), restart_continuity_required: false, review_scope: artifact(value.review_scope, "review_scope") };
  return { ...canonical, canonical_sha256: sha256(JSON.stringify(canonical)) };
}
function status() { return run("git", ["status", "--porcelain=v1", "--untracked-files=all"]).stdout; }
function sourceHashes() {
  return Object.fromEntries(contract.source_files.map((path) => {
    const file = resolve(root, path); if (!existsSync(file) || !statSync(file).isFile()) fail(`source missing: ${path}`);
    return [path, sha256(readFileSync(file))];
  }));
}
function validateContract() {
  if (contract.schema_version !== 1 || contract.packet !== "dlq-duplicate-moving-window-external-observer-execution-contract" || contract.status !== "runtime_execution_contract_locked" || contract.case !== expectedCase || contract.evidence_status !== "runtime_execution_pending") fail("execution contract identity drift");
  if (contract.moving_configuration?.partition_count !== 1 || contract.moving_configuration?.initial_offset !== 0 || contract.moving_configuration?.per_partition_messages !== 1 || contract.moving_configuration?.rolling_max_cycles !== 3) fail("moving configuration drift");
  if (contract.reviewed_reset?.required_initial_offset !== 0 || contract.reviewed_reset?.required_restart_continuity !== false) fail("reset contract drift");
}
function requirePass(output) {
  if (!/(?:^|\r?\n)running 1 test(?:\r?\n|$)/u.test(output)) fail("exactly one test was not run");
  const escaped = expectedCase.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
  if (!new RegExp(`(?:^|\\r?\\n)test ${escaped} \\.\\.\\. ok(?:\\r?\\n|$)`, "u").test(output)) fail("exact case did not pass");
  if (/skipping external Iggy moving-observer duplicate evidence/iu.test(output)) fail("exact case skipped");
  if (!output.includes(contract.required_runtime_marker)) fail("runtime marker missing");
}
function writeNoClobber(packet) {
  const repo = `${resolve(root)}${sep}`; if (!outputPath.startsWith(repo)) fail("output must stay inside repository");
  mkdirSync(dirname(outputPath), { recursive: true }); if (existsSync(outputPath)) fail("canonical evidence exists and will not be replaced");
  const temp = `${outputPath}.tmp-${process.pid}`;
  try { writeFileSync(temp, `${JSON.stringify(packet, null, 2)}\n`, { encoding: "utf8", flag: "wx" }); linkSync(temp, outputPath); }
  finally { if (existsSync(temp)) unlinkSync(temp); }
}

try {
  validateContract();
  const e = contract.required_environment;
  validateAddress(process.env[e.address] ?? "", e.address); validateCredentials();
  const config = parseDedup(externalPath(process.env[e.config_path] ?? "", e.config_path));
  const reset = parseReset(externalPath(process.env[e.reset_review_path] ?? "", e.reset_review_path));
  const serverArtifact = artifact(process.env[e.server_artifact] ?? "", e.server_artifact);
  if (status().trim()) fail("working tree must be clean before retained execution");
  const commit = commandLine("git", ["rev-parse", "HEAD"], "git_commit"); if (!/^[0-9a-f]{40}$/u.test(commit)) fail("commit must be a full SHA-1");
  const hashes = sourceHashes(); const cargo = commandLine("cargo", ["--version"], "cargo_version"); const rustc = commandLine("rustc", ["--version"], "rustc_version");
  const started = new Date().toISOString(); const result = run(contract.command.program, contract.command.args); const output = `${result.stdout}\n${result.stderr}`; requirePass(output);
  if (commandLine("git", ["rev-parse", "HEAD"], "final_commit") !== commit) fail("commit changed during execution");
  if (!same(sourceHashes(), hashes) || status().trim()) fail("source or working tree changed during execution");
  writeNoClobber({ schema_version: 1, module: "iggy", packet: "dlq-duplicate-moving-window-external-observer-runtime-evidence", status: "external_iggy_moving_observer_cross_cycle_runtime_executed", generated_from: contractPath, runner: contract.runner, verifier: contract.verifier, git_commit: commit, working_tree_clean_before_run: true, started_at: started, completed_at: new Date().toISOString(), environment_sources: { address_environment: e.address, configuration_path_environment: e.config_path, reset_review_path_environment: e.reset_review_path, server_artifact_environment: e.server_artifact, username_environment: contract.optional_environment[0], password_environment: contract.optional_environment[1] }, reviewed_artifacts: { iggy_server: serverArtifact }, reviewed_configuration: config, reviewed_reset: reset, toolchain: { cargo, rustc }, source_sha256: hashes, executed_case: { name: contract.case, result: "pass", command: contract.command, moving_configuration: contract.moving_configuration, required_first_summary: contract.required_first_summary, required_second_summary: contract.required_second_summary, required_comparison: contract.required_comparison, required_offset_observations: contract.required_offset_observations, runtime_marker: contract.required_runtime_marker, test_output_sha256: sha256(output), test_output_bytes: Buffer.byteLength(output) } });
  console.log(`Retained moving-observer evidence written to ${contract.evidence_path}`);
} catch (error) { console.error(`Moving-observer retained capture failed: ${error.message}`); process.exit(1); }
