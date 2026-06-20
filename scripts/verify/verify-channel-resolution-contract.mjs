import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-channel-resolution-contract] ${message}`);
  process.exit(1);
};
const indexOf = (source, marker) => {
  const index = source.indexOf(marker);
  if (index === -1) fail(`missing marker: ${marker}`);
  return index;
};
const assertOrder = (source, markers) => {
  let previous = -1;
  for (const marker of markers) {
    const current = indexOf(source, marker);
    if (current <= previous) fail(`marker order drift around: ${marker}`);
    previous = current;
  }
};

const resolution = read('crates/rustok-channel/src/resolution.rs');
const plan = read('crates/rustok-channel/docs/implementation-plan.md');
const docs = read('crates/rustok-channel/docs/README.md');
const readme = read('crates/rustok-channel/README.md');
const registry = read('docs/modules/registry.md');

assertOrder(resolution, [
  'stage: ResolutionStage::HeaderId',
  'stage: ResolutionStage::HeaderSlug',
  'stage: ResolutionStage::Query',
  'stage: ResolutionStage::Host',
  'self.resolve_policies(facts, &mut trace).await?',
  'self.service.get_default_channel(facts.tenant_id).await?',
]);

for (const marker of [
  'ChannelResolutionOrigin::Host',
  'ChannelResolutionOrigin::Policy',
  'ChannelResolutionOrigin::Default',
  'No tenant-scoped typed resolution policies are configured yet after built-in target slices.',
]) {
  if (!resolution.includes(marker)) fail(`resolution source missing contract marker: ${marker}`);
}

for (const source of [plan, docs, readme, registry]) {
  if (!source.includes('built-in host fast-path')) fail('docs must record the built-in host fast-path decision');
  if (!source.includes('explicit selectors -> built-in host slice -> typed policies -> explicit default -> unresolved')) {
    fail('docs must keep canonical resolution order in sync');
  }
}

if (!plan.includes('- [x] решить, остаётся ли built-in host slice отдельным fast-path после полного policy rollout.')) {
  fail('implementation plan must mark the built-in host fast-path decision closed');
}
if (!plan.includes('verify:channel:resolution-contract')) fail('implementation plan must mention the resolution contract verifier');
if (!registry.includes('verify:channel:resolution-contract')) fail('central registry must mention the resolution contract verifier');

console.log('[verify-channel-resolution-contract] Channel resolution order and built-in host fast-path decision are source-locked');
