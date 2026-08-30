#!/usr/bin/env node

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const cliRoot = path.join(root, 'crates/rustok-modules/cli');
const manifest = fs.readFileSync(path.join(cliRoot, 'Cargo.toml'), 'utf8');
const source = fs.readFileSync(path.join(cliRoot, 'src/lib.rs'), 'utf8');
const sandboxHarness = fs.readFileSync(
  path.join(root, 'crates/rustok-sandbox/src/harness.rs'),
  'utf8',
);
const authoringOwner = fs.readFileSync(
  path.join(root, 'crates/rustok-modules/src/authoring.rs'),
  'utf8',
);
const governanceOwner = fs.readFileSync(
  path.join(root, 'crates/rustok-modules/src/governance.rs'),
  'utf8',
);
const publishValidation = fs.readFileSync(
  path.join(root, 'crates/rustok-modules/src/publish_validation.rs'),
  'utf8',
);
const moduleManifest = fs.readFileSync(
  path.join(root, 'crates/rustok-modules/rustok-module.toml'),
  'utf8',
);
const registryManifest = fs.readFileSync(
  path.join(root, 'crates/rustok-cli-registry/Cargo.toml'),
  'utf8',
);
const registry = fs.readFileSync(
  path.join(root, 'crates/rustok-cli-registry/src/generated.rs'),
  'utf8',
);
const serverManifest = fs.readFileSync(path.join(root, 'apps/server/Cargo.toml'), 'utf8');

for (const dependency of [
  'sea-orm',
  'sqlx',
  'rustok-storage',
  'rustok-sandbox-transport',
  'rustok-sandbox-worker',
  'rustok-module-build-worker',
  'rustok-build-publication',
  'rustok-ai',
  'alloy',
]) {
  assert.ok(
    !new RegExp(`^${dependency}\\s*=`, 'm').test(manifest),
    `module authoring CLI must not depend on ${dependency}`,
  );
}

for (const marker of [
  'CommandDescriptor::new(\n                "module",\n                "init"',
  'CommandDescriptor::new(\n                "module",\n                "validate"',
  'CommandDescriptor::new(\n                "module",\n                "test"',
  'CommandDescriptor::new(\n                "module",\n                "build"',
  'CommandDescriptor::new(\n                "module",\n                "publish"',
  'ModuleArtifactSourceManifest::parse(&source_bytes)',
  'ModuleTemplateInput',
  'SourceArchiveBuilder::new(limits)',
  'SourceArchiveInspector::new(source_archive_limits()?)',
  '"package"',
  '"inspect"',
  'LocalSandboxHarness::wasm_component()',
  'LocalSandboxScenario::parse(&scenario_bytes)',
  'validate_scenario_capabilities(',
  'scenario.canonical_digest().map_err(invalid_input)',
  '"scenario_digest": self.scenario_digest',
  'scenario.comparison(&evaluated).map_err(command_failed)',
  '"comparison": comparison',
  'CARGO_NET_OFFLINE',
  '.env_clear()',
  'MAX_LOCAL_CARGO_OUTPUT_BYTES',
  'ModuleAuthoringSourceArchiveBuilder::new()',
  '.materialize(&files, root)',
  'Command::new("cargo")',
  '"generate-lockfile".to_string()',
  'Duration::from_secs(5 * 60)',
  'fs::remove_dir_all(&target)',
  'validate_lockfile(&lock_bytes',
  'FINAL_DESCRIPTOR_FILE',
  'MODULE_BUILD_COMPONENT_TARGET',
  'MODULE_BUILD_WIT_WORLD',
  'SharedModuleAuthoringBuildControl',
  'SeaOrmModuleAuthoringBuildService::new(',
  'ModuleAuthoringSourceArchiveBuilder::new()',
  '.prepare(&validation.path, &archive_path)',
  '.submit_build(command, archive)',
  '"transactional_outbox_to_remote_isolated_worker"',
  'SharedModuleAuthoringPublishControl',
  'SeaOrmModuleAuthoringPublishService::from_storage_settings(',
  '.submit_publish_request(command, bundle)',
  'build_module_publish_bundle(',
  '"pending_governance_and_platform_admission"',
  '"tenant_id"',
  '"actor_id"',
  '"trace_id"',
  '"correlation_id"',
  '"idempotency_key"',
]) {
  assert.ok(source.includes(marker), `module authoring CLI is missing ${marker}`);
}

