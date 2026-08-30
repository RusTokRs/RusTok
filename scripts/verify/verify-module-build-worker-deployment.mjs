#!/usr/bin/env node

import assert from 'node:assert/strict';
import { parseArguments, renderDeployment } from '../generate/render-module-build-worker-deployment.mjs';

const digest = `sha256:${'a'.repeat(64)}`;
const options = parseArguments([
  '--namespace', 'rustok', '--image', 'registry.example/rustok/module-build-worker', '--digest', digest,
  '--job-image-digest', `sha256:${'b'.repeat(64)}`,
  '--runtime', 'gvisor', '--tls-secret', 'module-build-worker-tls',
  '--attestation-config-map', 'module-build-worker-isolation', '--config-map', 'module-build-worker-config',
  '--source-pvc', 'module-build-source',
]);
const manifest = renderDeployment(options);
for (const marker of [
  `image: registry.example/rustok/module-build-worker@${digest}`, 'runtimeClassName: runsc',
  `RUSTOK_MODULE_BUILD_JOB_IMAGE_DIGEST\n              value: sha256:${'b'.repeat(64)}`,
  'replicas: 2', 'maxUnavailable: 0', 'automountServiceAccountToken: false', 'hostNetwork: false',
  'hostPID: false', 'hostIPC: false', 'readOnlyRootFilesystem: true', 'drop: ["ALL"]',
  'fsGroup: 10001', 'fsGroupChangePolicy: OnRootMismatch',
  'RUSTOK_MODULE_BUILD_ISOLATION_ATTESTATION', 'RUSTOK_MODULE_BUILD_PROBE_ENDPOINT',
  'RUSTOK_MODULE_BUILD_SOURCE_ROOT', 'persistentVolumeClaim: { claimName: module-build-source, readOnly: true }',
  'startupProbe:', 'readinessProbe:', 'livenessProbe:', 'rustok-module-build-worker-probe',
  'kind: PodDisruptionBudget', 'kind: NetworkPolicy', 'egress: []',
]) assert.ok(manifest.includes(marker), `missing ${marker}`);
assert.throws(() => parseArguments(['--namespace', 'rustok', '--image', 'worker', '--digest', 'latest', '--job-image-digest', digest, '--runtime', 'gvisor', '--tls-secret', 'tls', '--attestation-config-map', 'attestation', '--config-map', 'config', '--source-pvc', 'source']), /lowercase SHA-256 digest/);
assert.throws(() => parseArguments(['--namespace', 'rustok', '--image', 'worker', '--digest', digest, '--job-image-digest', 'latest', '--runtime', 'gvisor', '--tls-secret', 'tls', '--attestation-config-map', 'attestation', '--config-map', 'config', '--source-pvc', 'source']), /job-image-digest must be a lowercase SHA-256 digest/);
assert.ok(renderDeployment(parseArguments(['--namespace', 'rustok', '--image', 'worker', '--digest', digest, '--job-image-digest', digest, '--runtime', 'kata', '--tls-secret', 'tls', '--attestation-config-map', 'attestation', '--config-map', 'config', '--source-pvc', 'source'])).includes('runtimeClassName: kata'));
console.log('[verify-module-build-worker-deployment] hardened module build worker deployment renderer verified');
