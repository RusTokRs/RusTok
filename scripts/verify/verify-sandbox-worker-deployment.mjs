#!/usr/bin/env node

import assert from 'node:assert/strict';

import {
  parseArguments,
  renderDeployment,
} from '../generate/render-sandbox-worker-deployment.mjs';

const digest = `sha256:${'a'.repeat(64)}`;
const options = parseArguments([
  '--namespace',
  'rustok',
  '--image',
  'registry.example/rustok/sandbox-worker',
  '--digest',
  digest,
  '--runtime',
  'gvisor',
  '--tls-secret',
  'sandbox-worker-tls',
  '--attestation-config-map',
  'sandbox-worker-isolation',
]);
const manifest = renderDeployment(options);

for (const marker of [
  `image: registry.example/rustok/sandbox-worker@${digest}`,
  'replicas: 2',
  'maxUnavailable: 0',
  'runtimeClassName: runsc',
  'automountServiceAccountToken: false',
  'hostNetwork: false',
  'hostPID: false',
  'hostIPC: false',
  'runAsNonRoot: true',
  'seccompProfile:',
  'type: RuntimeDefault',
  'readOnlyRootFilesystem: true',
  'capabilities:\n              drop: ["ALL"]',
  'memory: 128Mi',
  'ephemeral-storage: 64Mi',
  'RUSTOK_SANDBOX_IMAGE_DIGEST',
  'RUSTOK_SANDBOX_ISOLATION_ATTESTATION',
  'RUSTOK_SANDBOX_PROBE_ENDPOINT',
  'startupProbe:',
  'readinessProbe:',
  'livenessProbe:',
  'command: ["/app/rustok-sandbox-worker-probe"]',
  'kind: PodDisruptionBudget',
  'kind: NetworkPolicy',
  'policyTypes: ["Ingress", "Egress"]',
  'port: 50051',
  'egress: []',
  'app.kubernetes.io/name: rustok-server',
  'medium: Memory',
  'sizeLimit: 64Mi',
]) {
  assert.ok(manifest.includes(marker), `manifest is missing ${marker}`);
}

assert.throws(
  () =>
    parseArguments([
      '--namespace',
      'rustok',
      '--image',
      'worker',
      '--digest',
      'latest',
      '--runtime',
      'gvisor',
      '--tls-secret',
      'tls',
      '--attestation-config-map',
      'isolation',
    ]),
  /lowercase SHA-256 digest/,
);

const kataManifest = renderDeployment(
  parseArguments([
    '--namespace',
    'rustok',
    '--image',
    'registry.example/rustok/sandbox-worker',
    '--digest',
    digest,
    '--runtime',
    'kata',
    '--tls-secret',
    'sandbox-worker-tls',
    '--attestation-config-map',
    'sandbox-worker-isolation',
  ]),
);
assert.ok(kataManifest.includes('runtimeClassName: kata'));

assert.throws(
  () =>
    parseArguments([
      '--namespace',
      'rustok',
      '--image',
      'worker@sha256:deadbeef',
      '--digest',
      digest,
      '--runtime',
      'gvisor',
      '--tls-secret',
      'tls',
      '--attestation-config-map',
      'isolation',
    ]),
  /repository without a digest/,
);

assert.throws(
  () =>
    parseArguments([
      '--namespace',
      'rustok',
      '--image',
      'worker',
      '--digest',
      digest,
      '--runtime',
      'gvisor',
      '--tls-secret',
      'tls',
      '--attestation-config-map',
      'isolation',
      '--replicas',
      '1',
    ]),
  /greater than or equal to 2/,
);

assert.throws(
  () => parseArguments(['--namespace', 'rustok', '--unknown', 'value']),
  /--image is required|unknown argument/,
);

console.log('[verify-sandbox-worker-deployment] hardened deployment renderer verified');
