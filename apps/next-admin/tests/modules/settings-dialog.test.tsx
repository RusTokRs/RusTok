import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test, { before } from 'node:test';
import { JSDOM } from 'jsdom';
import React, { act } from 'react';

const dom = new JSDOM('<!doctype html><html><body></body></html>', {
  url: 'http://localhost/'
});
Object.assign(globalThis, {
  // tsx uses the shared UI package's classic JSX transform outside this config.
  React,
  window: dom.window,
  document: dom.window.document,
  HTMLElement: dom.window.HTMLElement,
  HTMLInputElement: dom.window.HTMLInputElement,
  HTMLFormElement: dom.window.HTMLFormElement,
  HTMLTextAreaElement: dom.window.HTMLTextAreaElement,
  Element: dom.window.Element,
  Event: dom.window.Event,
  CustomEvent: dom.window.CustomEvent,
  Node: dom.window.Node,
  NodeFilter: dom.window.NodeFilter,
  MutationObserver: dom.window.MutationObserver,
  getComputedStyle: dom.window.getComputedStyle,
  IS_REACT_ACT_ENVIRONMENT: true
});
Object.defineProperty(globalThis, 'navigator', {
  value: dom.window.navigator,
  configurable: true
});

// Import DOM consumers only after installing a browser-like environment.
let createRoot: typeof import('react-dom/client').createRoot;
let FluentProvider: typeof import('@rustok/next-fluent/client').FluentProvider;
let ModuleSettingsDialog: typeof import('../../src/features/modules/components/module-settings-dialog').ModuleSettingsDialog;
let ForumModuleSettingsFields: typeof import('../../src/features/modules/components/module-settings-fields/forum').ForumModuleSettingsFields;
let BlogModuleSettingsFields: typeof import('../../src/features/modules/components/module-settings-fields/blog').BlogModuleSettingsFields;
let ModulePolicyError: typeof import('../../src/app/providers/module-policy-error').ModulePolicyError;
let AppRouterContext: typeof import('next/dist/shared/lib/app-router-context.shared-runtime').AppRouterContext;
before(async () => {
  ({ createRoot } = await import('react-dom/client'));
  ({ FluentProvider } = require('@rustok/next-fluent/client'));
  ({ ModuleSettingsDialog } =
    await import('../../src/features/modules/components/module-settings-dialog'));
  ({ ForumModuleSettingsFields } =
    await import('../../src/features/modules/components/module-settings-fields/forum'));
  ({ BlogModuleSettingsFields } =
    await import('../../src/features/modules/components/module-settings-fields/blog'));
  ({ ModulePolicyError } =
    await import('../../src/app/providers/module-policy-error'));
  ({
    AppRouterContext
  } = require('next/dist/shared/lib/app-router-context.shared-runtime'));
});
const messages = readFileSync('messages/en.ftl', 'utf8');

function setInputValue(
  input: HTMLInputElement | HTMLTextAreaElement,
  value: string
) {
  const prototype =
    input.tagName === 'TEXTAREA'
      ? dom.window.HTMLTextAreaElement.prototype
      : dom.window.HTMLInputElement.prototype;
  Object.getOwnPropertyDescriptor(prototype, 'value')!.set!.call(input, value);
  input.dispatchEvent(new dom.window.Event('input', { bubbles: true }));
}

function field(label: string): HTMLInputElement {
  const element = [...document.querySelectorAll('label')].find(
    (item) => item.textContent === label
  );
  assert.ok(element?.htmlFor, `missing label ${label}`);
  const input = document.getElementById(element.htmlFor);
  assert.ok(input instanceof dom.window.HTMLInputElement);
  return input;
}

function button(label: string): HTMLButtonElement {
  const result = [...document.querySelectorAll('button')].find(
    (item) => item.textContent?.trim() === label
  );
  assert.ok(result, `missing button ${label}`);
  return result;
}

function content(moduleSlug: 'forum' | 'blog', initialSettings: string) {
  return (
    <FluentProvider locale='en' messages={messages}>
      <ModuleSettingsDialog
        moduleSlug={moduleSlug}
        initialSettings={initialSettings}
        expectedRevision={1}
        open={true}
        onOpenChange={() => {}}
        onSaved={() => {}}
        settingsFields={(context) =>
          moduleSlug === 'forum' ? (
            <ForumModuleSettingsFields {...context} />
          ) : (
            <BlogModuleSettingsFields {...context} />
          )
        }
      />
    </FluentProvider>
  );
}

