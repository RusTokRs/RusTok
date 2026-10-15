import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const tooltip = readFileSync(new URL('./quote-tooltip.tsx', import.meta.url), 'utf8');
const section = readFileSync(new URL('./forum-section.tsx', import.meta.url), 'utf8');
const detail = readFileSync(new URL('./topic-detail-view.tsx', import.meta.url), 'utf8');

test('a quote from a closed composer opens a reply to the rendered topic before fetching its revision', () => {
  const click = tooltip.slice(tooltip.indexOf('async function onQuoteClick()'));
  assert.match(click, /if \(!state\.isOpen\) \{[\s\S]*?if \(!replyTopic\) return;[\s\S]*?openReply\(\{[\s\S]*?topicId: replyTopic\.id,[\s\S]*?topicTitle: replyTopic\.title,/);
  assert.match(click, /const parentReplyId = targetContext\.targetKind === 'REPLY' \? targetContext\.targetId : null/);
  assert.ok(click.indexOf('openReply({') < click.indexOf('await fetchStorefrontQuoteRevision('));
  assert.match(click, /current\.isOpen && !current\.isSubmitting &&[\s\S]*?current\.parentReplyId === expectedTarget\.parentReplyId/);
  assert.match(click, /if \(stillEditingTarget\(\)\) \{[\s\S]*?appendQuote\(null, snippet, authorHandle\)/);
  assert.match(section, /<QuoteTooltip[^>]*replyTopic=\{selectedTopic && canReplyToTopic\(selectedTopic\)/);
  assert.match(detail, /<QuoteTooltip[^>]*replyTopic=\{canReply \?/);
  assert.match(tooltip, /position && targetContext && \(state\.isOpen \|\| replyTopic !== null\)/);
});
