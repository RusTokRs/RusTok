/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { compileFunction } from 'node:vm';

// Helper to load TS modules cleanly
function loadTsModule(relativePath, extraImports = {}) {
  const source = readFileSync(new URL(relativePath, import.meta.url), 'utf8');
  const clean = stripTypeScriptTypes(
    source
      .replace(/import\s+[^;]+from\s+['"][^'"]+['"];?/g, '')
      .replaceAll('export ', '')
  );
  const exports = {};
  const argNames = ['exports', ...Object.keys(extraImports)];
  const argVals = [exports, ...Object.values(extraImports)];
  compileFunction(
    `${clean}\nObject.assign(exports, { ${extractExportNames(source).join(', ')} });`,
    argNames
  )(...argVals);
  return exports;
}

function extractExportNames(src) {
  const names = [];
  const matches = src.matchAll(
    /export\s+(?:function|const|let|var)\s+([a-zA-Z0-9_]+)/g
  );
  for (const m of matches) {
    names.push(m[1]);
  }
  return names;
}

// Load core modules
const identity = loadTsModule('./core/identity.ts');
const topicFork = loadTsModule('./core/topic-fork.ts', identity);
const topicMerge = loadTsModule('./core/topic-merge.ts', identity);
const topicSlugRename = loadTsModule('./core/topic-slug-rename.ts', identity);
const topicReplyRange = loadTsModule('./core/topic-reply-range.ts', identity);
const topicSplit = loadTsModule('./core/topic-split.ts', identity);

// Load nav.ts
const navSource = readFileSync(new URL('./nav.ts', import.meta.url), 'utf8');
const cleanNavSource = stripTypeScriptTypes(
  navSource
    .replace("import type { NavItem } from '@/shared/types';", '')
    .replaceAll('export ', '')
);
const navModule = {};
compileFunction(`${cleanNavSource}\nnavModule.forumNav = forumNav;`, [
  'navModule'
])(navModule);
const { forumNav } = navModule;

test('forumNav specifies correct plugin group and 8 permission-guarded sub-routes', () => {
  assert.equal(forumNav.title, 'Forum');
  assert.equal(forumNav.url, '/dashboard/forum');
  assert.equal(forumNav.group, 'modulePlugins');
  assert.equal(forumNav.moduleSlug, 'forum');
  assert.equal(forumNav.items.length, 8);

  const subroutes = forumNav.items.map((i) => ({
    title: i.title,
    url: i.url,
    permission: i.access?.permission
  }));

  assert.deepEqual(subroutes, [
    {
      title: 'Categories',
      url: '/dashboard/forum/categories',
      permission: 'forum_categories:manage'
    },
    {
      title: 'Topic Composer',
      url: '/dashboard/forum/topic',
      permission: undefined
    },
    {
      title: 'Reply Composer',
      url: '/dashboard/forum/reply',
      permission: undefined
    },
    {
      title: 'Rename Topic Route',
      url: '/dashboard/forum/rename-slug',
      permission: 'forum_topics:update'
    },
    {
      title: 'Merge Topics',
      url: '/dashboard/forum/merge',
      permission: 'forum_topics:manage'
    },
    {
      title: 'Fork Reply Branch',
      url: '/dashboard/forum/fork',
      permission: 'forum_topics:manage'
    },
    {
      title: 'Move Reply Range',
      url: '/dashboard/forum/reply-range',
      permission: 'forum_topics:manage'
    },
    {
      title: 'Split Topic',
      url: '/dashboard/forum/split',
      permission: 'forum_topics:manage'
    }
  ]);
});

test('identity module generates and validates RFC-compliant UUIDs', () => {
  const uuid = identity.newForumUuid();
  assert.equal(identity.isForumUuid(uuid), true);
  assert.equal(identity.isForumUuid('invalid-uuid-string'), false);
  assert.equal(identity.isForumUuid(''), false);
  assert.equal(
    identity.isForumUuid('123e4567-e89b-12d3-a456-426614174000'),
    true
  );
});

test('buildForumTopicForkCommand enforces identity, existence, and disallows self-fork', () => {
  const sourceTopicId = '123e4567-e89b-12d3-a456-426614174001';
  const targetTopicId = '123e4567-e89b-12d3-a456-426614174002';
  const rootReplyId = '123e4567-e89b-12d3-a456-426614174003';
  const operationId = '123e4567-e89b-12d3-a456-426614174004';

  const validReplies = {
    total: 1,
    items: [
      {
        id: rootReplyId,
        contentPreview: 'Starting reply',
        status: 'published',
        createdAt: '2026-10-31T00:00:00Z'
      }
    ]
  };

  // Self-fork disallowance
  assert.throws(
    () =>
      topicFork.buildForumTopicForkCommand({
        identity: { operationId, targetTopicId: sourceTopicId },
        sourceTopicId,
        replies: validReplies,
        rootReplyId,
        locale: 'en',
        title: 'New Topic',
        slug: 'new-topic',
        reason: 'Reasonable fork'
      }),
    /The new topic identity must differ from the source topic\./
  );

  // Success case
  const command = topicFork.buildForumTopicForkCommand({
    identity: { operationId, targetTopicId },
    sourceTopicId,
    replies: validReplies,
    rootReplyId,
    locale: 'en',
    title: 'New Forked Topic',
    slug: 'new-forked-topic',
    reason: 'Thread diverged'
  });

  assert.equal(command.sourceTopicId, sourceTopicId);
  assert.equal(command.targetTopicId, targetTopicId);
  assert.equal(command.rootReplyId, rootReplyId);
  assert.equal(command.title, 'New Forked Topic');
  assert.equal(command.slug, 'new-forked-topic');
  assert.equal(command.reason, 'Thread diverged');
});

