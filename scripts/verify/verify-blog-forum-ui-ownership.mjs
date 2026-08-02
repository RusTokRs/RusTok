#!/usr/bin/env node

import fs from 'node:fs';

function read(path) {
  return fs.readFileSync(path, 'utf8');
}

function fail(message) {
  console.error(`[verify-blog-forum-ui-ownership] ${message}`);
  process.exit(1);
}

function requireFile(path) {
  if (!fs.existsSync(path)) fail(`${path} is missing`);
  return read(path);
}

function requireAbsent(path) {
  if (fs.existsSync(path)) fail(`${path} must be absent from the canonical owner split`);
}

function hasAll(text, markers, label) {
  for (const marker of markers) {
    if (!text.includes(marker)) fail(`${label} missing ${marker}`);
  }
}

function hasNone(text, markers, label) {
  for (const marker of markers) {
    if (text.includes(marker)) fail(`${label} contains forbidden ${marker}`);
  }
}

const evidencePath =
  'crates/rustok-blog/contracts/evidence/blog-forum-ui-ownership.json';
const evidence = JSON.parse(requireFile(evidencePath));
if (
  evidence.schema_version !== 1 ||
  evidence.module !== 'blog' ||
  evidence.surface !== 'next_admin_forum_ui_ownership' ||
  evidence.status !== 'verified' ||
  evidence.compile_policy !== 'next_typecheck'
) {
  fail('evidence identity/status drift');
}

const blogIndex = requireFile('apps/next-admin/packages/blog/src/index.ts');
const blogNav = requireFile('apps/next-admin/packages/blog/src/nav.ts');
const blogPostForm = requireFile(
  'apps/next-admin/packages/blog/src/components/post-form.tsx'
);
const forumIndex = requireFile('apps/next-admin/packages/forum/src/index.ts');
const forumNav = requireFile('apps/next-admin/packages/forum/src/nav.ts');
const forumApi = requireFile('apps/next-admin/packages/forum/src/api/forum.ts');
const forumEditor = requireFile(
  'apps/next-admin/packages/forum/src/components/forum-reply-editor.tsx'
);
const sharedEditor = requireFile(
  'apps/next-admin/src/shared/ui/rich-text-editor.tsx'
);
const modulesIndex = requireFile('apps/next-admin/src/modules/index.ts');
const forumPage = requireFile(
  'apps/next-admin/src/app/dashboard/forum/reply/page.tsx'
);

for (const path of [
  'apps/next-admin/packages/blog/src/api/forum.ts',
  'apps/next-admin/packages/blog/src/components/forum-reply-editor.tsx',
  'apps/next-admin/packages/blog/src/components/rt-json-format.ts',
  'apps/next-admin/packages/blog/src/components/rich-text-editor.tsx',
  'apps/next-admin/packages/forum/src/components/rt-json-format.ts'
]) {
  requireAbsent(path);
}

hasAll(blogIndex, ["id: 'blog'", "export { blogNavItems } from './nav'"], 'Blog index');
hasNone(
  blogIndex,
  ["id: 'forum'", 'forumNavItems', 'ForumReplyEditor', "./api/forum", 'RichTextEditor'],
  'Blog index'
);
hasNone(blogNav, ['forumNavItems', "title: 'Forum'", '/dashboard/forum'], 'Blog navigation');
hasAll(
  blogPostForm,
  ["@/shared/ui/rich-text-editor", "profile='article'"],
  'Blog post form'
);
hasNone(blogPostForm, ["./rich-text-editor"], 'Blog post form');

hasAll(
  forumIndex,
  ["id: 'forum'", 'forumNavItems', 'ForumReplyEditor', "export * from './api/forum'"],
  'Forum index'
);
hasAll(forumNav, ["title: 'Forum'", '/dashboard/forum/reply'], 'Forum navigation');
hasAll(
  forumApi,
  [
    'export interface GqlOpts',
    'listForumTopics',
    'createForumReply',
    'content: RichTextDocument;'
  ],
  'Forum GraphQL adapter'
);
hasNone(
  forumApi,
  ['contentFormat', 'contentJson', 'bodyFormat', 'rt_json', 'markdown'],
  'Forum GraphQL adapter'
);
hasAll(
  forumEditor,
  [
    "@/shared/ui/rich-text-editor",
    "profile='discussion'",
    "from '../api/forum'",
    'validateRichTextDocument',
    'richTextDocumentHasText',
    'content: doc'
  ],
  'Forum reply editor'
);
hasNone(
  forumEditor,
  [
    'packages/blog',
    "../api/posts",
    "./rich-text-editor",
    "./rt-json-format",
    'normalizeRtJsonPayload',
    'stringifyRtDoc',
    'rt_json_v1'
  ],
  'Forum reply editor'
);
hasAll(
  sharedEditor,
  [
    "from '@rustok/richtext/react'",
    'profile: RichTextProfileId;',
    "frameUrl='/richtext/frame'"
  ],
  'Shared richtext adapter'
);
hasAll(modulesIndex, ["import '../../packages/blog/src';", "import '../../packages/forum/src';"], 'Host module registration');
hasAll(forumPage, ["../../../../../packages/forum/src", 'ForumReplyEditor', 'listForumTopics'], 'Forum route');
hasNone(forumPage, ['packages/blog/src'], 'Forum route');

if (
  evidence.owner_package !== 'apps/next-admin/packages/forum/src' ||
  evidence.former_owner_package !== 'apps/next-admin/packages/blog/src' ||
  evidence.shared_richtext_adapter !==
    'apps/next-admin/src/shared/ui/rich-text-editor.tsx' ||
  evidence.verifier !== 'scripts/verify/verify-blog-forum-ui-ownership.mjs'
) {
  fail('evidence path drift');
}

const packageJson = JSON.parse(requireFile('package.json'));
if (
  packageJson.scripts?.['verify:blog:forum-ui-ownership'] !==
  'node scripts/verify/verify-blog-forum-ui-ownership.mjs'
) {
  fail('package verifier command drift');
}
if (
  packageJson.scripts?.['test:verify:blog:forum-ui-ownership'] !==
  'node scripts/verify/verify-blog-forum-ui-ownership.test.mjs'
) {
  fail('package self-test command drift');
}
if (!packageJson.scripts?.['verify:blog:fba']?.includes('verify:blog:forum-ui-ownership')) {
  fail('Blog FBA aggregate does not include Forum ownership verifier');
}
if (!packageJson.scripts?.['test:verify:blog:fba']?.includes('test:verify:blog:forum-ui-ownership')) {
  fail('Blog FBA self-test aggregate does not include Forum ownership fixture');
}
requireFile('scripts/verify/verify-blog-forum-ui-ownership.test.mjs');

console.log(
  '[verify-blog-forum-ui-ownership] Forum owns its Next admin navigation, API, and canonical richtext reply editor; Blog and Forum share only the richtext lifecycle adapter'
);