test('forum numeric drafts block saving and preserve exact integers', async () => {
  const mount = document.createElement('div');
  document.body.append(mount);
  const root = createRoot(mount);
  const initial =
    '{"max_topic_title_length":9007199254740993,"topics_per_page":20,"replies_per_page":20}';
  try {
    await act(async () => root.render(content('forum', initial)));
    assert.equal(
      field('Max title length (0 = no limit)').value,
      '9007199254740993'
    );
    const json = document.querySelector('textarea')!;
    await act(async () =>
      setInputValue(field('Max title length (0 = no limit)'), '')
    );
    assert.equal(field('Max title length (0 = no limit)').value, '');
    assert.equal(
      field('Max title length (0 = no limit)').getAttribute('aria-invalid'),
      'true'
    );
    assert.match(
      document.body.textContent!,
      /Enter a non-negative whole number/
    );
    assert.equal(button('Save Settings').disabled, true);
    assert.equal(json.disabled, true);
    assert.match(json.value, /9007199254740993/);
    await act(async () => setInputValue(field('Topics per page'), '-1'));
    await act(async () =>
      setInputValue(
        field('Max title length (0 = no limit)'),
        '18446744073709551615'
      )
    );
    assert.equal(
      button('Save Settings').disabled,
      true,
      'other invalid draft still blocks saving'
    );
    assert.match(json.value, /18446744073709551615/);
    await act(async () => setInputValue(field('Topics per page'), '0'));
    assert.equal(button('Save Settings').disabled, false);
    assert.equal(json.disabled, false);
    assert.match(json.value, /"topics_per_page": 0/);
    assert.match(json.value, /"max_topic_title_length": 18446744073709551615/);

    await act(async () => setInputValue(json, ''));
    assert.equal(
      field('Topics per page').disabled,
      true,
      'invalid JSON disables structured controls'
    );
    assert.equal(json.value, '', 'invalid JSON is never silently replaced');
  } finally {
    await act(async () => root.unmount());
    mount.remove();
  }
});

test('blog controls do not overwrite an invalid or empty JSON draft', async () => {
  const mount = document.createElement('div');
  document.body.append(mount);
  const root = createRoot(mount);
  try {
    await act(async () => root.render(content('blog', '{}')));
    const json = document.querySelector('textarea')!;
    await act(async () => setInputValue(json, ''));
    assert.equal(document.querySelector('select')?.disabled, true);
    assert.equal(
      document.querySelector<HTMLButtonElement>('[role="switch"]')?.disabled,
      true
    );
    assert.equal(json.value, '');
    await act(async () =>
      setInputValue(json, '{"comments_mode":"open","comments_mode":"disabled"}')
    );
    assert.equal(document.querySelector('select')?.disabled, true);
  } finally {
    await act(async () => root.unmount());
    mount.remove();
  }
});

test('a replacement settings snapshot resets stale invalid drafts', async () => {
  const mount = document.createElement('div');
  document.body.append(mount);
  const root = createRoot(mount);
  try {
    await act(async () =>
      root.render(content('forum', '{"topics_per_page":20}'))
    );
    await act(async () => setInputValue(field('Topics per page'), ''));
    assert.equal(button('Save Settings').disabled, true);
    await act(async () =>
      root.render(content('forum', '{"topics_per_page":30}'))
    );
    assert.equal(field('Topics per page').value, '30');
    assert.equal(
      field('Topics per page').getAttribute('aria-invalid'),
      'false'
    );
    assert.equal(button('Save Settings').disabled, false);
  } finally {
    await act(async () => root.unmount());
    mount.remove();
  }
});