test('buildForumTopicMergeCommand rejects self-merge and empty reasons', () => {
  const sourceTopicId = '123e4567-e89b-12d3-a456-426614174011';
  const targetTopicId = '123e4567-e89b-12d3-a456-426614174012';
  const operationId = '123e4567-e89b-12d3-a456-426614174013';

  const sourceCandidate = { id: sourceTopicId, title: 'Source Topic' };
  const targetCandidate = { id: targetTopicId, title: 'Target Topic' };

  assert.throws(
    () =>
      topicMerge.buildForumTopicMergeCommand({
        operationId,
        source: sourceCandidate,
        target: sourceCandidate,
        reason: 'Merge duplicates'
      }),
    /Source and retained target topics must be different\./
  );

  assert.throws(
    () =>
      topicMerge.buildForumTopicMergeCommand({
        operationId,
        source: sourceCandidate,
        target: targetCandidate,
        reason: '   '
      }),
    /Merge reason is required\./
  );

  const command = topicMerge.buildForumTopicMergeCommand({
    operationId,
    source: sourceCandidate,
    target: targetCandidate,
    reason: 'Duplicate discussion'
  });
  assert.equal(command.sourceTopicId, sourceTopicId);
  assert.equal(command.targetTopicId, targetTopicId);
  assert.equal(command.reason, 'Duplicate discussion');
});

test('buildForumTopicSlugRenameCommand normalizes and validates slugs', () => {
  const topicId = '123e4567-e89b-12d3-a456-426614174021';
  const candidate = {
    id: topicId,
    locale: 'en',
    slug: 'old-slug',
    title: 'Topic Title'
  };

  assert.throws(
    () =>
      topicSlugRename.buildForumTopicSlugRenameCommand({
        candidate,
        slug: '   '
      }),
    /New topic slug is required\./
  );

  const command = topicSlugRename.buildForumTopicSlugRenameCommand({
    candidate,
    slug: '  updated-topic-slug  '
  });
  assert.equal(command.topicId, topicId);
  assert.equal(command.slug, 'updated-topic-slug');
  assert.equal(command.locale, 'en');
});

test('buildForumReplyRangeMoveCommand validates bounds and disallows self-move', () => {
  const sourceTopicId = '123e4567-e89b-12d3-a456-426614174031';
  const targetTopicId = '123e4567-e89b-12d3-a456-426614174032';
  const operationId = '123e4567-e89b-12d3-a456-426614174033';

  assert.throws(
    () =>
      topicReplyRange.buildForumReplyRangeMoveCommand({
        identity: { operationId },
        sourceTopicId,
        targetTopicId: sourceTopicId,
        startPosition: 5,
        endPosition: 10,
        reason: 'Move replies'
      }),
    /Source and target topics must differ\./
  );

  assert.throws(
    () =>
      topicReplyRange.buildForumReplyRangeMoveCommand({
        identity: { operationId },
        sourceTopicId,
        targetTopicId,
        startPosition: 10,
        endPosition: 5,
        reason: 'Move replies'
      }),
    /Start position must not exceed end position\./
  );

  const command = topicReplyRange.buildForumReplyRangeMoveCommand({
    identity: { operationId },
    sourceTopicId,
    targetTopicId,
    startPosition: 5,
    endPosition: 10,
    reason: 'Relocating off-topic replies'
  });
  assert.equal(command.startPosition, 5);
  assert.equal(command.endPosition, 10);
  assert.equal(command.reason, 'Relocating off-topic replies');
});

test('buildForumTopicSplitCommand validates split boundary and replies', () => {
  const sourceTopicId = '123e4567-e89b-12d3-a456-426614174041';
  const targetTopicId = '123e4567-e89b-12d3-a456-426614174042';
  const operationId = '123e4567-e89b-12d3-a456-426614174043';
  const replyId1 = '123e4567-e89b-12d3-a456-426614174044';
  const replyId2 = '123e4567-e89b-12d3-a456-426614174045';

  const loadedReplies = {
    total: 2,
    items: [
      {
        id: replyId1,
        contentPreview: 'Reply 1',
        status: 'published',
        createdAt: '2026-10-31T00:00:00Z'
      },
      {
        id: replyId2,
        contentPreview: 'Reply 2',
        status: 'published',
        createdAt: '2026-10-31T00:01:00Z'
      }
    ]
  };

  // Must retain at least one reply
  assert.throws(
    () =>
      topicSplit.buildForumTopicSplitCommand({
        identity: { operationId, targetTopicId },
        sourceTopicId,
        replies: loadedReplies,
        selectedReplyIds: [replyId1, replyId2],
        locale: 'en',
        title: 'Split Topic Title',
        slug: 'split-topic-title',
        reason: 'Splitting all replies'
      }),
    /The source topic must retain at least one reply\./
  );

  // Success case
  const command = topicSplit.buildForumTopicSplitCommand({
    identity: { operationId, targetTopicId },
    sourceTopicId,
    replies: loadedReplies,
    selectedReplyIds: [replyId1],
    locale: 'en',
    title: 'Split Topic Title',
    slug: 'split-topic-title',
    reason: 'Splitting off-topic branch'
  });

  assert.equal(command.sourceTopicId, sourceTopicId);
  assert.equal(command.targetTopicId, targetTopicId);
  assert.deepEqual(command.replyIds, [replyId1]);
  assert.equal(command.title, 'Split Topic Title');
});
