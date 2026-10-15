import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

test('the unread tab requests the owner-filtered page, not every read-state-enriched topic', () => {
  const api = readFileSync(new URL('./forum.ts', import.meta.url), 'utf8');
  const section = readFileSync(new URL('../components/forum-section.tsx', import.meta.url), 'utf8');
  const feed = readFileSync(new URL('../components/topic-feed.tsx', import.meta.url), 'utf8');
  const query = api.match(/const STOREFRONT_UNREAD_TOPICS_QUERY = `([^`]+)`;/)?.[1];
  assert.ok(query);
  assert.match(query, /forumStorefrontUnreadTopics\([^)]*unreadOnly: true\)/);
  assert.match(section, /activeTab === 'unread' && token\s*\? await fetchStorefrontUnreadTopics/);
  assert.match(feed, /activeTab === 'unread' && topics.length === 0 \? \(/);
  assert.match(feed, /forum\('noUnreadTopics'\)/);
});