for (const marker of [
  'LOCAL_SCENARIO_DIGEST_DOMAIN',
  'pub fn canonical_digest(&self) -> SandboxResult<String>',
  'pub fn comparison(',
  'LocalSandboxScenarioComparison',
  'LocalSandboxScenarioResult',
  'hasher.update(LOCAL_SCENARIO_DIGEST_DOMAIN)',
  '"sha256:b1d8a43f89551031131c687630f6191019c47a459ba6265d240e3d4cbfd00245"',
]) {
  assert.ok(
    sandboxHarness.includes(marker),
    `neutral sandbox scenario digest contract is missing ${marker}`,
  );
}

for (const marker of [
  'load_completed(tenant_id, command.build_request_id)',
  'ModulePublicationArtifactOrigin::PlatformBuilt',
  '.attach_publish_artifact(',
  '.stage_platform_build(',
  '.enqueue_validation_job(',
]) {
  assert.ok(authoringOwner.includes(marker), `module publication owner is missing ${marker}`);
}
assert.ok(
  governanceOwner.includes('DigestObjectKey::sha256('),
  'module governance owner must derive immutable digest-addressed artifact keys',
);
assert.ok(
  !authoringOwner.includes('rustok.module.authoring-build.request.v1'),
  'authoring build request identity must use one current unversioned domain',
);
assert.ok(
    authoringOwner.includes('pub struct PreparedModuleSourceArchive') &&
    authoringOwner.includes('pub struct ModuleAuthoringSourceArchiveBuilder') &&
    authoringOwner.includes('SourceArchiveBuilder::new(self.limits).write') &&
    authoringOwner.includes('SourceTreeMaterializer::new(self.limits).write') &&
    authoringOwner.includes('archive: PreparedModuleSourceArchive') &&
    !authoringOwner.includes('archive_path: PathBuf,\n    ) -> Result<ModuleAuthoringBuildSubmission'),
  'owner build control must accept a prepared archive rather than a raw path',
);

assert.ok(
  publishValidation.includes('MODULE_ARTIFACT_SOURCE_MANIFEST_FILE') &&
    publishValidation.includes('ModuleArtifactSourceManifest::parse(source.as_bytes())') &&
    !publishValidation.includes('#[serde(rename = "rustok-module.toml")]') &&
    !publishValidation.includes('require_file(\n        "rustok-module.toml"'),
  'module publication bundle must use the current artifact source manifest only',
);

assert.ok(
  manifest.includes('rustok-build-source.workspace = true'),
  'module authoring CLI must reuse the shared source archive boundary',
);
assert.ok(
  manifest.includes('rustok-sandbox = { workspace = true, features = ["wasm-component"] }'),
  'module test must use the neutral Component sandbox without a worker dependency',
);
assert.ok(
  moduleManifest.includes('[provides.cli]') &&
    moduleManifest.includes('namespace = "module"') &&
    moduleManifest.includes('factory = "rustok_modules_cli::command_provider"'),
  'module owner manifest must register the authoring provider',
);
assert.ok(
  registryManifest.includes('rustok-modules-cli.workspace = true') &&
    registry.includes('rustok_modules_cli::command_provider(runtime)'),
  'selected CLI distribution must contain the generated owner-local provider',
);
assert.ok(
  !/^rustok-modules-cli\s*=/m.test(serverManifest),
  'apps/server must not depend on the module authoring CLI',
);
assert.ok(
  !source.includes('#[allow(') &&
    !source.includes('todo!') &&
    !source.includes('unimplemented!'),
  'module authoring CLI must not contain lint suppression or stubs',
);

for (const forbidden of [
  'ModuleBuildWorker',
  'GrpcModuleBuildWorker',
  'OciDistributionArtifactPublisher',
  'ModuleBuildDispatcher',
  'Statement::',
  'SELECT module_build_requests',
  'INSERT INTO module_build_requests',
]) {
  assert.ok(
    !source.includes(forbidden),
    `module authoring CLI must not bypass the owner boundary through ${forbidden}`,
  );
}

console.log('[verify-module-authoring-cli] owner-local module authoring boundary verified');