test('empty or non-object owner snapshots are never silently submitted as settings', async () => {
  const mount = document.createElement('div');
  document.body.append(mount);
  const root = createRoot(mount);
  try {
    for (const initial of ['', 'null', '[]', '42']) {
      await act(async () => root.render(content('blog', initial)));
      const json = document.querySelector('textarea')!;
      assert.equal(json.value, initial);
      assert.equal(document.querySelector('select')?.disabled, true);
      await act(async () => button('Save Settings').click());
      assert.match(
        document.body.textContent!,
        initial === ''
          ? /JSON value expected/
          : /Settings must be a JSON object/
      );
      assert.equal(json.value, initial);
    }
  } finally {
    await act(async () => root.unmount());
    mount.remove();
  }
});

test('availability read error is localized and refreshes the current route', async () => {
  const mount = document.createElement('div');
  document.body.append(mount);
  const root = createRoot(mount);
  let refreshCount = 0;
  const router = {
    back() {},
    forward() {},
    push() {},
    replace() {},
    prefetch() {},
    refresh() {
      refreshCount++;
    },
    bfcacheId: ''
  };
  try {
    for (const [locale, title, retry] of [
      [
        'en',
        'Module availability could not be verified',
        'Retry availability check'
      ],
      [
        'ru',
        'Не удалось проверить доступность модулей',
        'Повторить проверку доступности'
      ]
    ]) {
      const localeMessages = readFileSync(`messages/${locale}.ftl`, 'utf8');
      await act(async () =>
        root.render(
          <FluentProvider locale={locale} messages={localeMessages}>
            <AppRouterContext.Provider value={router}>
              <ModulePolicyError />
            </AppRouterContext.Provider>
          </FluentProvider>
        )
      );
      assert.match(
        document.querySelector('[role="alert"]')!.textContent!,
        new RegExp(title)
      );
      await act(async () => button(retry).click());
    }
    assert.equal(refreshCount, 2);
  } finally {
    await act(async () => root.unmount());
    mount.remove();
  }
});

test('settings save suppresses rapid duplicate submits and dialog dismissal while pending', async () => {
  const mount = document.createElement('div');
  document.body.append(mount);
  const root = createRoot(mount);
  const calls: unknown[][] = [];
  const changes: boolean[] = [];
  const saved: unknown[][] = [];
  let release!: () => void;
  const pending = new Promise<void>((resolve) => {
    release = resolve;
  });
  const graphql: import('../../src/shared/api/graphql').AdminGraphqlExecutor =
    async (...args) => {
      calls.push(args);
      await pending;
      return {
        updateModuleSettings: {
          moduleSlug: 'blog',
          settings: '{"ok":true}',
          enabled: true,
          revision: 2
        }
      } as never;
    };
  try {
    await act(async () =>
      root.render(
        <FluentProvider locale='en' messages={messages}>
          <ModuleSettingsDialog
            moduleSlug='blog'
            initialSettings='{"ok":true}'
            expectedRevision={1}
            open={true}
            apiOpts={{ token: 'token', tenantSlug: 'tenant', graphql }}
            onOpenChange={(open) => changes.push(open)}
            onSaved={(...args) => saved.push(args)}
          />
        </FluentProvider>
      )
    );
    const submit = button('Save Settings');
    await act(async () => {
      submit.click();
      submit.click();
    });
    assert.equal(calls.length, 1);
    assert.equal(button('Saving...').disabled, true);
    await act(async () => button('Close').click());
    assert.deepEqual(changes, []);
    await act(async () => {
      release();
      await pending;
    });
    assert.deepEqual(saved, [['blog', '{"ok":true}', 2]]);
    assert.deepEqual(changes, [false]);
  } finally {
    release();
    await act(async () => root.unmount());
    mount.remove();
  }
});

