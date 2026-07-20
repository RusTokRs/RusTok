#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const workerRoot = path.join(root, 'crates/rustok-module-build-worker');
const workerManifest = path.join(workerRoot, 'Cargo.toml');
const jobLauncherPath = path.join(workerRoot, 'src/runner.rs');
const serverRoot = path.join(root, 'apps/server');
const dispatcherRoot = path.join(root, 'crates/rustok-module-build-dispatcher');
const transportServerPath = path.join(root, 'crates/rustok-module-build-transport/src/server.rs');
const signingPath = path.join(workerRoot, 'src/signing.rs');
const forbiddenDependencies = [
  'sea-orm',
  'sea-orm-migration',
  'sqlx',
  'tokio-postgres',
  'diesel',
  'postgres',
  'mysql',
  'mongodb',
  'rustok-secrets',
  'rustok-storage',
  'aws-sdk-s3',
];
const forbiddenServerDependencies = [
  'rustok-module-build-worker',
  'rustok-module-build-dispatcher',
  'rustok-module-build-transport',
];
const forbiddenServerSourcePatterns = [
  /\b(?:OciJobBuildWorker|CommandBuildWorker|GrpcModuleBuildWorker|ModuleBuildWorker)\b/,
  /\b(?:dispatch_queued|RUSTOK_MODULE_BUILD_(?:RUNNER|JOB_LAUNCHER))\b/,
];
const forbiddenDispatcherSourcePatterns = [
  /\b(?:OciJobBuildWorker|CommandBuildWorker|RUSTOK_MODULE_BUILD_(?:RUNNER|JOB_LAUNCHER))\b/,
];
const forbiddenSourcePatterns = [
  /\b(?:sea_orm|sqlx|tokio_postgres|diesel|DatabaseConnection|TransactionTrait|configure_tenant_scope)\b/,
  /\b(?:rustok_secrets|SecretResolver|SecretRef)\b/,
  /\b(?:DATABASE_URL|RUSTOK_DATABASE_)\b/,
];
const runnerSecretPatterns = [
  /RUSTOK_MODULE_BUILD_(?:SERVER_KEY|CLIENT_CA|REGISTRY_CREDENTIAL|COSIGN_KEY)/,
  /(?:DATABASE_URL|RUSTOK_DATABASE_|AWS_ACCESS_KEY_ID|AWS_SECRET_ACCESS_KEY|AWS_SESSION_TOKEN|GOOGLE_APPLICATION_CREDENTIALS|AZURE_CLIENT_SECRET)/,
];

function fail(message) {
  throw new Error(`[verify-module-build-worker-isolation] ${message}`);
}

function rustFiles(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) return rustFiles(entryPath);
    return entry.isFile() && entry.name.endsWith('.rs') ? [entryPath] : [];
  });
}

function relative(filePath) {
  return path.relative(root, filePath).replaceAll(path.sep, '/');
}

