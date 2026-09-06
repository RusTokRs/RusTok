import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { ConsumerFbaRuntimeOrderError, verifyConsumerFbaRuntimeOrder } from './verify-consumer-fba-runtime-order.mjs';

const repoRoot = process.cwd();
const files = [
  'crates/modules/rustok-blog/contracts/blog-fba-registry.json',
  'crates/modules/rustok-blog/contracts/evidence/blog-comments-consumer-runtime-order-smoke.json',
  'crates/modules/rustok-blog/rustok-module.toml',
  'crates/modules/rustok-blog/src/services/comment.rs',
  'crates/modules/rustok-blog/src/dto/comment.rs',
  'crates/modules/rustok-blog/src/error.rs',
  'crates/modules/rustok-comments/contracts/comments-fba-registry.json',
  'crates/modules/rustok-seo/contracts/seo-fba-registry.json',
  'crates/modules/rustok-seo/contracts/evidence/seo-media-consumer-runtime-order-smoke.json',
  'crates/modules/rustok-seo/rustok-module.toml',
  'crates/modules/rustok-seo/src/services/targets.rs',
  'crates/modules/rustok-media/contracts/media-fba-registry.json',
  'crates/modules/rustok-media/contracts/evidence/media-runtime-fallback-smoke.json',
  'crates/modules/rustok-media/src/ports.rs',
  'crates/modules/rustok-media/src/dto.rs',
  'crates/modules/rustok-seo-targets/src/lib.rs',
];

function fixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'consumer-fba-'));
  for (const file of files) {
    const target = path.join(root, file);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(path.join(repoRoot, file), target);
  }
  return root;
}

function expectFailure(root, pattern) {
  try {
    verifyConsumerFbaRuntimeOrder({ root });
  } catch (error) {
    if (error instanceof ConsumerFbaRuntimeOrderError && pattern.test(error.message)) return;
    throw error;
  }
  throw new Error(`expected failure matching ${pattern}`);
}

verifyConsumerFbaRuntimeOrder();

const blogDrift = fixture();
const commentService = path.join(blogDrift, 'crates/modules/rustok-blog/src/services/comment.rs');
fs.writeFileSync(
  commentService,
  fs.readFileSync(commentService, 'utf8').replace('self.ensure_post_exists(tenant_id, post_id).await?;', '/* missing post ownership guard */'),
);
expectFailure(blogDrift, /blog create_comment source marker missing: ensure_post_exists/);

const seoImageBoundaryDrift = fixture();
const seoTargets = path.join(seoImageBoundaryDrift, 'crates/modules/rustok-seo-targets/src/lib.rs');
fs.writeFileSync(
  seoTargets,
  fs.readFileSync(seoTargets, 'utf8').replace(
    'pub struct SeoTargetImageRecord {',
    'pub struct LegacySeoTargetImageRecord {',
  ),
);
expectFailure(
  seoImageBoundaryDrift,
  /seo independent typed image boundary missing pub struct SeoTargetImageRecord/,
);

console.log('consumer FBA runtime-order fixture regressions passed');
