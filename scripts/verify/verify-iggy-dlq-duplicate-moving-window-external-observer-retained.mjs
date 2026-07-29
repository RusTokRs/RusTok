#!/usr/bin/env node
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL("../../", import.meta.url));
const contractPath = "crates/rustok-iggy/contracts/evidence/dlq-duplicate-moving-window-external-observer-execution-contract.json";
const sourcePath = "crates/rustok-iggy/contracts/evidence/dlq-duplicate-moving-window-external-observer-runtime-source.json";
const c = JSON.parse(readFileSync(resolve(root, contractPath), "utf8")); const s = JSON.parse(readFileSync(resolve(root, sourcePath), "utf8"));
const evidencePath = resolve(root, c.evidence_path); const runner = readFileSync(resolve(root, c.runner), "utf8");
const failures=[]; const fail=(m)=>failures.push(m); const same=(a,b)=>JSON.stringify(a)===JSON.stringify(b); const sha=(v)=>createHash("sha256").update(v).digest("hex");
const validSha=(v,n)=>{if(typeof v!=="string"||!/^[0-9a-f]{64}$/u.test(v))fail(`${n} invalid SHA-256`)}; const line=(v,n,max=256)=>{if(typeof v!=="string"||!v||v.trim()!==v||v.length>max||/[\r\n\u0000-\u001f\u007f]/u.test(v))fail(`${n} invalid bounded line`)};
function currentCommit(){const r=spawnSync("git",["rev-parse","HEAD"],{cwd:root,encoding:"utf8"});if(r.status!==0)return fail("cannot read current commit");return r.stdout.trim();}
function hashes(){return Object.fromEntries(c.source_files.map((p)=>[p,existsSync(resolve(root,p))?sha(readFileSync(resolve(root,p))):null]));}
function collect(value, keys=[]){if(Array.isArray(value))value.forEach((x)=>collect(x,keys));else if(value&&typeof value==="object")for(const[k,v]of Object.entries(value)){keys.push(k);collect(v,keys)}return keys;}
if(c.schema_version!==1||c.packet!=="dlq-duplicate-moving-window-external-observer-execution-contract"||c.status!=="runtime_execution_contract_locked"||c.source_contract!==sourcePath||c.evidence_status!=="runtime_execution_pending")fail("execution contract identity drift");
if(s.retained_execution?.contract!==contractPath||s.retained_execution?.evidence_path!==c.evidence_path||s.retained_execution?.canonical_packet_present!==false||s.execution_status!=="not_run")fail("source retained relationship drift");
for(const marker of ["ensureCleanCommit", "sourceHashes", "required_runtime_marker", "writeNoClobber", "flag: \"wx\"", "linkSync(temp, outputPath)"])if(!runner.includes(marker))fail(`runner missing: ${marker}`);
if(!existsSync(evidencePath)){if(failures.length){console.error("Moving-observer retained verification failed:");failures.forEach((x)=>console.error(`- ${x}`));process.exit(1);}console.log("Iggy moving-observer retained evidence verified: clean-commit exact-case capture, reviewed dedup-disabled and reset projections, source hashes, cross-cycle/replacement assertions, absent stored offsets, privacy exclusions, and no-clobber publication are locked; canonical evidence is absent.");process.exit(0);}
let e;try{e=JSON.parse(readFileSync(evidencePath,"utf8"));}catch{fail("evidence is not valid JSON");}
if(e){
  const top=["schema_version","module","packet","status","generated_from","runner","verifier","git_commit","working_tree_clean_before_run","started_at","completed_at","environment_sources","reviewed_artifacts","reviewed_configuration","reviewed_reset","toolchain","source_sha256","executed_case"].sort();
  if(!same(Object.keys(e).sort(),top))fail("top-level keys drift");
  if(e.schema_version!==1||e.packet!=="dlq-duplicate-moving-window-external-observer-runtime-evidence"||e.status!=="external_iggy_moving_observer_cross_cycle_runtime_executed"||e.generated_from!==contractPath||e.runner!==c.runner||e.verifier!==c.verifier||e.working_tree_clean_before_run!==true)fail("evidence identity drift");
  const commit=currentCommit();if(commit&&e.git_commit!==commit)fail("evidence belongs to another commit");
  if(Number.isNaN(Date.parse(e.started_at))||Number.isNaN(Date.parse(e.completed_at))||Date.parse(e.completed_at)<Date.parse(e.started_at))fail("timestamps invalid");
  line(e.reviewed_artifacts?.iggy_server,"server artifact"); line(e.toolchain?.cargo,"cargo"); line(e.toolchain?.rustc,"rustc");
  const config={section:"system.message_deduplication",enabled:false}; if(!same({section:e.reviewed_configuration?.section,enabled:e.reviewed_configuration?.enabled},config))fail("dedup projection drift");validSha(e.reviewed_configuration?.canonical_sha256,"dedup digest");if(e.reviewed_configuration?.canonical_sha256!==sha(JSON.stringify(config)))fail("dedup digest mismatch");
  const rr=e.reviewed_reset??{};if(rr.schema_version!==1||rr.initial_offset!==0||rr.restart_continuity_required!==false)fail("reset projection drift");line(rr.acceptable_reset_frequency,"reset frequency");line(rr.review_scope,"review scope");const reset={schema_version:1,initial_offset:0,acceptable_reset_frequency:rr.acceptable_reset_frequency,restart_continuity_required:false,review_scope:rr.review_scope};validSha(rr.canonical_sha256,"reset digest");if(rr.canonical_sha256!==sha(JSON.stringify(reset)))fail("reset digest mismatch");
  if(!same(e.source_sha256,hashes()))fail("source hashes stale");Object.entries(e.source_sha256??{}).forEach(([p,v])=>validSha(v,`source ${p}`));
  const x=e.executed_case??{};if(x.name!==c.case||x.result!=="pass"||!same(x.command,c.command)||!same(x.moving_configuration,c.moving_configuration)||!same(x.required_first_summary,c.required_first_summary)||!same(x.required_second_summary,c.required_second_summary)||!same(x.required_comparison,c.required_comparison)||!same(x.required_offset_observations,c.required_offset_observations)||x.runtime_marker!==c.required_runtime_marker)fail("executed assertions drift");validSha(x.test_output_sha256,"output digest");if(!Number.isSafeInteger(x.test_output_bytes)||x.test_output_bytes<=0)fail("output byte count invalid");
  const forbidden=new Set(c.privacy_exclusions);for(const key of collect(e))if(forbidden.has(key))fail(`privacy exclusions violated by key: ${key}`);
}
if(failures.length){console.error("Moving-observer retained verification failed:");failures.forEach((x)=>console.error(`- ${x}`));process.exit(1);}console.log("Iggy moving-observer retained evidence verified: canonical packet is current, commit-bound, privacy-safe, and matches the locked cross-cycle and reset assertions.");
