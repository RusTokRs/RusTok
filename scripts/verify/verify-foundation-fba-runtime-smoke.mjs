import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptPath = fileURLToPath(import.meta.url);

export class FoundationFbaRuntimeSmokeError extends Error {}

export const foundationFbaRuntimeSmokeModules = [
  {
    module: 'channel',
    registry: 'crates/rustok-channel/contracts/channel-fba-registry.json',
    smoke: 'crates/rustok-channel/contracts/evidence/channel-runtime-fallback-smoke.json',
    markers: [
      ['crates/rustok-channel/src/ports.rs', [
        'impl ChannelReadPort for crate::ChannelService',
        'context.require_policy(PortCallPolicy::read())?;',
        'validate_channel_read_request(&request)?;',
        'ensure_tenant_scope(tenant_id, &detail)?;',
        'request.include_inactive || detail.channel.is_active',
        'channel.slug_empty',
        'channel.host_target_empty'
      ]],
      ['crates/rustok-channel/admin/src/transport/mod.rs', ['mod native_server_adapter;', 'mod rest_adapter;']]
    ]
  },
  {
    module: 'tenant',
    registry: 'crates/rustok-tenant/contracts/tenant-fba-registry.json',
    smoke: 'crates/rustok-tenant/contracts/evidence/tenant-runtime-fallback-smoke.json',
    markers: [
      ['crates/rustok-tenant/src/ports.rs', [
        'impl TenantReadPort for crate::TenantService',
        'context.require_policy(PortCallPolicy::read())?;',
        'validate_tenant_read_request(&request)?;',
        'TenantReadSelector::Id',
        'TenantReadSelector::Slug',
        'TenantReadSelector::Domain',
        'if !request.include_inactive && !tenant.is_active',
        'tenant.slug_empty',
        'tenant.domain_empty',
        'PortErrorKind::NotFound'
      ]],
      ['apps/server/src/middleware/tenant.rs', [
        'TenantReadPort',
        'tenant_read_request(&identifier)',
        'tenant_read_context(&identifier)',
        '.read_tenant(tenant_port_context, tenant_request)',
        'include_inactive: true',
        'set_negative(negative_key_clone.clone(), CachedTenantMiss::Disabled)',
        'get_or_load_with_coalescing'
      ]],
      ['apps/server/src/installer_execution.rs', [
        'TenantReadPort',
        'read_installer_tenant_by_slug(db, &plan.tenant.slug)',
        'TenantReadSelector::Slug(slug.to_string())',
        '.with_deadline(INSTALLER_TENANT_READ_DEADLINE)',
        'treat missing tenant as create candidate'
      ]]
    ]
  },
  {
    module: 'email',
    registry: 'crates/rustok-email/contracts/email-fba-registry.json',
    smoke: 'crates/rustok-email/contracts/evidence/email-runtime-fallback-smoke.json',
    markers: [
      ['crates/rustok-email/src/ports.rs', [
        'impl EmailDeliveryPort for crate::EmailService',
        'require_email_delivery_policy(&context)?;',
        'context\n        .require_policy(PortCallPolicy::write())',
        'validate_delivery_request(&request)?;',
        'EmailProviderMode::DisabledNoop',
        'EmailProviderMode::Smtp',
        'PortError::invariant_violation("email.template_failed"',
        'PortError::validation("email.delivery_invalid"',
        'PortError::unavailable("email.delivery_failed"',
        'email.idempotency_required',
        'email.deadline_required'
      ]]
    ]
  }
];

function read(root, filePath) {
  return fs.readFileSync(path.join(root, filePath), 'utf8');
}

function json(root, filePath) {
  return JSON.parse(read(root, filePath));
}

function fail(message) {
  throw new FoundationFbaRuntimeSmokeError(`[verify-foundation-fba-runtime-smoke] ${message}`);
}

function sameSet(actual, expected, label) {
  const a = [...(actual ?? [])].sort().join('|');
  const e = [...(expected ?? [])].sort().join('|');
  if (a !== e) fail(`${label} drift: expected ${e}, got ${a}`);
}

function fallbackProfiles(registry) {
  return registry.contract_tests?.fallback_smoke?.profiles ?? registry.fallback_profiles ?? [];
}

function degradedModes(registry) {
  const fallback = registry.contract_tests?.fallback_smoke?.degraded_modes;
  if (fallback) return fallback;
  return (registry.consumers ?? []).flatMap((consumer) => consumer.degraded_modes ?? []);
}

export function verifyFoundationFbaRuntimeSmoke({ root = process.cwd() } = {}) {
  for (const config of foundationFbaRuntimeSmokeModules) {
    const registry = json(root, config.registry);
    const smoke = json(root, config.smoke);

    if (registry.module !== config.module) fail(`${config.module} registry module drift`);
    if (smoke.schema_version !== 1 || smoke.module !== config.module) fail(`${config.module} smoke identity drift`);
    if (!['no_compile_executable_runtime_fallback_smoke', 'no_compile_source_locked_runtime_adapter_smoke', 'no_compile_source_locked_runtime_fallback_smoke'].includes(smoke.status)) fail(`${config.module} smoke status drift`);
    if (smoke.generated_from !== config.registry || smoke.contract_version !== registry.contract_version) fail(`${config.module} smoke registry/version drift`);
    if (!smoke.runner?.startsWith('scripts/verify/')) fail(`${config.module} smoke runner drift`);
    sameSet(smoke.profiles, fallbackProfiles(registry), `${config.module} fallback profiles`);
    sameSet(smoke.degraded_modes ?? degradedModes(registry), degradedModes(registry), `${config.module} degraded modes`);

    for (const profile of fallbackProfiles(registry)) {
      if (!smoke.smoke_cases?.some((entry) => entry.profile === profile)) fail(`${config.module} smoke missing profile ${profile}`);
    }
    for (const entry of smoke.smoke_cases ?? []) {
      if (!['no_compile_executable_locked', 'no_compile_source_locked'].includes(entry.execution_status)) fail(`${config.module}.${entry.operation} must be no-compile locked`);
      if (!entry.assertions?.length) fail(`${config.module}.${entry.operation} missing assertions`);
    }
    for (const [filePath, markers] of config.markers) {
      const source = read(root, filePath);
      for (const marker of markers) {
        if (!source.includes(marker)) fail(`${config.module} source marker missing in ${filePath}: ${marker}`);
      }
    }
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === scriptPath) {
  try {
    verifyFoundationFbaRuntimeSmoke();
    console.log('[verify-foundation-fba-runtime-smoke] foundation FBA runtime smoke metadata and source markers are consistent');
  } catch (error) {
    if (error instanceof FoundationFbaRuntimeSmokeError) {
      console.error(error.message);
      process.exit(1);
    }
    throw error;
  }
}
