export type TopicFilterTab = 'latest' | 'top' | 'unread' | 'solved';

/** Wire values of the canonical Forum owner selection, never a local topic predicate. */
export function topicFeedSelection(tab: TopicFilterTab, query: string) {
  return {
    search: query.trim() || undefined,
    order: tab === 'top' ? ('TOP' as const) : ('LATEST' as const),
    solvedOnly: tab === 'solved',
  };
}
