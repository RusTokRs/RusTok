import assert from 'node:assert/strict';
import test from 'node:test';

import { HttpError, readHttpError } from './http.ts';

const fallbackMessage = 'Registry request failed';
const responseFor = (status, fields) =>
  new Response(JSON.stringify(fields), {
    status,
    headers: { 'Content-Type': 'application/json' }
  });
const ownerBody = (retryable) => ({
  code: 'pricing.product_identity_capability_unavailable',
  message: 'The requested service is temporarily unavailable',
  retryable
});

test('503 with explicit false retains the owner refusal, not a server-status retry guess', async () => {
  const error = await readHttpError(
    responseFor(503, ownerBody(false)),
    fallbackMessage
  );
  assert.ok(error instanceof HttpError);
  assert.equal(error.status, 503);
  assert.equal(error.retryable, false);
  assert.equal(error.retryableState, 'owner');
  assert.equal(error.code, 'pricing.product_identity_capability_unavailable');
});

test('409 can carry explicit true without a status-derived downgrade', async () => {
  const error = await readHttpError(
    responseFor(409, ownerBody(true)),
    fallbackMessage
  );
  assert.equal(error.retryable, true);
  assert.equal(error.retryableState, 'owner');
});

test('an absent optional member remains unknown even on 503', async () => {
  const { retryable, ...body } = ownerBody(false);
  assert.equal(retryable, false);
  const error = await readHttpError(responseFor(503, body), fallbackMessage);
  assert.equal(error.retryable, undefined);
  assert.equal(error.retryableState, 'absent');
  assert.equal(error.message, body.message);
});

test('present null, strings, numbers, arrays and objects are invalid, never coerced', async () => {
  for (const value of [null, 'false', 'true', 0, 1, [], {}]) {
    const error = await readHttpError(
      responseFor(503, ownerBody(value)),
      fallbackMessage
    );
    assert.equal(error.retryable, undefined);
    assert.equal(error.retryableState, 'invalid');
    assert.equal(error.code, 'pricing.product_identity_capability_unavailable');
  }
});

test('HTML and non-canonical nested envelopes never become public response text or advice', async () => {
  const responses = [
    new Response('<html>private upstream diagnostic</html>', { status: 502 }),
    responseFor(503, { error: ownerBody(true) }),
    responseFor(503, {
      errors: [
        { message: 'nested diagnostic', extensions: { retryable: true } }
      ]
    }),
    responseFor(503, { retryable: true }),
    responseFor(503, [ownerBody(true)])
  ];
  for (const response of responses) {
    const error = await readHttpError(response, fallbackMessage);
    assert.equal(error.message, fallbackMessage);
    assert.equal(error.code, undefined);
    assert.equal(error.retryable, undefined);
    assert.equal(error.retryableState, 'invalid');
  }
});

test('a canonical body preserves public message/code but ignores unrelated outcome claims', async () => {
  const body = {
    ...ownerBody(false),
    outcome: 'rejected',
    idempotency_key: 'not-authority'
  };
  const error = await readHttpError(responseFor(503, body), fallbackMessage);
  assert.equal(error.message, body.message);
  assert.equal(error.code, body.code);
  assert.equal(error.retryable, false);
  assert.equal('outcome' in error, false);
  assert.equal('idempotency_key' in error, false);
});

test('oversized error bodies are cancelled and do not grant retry advice', async () => {
  let cancelled = false;
  const stream = new ReadableStream({
    start(controller) {
      controller.enqueue(
        new TextEncoder().encode(
          JSON.stringify({ ...ownerBody(true), message: 'x'.repeat(16_385) })
        )
      );
    },
    cancel() {
      cancelled = true;
    }
  });
  const error = await readHttpError(
    new Response(stream, { status: 503 }),
    fallbackMessage
  );
  assert.equal(cancelled, true);
  assert.equal(error.message, fallbackMessage);
  assert.equal(error.retryable, undefined);
  assert.equal(error.retryableState, 'invalid');
});

test('an unreadable error body preserves HTTP status and marks metadata invalid', async () => {
  const stream = new ReadableStream({
    start(controller) {
      controller.error(new Error('private stream failure'));
    }
  });
  const error = await readHttpError(
    new Response(stream, { status: 504 }),
    fallbackMessage
  );
  assert.equal(error.status, 504);
  assert.equal(error.message, fallbackMessage);
  assert.equal(error.retryable, undefined);
  assert.equal(error.retryableState, 'invalid');
});

test('the response is consumed once; no request, write key or replay is created', async () => {
  const response = responseFor(503, ownerBody(false));
  const error = await readHttpError(response, fallbackMessage);
  assert.equal(response.bodyUsed, true);
  assert.equal(error.retryable, false);
  assert.deepEqual(Object.keys(error).sort(), [
    'code',
    'name',
    'retryable',
    'retryableState',
    'status'
  ]);
});

test('byte admission counts UTF-8 bytes rather than JavaScript string length', async () => {
  const body = { ...ownerBody(true), message: 'é'.repeat(8_300) };
  const error = await readHttpError(responseFor(503, body), fallbackMessage);
  assert.equal(error.message, fallbackMessage);
  assert.equal(error.retryable, undefined);
  assert.equal(error.retryableState, 'invalid');
});

test(
  'unfinished error bodies expire even when upstream cancellation never resolves',
  { timeout: 5_000 },
  async () => {
    let cancelled = false;
    const stream = new ReadableStream({
      cancel() {
        cancelled = true;
        return new Promise(() => {});
      }
    });
    const error = await readHttpError(
      new Response(stream, { status: 503 }),
      fallbackMessage
    );
    assert.equal(cancelled, true);
    assert.equal(error.status, 503);
    assert.equal(error.message, fallbackMessage);
    assert.equal(error.retryable, undefined);
    assert.equal(error.retryableState, 'invalid');
  }
);
