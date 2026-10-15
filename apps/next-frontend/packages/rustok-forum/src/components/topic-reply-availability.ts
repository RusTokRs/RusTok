import type { ForumTopicDetail } from '../api/forum';

/** Mirror the write owner's status/lock gate for reply affordances. The owner
 * remains authoritative; this only avoids offering an action it will reject. */
export function canReplyToTopic(topic: Pick<ForumTopicDetail, 'status' | 'isLocked'>): boolean {
  return topic.status.toLowerCase() === 'open' && !topic.isLocked;
}