test('settings write errors visibly report an unconfirmed outcome', async () => {
  const mount = document.createElement('div');
  document.body.append(mount);
  const root = createRoot(mount);
  const calls: unknown[][] = [];
  const saved: unknown[][] = [];
  const graphql: import('../../src/shared/api/graphql').AdminGraphqlExecutor =
    async (...args) => {
      calls.push(args);
      if (calls.length < 3) throw new Error('connection interrupted');
      return {
        updateModuleSettings: {
          moduleSlug: 'blog',
          settings: '{}',
          enabled: true,
          revision: 2
        }
      } as never;
    };
  try {
    await act(async () =>
      root.render(
        <FluentProvider locale='en' messages={messages}>
          <ModuleSettingsDialog
            moduleSlug='blog'
            initialSettings='{}'
            expectedRevision={1}
            open={true}
            apiOpts={{ graphql, tenantSlug: 'tenant', token: 'token' }}
            onOpenChange={() => {}}
            onSaved={(...args) => saved.push(args)}
          />
        </FluentProvider>
      )
    );
    await act(async () => button('Save Settings').click());
    assert.match(document.body.textContent!, /may have completed/);
    assert.match(document.body.textContent!, /resend the same operation/);
    assert.equal(document.querySelector('textarea')?.disabled, true);
    assert.equal(button('Format JSON').disabled, true);
    await act(async () => button('Save Settings').click());
    await act(async () => button('Save Settings').click());
    assert.equal(calls.length, 3);
    assert.deepEqual(calls[1], calls[0]);
    assert.deepEqual(calls[2], calls[0]);
    assert.deepEqual(saved, [['blog', '{}', 2]]);
  } finally {
    await act(async () => root.unmount());
    mount.remove();
  }
});

test('a late settings response after dialog replacement cannot notify the new dialog', async () => {
  const mount = document.createElement('div');
  document.body.append(mount);
  const root = createRoot(mount);
  let release!: () => void;
  const pending = new Promise<void>((resolve) => {
    release = resolve;
  });
  const events: string[] = [];
  const graphql: import('../../src/shared/api/graphql').AdminGraphqlExecutor =
    async () => {
      await pending;
      return {
        updateModuleSettings: {
          moduleSlug: 'blog',
          settings: '{}',
          enabled: true,
          revision: 2
        }
      } as never;
    };
  const dialog = (slug: string) => (
    <FluentProvider locale='en' messages={messages}>
      <ModuleSettingsDialog
        key={slug}
        moduleSlug={slug}
        initialSettings='{}'
        expectedRevision={1}
        open={true}
        apiOpts={{ graphql }}
        onOpenChange={() => events.push(`close:${slug}`)}
        onSaved={() => events.push(`saved:${slug}`)}
      />
    </FluentProvider>
  );
  try {
    await act(async () => root.render(dialog('blog')));
    await act(async () => button('Save Settings').click());
    await act(async () => root.render(dialog('forum')));
    await act(async () => {
      release();
      await pending;
    });
    assert.deepEqual(events, []);
    assert.match(document.body.textContent!, /Configure Settings: forum/);
  } finally {
    release();
    await act(async () => root.unmount());
    mount.remove();
  }
});

test('a changed settings scope cannot retry or receive the previous operation', async () => {
  const mount = document.createElement('div');
  document.body.append(mount);
  const root = createRoot(mount);
  let release!: () => void;
  const pending = new Promise<void>((resolve) => {
    release = resolve;
  });
  const calls: unknown[][] = [];
  const events: string[] = [];
  const graphql: import('../../src/shared/api/graphql').AdminGraphqlExecutor =
    async (...args) => {
      calls.push(args);
      await pending;
      return {
        updateModuleSettings: {
          moduleSlug: 'blog',
          settings: '{}',
          enabled: true,
          revision: 2
        }
      } as never;
    };
  const dialog = (tenantSlug: string) => (
    <FluentProvider locale='en' messages={messages}>
      <ModuleSettingsDialog
        moduleSlug='blog'
        initialSettings='{}'
        expectedRevision={1}
        open={true}
        apiOpts={{ graphql, tenantSlug }}
        onOpenChange={() => events.push('close')}
        onSaved={() => events.push('saved')}
      />
    </FluentProvider>
  );
  try {
    await act(async () => root.render(dialog('tenant-a')));
    await act(async () => button('Save Settings').click());
    await act(async () => root.render(dialog('tenant-b')));
    await act(async () => root.render(dialog('tenant-a')));
    await act(async () => {
      release();
      await pending;
    });
    assert.deepEqual(events, []);
    assert.equal(button('Save Settings').disabled, true);
    assert.equal(document.querySelector('textarea')?.disabled, true);
    assert.match(document.body.textContent!, /may have completed/);
    await act(async () => button('Save Settings').click());
    assert.equal(calls.length, 1);
  } finally {
    release();
    await act(async () => root.unmount());
    mount.remove();
  }
});
