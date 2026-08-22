#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const scriptPath = path.resolve('scripts/verify/verify-blog-forum-ui-ownership.mjs');

function writeFixtureFile(root, relativePath, content) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'rustok-blog-forum-ui-ownership-'));
  writeFixtureFile(root, 'apps/next-admin/packages/blog/src/index.ts',
    options.blogOwnsForum
      ? "id: 'blog'\nname: 'Forum'\nforumNav\nForumReplyEditor\nexport { blogNavItems } from './nav'"
      : "id: 'blog'\nexport { blogNavItems } from './nav'");
  writeFixtureFile(root, 'apps/next-admin/packages/blog/src/nav.ts', 'export const blogNavItems = [];');
  writeFixtureFile(root, 'apps/next-admin/packages/blog/src/components/post-form.tsx',
    `@/shared/ui/rich-text-editor
profile='article'
${options.blogUsesHostLocale ? '' : 'initialData?.requestedLocale\ninitialData?.effectiveLocale'}
contentLocale={contentLocale}
disabled={form.formState.isSubmitting}`);
  writeFixtureFile(root, 'apps/next-admin/packages/blog/src/api/posts.ts',
    'requestedLocale: string;\neffectiveLocale: string;');
  writeFixtureFile(root, 'apps/next-admin/packages/forum/src/index.ts',
    "id: 'forum'\nregisterAdminModule\nnavItems: [forumNav]\nforumNav\nForumReplyEditor\nForumTopicEditor\nexport * from './api/forum'");
  writeFixtureFile(root, 'apps/next-admin/packages/forum/src/nav.ts',
    "title: 'Forum'\n/dashboard/forum/topic\n/dashboard/forum/reply");
  writeFixtureFile(root, 'apps/next-admin/packages/forum/src/api/forum.ts',
    `export interface GqlOpts
listForumCategories
export interface ForumTopicSummary extends ForumTopicMergeCandidate {
  locale: string;
  ${options.selectorMissingEffectiveLocale ? '' : 'effectiveLocale: string;'}
  slug: string;
}
listForumTopics
items {
          id
          locale
          ${options.selectorMissingEffectiveLocale ? '' : 'effectiveLocale'}
          title
}
getForumTopic
createForumTopic
updateForumTopic
body: RichTextDocument;
createForumReply
content: RichTextDocument;
effectiveLocale: string;`);
  writeFixtureFile(root, 'apps/next-admin/packages/forum/src/components/forum-reply-editor.tsx',
    `@/shared/ui/rich-text-editor
profile='${options.articleProfile ? 'article' : 'discussion'}'
from '../api/forum'
${options.forumReplyUsesHostLocale ? 'useLocale\nhostLocale' : 'initialContentLocale: string;\ndefaultValues: { locale: initialContentLocale }'}
validateRichTextDocument
richTextDocumentHasText
contentLocale={contentLocale}
disabled={form.formState.isSubmitting}
content: doc
${options.forumPlainTextMissingBidi ? '' : "dir='ltr'"}
${options.legacyAdapterImport ? "from './rt-json-format'" : ''}`);
  writeFixtureFile(root, 'apps/next-admin/packages/forum/src/components/forum-topic-editor.tsx',
    `@/shared/ui/rich-text-editor
profile='discussion'
from '../api/forum'
initialData?.requestedLocale
initialData?.effectiveLocale
validateRichTextDocument
richTextDocumentHasText
contentLocale={contentLocale}
disabled={form.formState.isSubmitting}
createForumTopic
updateForumTopic
${options.forumPlainTextMissingBidi ? '' : "lang={contentLocale}\ndir='auto'\ndir='ltr'\nlang: category.effectiveLocale\ndir: 'auto' as const"}`);
  if (options.legacyAdapterFile) {
    writeFixtureFile(root, 'apps/next-admin/packages/forum/src/components/rt-json-format.ts',
      "normalizeRtJsonPayload\nstringifyRtDoc\nversion: 'rt_json_v1'");
  }
  writeFixtureFile(root, 'apps/next-admin/src/shared/ui/rich-text-editor.tsx',
    "from '@rustok/richtext/react'\nprofile: RichTextProfileId;\ncontentLocale: string;\ndisabled?: boolean;\ncontentLocale={contentLocale}\nframeUrl='/richtext/frame'");
  writeFixtureFile(
    root,
    'apps/next-admin/src/shared/types/base-form.ts',
    options.sharedFormMissingBidi
      ? 'export interface FormOption { value: string; label: string; }'
      : "lang?: string;\ndir?: 'auto' | 'ltr' | 'rtl';"
  );
  writeFixtureFile(
    root,
    'apps/next-admin/src/shared/ui/forms/form-input.tsx',
    options.sharedFormMissingBidi
      ? 'export function FormInput() {}'
      : "lang?: string;\ndir?: 'auto' | 'ltr' | 'rtl';\nlang={lang}\ndir={dir}"
  );
  writeFixtureFile(
    root,
    'apps/next-admin/src/shared/ui/forms/form-select.tsx',
    options.sharedFormMissingBidi
      ? 'export function FormSelect() {}'
      : '<span lang={option.lang} dir={option.dir}>'
  );
  writeFixtureFile(root, 'apps/next-admin/src/modules/index.ts',
    "import '../../packages/blog/src';\nimport '../../packages/forum/src';");
  const selectorBidi = options.selectorMissingEffectiveLocale
    ? ''
    : "lang={topic.effectiveLocale}\ndir='auto'";
  writeFixtureFile(root, 'apps/next-admin/src/app/dashboard/forum/reply/page.tsx',
    `../../../../../packages/forum/src
${options.blogRoute ? '../../../../../packages/blog/src\n' : ''}ForumReplyEditor
listForumTopics
getForumTopic
selectedTopicSummary.locale
initialContentLocale={selectedTopic.effectiveLocale}
${selectorBidi}`);
  writeFixtureFile(root, 'apps/next-admin/src/app/dashboard/forum/topic/page.tsx',
    `../../../../../packages/forum/src
ForumTopicEditor
listForumCategories
listForumTopics
getForumTopic
selectedTopicSummary.locale
${selectorBidi}
Edit the selected forum topic translation.`);
  if (options.blogOwnsForum) {
    writeFixtureFile(root, 'apps/next-admin/packages/blog/src/api/forum.ts', 'legacy owner');
  }
  writeFixtureFile(root, 'crates/rustok-blog/contracts/evidence/blog-forum-ui-ownership.json',
    JSON.stringify({
      schema_version: 1,
      module: 'blog',
      surface: 'next_admin_forum_ui_ownership',
      status: 'verified',
      compile_policy: 'next_typecheck',
      owner_package: options.evidenceDrift
        ? 'apps/next-admin/packages/blog/src'
        : 'apps/next-admin/packages/forum/src',
      former_owner_package: 'apps/next-admin/packages/blog/src',
      shared_richtext_adapter: 'apps/next-admin/src/shared/ui/rich-text-editor.tsx',
      topic_route: 'apps/next-admin/src/app/dashboard/forum/topic/page.tsx',
      verifier: 'scripts/verify/verify-blog-forum-ui-ownership.mjs',
    }));
  writeFixtureFile(root, 'scripts/verify/verify-blog-forum-ui-ownership.test.mjs', 'fixture marker');
  writeFixtureFile(root, 'package.json', JSON.stringify({
    scripts: {
      'verify:blog:forum-ui-ownership': 'node scripts/verify/verify-blog-forum-ui-ownership.mjs',
      'test:verify:blog:forum-ui-ownership': 'node scripts/verify/verify-blog-forum-ui-ownership.test.mjs',
      'verify:blog:fba': 'node scripts/verify/verify-blog-fba.mjs && npm run verify:blog:forum-ui-ownership',
      'test:verify:blog:fba': options.omitTestAggregate
        ? 'node scripts/verify/verify-blog-fba.test.mjs'
        : 'node scripts/verify/verify-blog-fba.test.mjs && npm run test:verify:blog:forum-ui-ownership',
    },
  }));
  return root;
}

