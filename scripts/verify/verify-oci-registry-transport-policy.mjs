#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const sourcePath = path.join(root, 'crates/rustok-modules/src/oci.rs');

function fail(message) {
  throw new Error(`[verify-oci-registry-transport-policy] ${message}`);
}

try {
  const source = fs.readFileSync(sourcePath, 'utf8');
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
    'strict_oci_distribution_client_with_policy',
    'policy.validate()?',
    'config.protocol = ClientProtocol::Https',
    'config.accept_invalid_certificates = !policy.verify_tls',
    'config.platform_resolver = None',
    'config.max_concurrent_upload = policy.max_concurrent_requests',
    'config.max_concurrent_download = policy.max_concurrent_requests',
  ]) {
    if (!source.includes(marker)) fail(`OCI transport policy is missing marker: ${marker}`);
  }

  const validation = source.slice(source.indexOf('pub fn validate(&self) -> Result<(), String>'));
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

  console.log('[verify-oci-registry-transport-policy] strict OCI transport policy verified');
} catch (error) {
  if (error instanceof Error && error.message.startsWith('[verify-oci-registry-transport-policy]')) {
    console.error(error.message);
    process.exit(1);
  }
  throw error;
}