try {
  const manifest = fs.readFileSync(workerManifest, 'utf8');
  const forbiddenManifestDependencies = forbiddenDependencies.filter((dependency) =>
    new RegExp(`^${dependency.replaceAll('-', '\\-')}\\s*=`, 'm').test(manifest),
  );
  if (forbiddenManifestDependencies.length > 0) {
    fail(
      `worker must not depend on tenant DB, platform storage, or general secret crates: ${forbiddenManifestDependencies.join(', ')}`,
    );
  }

  const sourceViolations = rustFiles(path.join(workerRoot, 'src'))
    .filter((filePath) => {
      const source = fs.readFileSync(filePath, 'utf8');
      return forbiddenSourcePatterns.some((pattern) => pattern.test(source));
    })
    .map(relative);
  if (sourceViolations.length > 0) {
    fail(`worker source accesses forbidden tenant or general-secret APIs: ${sourceViolations.join(', ')}`);
  }

  const serverManifest = fs.readFileSync(path.join(serverRoot, 'Cargo.toml'), 'utf8');
  const forbiddenServerWorkerDependencies = forbiddenServerDependencies.filter((dependency) =>
    new RegExp(`^${dependency.replaceAll('-', '\\-')}\\s*=`, 'm').test(serverManifest),
  );
  if (forbiddenServerWorkerDependencies.length > 0) {
    fail(
      `apps/server must not depend on module build delivery or worker crates: ${forbiddenServerWorkerDependencies.join(', ')}`,
    );
  }
  const serverBuildViolations = rustFiles(path.join(serverRoot, 'src'))
    .filter((filePath) => {
      const source = fs.readFileSync(filePath, 'utf8');
      return forbiddenServerSourcePatterns.some((pattern) => pattern.test(source));
    })
    .map(relative);
  if (serverBuildViolations.length > 0) {
    fail(`apps/server contains a prohibited module build worker path: ${serverBuildViolations.join(', ')}`);
  }

  const dispatcherManifest = fs.readFileSync(path.join(dispatcherRoot, 'Cargo.toml'), 'utf8');
  if (/^rustok-module-build-worker\s*=/m.test(dispatcherManifest)) {
    fail('module build dispatcher must not depend on the untrusted worker crate');
  }
  const dispatcherHost = fs.readFileSync(path.join(dispatcherRoot, 'src/host.rs'), 'utf8');
  if (
    !dispatcherHost.includes('GrpcModuleBuildWorker::connect_with_tls') ||
    !dispatcherHost.includes('.check_readiness()')
  ) {
    fail('module build dispatcher must use the mTLS remote worker with readiness verification');
  }
  const dispatcherDelivery = fs.readFileSync(path.join(dispatcherRoot, 'src/lib.rs'), 'utf8');
  if (!dispatcherDelivery.includes('validate_delivery_envelope(&consumed.envelope)?')) {
    fail('module build dispatcher must validate broker envelope and queued-event identity before owner delivery');
  }
  if (!dispatcherHost.includes('required_true("RUSTOK_MODULE_BUILD_DISPATCHER_IGGY_TLS_ENABLED")')) {
    fail('module build dispatcher must require TLS for its credential-bearing external broker connection');
  }
  const dispatcherBuildViolations = rustFiles(path.join(dispatcherRoot, 'src'))
    .filter((filePath) => {
      const source = fs.readFileSync(filePath, 'utf8');
      return forbiddenDispatcherSourcePatterns.some((pattern) => pattern.test(source));
    })
    .map(relative);
  if (dispatcherBuildViolations.length > 0) {
    fail(`dispatcher contains a prohibited in-process worker path: ${dispatcherBuildViolations.join(', ')}`);
  }

  const transportServer = fs.readFileSync(transportServerPath, 'utf8');
  if (
    !transportServer.includes('ModuleBuildWorkerReadiness') ||
    !transportServer.includes('ready: self.worker.is_ready()') ||
    transportServer.includes('ReadinessResponse { ready: true }')
  ) {
    fail('module build transport readiness must use the worker-owned OCI-job probe');
  }

  const jobLauncher = fs.readFileSync(jobLauncherPath, 'utf8');
  if (
    !jobLauncher.includes('RUSTOK_MODULE_BUILD_JOB_LAUNCHER') ||
    !jobLauncher.includes('RUSTOK_MODULE_BUILD_JOB_RUNTIME') ||
    !jobLauncher.includes('RUSTOK_MODULE_BUILD_JOB_IMAGE_DIGEST') ||
    !jobLauncher.includes('RUSTOK_MODULE_BUILD_REQUEST_DIGEST') ||
    !jobLauncher.includes('OciJobRuntime') ||
    !jobLauncher.includes('verify_oci_job_receipt') ||
    !jobLauncher.includes('oci-job-receipt.json') ||
    !jobLauncher.includes('OCI_JOB_RECEIPT_PROTOCOL_VERSION') ||
    !jobLauncher.includes('oci_job_request_digest') ||
    !jobLauncher.includes('"attempt"') ||
    !jobLauncher.includes('"dependency_lock_digest"') ||
    !jobLauncher.includes('"toolchain_digest"') ||
    !jobLauncher.includes('"wit_digest"') ||
    !jobLauncher.includes('"request_digest"')
  ) {
    fail('worker must require fixed hardened OCI job launcher, image, runtime, and immutable receipt evidence');
  }
  if (!jobLauncher.includes('.env_clear()') || !jobLauncher.includes('.kill_on_drop(true)')) {
    fail('untrusted OCI job launcher must clear its environment and be killed on drop');
  }
  if (
    !jobLauncher.includes('publication_target\n            .validate()') ||
    !jobLauncher.includes('credentials.ensure_valid()')
  ) {
    fail('worker must validate its fixed publication target and credential lease before OCI publication');
  }
  const spawnStart = jobLauncher.indexOf('let mut child = Command::new(&self.job_launcher_path)');
  const spawnEnd = jobLauncher.indexOf('.spawn()', spawnStart);
  if (spawnStart < 0 || spawnEnd < 0) {
    fail('fixed OCI job-launcher spawn contract is missing');
  }
  const jobLauncherEnvironment = jobLauncher.slice(spawnStart, spawnEnd);
  if (!jobLauncherEnvironment.includes('"RUSTOK_MODULE_BUILD_OCI_RUNTIME"')) {
    fail('fixed OCI job launcher does not receive its configured hardened runtime');
  }
  if (!jobLauncherEnvironment.includes('"RUSTOK_MODULE_BUILD_REQUEST_DIGEST"')) {
    fail('fixed OCI job launcher does not receive the canonical build-request digest');
  }
  if (runnerSecretPatterns.some((pattern) => pattern.test(jobLauncherEnvironment))) {
    fail('untrusted OCI job launcher receives a tenant, credential, or signing environment value');
  }

  const signing = fs.readFileSync(signingPath, 'utf8');
  if (
    !signing.includes('CosignArtifactSigner') ||
    !signing.includes('.env_clear()') ||
    !signing.includes('.env("DOCKER_CONFIG", &docker_config)')
  ) {
    fail('Cosign signing must clear its environment and use only the private Docker configuration');
  }

  console.log('[verify-module-build-worker-isolation] worker isolation boundaries verified');
} catch (error) {
  if (
    error instanceof Error &&
    error.message.startsWith('[verify-module-build-worker-isolation]')
  ) {
    console.error(error.message);
    process.exit(1);
  }
  throw error;
}
