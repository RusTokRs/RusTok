import * as z from 'zod';
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { compileFunction } from 'node:vm';

const source = readFileSync(new URL('./editor.ts', import.meta.url), 'utf8')
  .replace(/^import type.*;$/gm, '')
  .replace("import * as z from 'zod';", '')
  .replaceAll('export ', '');
const {
  buildPostCommand,
  targetPostCopy,
  buildCategoryUpdate,
  targetCategoryCopy,
  canSubmitEditor,
  completeEditorWrite,
  EditorValidationError,
  PostCreationAttempt
} = compileFunction(
  stripTypeScriptTypes(source) +
    '\nreturn { PostCreationAttempt, buildPostCommand, targetPostCopy, buildCategoryUpdate, targetCategoryCopy, EditorValidationError, canSubmitEditor, completeEditorWrite };',
  ['z']
)(z);
const doc = (text) => ({
  type: 'doc',
  content: text
    ? [{ type: 'paragraph', content: [{ type: 'text', text }] }]
    : []
});
const empty = doc('');
const hasText = (value) =>
  value.content.some((node) =>
    node.content?.some((child) => child.text?.trim())
  );
const post = (overrides) => ({
  id: 'p',
  requestedLocale: 'en',
  effectiveLocale: 'en',
  title: 'Source title',
  content: { document: doc('Source body') },
  excerpt: 'Source excerpt',
  slug: 'post',
  tags: ['tag'],
  categoryId: 'a',
  channelSlugs: [],
  featuredImageUrl: '/cover',
  seoTitle: 'SEO',
  seoDescription: 'Description',
  version: 8,
  status: 'DRAFT',
  scheduledAt: null,
  ...overrides
});
const values = (row, overrides = {}) => ({
  title: targetPostCopy(row).title,
  locale: row.requestedLocale,
  excerpt: targetPostCopy(row).excerpt,
  seoTitle: targetPostCopy(row).seoTitle,
  seoDescription: targetPostCopy(row).seoDescription,
  slug: row.slug,
  tags: row.tags.join(', '),
  categoryId: row.categoryId,
  channelSlugs: row.channelSlugs.join(', '),
  featuredImageUrl: row.featuredImageUrl,
  publish: false,
  scheduledAt: row.scheduledAt ?? '',
  ...overrides
});
const update = (
  row,
  form = values(row),
  document = targetPostCopy(row).document ?? empty
) => buildPostCommand(form, document, row, empty, hasText).input;

test('acknowledgement consumes submit ability while navigation is pending', () => {
  assert.equal(canSubmitEditor('idle'), true);
  assert.equal(canSubmitEditor('saving'), false);
  assert.equal(canSubmitEditor(completeEditorWrite(true)), false);
  assert.equal(canSubmitEditor(completeEditorWrite(false)), true);
});

test('clearing an existing canonical route is an explicit validation error', () => {
  const row = post();
  assert.throws(
    () => update(row, values(row, { slug: '  ' })),
    (error) => error.code === 'slugRequired'
  );
});

test('category routes cannot be cleared in existing copy; missing target may generate one', () => {
  const category = {
    id: 'c',
    locale: 'en',
    effectiveLocale: 'en',
    name: 'Source',
    slug: 'source',
    description: null
  };
  assert.throws(
    () => buildCategoryUpdate(category, 'en', { name: 'Source', slug: '' }),
    (error) => error.code === 'slugRequired'
  );
  assert.deepEqual(
    buildCategoryUpdate({ ...category, locale: 'fr' }, 'fr', {
      name: 'Cible',
      slug: ''
    }),
    { locale: 'fr', name: 'Cible' }
  );
});

