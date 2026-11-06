import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { compileFunction } from 'node:vm';
import { createRequire, stripTypeScriptTypes } from 'node:module';
import { Children, createElement, isValidElement } from 'react';
import { FluentBundle, FluentResource } from '@fluent/bundle';
import { createFormatter } from '@rustok/next-fluent/formatter';

const require = createRequire(import.meta.url);
const { transformSync } = require('next/dist/compiled/babel/core');
const modelSource = readFileSync(
  new URL('./presentation.ts', import.meta.url),
  'utf8'
);
const { postStatusPresentation } = compileFunction(
  stripTypeScriptTypes(modelSource.replaceAll('export ', '')) +
    '\nreturn {postStatusPresentation};'
)();
const pageSource = transformSync(
  readFileSync(
    new URL('../pages/post-detail-page.tsx', import.meta.url),
    'utf8'
  ),
  {
    filename: 'post-detail-page.tsx',
    configFile: false,
    babelrc: false,
    presets: [
      [
        require('next/babel'),
        {
          'preset-env': { modules: 'commonjs', targets: { node: 'current' } },
          'preset-react': { runtime: 'automatic' }
        }
      ]
    ]
  }
).code;

function catalog(locale) {
  const bundle = new FluentBundle(locale, { useIsolating: false });
  const errors = bundle.addResource(
    new FluentResource(
      readFileSync(
        new URL(`../../../../messages/${locale}.ftl`, import.meta.url),
        'utf8'
      )
    )
  );
  assert.deepEqual(errors, []);
  return (key, args) => {
    const id = `blog-${key.replaceAll('.', '-')}`;
    const message = bundle.getMessage(id);
    assert.ok(message?.value, `catalog message ${id}`);
    const errors = [];
    const value = bundle.formatPattern(message.value, args, errors);
    assert.deepEqual(errors, []);
    return value;
  };
}

function viewNodes(node) {
  return Children.toArray(node).flatMap((child) =>
    isValidElement(child)
      ? [child, ...viewNodes(child.props.children)]
      : [child]
  );
}

async function executeDetail(locale, post) {
  const calls = [];
  const t = catalog(locale);
  const formatter = createFormatter({ locale, timeZone: 'Europe/Minsk' });
  const passthrough = ({ children }) => createElement('div', {}, children);
  const exports = {};
  const dependencies = {
    '@rustok/next-fluent/server': {
      getLocale: async () => locale,
      getTranslations: async (options) => {
        assert.deepEqual(options, { namespace: 'blog', locale });
        return t;
      },
      getFormatter: async (options) => {
        assert.deepEqual(options, { locale });
        return formatter;
      }
    },
    '../api/posts': {
      getPost: async (id, options) => {
        calls.push({ id, options });
        return post;
      }
    },
    '../model/presentation': { postStatusPresentation },
    '@/components/ui/badge': {
      Badge: ({ children, variant }) =>
        createElement('span', { 'data-variant': variant }, children)
    },
    '@/components/ui/card': {
      Card: passthrough,
      CardContent: passthrough,
      CardHeader: passthrough,
      CardTitle: passthrough
    },
    '@rustok/richtext/view': { RichTextHtml: passthrough }
  };
  compileFunction(pageSource, ['require', 'exports'])(
    (id) => (id in dependencies ? dependencies[id] : require(id)),
    exports
  );
  const result = await exports.default({
    postId: 'post-a',
    token: 'token',
    tenantSlug: 'tenant',
    tenantId: 'tenant-a'
  });
  const nodes = viewNodes(result);
  const text = nodes.filter((node) => typeof node === 'string').join('');
  return { nodes, text, calls, t, formatter };
}

test('pure status presentation covers the complete lifecycle and unknown input', () => {
  assert.deepEqual(postStatusPresentation('DRAFT'), {
    labelKey: 'status.draft',
    variant: 'secondary'
  });
  assert.deepEqual(postStatusPresentation('PUBLISHED'), {
    labelKey: 'status.published',
    variant: 'default'
  });
  assert.deepEqual(postStatusPresentation('ARCHIVED'), {
    labelKey: 'status.archived',
    variant: 'outline'
  });
  assert.deepEqual(postStatusPresentation('UNEXPECTED'), {
    labelKey: 'status.unknown',
    variant: 'outline'
  });
});

test('actual detail component binds owner locale and formats publication through host i18n', async () => {
  const publishedAt = '2026-11-05T23:30:00Z';
  for (const locale of ['en', 'ru']) {
    const { nodes, text, calls, t, formatter } = await executeDetail(locale, {
      status: 'PUBLISHED',
      title: 'Owner title',
      slug: 'owner-post',
      publishedAt,
      tags: [],
      content: null
    });
    assert.deepEqual(calls, [
      {
        id: 'post-a',
        options: {
          token: 'token',
          tenantSlug: 'tenant',
          tenantId: 'tenant-a',
          locale
        }
      }
    ]);
    assert.ok(text.includes(t('status.published')));
    assert.ok(
      text.includes(
        t('detail.publishedAt', {
          date: formatter.dateTime(publishedAt, { dateStyle: 'medium' })
        })
      )
    );
    assert.ok(
      nodes.some(
        (node) => isValidElement(node) && node.props.variant === 'default'
      )
    );
  }
});

test('actual missing detail uses the owner catalog without inventing another locale', async () => {
  const { text, t, calls } = await executeDetail('ru', null);
  assert.equal(text, t('detail.notFound'));
  assert.equal(calls[0].options.locale, 'ru');
});
