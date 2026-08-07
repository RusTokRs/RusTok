#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const policyPath = path.join(root, 'crates/rustok-modules/src/oci.rs');
const transportPath = path.join(root, 'crates/rustok-modules/src/oci_transport.rs');

function fail(message) {
  throw new Error(`[verify-oci-registry-transport-policy] ${message}`);
}

try {
  const policy = fs.readFileSync(policyPath, 'utf8');
  const transport = fs.readFileSync(transportPath, 'utf8');
  for (const marker of [
    'pub struct OciRegistryTransportPolicy',
    'pub enum OciRegistryProxyMode',
    'allow_redirects: bool',
    'allow_cross_host_auth: bool',
    'verify_tls: bool',
    'request_timeout_ms: u64',
    'max_retries: u8',
    'max_transfer_bytes: u64',
    'max_decompressed_bytes: u64',
    'strict_oci_registry_transport',
  ]) {
    if (!policy.includes(marker)) fail(`OCI transport policy is missing marker: ${marker}`);
  }

  const clientConstruction = transport.slice(
    transport.indexOf('pub(crate) fn with_policy'),
    transport.indexOf('pub(crate) async fn pull_manifest'),
  );
  for (const marker of [
    'Client::builder()',
    '.https_only(true)',
    '.redirect(RedirectPolicy::none())',
    '.no_proxy()',
    '.timeout(timeout)',
    '.connect_timeout(timeout)',
    '.no_gzip()',
    '.no_brotli()',
    '.no_deflate()',
    '.no_zstd()',
    '.retry(reqwest::retry::never())',
    'policy.validate()?',
    'Semaphore::new(policy.max_concurrent_requests)',
  ]) {
    if (!clientConstruction.includes(marker)) {
      fail(`strict OCI client construction is missing: ${marker}`);
    }
  }

  const validation = policy.slice(policy.indexOf('pub fn validate(&self) -> Result<(), String>'));
  for (const marker of [
    'self.allow_redirects',
    'self.allow_cross_host_auth',
    '!self.verify_tls',
    'self.request_timeout_ms == 0',
    'self.max_retries > 3',
    'self.max_transfer_bytes == 0',
    'self.max_decompressed_bytes == 0',
    'self.max_decompressed_bytes > self.max_transfer_bytes',
  ]) {
    if (!validation.includes(marker)) fail(`OCI transport policy validation is missing: ${marker}`);
  }

  for (const marker of [
    'if same_origin(&registry_origin, &challenge.realm)',
    'Authorization::None',
    'OCI registry returned an upload location outside its origin',
    'response.bytes_stream()',
    'max_decompressed_bytes',
  ]) {
    if (!transport.includes(marker)) {
      fail(`strict OCI transport is missing a required boundary control: ${marker}`);
    }
  }

  console.log('[verify-oci-registry-transport-policy] strict OCI transport policy verified');
} catch (error) {
  if (error instanceof Error && error.message.startsWith('[verify-oci-registry-transport-policy]')) {
    console.error(error.message);
    process.exit(1);
  }
  throw error;
}