test('fallback source never hydrates editable target copy; category-only save creates no translation', () => {
  const row = post({ requestedLocale: 'fr' });
  assert.deepEqual(targetPostCopy(row), {
    title: '',
    excerpt: '',
    seoTitle: '',
    seoDescription: '',
    document: undefined
  });
  assert.deepEqual(update(row, values(row, { categoryId: 'b' })), {
    version: 8,
    categoryId: 'b'
  });
});
test('new target translation requires independently authored title and body together', () => {
  const row = post({ requestedLocale: 'fr' });
  assert.throws(
    () => update(row, values(row, { title: 'French' })),
    (error) => error.code === 'requiredCopy'
  );
  assert.throws(
    () => update(row, values(row), doc('French')),
    (error) => error.code === 'requiredCopy'
  );
  assert.deepEqual(
    update(row, values(row, { title: 'French' }), doc('French body')),
    { version: 8, title: 'French', content: doc('French body'), locale: 'fr' }
  );
});
test('unchanged exact translation emits only revision; nullable keep clear and set are distinct', () => {
  const row = post({});
  assert.deepEqual(update(row), { version: 8 });
  assert.deepEqual(
    update(row, values(row, { excerpt: '', seoTitle: 'New SEO' })),
    { version: 8, excerpt: null, seoTitle: 'New SEO', locale: 'en' }
  );
  assert.deepEqual(
    update(row, values(row, { categoryId: '__none__', featuredImageUrl: '' })),
    { version: 8, categoryId: null, featuredImageUrl: null }
  );
});
test('revision and locale are bound to the loaded snapshot; failed validation does not mutate it', () => {
  const row = post({});
  const before = structuredClone(row);
  assert.throws(
    () => update(row, values(row, { locale: 'de' })),
    (error) => error.code === 'localeChanged'
  );
  assert.throws(
    () => update({ ...row, version: undefined }),
    (error) => error.code === 'revisionMissing'
  );
  assert.deepEqual(row, before);
});
test('tag changes require explicit locale while owner-only changes omit localized fields', () => {
  const row = post({ requestedLocale: 'fr' });
  assert.deepEqual(update(row, values(row, { tags: 'tag, nouveau' })), {
    version: 8,
    tags: ['tag', 'nouveau'],
    locale: 'fr'
  });
});
test('schedule is offset-bearing, unchanged instants stay omitted, published schedules are preserved', () => {
  const row = post({ scheduledAt: '2027-01-01T10:00:00Z' });
  assert.deepEqual(
    update(row, values(row, { scheduledAt: '2027-01-01T13:00:00+03:00' })),
    { version: 8 }
  );
  assert.throws(
    () => update(row, values(row, { scheduledAt: '2027-01-01T10:00' })),
    (error) => error.code === 'invalidSchedule'
  );
  assert.throws(
    () => update(row, values(row, { scheduledAt: '2027-02-31T10:00:00Z' })),
    (error) => error.code === 'invalidSchedule'
  );
  assert.deepEqual(update(row, values(row, { scheduledAt: '' })), {
    version: 8,
    scheduledAt: null
  });
  assert.deepEqual(
    update({ ...row, status: 'PUBLISHED' }, values(row, { scheduledAt: '' })),
    { version: 8 }
  );
});
test('category fallback starts empty and unrelated slug edits cannot copy source identity', () => {
  const category = {
    id: 'c',
    locale: 'fr',
    effectiveLocale: 'en',
    name: 'Source',
    description: 'Source description',
    slug: 'source'
  };
  assert.deepEqual(targetCategoryCopy(category), {
    name: '',
    slug: '',
    description: ''
  });
  assert.throws(
    () =>
      buildCategoryUpdate(category, 'fr', {
        name: '',
        slug: 'new-route',
        description: ''
      }),
    (error) => error.code === 'requiredCopy'
  );
  assert.throws(
    () =>
      buildCategoryUpdate(category, 'fr', {
        name: '',
        description: 'Only optional copy'
      }),
    (error) => error.code === 'requiredCopy'
  );
  assert.deepEqual(
    buildCategoryUpdate(category, 'fr', {
      name: 'Cible',
      description: 'Description cible'
    }),
    { locale: 'fr', name: 'Cible', description: 'Description cible' }
  );
});
test('exact category update keeps unchanged copy and clears optional description explicitly', () => {
  const category = {
    locale: 'en',
    effectiveLocale: 'en',
    name: 'Source',
    description: 'Description',
    slug: 'route'
  };
  assert.deepEqual(
    buildCategoryUpdate(category, 'en', {
      name: 'Source',
      description: 'Description',
      slug: 'route'
    }),
    { locale: 'en' }
  );
  assert.deepEqual(
    buildCategoryUpdate(category, 'en', { name: 'Source', description: '' }),
    { locale: 'en', description: null }
  );
});

const apiSource = readFileSync(
  new URL('../api/posts.ts', import.meta.url),
  'utf8'
)
  .replace("import { graphqlRequest } from '@/lib/graphql';", '')
  .replaceAll('export ', '');
function api(request) {
  return compileFunction(
    stripTypeScriptTypes(apiSource) +
      '\nreturn { getPost, updatePost, createPost };',
    ['graphqlRequest']
  )(request);
}
test('detail transport carries the explicit host locale and current tenant identity', async () => {
  const calls = [];
  const transport = api(async (...args) => {
    calls.push(args);
    return { post: null };
  });
  await transport.getPost('p', {
    tenantId: 'tenant-a',
    tenantSlug: 'tenant',
    token: 'token',
    locale: 'fr'
  });
  assert.equal(calls.length, 1);
  assert.match(
    calls[0][0],
    /post\(tenantId: \$tenantId, id: \$id, locale: \$locale\)/
  );
  assert.deepEqual(calls[0][1], {
    tenantId: 'tenant-a',
    id: 'p',
    locale: 'fr'
  });
  assert.equal(calls[0][4].locale, 'fr');
});
test('post write acknowledgement never performs a second privileged detail query', async () => {
  const calls = [];
  const transport = api(async (query, variables) => {
    calls.push({ query, variables });
    return { updatePost: true };
  });
  await transport.updatePost(
    'p',
    { version: 8, categoryId: 'b' },
    { tenantId: 'tenant-a' }
  );
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0].variables.input, { version: 8, categoryId: 'b' });
  assert.match(calls[0].query, /mutation UpdatePost/);
  await assert.rejects(
    api(async () => ({ updatePost: false })).updatePost('p', { version: 8 }),
    /rejected/
  );
});

test('creation retries retain exact identity and payload after an unknown response', () => {
  const attempt = new PostCreationAttempt();
  let calls = 0;
  const allocate = () => `command-${++calls}`;
  const input = {
    title: 'Original',
    locale: 'en',
    tags: [],
    publish: false,
    content: doc('Body')
  };
  const first = attempt.prepare(input, allocate);
  first.content.content.length = 0;
  input.title = 'Changed';
  attempt.resolve(new Error('network'));
  attempt.resolve({ code: 'UNAUTHORIZED' });
  assert.equal(attempt.prepare(input, allocate).title, 'Original');
  assert.equal(attempt.request.content.content.length, 1);
  assert.equal(attempt.request.commandId, 'command-1');
  assert.equal(calls, 1);
  attempt.resolve({ outcome: 'rejected' });
  assert.equal(attempt.request, undefined);
  assert.equal(attempt.prepare(input, allocate).commandId, 'command-2');
  assert.equal(attempt.request.title, 'Changed');
});
