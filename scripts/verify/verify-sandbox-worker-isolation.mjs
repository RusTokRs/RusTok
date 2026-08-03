#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const workerRoot = path.join(root, 'crates/rustok-sandbox-worker');
const transportRoot = path.join(root, 'crates/rustok-sandbox-transport');
const serverRoot = path.join(root, 'apps/server');

const forbiddenDependencies = [
  'alloy',
  'rustok-ai',
  'rustok-modules',
  'rustok-mcp',
  'rustok-secrets',
  'rustok-storage',
  'rustok-server',
  'sea-orm',
  'sea-orm-migration',
  'sqlx',
  'reqwest',
  'object_store',
  'aws-config',
  'aws-sdk-s3',
  'aws-sdk-secretsmanager',
  'azure_core',
  'google-cloud-secretmanager-v1',
];

const forbiddenSourcePatterns = [
  /\b(?:alloy|rustok_ai|rustok_modules|rustok_mcp|rustok_secrets|rustok_storage)\b/,
  /\b(?:sea_orm|sqlx|DatabaseConnection|TransactionTrait|ObjectStore|SecretResolver)\b/,
  /\b(?:DATABASE_URL|AWS_ACCESS_KEY_ID|AWS_SECRET_ACCESS_KEY|AZURE_CLIENT_SECRET)\b/,
];

function fail(message) {
  throw new Error(`[verify-sandbox-worker-isolation] ${message}`);
}

function rustFiles(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) return rustFiles(entryPath);
    return entry.isFile() && entry.name.endsWith('.rs') ? [entryPath] : [];
  });
}

function dependencyViolations(manifest) {
  return forbiddenDependencies.filter((dependency) =>
    new RegExp(`^${dependency.replaceAll('-', '\\-')}\\s*=`, 'm').test(manifest),
  );
}

function sourceViolations(directory) {
  return rustFiles(directory)
    .filter((filePath) => {
      const source = fs.readFileSync(filePath, 'utf8');
      return forbiddenSourcePatterns.some((pattern) => pattern.test(source));
    })
    .map((filePath) => path.relative(root, filePath).replaceAll(path.sep, '/'));
}

try {
  for (const crateRoot of [workerRoot, transportRoot]) {
    const manifest = fs.readFileSync(path.join(crateRoot, 'Cargo.toml'), 'utf8');
    const violations = dependencyViolations(manifest);
    if (violations.length > 0) {
      fail(`${path.basename(crateRoot)} has forbidden infrastructure dependencies: ${violations.join(', ')}`);
    }
    const sources = sourceViolations(path.join(crateRoot, 'src'));
    if (sources.length > 0) {
      fail(`${path.basename(crateRoot)} accesses forbidden product or infrastructure APIs: ${sources.join(', ')}`);
    }
  }

  const workerMain = fs.readFileSync(path.join(workerRoot, 'src/main.rs'), 'utf8');
  const isolation = fs.readFileSync(path.join(workerRoot, 'src/lib.rs'), 'utf8');
  if (
    !workerMain.includes('MutualTlsListenerConfig::from_env_prefix') ||
    !workerMain.includes('SandboxWorkerGrpcService::new') ||
    !workerMain.includes('IsolationPolicy::from_env') ||
    !workerMain.includes('WorkerMemoryObserver::cgroup_v2') ||
    !workerMain.includes('ObservedWorkerReadiness::new') ||
    !workerMain.includes('ObservedRhaiExecutor::new')
  ) {
    fail('worker binary must compose mTLS, deployment isolation, and observed memory readiness');
  }
  for (const marker of [
    'RUSTOK_SANDBOX_RUNTIME',
    'RUSTOK_SANDBOX_IMAGE_DIGEST',
    'RUSTOK_SANDBOX_ISOLATION_ATTESTATION',
    'gvisor',
    'kata',
    'host_network',
    'network_mode',
    'rpc_only',
    'ingress_mode',
    'mtls_grpc',
    'egress_denied',
    'database_access',
    'secret_access',
    'read_only_root',
    '/sys/fs/cgroup/memory.current',
    'ObservedWorkerReadiness',
    'peak_memory_bytes',
    'admit_limits',
  ]) {
    if (!isolation.includes(marker)) {
      fail(`worker isolation policy is missing ${marker}`);
    }
  }

  const transportClient = fs.readFileSync(path.join(transportRoot, 'src/client.rs'), 'utf8');
  const transportServer = fs.readFileSync(path.join(transportRoot, 'src/server.rs'), 'utf8');
  if (
    !transportClient.includes('pub async fn connect_with_tls') ||
    !transportClient.includes('pub(crate) fn from_channel') ||
    transportClient.includes('pub fn from_channel') ||
    transportClient.includes('pub async fn connect(')
  ) {
    fail('sandbox transport must expose only the mTLS production client constructor');
  }
  if (
    !transportServer.includes('Semaphore::new(1)') ||
    !transportServer.includes('.admit_limits(&sandbox_request.policy.limits)') ||
    !transportServer.includes('CallbackBroker')
  ) {
    fail('sandbox transport must enforce one execution, isolation limits, and broker callbacks');
  }

  const serverManifest = fs.readFileSync(path.join(serverRoot, 'Cargo.toml'), 'utf8');
  if (!/^rustok-sandbox-transport\.workspace\s*=\s*true$/m.test(serverManifest)) {
    fail('server must depend on the remote sandbox transport');
  }
  if (/^rustok-sandbox-worker\s*=/m.test(serverManifest)) {
    fail('server must not depend on or embed the sandbox worker process');
  }
  const artifactRuntime = fs.readFileSync(
    path.join(serverRoot, 'src/services/artifact_runtime.rs'),
    'utf8',
  );
  if (
    !artifactRuntime.includes('GrpcRhaiExecutor::connect_with_tls') ||
    !artifactRuntime.includes('.check_readiness()') ||
    !artifactRuntime.includes('SharedSandboxRhaiExecutor') ||
    !artifactRuntime.includes('sandbox_rhai_executor(ctx).await?') ||
    !artifactRuntime.includes('.register_isolated_worker(rhai)') ||
    artifactRuntime.includes('RhaiExecutor::new')
  ) {
    fail('artifact server composition must use only the ready remote Rhai executor');
  }
  const appRuntime = fs.readFileSync(
    path.join(serverRoot, 'src/services/app_runtime.rs'),
    'utf8',
  );
  if (
    !appRuntime.includes('sandbox_rhai_executor(ctx).await?') ||
    !appRuntime.includes('executors.register_isolated_worker(rhai)') ||
    !appRuntime.includes('alloy::build_alloy_runtime') ||
    appRuntime.includes('RhaiExecutor::new')
  ) {
    fail('Alloy server composition must share only the ready remote Rhai executor');
  }

  console.log('[verify-sandbox-worker-isolation] sandbox worker isolation boundaries verified');
} catch (error) {
  if (error instanceof Error && error.message.startsWith('[verify-sandbox-worker-isolation]')) {
    console.error(error.message);
    process.exit(1);
  }
  throw error;
}
