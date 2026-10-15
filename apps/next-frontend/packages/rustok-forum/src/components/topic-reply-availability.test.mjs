import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { canReplyToTopic } from './topic-reply-availability.ts';

for (const [status, isLocked, expected] of [
  ['open', false, true], ['OPEN', false, true], ['open', true, false],
  ['closed', false, false], ['archived', false, false], ['pending', false, false],
]) {
  test(`reply affordance matches owner gate for ${status} (locked=${isLocked})`, () => {
    assert.equal(canReplyToTopic({ status, isLocked }), expected);
  });
}

test('both forum views gate reply buttons and quote entry on the owner status', () => {
  const thread = readFileSync(new URL('./thread-panel.tsx', import.meta.url), 'utf8');
  const detail = readFileSync(new URL('./topic-detail-view.tsx', import.meta.url), 'utf8');
  assert.match(thread, /const canReply = canReplyToTopic\(topic\)/);
  assert.match(thread, /\{canReply \? \([\s\S]*?openReply\(\{ topicId: topic\.id/);
  assert.match(thread, /\{canReply && \([\s\S]*?parentReplyId: reply\.id/);
  assert.match(detail, /const canReply = canReplyToTopic\(topic\)/);
  assert.match(detail, /\{canReply \? \([\s\S]*?joinTheConversation/);
  assert.match(detail, /replyTopic=\{canReply \? \{ id: topic\.id/);
});
