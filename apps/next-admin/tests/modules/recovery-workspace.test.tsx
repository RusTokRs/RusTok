import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test, { before } from 'node:test';
import { JSDOM } from 'jsdom';
import React, { act } from 'react';

const dom = new JSDOM('<!doctype html><html><body></body></html>', {
  url: 'http://localhost/'
});
Object.assign(globalThis, {
  React,
  window: dom.window,
  document: dom.window.document,
  HTMLElement: dom.window.HTMLElement,
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
let requests: string[] = [];
globalThis.fetch = async (_input, init) => {
  const body = JSON.parse(String(init?.body));
  requests.push(body.query);
  if (!body.query.includes('FailedModuleOperationRecoveryPlans')) {
    throw new Error(
      'disabled recovery controls must not issue a mutation or revision query'
    );
  }
  return new Response(
    JSON.stringify({
      data: {
        failedModuleOperationRecoveryPlans: [
          {
            operationId: 'operation',
            tenantId: 'tenant',
            moduleSlug: 'forum',
            requestedEnabled: true,
            previousEffectiveEnabled: false,
            status: 'committed_hook_failed',
            issue: 'post_hook_failed',
            retryable: false,
            recommendedAction: 'none',
            correlationId: null,
            requestedBy: null,
            errorMessage: 'timeout'
          }
        ]
      }
    }),
    { status: 200, headers: { 'content-type': 'application/json' } }
  );
};
let createRoot: typeof import('react-dom/client').createRoot;
let FluentProvider: typeof import('@rustok/next-fluent/client').FluentProvider;
let RecoveryWorkspace: typeof import('../../src/features/modules/components/recovery-workspace').RecoveryWorkspace;
before(async () => {
  ({ createRoot } = await import('react-dom/client'));
  ({ FluentProvider } = require('@rustok/next-fluent/client'));
  ({ RecoveryWorkspace } =
    await import('../../src/features/modules/components/recovery-workspace'));
});
const messages = readFileSync('messages/en.ftl', 'utf8');
const opts = { tenantSlug: 'tenant' };

test('owner-denied recovery disables retry and compensation and explains uncertainty', async () => {
  requests = [];
  const mount = document.createElement('div');
  document.body.append(mount);
  const root = createRoot(mount);
  try {
    await act(async () => {
      root.render(
        <FluentProvider locale='en' messages={messages}>
          <RecoveryWorkspace apiOpts={opts} />
        </FluentProvider>
      );
    });
    const controls = [...mount.querySelectorAll('button')];
    const retry = controls.find((item) => item.textContent?.trim() === 'Retry');
    const compensation = controls.find(
      (item) => item.textContent?.trim() === 'Compensate'
    );
    assert.ok(retry && compensation, mount.textContent);
    assert.equal(retry.disabled, true);
    assert.equal(compensation.disabled, true);
    assert.match(mount.textContent!, /Automatic recovery is unavailable/);
    await act(async () => {
      retry.click();
      compensation.click();
    });
    assert.equal(
      requests.length,
      1,
      'no recovery or revision requests after clicks'
    );
  } finally {
    await act(async () => root.unmount());
    mount.remove();
  }
});