function run(root) {
  return spawnSync(process.execPath, [scriptPath], { cwd: root, encoding: 'utf8' });
}

test('Blog Forum UI ownership verifier accepts the canonical owner split', () => {
  const result = run(fixture());
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /Forum owns its Next admin navigation/);
});

test('Blog Forum UI ownership verifier rejects Forum files returning to Blog', () => {
  const result = run(fixture({ blogOwnsForum: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /must be absent from the canonical owner split|Blog index contains forbidden/);
});

test('Blog Forum UI ownership verifier rejects the Article profile in Forum editor', () => {
  const result = run(fixture({ articleProfile: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Forum reply editor missing profile='discussion'/);
});

test('Blog Forum UI ownership verifier rejects host locale as the edit-content locale', () => {
  const result = run(fixture({ blogUsesHostLocale: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Blog post form missing initialData\?\.requestedLocale/);
});

test('Blog Forum UI ownership verifier rejects host locale in Forum reply composer', () => {
  const result = run(fixture({ forumReplyUsesHostLocale: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Forum reply editor missing initialContentLocale: string;|Forum reply editor contains forbidden useLocale/);
});

test('Blog Forum UI ownership verifier rejects shared form bidi boundary regression', () => {
  const result = run(fixture({ sharedFormMissingBidi: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Shared form option type missing|Shared form input bidi boundary missing|Shared form select bidi boundary missing/);
});

test('Blog Forum UI ownership verifier rejects Forum plain-text bidi regression', () => {
  const result = run(fixture({ forumPlainTextMissingBidi: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Forum reply editor missing dir='ltr'|Forum topic editor missing lang=\{contentLocale\}/);
});

test('Blog Forum UI ownership verifier rejects selector effective-locale regression', () => {
  const result = run(fixture({ selectorMissingEffectiveLocale: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Forum GraphQL adapter missing|Forum route missing lang=\{topic\.effectiveLocale\}|Forum topic route missing lang=\{topic\.effectiveLocale\}/);
});

test('Blog Forum UI ownership verifier rejects a restored Forum format adapter', () => {
  const result = run(fixture({ legacyAdapterFile: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /must be absent from the canonical owner split/);
});

test('Blog Forum UI ownership verifier rejects a restored Forum adapter import', () => {
  const result = run(fixture({ legacyAdapterImport: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Forum reply editor contains forbidden/);
});

test('Blog Forum UI ownership verifier rejects a Forum route importing Blog', () => {
  const result = run(fixture({ blogRoute: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Forum route contains forbidden packages\/blog\/src/);
});

test('Blog Forum UI ownership verifier rejects evidence ownership drift', () => {
  const result = run(fixture({ evidenceDrift: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /evidence path drift/);
});

test('Blog Forum UI ownership verifier rejects missing aggregate self-test wiring', () => {
  const result = run(fixture({ omitTestAggregate: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /self-test aggregate does not include Forum ownership fixture/);
});
